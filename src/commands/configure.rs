use crate::config::{Config, ModularRule, RuleSeverity};
use crate::utils::Result;
use ratatui::{
    backend::CrosstermBackend,
    layout::{Constraint, Direction, Layout, Rect},
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::{Block, Borders, Clear, List, ListItem, ListState, Paragraph, Wrap},
    Frame, Terminal,
};
use std::io;
use std::path::PathBuf;
use tui_widget_list::ListState as WidgetListState;
use crossterm::{
    event::{self, DisableMouseCapture, EnableMouseCapture, Event, KeyCode, KeyEvent},
    execute,
    terminal::{disable_raw_mode, enable_raw_mode, EnterAlternateScreen, LeaveAlternateScreen},
};
use tracing::{debug, error, info};

#[derive(Debug, Clone, PartialEq)]
pub enum AppMode {
    Rules,
    Profiles,
    Settings,
}

#[derive(Debug)]
pub struct App {
    pub mode: AppMode,
    pub config: Config,
    pub rules_state: ListState,
    pub profiles_state: ListState,
    pub settings_state: ListState,
    pub selected_rule: Option<usize>,
    pub selected_profile: Option<usize>,
    pub selected_setting: usize,
    pub modified: bool,
    pub config_path: Option<PathBuf>,
    pub message: Option<String>,
    pub message_style: Style,
}

impl App {
    pub fn new(config: Config, config_path: Option<PathBuf>) -> Self {
        let mut app = Self {
            mode: AppMode::Rules,
            config,
            rules_state: ListState::default(),
            profiles_state: ListState::default(),
            settings_state: ListState::default(),
            selected_rule: None,
            selected_profile: None,
            selected_setting: 0,
            modified: false,
            config_path,
            message: None,
            message_style: Style::default(),
        };
        
        // Initialize list states
        if !app.config.modular_rules.is_empty() {
            app.rules_state.select(Some(0));
            app.selected_rule = Some(0);
        }
        
        if !app.config.active_profiles.is_empty() {
            app.profiles_state.select(Some(0));
            app.selected_profile = Some(0);
        }
        
        app.settings_state.select(Some(0));
        
        app
    }
    
    pub fn next_rule(&mut self) {
        if self.config.modular_rules.is_empty() {
            return;
        }
        
        let i = match self.rules_state.selected() {
            Some(i) => {
                if i >= self.config.modular_rules.len() - 1 {
                    0
                } else {
                    i + 1
                }
            }
            None => 0,
        };
        self.rules_state.select(Some(i));
        self.selected_rule = Some(i);
    }
    
    pub fn previous_rule(&mut self) {
        if self.config.modular_rules.is_empty() {
            return;
        }
        
        let i = match self.rules_state.selected() {
            Some(i) => {
                if i == 0 {
                    self.config.modular_rules.len() - 1
                } else {
                    i - 1
                }
            }
            None => 0,
        };
        self.rules_state.select(Some(i));
        self.selected_rule = Some(i);
    }
    
    pub fn toggle_rule(&mut self) {
        if let Some(i) = self.selected_rule {
            if let Some(rule) = self.config.modular_rules.get_mut(i) {
                rule.enabled = !rule.enabled;
                self.modified = true;
                self.show_message(
                    format!("Rule '{}' {}", rule.name, if rule.enabled { "enabled" } else { "disabled" }),
                    Style::default().fg(Color::Green),
                );
            }
        }
    }
    
    pub fn next_profile(&mut self) {
        if self.config.active_profiles.is_empty() {
            return;
        }
        
        let i = match self.profiles_state.selected() {
            Some(i) => {
                if i >= self.config.active_profiles.len() - 1 {
                    0
                } else {
                    i + 1
                }
            }
            None => 0,
        };
        self.profiles_state.select(Some(i));
        self.selected_profile = Some(i);
    }
    
    pub fn previous_profile(&mut self) {
        if self.config.active_profiles.is_empty() {
            return;
        }
        
        let i = match self.profiles_state.selected() {
            Some(i) => {
                if i == 0 {
                    self.config.active_profiles.len() - 1
                } else {
                    i - 1
                }
            }
            None => 0,
        };
        self.profiles_state.select(Some(i));
        self.selected_profile = Some(i);
    }
    
    pub fn show_message(&mut self, message: String, style: Style) {
        self.message = Some(message);
        self.message_style = style;
    }
    
    pub fn clear_message(&mut self) {
        self.message = None;
    }
    
    pub fn save_config(&mut self) -> Result<()> {
        if let Some(path) = &self.config_path {
            let content = toml::to_string_pretty(&self.config)?;
            std::fs::write(path, content)?;
            self.modified = false;
            self.show_message("Configuration saved successfully".to_string(), Style::default().fg(Color::Green));
            info!("Configuration saved to {:?}", path);
        } else {
            self.show_message("No config file path available".to_string(), Style::default().fg(Color::Red));
        }
        Ok(())
    }
}

