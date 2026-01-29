use crate::hooks::{ProjectLintEvent, HookResult, Decision};
use crate::config::{Config, CustomRule, ModularRule, RuleSeverity};
use crate::utils::Result;
use tracing::{debug, info};

pub struct RuleEngine<'a> {
    config: &'a Config,
}

impl<'a> RuleEngine<'a> {
    pub fn new(config: &'a Config) -> Self {
        Self { config }
    }

    pub fn evaluate_event(&self, event: &ProjectLintEvent) -> Result<HookResult> {
        let mut result = HookResult::default();
        let mut issues = Vec::new();

        // 1. Evaluate modular rules
        for rule in &self.config.modular_rules {
            if !rule.enabled {
                continue;
            }

            if self.matches_triggers(&rule.triggers, event)? {
                debug!("Rule '{}' triggered by event", rule.name);
                if let Some(custom_rules) = &rule.rules {
                    for custom_rule in custom_rules {
                        if let Some(issue) = self.evaluate_custom_rule(custom_rule, event)? {
                            issues.push(issue);
                        }
                    }
                }
            }
        }

        // 2. Evaluate top-level custom rules
        for rule in &self.config.rules.custom_rules {
            if self.matches_triggers(&rule.triggers, event)? {
                debug!("Top-level rule '{}' triggered by event", rule.name);
                if let Some(issue) = self.evaluate_custom_rule(rule, event)? {
                    issues.push(issue);
                }
            }
        }

        // 3. Process issues and determine result
        if !issues.is_empty() {
            let has_errors = issues.iter().any(|i| matches!(i.severity, RuleSeverity::Error));

            let mut message = String::from("Project Lint violations detected:\n");
            for issue in &issues {
                let icon = match issue.severity {
                    RuleSeverity::Error => "❌",
                    RuleSeverity::Warning => "⚠️",
                    RuleSeverity::Info => "ℹ️",
                };
                message.push_str(&format!("{} {}: {}\n", icon, issue.name, issue.message));
            }

            result.message = Some(message);
            if has_errors {
                result.decision = Decision::Deny;
            } else {
                result.decision = Decision::Warn;
            }
        }

        Ok(result)
    }

    fn matches_triggers(&self, triggers: &[String], event: &ProjectLintEvent) -> Result<bool> {
        if triggers.is_empty() {
            return Ok(false);
        }

        let event_type_str = serde_json::to_string(&event.event_type)?
            .trim_matches('"')
            .to_string();

        for trigger in triggers {
            if trigger == "all" || trigger == &event_type_str {
                return Ok(true);
            }

            // IDE specific triggers
            if let Some(original_payload) = &event.context.original_payload {
                if let Some(action_name) = original_payload["agent_action_name"].as_str() {
                    if trigger == action_name {
                        return Ok(true);
                    }
                }
                if let Some(hook_event_name) = original_payload["hook_event_name"].as_str() {
                    if trigger == hook_event_name {
                        return Ok(true);
                    }
                }
            }
        }

        Ok(false)
    }

    fn evaluate_custom_rule(&self, rule: &CustomRule, event: &ProjectLintEvent) -> Result<Option<DetectedIssue>> {
        // For event hooks, we mainly check context like file path or prompt content
        let mut matched = false;

        // Check if rule mentions a pattern that matches event file path
        if let Some(file_path) = &event.context.file_path {
            let path_str = file_path.to_string_lossy();
            if crate::utils::matches_pattern(&path_str, &rule.pattern) {
                matched = true;
            }
        }

        // If no file path match, maybe the rule is generic but triggered by event
        if !matched && rule.pattern == "*" {
            matched = true;
        }

        if !matched {
            return Ok(None);
        }

        // Check content patterns against user prompt or file content if available
        if rule.check_content {
            let mut content_to_check = String::new();
            if let Some(prompt) = &event.context.user_prompt {
                content_to_check.push_str(prompt);
            }
            if let Some(file_content) = &event.context.file_content {
                content_to_check.push_str(file_content);
            }

            if let Some(pattern) = &rule.content_pattern {
                let contains = content_to_check.contains(pattern);
                let is_violation = match rule.condition.as_deref() {
                    Some("must_contain") => !contains,
                    _ => contains, // default is denylist
                };

                if is_violation {
                    return Ok(Some(DetectedIssue {
                        name: rule.name.clone(),
                        message: rule.message.clone(),
                        severity: rule.severity.clone(),
                    }));
                }
            }
        } else if !rule.required {
            // If it's a denylist rule (not required) and we matched the pattern, it's an issue
            return Ok(Some(DetectedIssue {
                name: rule.name.clone(),
                message: rule.message.clone(),
                severity: rule.severity.clone(),
            }));
        }

        Ok(None)
    }
}

pub struct DetectedIssue {
    pub name: String,
    pub message: String,
    pub severity: RuleSeverity,
}
