use std::collections::HashMap;
use std::path::Path;
use tracing::{debug, warn};
use tree_sitter::{Parser, Query, QueryCursor};
use utils::Result;

pub struct ASTAnalyzer {
    parsers: HashMap<String, Parser>,
    queries: HashMap<String, Query>,
}

#[derive(Debug, Clone)]
pub struct ASTIssue {
    pub file: String,
    pub line: u32,
    pub column: u32,
    pub message: String,
    pub severity: String,
    pub rule: String,
}

impl ASTAnalyzer {
    pub fn new() -> Result<Self> {
        let mut parsers = HashMap::new();
        let mut queries = HashMap::new();

        // Initialize Rust parser
        let mut rust_parser = Parser::new();
        rust_parser
            .set_language(tree_sitter_rust::language())
            .unwrap();
        parsers.insert("rust".to_string(), rust_parser);

        // Initialize Python parser
        let mut python_parser = Parser::new();
        python_parser
            .set_language(tree_sitter_python::language())
            .unwrap();
        parsers.insert("python".to_string(), python_parser);

        // Initialize JavaScript parser
        let mut js_parser = Parser::new();
        js_parser
            .set_language(tree_sitter_javascript::language())
            .unwrap();
        parsers.insert("javascript".to_string(), js_parser);

        // Initialize TypeScript parser
        let mut ts_parser = Parser::new();
        ts_parser
            .set_language(tree_sitter_typescript::language())
            .unwrap();
        parsers.insert("typescript".to_string(), ts_parser);

        // Initialize JSON parser
        let mut json_parser = Parser::new();
        json_parser
            .set_language(tree_sitter_json::language())
            .unwrap();
        parsers.insert("json".to_string(), json_parser);

        // Initialize YAML parser
        let mut yaml_parser = Parser::new();
        yaml_parser
            .set_language(tree_sitter_yaml::language())
            .unwrap();
        parsers.insert("yaml".to_string(), yaml_parser);

        // Initialize TOML parser
        let mut toml_parser = Parser::new();
        toml_parser
            .set_language(tree_sitter_toml::language())
            .unwrap();
        parsers.insert("toml".to_string(), toml_parser);

        // Load queries for different languages
        Self::load_queries(&mut queries)?;

        Ok(ASTAnalyzer { parsers, queries })
    }

    fn load_queries(&mut queries: &mut HashMap<String, Query>) -> Result<()> {
        // Rust queries
        let rust_queries = vec![
            ("unused_imports", "(use_declaration) @import"),
            ("dead_code", "(function_item) @function"),
            ("println_debug", "(macro_invocation (identifier) @macro)"),
            ("todo_comments", "(line_comment) @comment"),
        ];

        for (name, query_str) in rust_queries {
            if let Ok(query) = Query::new(tree_sitter_rust::language(), query_str) {
                queries.insert(format!("rust_{}", name), query);
            }
        }

        // Python queries
        let python_queries = vec![
            ("unused_imports", "(import_statement) @import"),
            ("print_debug", "(call function: (identifier) @function)"),
            ("todo_comments", "(comment) @comment"),
        ];

        for (name, query_str) in python_queries {
            if let Ok(query) = Query::new(tree_sitter_python::language(), query_str) {
                queries.insert(format!("python_{}", name), query);
            }
        }

        // JavaScript queries
        let js_queries = vec![
            (
                "console_log",
                "(call_expression function: (identifier) @function)",
            ),
            ("todo_comments", "(comment) @comment"),
        ];

        for (name, query_str) in js_queries {
            if let Ok(query) = Query::new(tree_sitter_javascript::language(), query_str) {
                queries.insert(format!("javascript_{}", name), query);
            }
        }

        Ok(())
    }

    pub fn analyze_file(&self, file_path: &Path, content: &str) -> Result<Vec<ASTIssue>> {
        let mut issues = Vec::new();
        let extension = file_path.extension().unwrap_or_default().to_string_lossy();
        let language = self.get_language_from_extension(&extension);

        if let Some(parser) = self.parsers.get(&language) {
            debug!("Analyzing {} file: {:?}", language, file_path);

            let tree = parser.parse(content, None).unwrap();
            let root_node = tree.root_node();

            // Analyze based on language
            match language.as_str() {
                "rust" => self.analyze_rust(&root_node, content, file_path, &mut issues)?,
                "python" => self.analyze_python(&root_node, content, file_path, &mut issues)?,
                "javascript" | "typescript" => {
                    self.analyze_javascript(&root_node, content, file_path, &mut issues)?
                }
                _ => {}
            }
        }

        Ok(issues)
    }

    fn get_language_from_extension(&self, extension: &str) -> String {
        match extension.to_lowercase().as_str() {
            "rs" => "rust".to_string(),
            "py" => "python".to_string(),
            "js" => "javascript".to_string(),
            "ts" | "tsx" => "typescript".to_string(),
            "json" => "json".to_string(),
            "yaml" | "yml" => "yaml".to_string(),
            "toml" => "toml".to_string(),
            _ => "unknown".to_string(),
        }
    }

