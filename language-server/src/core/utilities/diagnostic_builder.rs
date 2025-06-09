use crate::core::utilities::ast_analyzer::AstAnalyzer;
use proc_macro2::Span;
use syn::spanned::Spanned;
use tower_lsp::lsp_types::{Diagnostic, DiagnosticSeverity, Position, Range};

/// Enhanced diagnostic builder with span-aware functionality
pub struct DiagnosticBuilder;

impl DiagnosticBuilder {
    /// Create a diagnostic with the given parameters
    pub fn create(
        range: Range,
        message: String,
        severity: DiagnosticSeverity,
        code: String,
        source: Option<String>,
    ) -> Diagnostic {
        Diagnostic {
            range,
            severity: Some(severity),
            code: Some(tower_lsp::lsp_types::NumberOrString::String(code)),
            message,
            source: source.or_else(|| Some("anchor-security".to_string())),
            ..Default::default()
        }
    }

    /// Create a diagnostic from a span
    pub fn create_from_span(
        content: &str,
        span: Span,
        message: String,
        severity: DiagnosticSeverity,
        code: String,
    ) -> Diagnostic {
        let range = AstAnalyzer::span_to_range(content, span);
        Self::create(range, message, severity, code, None)
    }

    /// Create a diagnostic from a spanned AST node
    #[allow(dead_code)]
    pub fn from_spanned<T: Spanned>(
        content: &str,
        node: &T,
        message: String,
        severity: DiagnosticSeverity,
        code: String,
    ) -> Diagnostic {
        let span = AstAnalyzer::get_span(node);
        Self::create_from_span(content, span, message, severity, code)
    }

    /// Create a range from line and character positions
    #[allow(dead_code)]
    pub fn create_range(start_line: u32, start_char: u32, end_line: u32, end_char: u32) -> Range {
        Range {
            start: Position {
                line: start_line - 1,
                character: start_char,
            },
            end: Position {
                line: end_line - 1,
                character: end_char,
            },
        }
    }

    pub fn create_range_from_span(span: Span) -> Range {
        Range {
            start: Position {
                line: (span.start().line as u32) - 1,
                character: span.start().column as u32,
            },
            end: Position {
                line: (span.end().line as u32) - 1,
                character: span.end().column as u32,
            },
        }
    }

    /// Create a simple diagnostic for a single line
    #[allow(dead_code)]
    pub fn create_line_diagnostic(
        line: u32,
        message: String,
        severity: DiagnosticSeverity,
        code: String,
    ) -> Diagnostic {
        let range = Self::create_range(line, 0, line, 100);
        Self::create(range, message, severity, code, None)
    }
}
