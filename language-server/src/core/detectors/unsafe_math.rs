use super::detector::Detector;
use super::detector_config::DetectorConfig;
use crate::core::utilities::DiagnosticBuilder;
use syn::spanned::Spanned;
use syn::{BinOp, parse_str, visit::Visit};
use tower_lsp::lsp_types::{Diagnostic, DiagnosticSeverity};

pub struct UnsafeMathDetector {
    diagnostics: Vec<Diagnostic>,
    config: DetectorConfig,
}

impl UnsafeMathDetector {
    pub fn new() -> Self {
        Self {
            diagnostics: Vec::new(),
            config: DetectorConfig::default(),
        }
    }

    #[allow(dead_code)]
    pub fn with_config(config: DetectorConfig) -> Self {
        Self {
            diagnostics: Vec::new(),
            config,
        }
    }

    /// Check for custom patterns in the content
    fn check_custom_patterns(&mut self, content: &str) {
        for pattern in &self.config.custom_patterns {
            let mut start_pos = 0;
            while let Some(pos) = content[start_pos..].find(pattern) {
                let actual_pos = start_pos + pos;

                // Calculate line and column for the match
                let lines_before = content[..actual_pos].matches('\n').count();
                let line_start = content[..actual_pos]
                    .rfind('\n')
                    .map(|p| p + 1)
                    .unwrap_or(0);
                let line_end = content[actual_pos..]
                    .find('\n')
                    .map(|p| actual_pos + p)
                    .unwrap_or(content.len());
                let current_line = &content[line_start..line_end];
                let column = actual_pos - line_start;

                // Skip matches in import statements
                if current_line.trim_start().starts_with("use ") {
                    start_pos = actual_pos + pattern.len();
                    continue;
                }

                // Skip matches in single-line comments
                if let Some(comment_start) = current_line.find("//") {
                    let comment_start_abs = line_start + comment_start;
                    if actual_pos >= comment_start_abs {
                        start_pos = actual_pos + pattern.len();
                        continue;
                    }
                }

                // Skip matches in multi-line comments
                let mut is_in_multiline_comment = false;
                let mut search_pos = 0;
                while search_pos < actual_pos {
                    if let Some(comment_start) = content[search_pos..].find("/*") {
                        let comment_start_abs = search_pos + comment_start;
                        if comment_start_abs < actual_pos {
                            if let Some(comment_end) = content[comment_start_abs + 2..].find("*/") {
                                let comment_end_abs = comment_start_abs + 2 + comment_end + 2;
                                if actual_pos < comment_end_abs {
                                    is_in_multiline_comment = true;
                                    break;
                                }
                                search_pos = comment_end_abs;
                            } else {
                                // Unclosed comment, assume everything after is commented
                                is_in_multiline_comment = true;
                                break;
                            }
                        } else {
                            break;
                        }
                    } else {
                        break;
                    }
                }

                if is_in_multiline_comment {
                    start_pos = actual_pos + pattern.len();
                    continue;
                }

                // Create diagnostic for custom pattern
                let diagnostic = DiagnosticBuilder::create(
                    tower_lsp::lsp_types::Range {
                        start: tower_lsp::lsp_types::Position {
                            line: lines_before as u32,
                            character: column as u32,
                        },
                        end: tower_lsp::lsp_types::Position {
                            line: lines_before as u32,
                            character: (column + pattern.len()) as u32,
                        },
                    },
                    format!("Custom pattern '{}' detected. {}", pattern, self.message()),
                    self.config
                        .severity_override
                        .unwrap_or(self.default_severity()),
                    format!("{}_CUSTOM", self.id()),
                    None,
                );

                self.diagnostics.push(diagnostic);
                start_pos = actual_pos + pattern.len();
            }
        }
    }
}

impl Detector for UnsafeMathDetector {
    fn id(&self) -> &'static str {
        "UNSAFE_ARITHMETIC"
    }

    fn name(&self) -> &'static str {
        "Unsafe Math Operations"
    }

    fn description(&self) -> &'static str {
        "Detects unchecked arithmetic operations that could lead to overflow/underflow vulnerabilities"
    }

    fn message(&self) -> &'static str {
        "Unchecked arithmetic operation detected. Consider using checked_add(), checked_sub(), checked_mul(), or checked_div() to prevent overflow/underflow."
    }

    fn default_severity(&self) -> DiagnosticSeverity {
        DiagnosticSeverity::ERROR
    }

    fn analyze(&mut self, content: &str) -> Vec<Diagnostic> {
        self.diagnostics.clear();

        // Run default detection logic
        if let Ok(syntax_tree) = parse_str::<syn::File>(content) {
            self.visit_file(&syntax_tree);
        }

        // Check custom patterns
        self.check_custom_patterns(content);

        self.diagnostics.clone()
    }

    fn should_run(&self, content: &str) -> bool {
        // Always run if custom patterns are configured
        if !self.config.custom_patterns.is_empty() {
            return content.contains("anchor_lang") || content.contains("anchor_spl");
        }

        // Run on Rust files that contain arithmetic operations and anchor imports
        if !(content.contains("anchor_lang") || content.contains("anchor_spl")) {
            return false;
        }

        // Look for arithmetic operations in more specific contexts to avoid false positives
        // from import statements like "use anchor_lang::prelude::*;"
        let lines: Vec<&str> = content.lines().collect();
        for line in lines {
            let trimmed = line.trim();
            // Skip import lines and comments
            if trimmed.starts_with("use ") || trimmed.starts_with("//") || trimmed.starts_with("/*")
            {
                continue;
            }

            // Look for arithmetic operators in actual code
            if trimmed.contains(" + ")
                || trimmed.contains(" - ")
                || trimmed.contains(" * ")
                || trimmed.contains(" / ")
                || trimmed.contains("+=")
                || trimmed.contains("-=")
                || trimmed.contains("*=")
                || trimmed.contains("/=")
            {
                return true;
            }
        }

        false
    }
}

impl<'ast> Visit<'ast> for UnsafeMathDetector {
    fn visit_expr_binary(&mut self, node: &'ast syn::ExprBinary) {
        if let BinOp::Add(_) = node.op {
            let severity = self
                .config
                .severity_override
                .unwrap_or(self.default_severity());
            self.diagnostics.push(DiagnosticBuilder::create(
                DiagnosticBuilder::create_range_from_span(node.span()),
                self.message().to_string(),
                severity,
                self.id().to_string(),
                None,
            ));
        }

        // Continue visiting children
        syn::visit::visit_expr_binary(self, node);
    }
}