    fn analyze_rust(
        &self,
        root_node: &tree_sitter::Node,
        content: &str,
        file_path: &Path,
        issues: &mut Vec<ASTIssue>,
    ) -> Result<()> {
        // Check for println! debug statements
        let query_str = "(macro_invocation (identifier) @macro)";
        if let Ok(query) = Query::new(tree_sitter_rust::language(), query_str) {
            let mut cursor = QueryCursor::new();
            let matches = cursor.matches(&query, root_node, content.as_bytes());

            for m in matches {
                for capture in m.captures {
                    let node = capture.node;
                    let node_text = node.utf8_text(content.as_bytes()).unwrap_or("");

                    if node_text == "println" {
                        let start_pos = node.start_position();
                        issues.push(ASTIssue {
                            file: file_path.to_string_lossy().to_string(),
                            line: start_pos.row as u32 + 1,
                            column: start_pos.column as u32 + 1,
                            message: "Remove debug println! statement before committing"
                                .to_string(),
                            severity: "warning".to_string(),
                            rule: "no_debug_prints".to_string(),
                        });
                    }
                }
            }
        }

        // Check for TODO comments
        let query_str = "(line_comment) @comment";
        if let Ok(query) = Query::new(tree_sitter_rust::language(), query_str) {
            let mut cursor = QueryCursor::new();
            let matches = cursor.matches(&query, root_node, content.as_bytes());

            for m in matches {
                for capture in m.captures {
                    let node = capture.node;
                    let node_text = node.utf8_text(content.as_bytes()).unwrap_or("");

                    if node_text.to_lowercase().contains("todo") {
                        let start_pos = node.start_position();
                        issues.push(ASTIssue {
                            file: file_path.to_string_lossy().to_string(),
                            line: start_pos.row as u32 + 1,
                            column: start_pos.column as u32 + 1,
                            message: "TODO comment found - consider addressing".to_string(),
                            severity: "info".to_string(),
                            rule: "todo_comment".to_string(),
                        });
                    }
                }
            }
        }

        Ok(())
    }

    fn analyze_python(
        &self,
        root_node: &tree_sitter::Node,
        content: &str,
        file_path: &Path,
        issues: &mut Vec<ASTIssue>,
    ) -> Result<()> {
        // Check for print statements
        let query_str = "(call function: (identifier) @function)";
        if let Ok(query) = Query::new(tree_sitter_python::language(), query_str) {
            let mut cursor = QueryCursor::new();
            let matches = cursor.matches(&query, root_node, content.as_bytes());

            for m in matches {
                for capture in m.captures {
                    let node = capture.node;
                    let node_text = node.utf8_text(content.as_bytes()).unwrap_or("");

                    if node_text == "print" {
                        let start_pos = node.start_position();
                        issues.push(ASTIssue {
                            file: file_path.to_string_lossy().to_string(),
                            line: start_pos.row as u32 + 1,
                            column: start_pos.column as u32 + 1,
                            message: "Remove debug print statement before committing".to_string(),
                            severity: "warning".to_string(),
                            rule: "no_debug_prints".to_string(),
                        });
                    }
                }
            }
        }

        // Check for TODO comments
        let query_str = "(comment) @comment";
        if let Ok(query) = Query::new(tree_sitter_python::language(), query_str) {
            let mut cursor = QueryCursor::new();
            let matches = cursor.matches(&query, root_node, content.as_bytes());

            for m in matches {
                for capture in m.captures {
                    let node = capture.node;
                    let node_text = node.utf8_text(content.as_bytes()).unwrap_or("");

                    if node_text.to_lowercase().contains("todo") {
                        let start_pos = node.start_position();
                        issues.push(ASTIssue {
                            file: file_path.to_string_lossy().to_string(),
                            line: start_pos.row as u32 + 1,
                            column: start_pos.column as u32 + 1,
                            message: "TODO comment found - consider addressing".to_string(),
                            severity: "info".to_string(),
                            rule: "todo_comment".to_string(),
                        });
                    }
                }
            }
        }

        Ok(())
    }

    fn analyze_javascript(
        &self,
        root_node: &tree_sitter::Node,
        content: &str,
        file_path: &Path,
        issues: &mut Vec<ASTIssue>,
    ) -> Result<()> {
        // Check for console.log statements
        let query_str = "(call_expression function: (identifier) @function)";
        if let Ok(query) = Query::new(tree_sitter_javascript::language(), query_str) {
            let mut cursor = QueryCursor::new();
            let matches = cursor.matches(&query, root_node, content.as_bytes());

            for m in matches {
                for capture in m.captures {
                    let node = capture.node;
                    let node_text = node.utf8_text(content.as_bytes()).unwrap_or("");

                    if node_text == "console" {
                        // Check if it's console.log
                        if let Some(parent) = node.parent() {
                            if let Some(grandparent) = parent.parent() {
                                let grandparent_text =
                                    grandparent.utf8_text(content.as_bytes()).unwrap_or("");
                                if grandparent_text.contains("console.log") {
                                    let start_pos = node.start_position();
                                    issues.push(ASTIssue {
                                        file: file_path.to_string_lossy().to_string(),
                                        line: start_pos.row as u32 + 1,
                                        column: start_pos.column as u32 + 1,
                                        message:
                                            "Remove debug console.log statement before committing"
                                                .to_string(),
                                        severity: "warning".to_string(),
                                        rule: "no_debug_prints".to_string(),
                                    });
                                }
                            }
                        }
                    }
                }
            }
        }

        // Check for TODO comments
        let query_str = "(comment) @comment";
        if let Ok(query) = Query::new(tree_sitter_javascript::language(), query_str) {
            let mut cursor = QueryCursor::new();
            let matches = cursor.matches(&query, root_node, content.as_bytes());

            for m in matches {
                for capture in m.captures {
                    let node = capture.node;
                    let node_text = node.utf8_text(content.as_bytes()).unwrap_or("");

                    if node_text.to_lowercase().contains("todo") {
                        let start_pos = node.start_position();
                        issues.push(ASTIssue {
                            file: file_path.to_string_lossy().to_string(),
                            line: start_pos.row as u32 + 1,
                            column: start_pos.column as u32 + 1,
                            message: "TODO comment found - consider addressing".to_string(),
                            severity: "info".to_string(),
                            rule: "todo_comment".to_string(),
                        });
                    }
                }
            }
        }

        Ok(())
    }
}
