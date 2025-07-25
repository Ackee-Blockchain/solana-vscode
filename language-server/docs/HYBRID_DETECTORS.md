# Hybrid Detector System: Syn + Clippy Integration

This document explains how to use and extend the hybrid detector system that combines fast syn-based analysis with comprehensive clippy-style type-aware detection.

## Architecture Overview

The hybrid system provides three types of analysis:

1. **Immediate Syn-based Analysis** - Fast, syntax-only analysis for real-time feedback
2. **Comprehensive Clippy-style Analysis** - Slower, type-aware analysis with compilation context
3. **Hybrid Analysis** - Combines both approaches for optimal coverage

## Key Components

### 1. Detector Traits

```rust
// Base trait for all detectors
pub trait Detector: Send + Sync {
    fn id(&self) -> &'static str;
    fn name(&self) -> &'static str;
    fn description(&self) -> &'static str;
    fn message(&self) -> &'static str;
    fn default_severity(&self) -> DiagnosticSeverity;
    fn analyze(&mut self, content: &str, file_path: Option<&PathBuf>) -> Vec<Diagnostic>;
    fn detector_type(&self) -> DetectorType;
}

// For fast syn-based detectors
pub trait SynDetector: Detector {
    fn analyze_syn(&mut self, content: &str, file_path: Option<&PathBuf>) -> Vec<Diagnostic>;
}

// For comprehensive clippy-style detectors
pub trait ClippyDetector: Detector {
    fn analyze_with_context(&mut self, context: &ClippyAnalysisContext) -> Vec<Diagnostic>;
    fn requires_type_info(&self) -> bool;
    fn supports_incremental(&self) -> bool;
}
```

### 2. ClippyAnalyzer

The `ClippyAnalyzer` manages clippy-style detectors with:
- **Caching** - Avoids re-analysis of unchanged files
- **Background Processing** - Runs analysis asynchronously
- **Temporary Project Setup** - Creates compilation context
- **Type Information** - Provides access to HIR and type data

### 3. DetectorRegistry

The registry manages both detector types:
- **Immediate Analysis** - `analyze_immediate()` for syn detectors
- **Comprehensive Analysis** - `analyze_comprehensive()` for clippy detectors  
- **Hybrid Analysis** - `analyze_hybrid()` combines both approaches

## Implementation Guide

### Creating a Syn-based Detector

```rust
use super::detector::{SynDetector, DetectorType};
use syn::{parse_str, visit::Visit};

#[derive(Default)]
pub struct MySynDetector {
    diagnostics: Vec<Diagnostic>,
}

impl SynDetector for MySynDetector {
    fn id(&self) -> &'static str {
        "MY_SYN_DETECTOR"
    }

    fn name(&self) -> &'static str {
        "My Syn Detector"
    }

    fn description(&self) -> &'static str {
        "Detects something using syntax analysis"
    }

    fn message(&self) -> &'static str {
        "Issue detected by syn analysis"
    }

    fn default_severity(&self) -> DiagnosticSeverity {
        DiagnosticSeverity::WARNING
    }

    fn analyze_syn(&mut self, content: &str, _file_path: Option<&PathBuf>) -> Vec<Diagnostic> {
        self.diagnostics.clear();
        
        if let Ok(syntax_tree) = parse_str::<syn::File>(content) {
            self.visit_file(&syntax_tree);
        }
        
        self.diagnostics.clone()
    }
}

impl<'ast> Visit<'ast> for MySynDetector {
    fn visit_expr(&mut self, node: &'ast syn::Expr) {
        // Your detection logic here
        syn::visit::visit_expr(self, node);
    }
}
```

### Creating a Clippy-style Detector

```rust
use super::detector::{ClippyDetector, ClippyAnalysisContext};

#[derive(Default)]
pub struct MyClippyDetector {
    diagnostics: Vec<Diagnostic>,
}

impl ClippyDetector for MyClippyDetector {
    fn analyze_with_context(&mut self, context: &ClippyAnalysisContext) -> Vec<Diagnostic> {
        self.diagnostics.clear();
        
        // Parse source code
        if let Ok(syntax_tree) = parse_str::<syn::File>(&context.source_code) {
            self.visit_file(&syntax_tree);
            
            // Use compilation context for enhanced analysis
            if context.compilation_result.type_info_available {
                self.enhance_with_type_info(context);
            }
        }
        
        self.diagnostics.clone()
    }

    fn requires_type_info(&self) -> bool {
        true
    }

    fn supports_incremental(&self) -> bool {
        false
    }
}

impl super::detector::Detector for MyClippyDetector {
    fn id(&self) -> &'static str {
        "MY_CLIPPY_DETECTOR"
    }

    fn name(&self) -> &'static str {
        "My Clippy Detector"
    }

    fn description(&self) -> &'static str {
        "Type-aware detection using compilation context"
    }

    fn message(&self) -> &'static str {
        "Issue detected by clippy-style analysis"
    }

    fn default_severity(&self) -> DiagnosticSeverity {
        DiagnosticSeverity::ERROR
    }

    fn analyze(&mut self, content: &str, _file_path: Option<&PathBuf>) -> Vec<Diagnostic> {
        // Fallback syn-based analysis when compilation context unavailable
        // ... basic analysis logic
        Vec::new()
    }

    fn detector_type(&self) -> DetectorType {
        DetectorType::Clippy
    }
}
```

