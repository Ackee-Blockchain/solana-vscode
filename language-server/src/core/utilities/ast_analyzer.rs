use proc_macro2::Span;
use syn::File;
use syn::spanned::Spanned;
use tower_lsp::lsp_types::{Position, Range};

/// Enhanced AST analyzer with better span tracking and utilities
pub struct AstAnalyzer;

impl AstAnalyzer {
    /// Parse content into a syn::File AST with error handling
    pub fn parse_file(content: &str) -> Result<File, syn::Error> {
        syn::parse_str::<File>(content)
    }

    /// Convert a proc_macro2::Span to LSP Range (simplified for now)
    pub fn span_to_range(_content: &str, _span: Span) -> Range {
        // For now, return a simple range. In a full implementation,
        // you would need to track source positions more carefully
        Range {
            start: Position {
                line: 0,
                character: 0,
            },
            end: Position {
                line: 0,
                character: 10,
            },
        }
    }

    /// Get the span of any spanned AST node
    pub fn get_span<T: Spanned>(node: &T) -> Span {
        node.span()
    }

    /// Convert byte offset to line/column position
    pub fn byte_offset_to_position(content: &str, offset: usize) -> Position {
        let mut line = 0u32;
        let mut character = 0u32;

        for (i, ch) in content.char_indices() {
            if i >= offset {
                break;
            }
            if ch == '\n' {
                line += 1;
                character = 0;
            } else {
                character += 1;
            }
        }

        Position { line, character }
    }

    /// Check if a string contains any of the given patterns
    pub fn contains_any(text: &str, patterns: &[&str]) -> bool {
        patterns.iter().any(|pattern| text.contains(pattern))
    }

    /// Extract function names from AST
    pub fn extract_function_names(file: &File) -> Vec<String> {
        use syn::visit::Visit;

        struct FunctionVisitor {
            names: Vec<String>,
        }

        impl<'ast> Visit<'ast> for FunctionVisitor {
            fn visit_item_fn(&mut self, node: &'ast syn::ItemFn) {
                self.names.push(node.sig.ident.to_string());
                syn::visit::visit_item_fn(self, node);
            }
        }

        let mut visitor = FunctionVisitor { names: Vec::new() };
        visitor.visit_file(file);
        visitor.names
    }

    /// Check if an expression is within a specific context (e.g., inside a macro call)
    pub fn is_in_context<F>(expr: &syn::Expr, predicate: F) -> bool
    where
        F: Fn(&syn::Expr) -> bool,
    {
        predicate(expr)
    }

    /// Find all attribute usages in the file
    pub fn find_attributes<'a>(file: &'a File, attr_name: &'a str) -> Vec<&'a syn::Attribute> {
        use syn::visit::Visit;

        struct AttrVisitor<'a> {
            target_name: &'a str,
            found: Vec<&'a syn::Attribute>,
        }

        impl<'a, 'ast> Visit<'ast> for AttrVisitor<'a>
        where
            'ast: 'a,
        {
            fn visit_attribute(&mut self, node: &'ast syn::Attribute) {
                if node.path().is_ident(self.target_name) {
                    self.found.push(node);
                }
            }
        }

        let mut visitor = AttrVisitor {
            target_name: attr_name,
            found: Vec::new(),
        };
        visitor.visit_file(file);
        visitor.found
    }
}