pub fn run_tui(config: Config, config_path: Option<PathBuf>) -> Result<()> {
    // Setup terminal
    enable_raw_mode()?;
    let mut stdout = io::stdout();
    execute!(stdout, EnterAlternateScreen, EnableMouseCapture)?;
    let backend = CrosstermBackend::new(stdout);
    let mut terminal = Terminal::new(backend)?;
    
    // Create app
    let mut app = App::new(config, config_path);
    
    let mut should_quit = false;
    
    while !should_quit {
        terminal.draw(|f| ui(f, &mut app))?;
        
        if let Event::Key(key) = event::read()? {
            match app.mode {
                AppMode::Rules => {
                    match key.code {
                        KeyCode::Char('q') => should_quit = true,
                        KeyCode::Char('s') => {
                            if let Err(e) = app.save_config() {
                                error!("Failed to save config: {}", e);
                                app.show_message(format!("Failed to save: {}", e), Style::default().fg(Color::Red));
                            }
                        }
                        KeyCode::Down | KeyCode::Char('j') => app.next_rule(),
                        KeyCode::Up | KeyCode::Char('k') => app.previous_rule(),
                        KeyCode::Char(' ') | KeyCode::Enter => app.toggle_rule(),
                        KeyCode::Char('1') => app.mode = AppMode::Rules,
                        KeyCode::Char('2') => app.mode = AppMode::Profiles,
                        KeyCode::Char('3') => app.mode = AppMode::Settings,
                        _ => {}
                    }
                }
                AppMode::Profiles => {
                    match key.code {
                        KeyCode::Char('q') => should_quit = true,
                        KeyCode::Down | KeyCode::Char('j') => app.next_profile(),
                        KeyCode::Up | KeyCode::Char('k') => app.previous_profile(),
                        KeyCode::Char('1') => app.mode = AppMode::Rules,
                        KeyCode::Char('2') => app.mode = AppMode::Profiles,
                        KeyCode::Char('3') => app.mode = AppMode::Settings,
                        _ => {}
                    }
                }
                AppMode::Settings => {
                    match key.code {
                        KeyCode::Char('q') => should_quit = true,
                        KeyCode::Down | KeyCode::Char('j') => {
                            app.selected_setting = (app.selected_setting + 1) % 3;
                        }
                        KeyCode::Up | KeyCode::Char('k') => {
                            app.selected_setting = if app.selected_setting == 0 { 2 } else { app.selected_setting - 1 };
                        }
                        KeyCode::Char('1') => app.mode = AppMode::Rules,
                        KeyCode::Char('2') => app.mode = AppMode::Profiles,
                        KeyCode::Char('3') => app.mode = AppMode::Settings,
                        _ => {}
                    }
                }
            }
        }
        
        // Clear message after a short time
        if app.message.is_some() {
            app.clear_message();
        }
    }
    
    // Restore terminal
    disable_raw_mode()?;
    execute!(
        terminal.backend_mut(),
        LeaveAlternateScreen,
        DisableMouseCapture
    )?;
    terminal.show_cursor()?;
    
    Ok(())
}

fn ui(f: &mut Frame, app: &mut App) {
    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(3),
            Constraint::Min(0),
            Constraint::Length(3),
        ])
        .split(f.area());
    
    // Header
    let header_text = vec![
        Line::from("Project-Lint Configuration"),
        Line::from(vec![
            Span::styled("1:Rules", if app.mode == AppMode::Rules { Style::default().fg(Color::Yellow) } else { Style::default() }),
            Span::raw(" "),
            Span::styled("2:Profiles", if app.mode == AppMode::Profiles { Style::default().fg(Color::Yellow) } else { Style::default() }),
            Span::raw(" "),
            Span::styled("3:Settings", if app.mode == AppMode::Settings { Style::default().fg(Color::Yellow) } else { Style::default() }),
            Span::raw(" "),
            Span::styled("q:Quit", Style::default().fg(Color::Gray)),
            if app.modified { Span::styled(" [Modified]", Style::default().fg(Color::Red).add_modifier(Modifier::BOLD)) } else { Span::raw("") },
        ]),
    ];
    
    let header = Paragraph::new(header_text)
        .block(Block::default().borders(Borders::ALL))
        .style(Style::default().fg(Color::White));
    f.render_widget(header, chunks[0]);
    
    // Main content
    match app.mode {
        AppMode::Rules => render_rules(f, app, chunks[1]),
        AppMode::Profiles => render_profiles(f, app, chunks[1]),
        AppMode::Settings => render_settings(f, app, chunks[1]),
    }
    
    // Footer with message
    if let Some(message) = &app.message {
        let footer = Paragraph::new(message.clone())
            .style(app.message_style)
            .block(Block::default().borders(Borders::ALL));
        f.render_widget(footer, chunks[2]);
    } else {
        let help_text = match app.mode {
            AppMode::Rules => "↑↓/jk: Navigate | Space/Enter: Toggle | s: Save | q: Quit",
            AppMode::Profiles => "↑↓/jk: Navigate | q: Quit",
            AppMode::Settings => "↑↓/jk: Navigate | q: Quit",
        };
        
        let footer = Paragraph::new(help_text)
            .style(Style::default().fg(Color::Gray))
            .block(Block::default().borders(Borders::ALL));
        f.render_widget(footer, chunks[2]);
    }
}