### Registering Detectors

```rust
use language_server::core::{DetectorRegistryBuilder, MySynDetector, MyClippyDetector};

let mut registry = DetectorRegistryBuilder::new()
    // Register syn-based detectors
    .with_detector(MySynDetector::default())
    
    // Register clippy-style detectors
    .with_clippy_detector(MyClippyDetector::default())
    
    // Set workspace root for compilation context
    .with_workspace_root(workspace_path)
    
    .build();
```

## Usage Patterns

### Language Server Integration

```rust
async fn on_change(&self, params: TextDocumentItem) {
    // 1. Immediate feedback (syn-based)
    let immediate_diagnostics = {
        let mut registry = self.detector_registry.lock().await;
        registry.analyze_immediate(&params.text, file_path.as_ref())
    };
    
    // Publish immediate results
    self.client.publish_diagnostics(uri.clone(), immediate_diagnostics, version).await;
    
    // 2. Background comprehensive analysis (clippy-style)
    tokio::spawn(async move {
        let comprehensive_diagnostics = {
            let mut registry = registry.lock().await;
            registry.analyze_comprehensive(&file_path, &text).await
        };
        
        // Merge and republish with comprehensive results
        let all_diagnostics = merge_diagnostics(immediate, comprehensive);
        client.publish_diagnostics(uri, all_diagnostics, version).await;
    });
}
```

### Standalone Analysis

```rust
// For immediate feedback
let syn_results = registry.analyze_immediate(code, Some(&file_path));

// For comprehensive analysis
let clippy_results = registry.analyze_comprehensive(&file_path, code).await;

// For both combined
let hybrid_results = registry.analyze_hybrid(code, Some(&file_path)).await;
```

## Example: Unchecked Arithmetic Detection

The system includes an example comparing syn vs clippy approaches:

### Syn Version (`UnsafeMathDetector`)
- **Fast**: Immediate syntax-based detection
- **Simple**: Pattern matching on AST nodes
- **Limited**: No type information

### Clippy Version (`ClippyUncheckedArithmeticDetector`)
- **Comprehensive**: Uses compilation context
- **Type-aware**: Knows actual types involved
- **Slower**: Requires compilation step
- **Enhanced**: Better suggestions and accuracy

## Performance Characteristics

| Aspect | Syn Detectors | Clippy Detectors |
|--------|---------------|------------------|
| Speed | ~1-10ms | ~100-1000ms |
| Accuracy | Good for syntax | Excellent with types |
| False Positives | Higher | Lower |
| Context | Syntax only | Full compilation |
| Caching | Not needed | Essential |
| Background | Optional | Recommended |

## Best Practices

### 1. Choose the Right Detector Type
- **Use Syn** for: Syntax patterns, code style, simple rules
- **Use Clippy** for: Type-dependent logic, complex analysis, semantic rules

### 2. Implement Both When Beneficial
- Syn version for immediate feedback
- Clippy version for comprehensive analysis
- Same detector ID to avoid duplication

### 3. Optimize Performance
- Cache clippy analysis results
- Use background processing for clippy detectors
- Implement incremental analysis where possible

### 4. Handle Edge Cases
- Provide syn fallback for clippy detectors
- Handle compilation failures gracefully
- Clear caches on significant changes

## Running the Demo

```bash
cd language-server
cargo run --example hybrid_detector_demo
```

This demonstrates:
- Immediate syn-based analysis
- Background clippy-style analysis  
- Hybrid approach combining both
- Performance and accuracy differences

## Future Enhancements

1. **Full rustc Integration**: Direct HIR/MIR access
2. **Incremental Compilation**: Faster clippy analysis
3. **Smart Caching**: Content-aware cache invalidation
4. **Parallel Analysis**: Multiple detectors simultaneously
5. **Custom Lint Integration**: Load external clippy lints

## Troubleshooting

### Common Issues

1. **Clippy analysis not running**
   - Check workspace root is set
   - Verify Cargo.toml exists
   - Check compilation succeeds

2. **Performance problems**
   - Enable caching
   - Use background processing
   - Limit clippy detector frequency

3. **Type information unavailable**
   - Ensure dependencies are available
   - Check compilation succeeds
   - Verify temporary project setup

This hybrid approach provides the best of both worlds: immediate feedback for developers and comprehensive analysis for thorough security checking. 