import * as vscode from 'vscode';
import * as path from 'path';
import * as fs from 'fs';
import { exec } from 'child_process';
import { promisify } from 'util';

const execAsync = promisify(exec);

export enum StatusBarState {
    Chill = 'chill',
    Running = 'running',
    Warn = 'warn',
    Error = 'error',
}

export class StatusBarManager implements vscode.Disposable {
    private statusBarItem: vscode.StatusBarItem;
    private currentState: StatusBarState = StatusBarState.Chill;
    private rustToolchainChecked: boolean = false;
    private isNightlyAvailable: boolean = false;
    private extensionVersion: string = 'unknown';

    constructor() {
        // Get extension version
        const extension = vscode.extensions.getExtension('AckeeBlockchain.solana');
        if (extension) {
            this.extensionVersion = extension.packageJSON.version || 'unknown';
        }
        // Create status bar item with priority 100 (default)
        this.statusBarItem = vscode.window.createStatusBarItem(
            vscode.StatusBarAlignment.Left,
            100
        );
        this.statusBarItem.name = 'Solana';

        // Set initial state to running (will be updated after toolchain check and server connection)
        this.updateStatus(StatusBarState.Running, 'Initializing...');
        this.statusBarItem.show();

        // Check Rust toolchain on initialization (async, will update status when done)
        this.checkRustToolchain().then(() => {
            // After toolchain check, if still in initial state, update to chill or warn
            if (this.currentState === StatusBarState.Running) {
                if (!this.isNightlyAvailable) {
                    this.updateStatus(StatusBarState.Warn, 'Nightly Rust version not used. Click to install nightly.');
                } else {
                    this.updateStatus(StatusBarState.Chill, 'Solana extension is ready');
                }
            }
        });
    }

    /**
     * Create a rich tooltip with clickable actions (rust-analyzer style)
     * Actions are directly clickable in the tooltip using command links
     */
    private createRichTooltip(state: StatusBarState, message?: string): vscode.MarkdownString {
        const tooltip = new vscode.MarkdownString();
        tooltip.isTrusted = true;

        // Header with version info (rust-analyzer style)
        tooltip.appendMarkdown('### Solana Language Server\n\n');
        tooltip.appendMarkdown(`**Extension:** v${this.extensionVersion}\n`);
        tooltip.appendMarkdown('\n---\n\n');

        switch (state) {
            case StatusBarState.Chill:
                tooltip.appendMarkdown(message || 'Ready');
                if (this.isNightlyAvailable) {
                    tooltip.appendMarkdown('\n\n✅ Nightly Rust toolchain available');
                }
                tooltip.appendMarkdown('\n\n---\n\n');
                tooltip.appendMarkdown('**Actions:**\n\n');
                tooltip.appendMarkdown(`🔄 [Reload Detectors](command:solana.reloadDetectors "Reload all security detectors")\n`);
                break;

            case StatusBarState.Running:
                tooltip.appendMarkdown(message || 'Running...');
                break;

            case StatusBarState.Warn:
                tooltip.appendMarkdown('⚠️ ' + (message || 'Nightly Rust version not used'));
                tooltip.appendMarkdown('\n\n---\n\n');
                tooltip.appendMarkdown('**Actions:**\n\n');
                tooltip.appendMarkdown(`⬇️ [Install Nightly Rust](command:solana.installNightly "Install nightly Rust toolchain")\n`);
                break;

            case StatusBarState.Error:
                tooltip.appendMarkdown('❌ ' + (message || 'Error occurred'));
                tooltip.appendMarkdown('\n\n---\n\n');
                tooltip.appendMarkdown('**Actions:**\n\n');
                tooltip.appendMarkdown(`ℹ️ [Show Details](command:solana.showStatusDetails "View error details")\n`);
                break;
        }

        return tooltip;
    }