fn render_rules(f: &mut Frame, app: &mut App, area: Rect) {
    let chunks = Layout::default()
        .direction(Direction::Horizontal)
        .constraints([Constraint::Percentage(50), Constraint::Percentage(50)])
        .split(area);
    
    // Rules list
    let items: Vec<ListItem> = app.config.modular_rules
        .iter()
        .enumerate()
        .map(|(i, rule)| {
            let style = if rule.enabled {
                Style::default().fg(Color::Green)
            } else {
                Style::default().fg(Color::Red)
            };
            
            let content = format!(
                "{} {} [{}]",
                if rule.enabled { "✓" } else { "✗" },
                rule.name,
                if rule.enabled { "ON" } else { "OFF" }
            );
            
            ListItem::new(content).style(style)
        })
        .collect();
    
    let rules_list = List::new(items)
        .block(Block::default().borders(Borders::ALL).title("Rules"))
        .highlight_style(Style::default().add_modifier(Modifier::REVERSED));
    
    f.render_stateful_widget(rules_list, chunks[0], &mut app.rules_state);
    
    // Rule details
    if let Some(i) = app.selected_rule {
        if let Some(rule) = app.config.modular_rules.get(i) {
            let details = vec![
                Line::from(format!("Name: {}", rule.name)),
                Line::from(format!("Description: {}", rule.description)),
                Line::from(format!("Enabled: {}", rule.enabled)),
                Line::from(format!("Severity: {:?}", rule.severity)),
                Line::from(""),
                Line::from("Triggers:"),
                Line::from(format!("  {}", rule.triggers.join(", "))),
            ];
            
            let details_paragraph = Paragraph::new(details)
                .block(Block::default().borders(Borders::ALL).title("Details"))
                .wrap(Wrap { trim: true });
            f.render_widget(details_paragraph, chunks[1]);
        }
    }
}

fn render_profiles(f: &mut Frame, app: &mut App, area: Rect) {
    let chunks = Layout::default()
        .direction(Direction::Horizontal)
        .constraints([Constraint::Percentage(50), Constraint::Percentage(50)])
        .split(area);
    
    // Profiles list
    let items: Vec<ListItem> = app.config.active_profiles
        .iter()
        .map(|profile| {
            ListItem::new(format!("📁 {}", profile.metadata.name))
                .style(Style::default().fg(Color::Cyan))
        })
        .collect();
    
    let profiles_list = List::new(items)
        .block(Block::default().borders(Borders::ALL).title("Active Profiles"))
        .highlight_style(Style::default().add_modifier(Modifier::REVERSED));
    
    f.render_stateful_widget(profiles_list, chunks[0], &mut app.profiles_state);
    
    // Profile details
    if let Some(i) = app.selected_profile {
        if let Some(profile) = app.config.active_profiles.get(i) {
            let details = vec![
                Line::from(format!("Name: {}", profile.metadata.name)),
                Line::from(format!("Version: {}", profile.metadata.version)),
                Line::from(format!("Enabled: {}", profile.enable.enabled)),
            ];
            
            let details_paragraph = Paragraph::new(details)
                .block(Block::default().borders(Borders::ALL).title("Profile Details"))
                .wrap(Wrap { trim: true });
            f.render_widget(details_paragraph, chunks[1]);
        }
    }
}

fn render_settings(f: &mut Frame, app: &mut App, area: Rect) {
    let settings_items = vec![
        "Global Settings",
        "Output Configuration", 
        "Logging Configuration",
    ];
    
    let items: Vec<ListItem> = settings_items
        .iter()
        .enumerate()
        .map(|(i, setting)| {
            let style = if i == app.selected_setting {
                Style::default().fg(Color::Yellow).add_modifier(Modifier::BOLD)
            } else {
                Style::default()
            };
            ListItem::new(*setting).style(style)
        })
        .collect();
    
    let settings_list = List::new(items)
        .block(Block::default().borders(Borders::ALL).title("Settings"))
        .highlight_style(Style::default().add_modifier(Modifier::REVERSED));
    
    f.render_stateful_widget(settings_list, area, &mut app.settings_state);
}
