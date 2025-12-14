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
    private isDylintDriverAvailable: boolean = false;
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

        // Check Rust toolchain and dylint-driver on initialization (async, will update status when done)
        this.checkRustToolchain().then(() => {
            // After toolchain check, if still in initial state, update to chill or warn
            if (this.currentState === StatusBarState.Running) {
                if (!this.isNightlyAvailable) {
                    this.updateStatus(StatusBarState.Warn, 'Rust nightly-2025-09-18 not installed.');
                } else if (!this.isDylintDriverAvailable) {
                    this.updateStatus(StatusBarState.Warn, 'dylint-driver not installed.');
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
                    tooltip.appendMarkdown('\n\n✅ Rust nightly-2025-09-18 available');
                }
                if (this.isDylintDriverAvailable) {
                    tooltip.appendMarkdown('\n\n✅ dylint-driver available');
                }
                tooltip.appendMarkdown('\n\n---\n\n');
                tooltip.appendMarkdown('**Actions:**\n\n');
                tooltip.appendMarkdown(`🔄 [Reload Detectors](command:solana.reloadDetectors "Reload all security detectors")\n`);
                break;

            case StatusBarState.Running:
                tooltip.appendMarkdown(message || 'Running...');
                break;

            case StatusBarState.Warn:
                tooltip.appendMarkdown('⚠️ ' + (message || 'Setup required'));
                tooltip.appendMarkdown('\n\n---\n\n');
                if (!this.isNightlyAvailable) {
                    tooltip.appendMarkdown('❌ Rust nightly-2025-09-18 not installed\n\n');
                } else {
                    tooltip.appendMarkdown('✅ Rust nightly-2025-09-18 installed\n\n');
                }
                if (!this.isDylintDriverAvailable) {
                    tooltip.appendMarkdown('❌ dylint-driver not installed\n\n');
                } else {
                    tooltip.appendMarkdown('✅ dylint-driver installed\n\n');
                }
                tooltip.appendMarkdown('---\n\n');
                tooltip.appendMarkdown('**Actions:**\n\n');
                if (!this.isNightlyAvailable) {
                    tooltip.appendMarkdown(`⬇️ [Install nightly-2025-09-18](command:solana.installNightly "Install Rust nightly-2025-09-18 toolchain")\n`);
                }
                if (!this.isDylintDriverAvailable) {
                    tooltip.appendMarkdown(`⬇️ [Install dylint-driver](command:solana.installDylintDriver "Install dylint-driver")\n`);
                }
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
                // If required toolchain or dylint-driver is not available, show warning instead
                if ((!this.isNightlyAvailable || !this.isDylintDriverAvailable) && this.rustToolchainChecked) {
                    this.statusBarItem.text = '$(warning) Solana';
                    this.statusBarItem.tooltip = this.createRichTooltip(StatusBarState.Warn, message);
                    this.statusBarItem.color = new vscode.ThemeColor('statusBarItem.warningForeground');
                    this.statusBarItem.command = undefined; // Actions are in tooltip
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
     * Check if nightly Rust toolchain and dylint-driver are available/configured
     */
    private async checkRustToolchain(): Promise<void> {
        if (this.rustToolchainChecked) {
            return;
        }

        const REQUIRED_TOOLCHAIN = 'nightly-2025-09-18';

        try {
            // Check if the specific required toolchain is installed
            try {
                const { stdout } = await execAsync('rustup toolchain list');
                // Check if the specific nightly toolchain is installed
                // Format: "nightly-2025-09-18-x86_64-apple-darwin (default)" or "nightly-2025-09-18-x86_64-apple-darwin"
                if (stdout.includes(REQUIRED_TOOLCHAIN)) {
                    this.isNightlyAvailable = true;
                }
            } catch {
                // rustup might not be available
            }

            // Check if dylint-driver is installed
            await this.checkDylintDriver();

            this.rustToolchainChecked = true;

            // Update status if we're in chill state and something is not available
            // Only update to warn if we're currently in chill state (not running or error)
            if (this.currentState === StatusBarState.Chill) {
                if (!this.isNightlyAvailable) {
                    this.updateStatus(StatusBarState.Warn, `Rust ${REQUIRED_TOOLCHAIN} not installed.`);
                } else if (!this.isDylintDriverAvailable) {
                    this.updateStatus(StatusBarState.Warn, 'dylint-driver not installed.');
                }
            }
        } catch (error) {
            console.error('Error checking Rust toolchain:', error);
            this.rustToolchainChecked = true;
        }
    }

    /**
     * Check if dylint-driver is installed
     */
    private async checkDylintDriver(): Promise<void> {
        const REQUIRED_TOOLCHAIN = 'nightly-2025-09-18';
        try {
            // Get home directory
            const homeDir = process.env.HOME || process.env.USERPROFILE;
            if (!homeDir) {
                this.isDylintDriverAvailable = false;
                return;
            }

            // Determine platform-specific path
            const arch = process.arch === 'x64' ? 'x86_64' : process.arch === 'arm64' ? 'aarch64' : process.arch;
            const os = process.platform === 'darwin' ? 'apple-darwin' :
                      process.platform === 'linux' ? 'unknown-linux-gnu' : 'unknown';

            const toolchainTarget = `${REQUIRED_TOOLCHAIN}-${arch}-${os}`;
            const dylintDriverPath = path.join(homeDir, '.dylint_drivers', toolchainTarget, 'dylint-driver');

            // Check if dylint-driver exists
            this.isDylintDriverAvailable = fs.existsSync(dylintDriverPath);
        } catch (error) {
            console.error('Error checking dylint-driver:', error);
            this.isDylintDriverAvailable = false;
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
     * Check if dylint-driver is available
     */
    isDylintDriverInstalled(): boolean {
        return this.isDylintDriverAvailable;
    }

    /**
     * Re-check Rust toolchain (useful after installing nightly)
     */
    async recheckRustToolchain(): Promise<void> {
        this.rustToolchainChecked = false;
        await this.checkRustToolchain();

        // Update status based on new toolchain check
        if (this.currentState === StatusBarState.Warn && this.isNightlyAvailable && this.isDylintDriverAvailable) {
            this.updateStatus(StatusBarState.Chill, 'All requirements installed');
        } else if (this.currentState === StatusBarState.Chill && (!this.isNightlyAvailable || !this.isDylintDriverAvailable)) {
            if (!this.isNightlyAvailable) {
                this.updateStatus(StatusBarState.Warn, 'Rust nightly-2025-09-18 not installed.');
            } else if (!this.isDylintDriverAvailable) {
                this.updateStatus(StatusBarState.Warn, 'dylint-driver not installed.');
            }
        }
    }

    dispose(): void {
        this.statusBarItem.dispose();
    }
}