    /**
     * Update the status bar badge based on the current state
     */
    updateStatus(state: StatusBarState, message?: string): void {
        this.currentState = state;

        switch (state) {
            case StatusBarState.Chill:
                // Check nightly status when transitioning to chill
                // If nightly is not available, show warning instead
                if (!this.isNightlyAvailable && this.rustToolchainChecked) {
                    this.statusBarItem.text = '$(warning) Solana';
                    this.statusBarItem.tooltip = this.createRichTooltip(StatusBarState.Warn, message);
                    this.statusBarItem.color = new vscode.ThemeColor('statusBarItem.warningForeground');
                    this.statusBarItem.command = 'solana.installNightly';
                } else {
                    this.statusBarItem.text = 'Solana';
                    this.statusBarItem.tooltip = this.createRichTooltip(StatusBarState.Chill, message);
                    this.statusBarItem.color = undefined; // Use default color
                    this.statusBarItem.command = undefined; // Actions are in tooltip, no click needed
                }
                break;

            case StatusBarState.Running:
                this.statusBarItem.text = '$(sync~spin) Solana';
                this.statusBarItem.tooltip = this.createRichTooltip(StatusBarState.Running, message);
                this.statusBarItem.color = undefined;
                this.statusBarItem.command = undefined; // Disable click during running
                break;

            case StatusBarState.Warn:
                this.statusBarItem.text = '$(warning) Solana';
                this.statusBarItem.tooltip = this.createRichTooltip(StatusBarState.Warn, message);
                this.statusBarItem.color = new vscode.ThemeColor('statusBarItem.warningForeground');
                this.statusBarItem.command = undefined; // Actions are in tooltip
                break;

            case StatusBarState.Error:
                this.statusBarItem.text = '$(error) Solana';
                this.statusBarItem.tooltip = this.createRichTooltip(StatusBarState.Error, message);
                this.statusBarItem.color = new vscode.ThemeColor('statusBarItem.errorForeground');
                this.statusBarItem.command = undefined; // Actions are in tooltip
                break;
        }

        this.statusBarItem.show();
    }

    /**
     * Check if nightly Rust toolchain is available/configured
     */
    private async checkRustToolchain(): Promise<void> {
        if (this.rustToolchainChecked) {
            return;
        }

        try {
            // First, check workspace toolchain files
            const workspaceFolders = vscode.workspace.workspaceFolders;
            if (workspaceFolders && workspaceFolders.length > 0) {
                const workspaceRoot = workspaceFolders[0].uri.fsPath;

                // Check for rust-toolchain.toml
                const rustToolchainToml = path.join(workspaceRoot, 'rust-toolchain.toml');
                if (fs.existsSync(rustToolchainToml)) {
                    const content = fs.readFileSync(rustToolchainToml, 'utf-8');
                    if (content.includes('channel = "nightly"') || content.includes('channel="nightly"')) {
                        this.isNightlyAvailable = true;
                        this.rustToolchainChecked = true;
                        return;
                    }
                }

                // Check for rust-toolchain file (without .toml extension)
                const rustToolchain = path.join(workspaceRoot, 'rust-toolchain');
                if (fs.existsSync(rustToolchain)) {
                    const content = fs.readFileSync(rustToolchain, 'utf-8');
                    if (content.includes('nightly')) {
                        this.isNightlyAvailable = true;
                        this.rustToolchainChecked = true;
                        return;
                    }
                }
            }

            // Fallback: check if nightly is installed (regardless of whether it's active)
            try {
                const { stdout } = await execAsync('rustup toolchain list');
                // Check if any nightly toolchain is installed
                // Format: "nightly-YYYY-MM-DD-x86_64-apple-darwin (default)" or "nightly-YYYY-MM-DD-x86_64-apple-darwin"
                if (stdout.includes('nightly')) {
                    this.isNightlyAvailable = true;
                    this.rustToolchainChecked = true;
                    return;
                }
            } catch {
                // rustup might not be available
            }

            // No nightly found
            this.isNightlyAvailable = false;
            this.rustToolchainChecked = true;

            // Update status if we're in chill state and nightly is not available
            // Only update to warn if we're currently in chill state (not running or error)
            if (this.currentState === StatusBarState.Chill) {
                this.updateStatus(StatusBarState.Warn, 'Nightly Rust version not used. Click to install nightly.');
            }
        } catch (error) {
            console.error('Error checking Rust toolchain:', error);
            this.rustToolchainChecked = true;
        }
    }

    /**
     * Get current state
     */
    getCurrentState(): StatusBarState {
        return this.currentState;
    }

    /**
     * Check if nightly is available
     */
    isNightlyRustAvailable(): boolean {
        return this.isNightlyAvailable;
    }

    /**
     * Re-check Rust toolchain (useful after installing nightly)
     */
    async recheckRustToolchain(): Promise<void> {
        this.rustToolchainChecked = false;
        await this.checkRustToolchain();

        // Update status based on new toolchain check
        if (this.currentState === StatusBarState.Warn && this.isNightlyAvailable) {
            this.updateStatus(StatusBarState.Chill, 'Nightly Rust is now available');
        } else if (this.currentState === StatusBarState.Chill && !this.isNightlyAvailable) {
            this.updateStatus(StatusBarState.Warn, 'Nightly Rust version not used. Click to install nightly.');
        }
    }

    dispose(): void {
        this.statusBarItem.dispose();
    }
}
