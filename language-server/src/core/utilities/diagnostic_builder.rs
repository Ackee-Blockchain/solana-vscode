use crate::core::utilities::ast_analyzer::AstAnalyzer;
use proc_macro2::Span;
use std::path::Path;
use syn::spanned::Spanned;
use tower_lsp::lsp_types::{
    Diagnostic, DiagnosticRelatedInformation, DiagnosticSeverity, Location, Position, Range, Url,
};

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

    /// Create a diagnostic with related information
    pub fn create_with_related(
        range: Range,
        message: String,
        severity: DiagnosticSeverity,
        code: String,
        source: Option<String>,
        related_range: Range,
        related_message: String,
        file_path: &Path,
    ) -> Diagnostic {
        let mut diagnostic = Self::create(range, message, severity, code, source);
        diagnostic.related_information = Some(vec![DiagnosticRelatedInformation {
            location: Location::new(
                Url::from_file_path(file_path).unwrap_or_else(|_| Url::parse("file:///").unwrap()),
                related_range,
            ),
            message: related_message,
        }]);
        diagnostic
    }

    /// Create a diagnostic with bidirectional related information
    #[allow(dead_code, clippy::too_many_arguments)]
    pub fn create_with_bidirectional_relation(
        primary_range: Range,
        primary_message: String,
        related_range: Range,
        related_message: String,
        primary_to_related_message: String,
        related_to_primary_message: String,
        severity: DiagnosticSeverity,
        code: String,
        source: Option<String>,
        file_path: &Path,
    ) -> (Diagnostic, Diagnostic) {
        let file_url =
            Url::from_file_path(file_path).unwrap_or_else(|_| Url::parse("file:///").unwrap());
        let mut primary = Self::create(
            primary_range,
            primary_message,
            severity,
            code.clone(),
            source.clone(),
        );
        let mut related = Self::create(related_range, related_message, severity, code, source);

        primary.related_information = Some(vec![DiagnosticRelatedInformation {
            location: Location::new(file_url.clone(), related_range),
            message: primary_to_related_message,
        }]);

        related.related_information = Some(vec![DiagnosticRelatedInformation {
            location: Location::new(file_url, primary_range),
            message: related_to_primary_message,
        }]);

        (primary, related)
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

    /// Create a diagnostic from a span with related information
    #[allow(dead_code)]
    pub fn create_from_span_with_related(
        content: &str,
        span: Span,
        message: String,
        severity: DiagnosticSeverity,
        code: String,
        related_span: Span,
        related_message: String,
        file_path: &Path,
    ) -> Diagnostic {
        let range = AstAnalyzer::span_to_range(content, span);
        let related_range = AstAnalyzer::span_to_range(content, related_span);
        Self::create_with_related(
            range,
            message,
            severity,
            code,
            None,
            related_range,
            related_message,
            file_path,
        )
    }

    /// Create a diagnostic from a span with bidirectional related information
    #[allow(dead_code, clippy::too_many_arguments)]
    pub fn create_from_spans_with_bidirectional_relation(
        content: &str,
        primary_span: Span,
        primary_message: String,
        related_span: Span,
        related_message: String,
        primary_to_related_message: String,
        related_to_primary_message: String,
        severity: DiagnosticSeverity,
        code: String,
        file_path: &Path,
    ) -> (Diagnostic, Diagnostic) {
        let primary_range = AstAnalyzer::span_to_range(content, primary_span);
        let related_range = AstAnalyzer::span_to_range(content, related_span);
        Self::create_with_bidirectional_relation(
            primary_range,
            primary_message,
            related_range,
            related_message,
            primary_to_related_message,
            related_to_primary_message,
            severity,
            code,
            None,
            file_path,
        )
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
    pub fn create_range(start_line: u32, start_char: u32, end_line: u32, end_char: u32) -> Range {
        Range {
            start: Position {
                line: start_line,
                character: start_char,
            },
            end: Position {
                line: end_line,
                character: end_char,
            },
        }
    }

    pub fn create_range_from_line(line: u32) -> Range {
        Self::create_range(line, 0, line, 200)
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
