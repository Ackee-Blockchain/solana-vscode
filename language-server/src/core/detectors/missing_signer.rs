use super::detector::Detector;
use super::detector_config::DetectorConfig;
use crate::core::utilities::DiagnosticBuilder;
use syn::spanned::Spanned;
use syn::{Attribute, DeriveInput, Fields, Meta, parse_str, visit::Visit};
use tower_lsp::lsp_types::{Diagnostic, DiagnosticSeverity};

pub struct MissingSignerDetector {
    diagnostics: Vec<Diagnostic>,
    config: DetectorConfig,
}

impl MissingSignerDetector {
    pub fn new() -> Self {
        Self {
            diagnostics: Vec::new(),
            config: DetectorConfig::default(),
        }
    }

    pub fn with_config(config: DetectorConfig) -> Self {
        Self {
            diagnostics: Vec::new(),
            config,
        }
    }

    /// Check if a field has the Signer type
    fn is_signer_field(&self, field: &syn::Field) -> bool {
        if let syn::Type::Path(type_path) = &field.ty {
            if let Some(segment) = type_path.path.segments.last() {
                return segment.ident == "Signer";
            }
        }
        false
    }

    /// Check if a struct has the #[derive(Accounts)] attribute
    fn has_accounts_derive(&self, attrs: &[Attribute]) -> bool {
        for attr in attrs {
            if let Meta::List(meta_list) = &attr.meta {
                if meta_list.path.is_ident("derive") {
                    let tokens = meta_list.tokens.to_string();
                    if tokens.contains("Accounts") {
                        return true;
                    }
                }
            }
        }
        false
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
                let column = actual_pos - line_start;

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

impl Detector for MissingSignerDetector {
    fn id(&self) -> &'static str {
        "MISSING_SIGNER"
    }

    fn name(&self) -> &'static str {
        "Missing Signer Check"
    }

    fn description(&self) -> &'static str {
        "Detects Anchor accounts structs that have no signer accounts, which could allow unauthorized access"
    }

    fn message(&self) -> &'static str {
        "Accounts struct has no signer. Consider adding a Signer<'info> field to ensure proper authorization."
    }

    fn default_severity(&self) -> DiagnosticSeverity {
        DiagnosticSeverity::WARNING
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

        // Run on Anchor files that contain #[derive(Accounts)]
        (content.contains("anchor_lang") || content.contains("anchor_spl"))
            && content.contains("#[derive(Accounts)]")
    }
}

impl<'ast> Visit<'ast> for MissingSignerDetector {
    fn visit_item_struct(&mut self, node: &'ast syn::ItemStruct) {
        // Check if this struct has #[derive(Accounts)]
        if !self.has_accounts_derive(&node.attrs) {
            return;
        }

        // Check if any field is a Signer
        let mut has_signer = false;

        if let Fields::Named(fields) = &node.fields {
            for field in &fields.named {
                if self.is_signer_field(field) {
                    has_signer = true;
                    break;
                }
            }
        }

        // If no signer found, create a diagnostic
        if !has_signer {
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
        syn::visit::visit_item_struct(self, node);
    }
}
