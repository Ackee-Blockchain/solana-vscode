# Dylint Runner

Integrates custom Rust lints (dylint) into the Solana VSCode extension's language server.

## Structure

```
dylint_runner/
├── runner.rs       - Spawns cargo check with dylint-driver, manages execution
├── parser.rs       - Parses cargo JSON output, whitelists only loaded lints
├── diagnostics.rs  - Converts dylint diagnostics to LSP format
└── mod.rs          - Public API
```

## How It Works

1. **Initialization** (`backend.rs` - `initialize` handler):
   - Creates `DylintRunner` with extension path
   - Discovers all `.dylib` files in `lints_compiled/<platform>/`
   - Finds dylint-driver at `~/.dylint_drivers/<toolchain>/dylint-driver`

2. **Running Lints** (`backend.rs` - `execute_command` handler):
   - User triggers command: `solana.runDylintLints` (Keybinding: `Cmd+Alt+D` / `Ctrl+Alt+D`)
   - Runner spawns: `cargo +nightly check --workspace --message-format=json`
   - Sets environment:
     - `RUSTC_WORKSPACE_WRAPPER=<dylint-driver>` 
     - `DYLINT_LIBS=["<lint1.dylib>", "<lint2.dylib>", ...]`
   - Parses JSON output, filters only our custom lints (whitelist approach)
   - Converts relative paths to absolute by joining with workspace path
   - Publishes diagnostics to VS Code

3. **Display**:
   - Diagnostics appear as squiggly lines in the editor
   - Hover shows lint message
   - Source: `dylint`, Code: `<lint_name>`

## Setup

### 1. Build Lints (Required First Time)

```bash
cd lints
./build_all_lints.sh
```

This compiles all lints and copies them to `lints_compiled/<platform>/`.

### 2. Install Dylint Driver (If Not Already Installed)

```bash
cargo install cargo-dylint dylint-link
cargo +nightly-2025-08-07 dylint --list  # Triggers driver installation
```

Driver is installed to: `~/.dylint_drivers/nightly-2025-08-07-<target>/dylint-driver`

## Adding New Lints

1. **Create lint**:
   ```bash
   cd lints
   cargo dylint new my_detector
   ```

2. **Implement the lint** in `lints/my_detector/src/lib.rs`

3. **Build all lints**:
   ```bash
   ./build_all_lints.sh
   ```

4. **Reload extension** - New lint automatically discovered and loaded!

No code changes needed - the runner automatically:
- Discovers all `.dylib` files
- Extracts lint names (e.g., `libmy_detector@...dylib` → `my_detector`)
- Whitelists them in the parser

## Connection to Backend

In `backend.rs`:

```rust
// Initialization
impl LanguageServer for Backend {
    async fn initialize(...) {
        // DylintRunner created and stored in backend
        let runner = DylintRunner::new(&extension_path)?;
        self.dylint_runner = Some(runner);
    }
    
    async fn execute_command(...) {
        match command.as_str() {
            "solana.runDylintLints" => {
                // 1. Run lints
                let diagnostics = runner.run_lints(workspace_path).await?;
                
                // 2. Group by file (convert relative → absolute paths)
                let file_path = workspace_path.join(&diag.file_name);
                
                // 3. Convert to LSP format
                let lsp_diag = diag.to_lsp_diagnostic();
                
                // 4. Publish to VS Code
                client.publish_diagnostics(uri, diagnostics, None).await;
            }
        }
    }
}
```

## Current Limitations

- **Manual trigger only** - Run via command, not automatic on save
- **Full workspace check** - Runs on entire workspace, not individual files
- **No caching** - Rebuilds all workspace code on each run

## Troubleshooting

**"DylintRunner not initialized"**
- Run `cd lints && ./build_all_lints.sh`
- Reload VS Code

**"dylint-driver not found"**
- Install: `cargo install cargo-dylint dylint-link`
- Trigger installation: `cargo +nightly-2025-08-07 dylint --list`

**"No diagnostics shown"**
- Check output panel: "Solana Language Server"
- Look for: `📍 Publishing X diagnostics for: file://...`
- File paths should be absolute (not relative)
