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

    constructor() {
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
                    this.statusBarItem.tooltip = message || 'Nightly Rust version not used. Click to install nightly.';
                    this.statusBarItem.color = new vscode.ThemeColor('statusBarItem.warningForeground');
                    this.statusBarItem.command = 'solana.installNightly';
                } else {
                    this.statusBarItem.text = '$(check) Solana';
                    this.statusBarItem.tooltip = message || 'Solana extension is ready';
                    this.statusBarItem.color = undefined; // Use default color
                    this.statusBarItem.command = 'solana.reloadDetectors';
                }
                break;

            case StatusBarState.Running:
                this.statusBarItem.text = '$(sync~spin) Solana';
                this.statusBarItem.tooltip = message || 'Solana extension is running...';
                this.statusBarItem.color = undefined;
                this.statusBarItem.command = undefined; // Disable click during running
                break;

            case StatusBarState.Warn:
                this.statusBarItem.text = '$(warning) Solana';
                this.statusBarItem.tooltip = message || 'Nightly Rust version not used. Click to install nightly.';
                this.statusBarItem.color = new vscode.ThemeColor('statusBarItem.warningForeground');
                this.statusBarItem.command = 'solana.installNightly';
                break;

            case StatusBarState.Error:
                this.statusBarItem.text = '$(error) Solana';
                this.statusBarItem.tooltip = message || 'Solana extension error';
                this.statusBarItem.color = new vscode.ThemeColor('statusBarItem.errorForeground');
                this.statusBarItem.command = 'solana.showStatusDetails';
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
