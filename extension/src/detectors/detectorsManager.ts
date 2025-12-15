import { OutputChannel, window, workspace } from 'vscode';
import { LanguageClient, LanguageClientOptions, RevealOutputChannelOn, ServerOptions, StateChangeEvent, TransportKind, State } from 'vscode-languageclient/node';
import * as path from 'path';
import * as fs from 'fs';
import * as os from 'os';
import * as vscode from 'vscode';
import { SOLANA_OUTPUT_CHANNEL } from '../output';
import { StatusBarState } from '../statusBar/statusBarManager';

// Interface for scan summary data from language server
// Only Rust files are scanned for security issues
interface ScanSummary {
    total_rust_files: number;
    anchor_program_files: number;
    files_with_issues: number;
    total_issues: number;
    issues_by_file: FileIssueInfo[];
    is_manual_scan: boolean;
}

interface FileIssueInfo {
    path: string;
    issue_count: number;
    is_anchor_program: boolean;
}

interface DetectorStatus {
    status: string; // "initializing", "building", "running", "complete", "idle"
    message: string;
}

export class DetectorsManager {
    private client?: LanguageClient;
    private outputChannel: OutputChannel;
    private statusBarUpdateCallback?: (state: StatusBarState, message?: string) => void;

    constructor() {
        console.log('Security Server initialized');
        this.outputChannel = SOLANA_OUTPUT_CHANNEL;

        // Improved server path resolution
        const serverPath = this.resolveServerPath();

        if (!serverPath) {
            this.handleServerNotFound();
            this.updateStatusBar(StatusBarState.Error, 'Language server binary not found');
            return;
        }

        this.outputChannel.appendLine(`Using language server: ${serverPath}`);
        this.startLanguageServer(serverPath);
    }

    private resolveServerPath(): string | null {
        this.outputChannel.appendLine('Resolving language server path...');

        // Strategy 1: User-configured path
        const configuredPath = this.getConfiguredPath();
        if (configuredPath) {
            this.outputChannel.appendLine(`Trying configured path: ${configuredPath}`);
            if (this.validateBinary(configuredPath)) {
                return configuredPath;
            }
            this.outputChannel.appendLine(`Configured path invalid: ${configuredPath}`);
            return null; // If user specified a path but it's invalid, don't fallback
        }

        // Strategy 2: Bundled binary (platform-specific)
        const bundledPath = this.getBundledServerPath();
        if (bundledPath) {
            this.outputChannel.appendLine(`Trying bundled binary: ${bundledPath}`);
            if (this.validateBinary(bundledPath)) {
                return bundledPath;
            }
            this.outputChannel.appendLine(`Bundled binary not found: ${bundledPath}`);
        }

        this.outputChannel.appendLine('No valid language server binary found');
        return null;
    }

    private getConfiguredPath(): string | null {
        const config = workspace.getConfiguration('server');
        const configPath = config.get<string>('path');

        // Handle empty strings and whitespace-only strings
        if (!configPath || configPath.trim() === '') {
            return null;
        }

        // Expand home directory if needed
        if (configPath.startsWith('~/')) {
            return path.join(os.homedir(), configPath.slice(2));
        }

        return configPath;
    }

    private getBundledServerPath(): string | null {
        // Use extension path instead of workspace
        const extensionContext = vscode.extensions.getExtension('AckeeBlockchain.solana');
        if (!extensionContext) {
            this.outputChannel.appendLine('Extension context not found');
            return null;
        }

        const extensionPath = extensionContext.extensionPath;
        const platform = process.platform;

        // Determine binary name based on platform
        const binaryName = platform === 'win32' ? 'language-server.exe' : 'language-server';

        // Look for binary directly in the bin directory
        const bundledPath = path.join(extensionPath, 'bin', binaryName);

        if (fs.existsSync(bundledPath)) {
            return bundledPath;
        }

        this.outputChannel.appendLine(`Binary not found at expected path: ${bundledPath}`);
        return null;
    }

    private validateBinary(binaryPath: string): boolean {
        if (!fs.existsSync(binaryPath)) {
            this.outputChannel.appendLine(`Binary not found: ${binaryPath}`);
            return false;
        }
        return true;
    }

    private handleServerNotFound(): void {
        const message = 'Language server binary not found. Please install or configure the server path.';
        this.outputChannel.appendLine(message);

        // Show user-friendly error with actionable options
        window.showErrorMessage(
            'Solana language server not found',
            'Open Settings',
            'View Output',
            'Learn More'
        ).then(selection => {
            switch (selection) {
                case 'Open Settings':
                    workspace.getConfiguration().update('server.path', '', true);
                    break;
                case 'View Output':
                    this.outputChannel.show();
                    break;
                case 'Learn More':
                    // Could open documentation or README
                    window.showInformationMessage(
                        'Please set "server.path" in your settings to point to your language server binary, ' +
                        'or ensure it\'s installed in a standard location like ~/.cargo/bin/'
                    );
                    break;
            }
        });
    }

    private startLanguageServer(serverPath: string): void {
        // If the extension is launched in debug mode then the debug server options are used
        // Otherwise the run options are used

        const serverOptions: ServerOptions = {
            run: { command: serverPath, transport: TransportKind.stdio, options: { env: { RUST_LOG: 'info' } } },
            debug: { command: serverPath, transport: TransportKind.stdio, options: { env: { RUST_LOG: 'debug' } } }
        };

        // Options to control the language client
        const clientOptions: LanguageClientOptions = {
            // Register the server for Rust files
            documentSelector: [{ scheme: 'file', language: 'rust' }],
		    diagnosticCollectionName: 'solana',
		    revealOutputChannelOn: RevealOutputChannelOn.Debug,
		    progressOnInitialization: true,
            synchronize: {
               // Notify the server about file changes to '.clientrc files contained in the workspace
            fileEvents: workspace.createFileSystemWatcher('**/*.rs')
            }
        };

        // Create the language client and start the client.
        this.client = new LanguageClient(
            'solana-language-server',
            'Solana Language Server',
            serverOptions,
            clientOptions
        );

        this.client.start();

        this.client.onDidChangeState((e: StateChangeEvent) => {
		    this.outputChannel.appendLine(`Server state changed: ${e.newState}`);
            this.updateStatusBarFromServerState(e.newState);
	    });

	    this.client.onNotification('window/logMessage', (params) => {
		    this.outputChannel.appendLine(params.message);
	    });

        // Listen for scan complete notifications
        this.client.onNotification('solana/scanComplete', (scanSummary: ScanSummary) => {
            this.handleScanComplete(scanSummary);
        });

        // Listen for detector status notifications
        this.client.onNotification('solana/detectorStatus', (detectorStatus: DetectorStatus) => {
            this.handleDetectorStatus(detectorStatus);
        });
    }

    private handleScanComplete(scanSummary: ScanSummary) {
        console.log('Received scan complete notification:', scanSummary);

        // Log scan results to output channel
        this.outputChannel.appendLine('=== Workspace Scan Complete ===');
        this.outputChannel.appendLine(`Rust files scanned: ${scanSummary.total_rust_files}`);
        this.outputChannel.appendLine(`Anchor programs detected: ${scanSummary.anchor_program_files}`);
        this.outputChannel.appendLine(`Files with security issues: ${scanSummary.files_with_issues}`);
        this.outputChannel.appendLine(`Total security issues found: ${scanSummary.total_issues}`);

        if (scanSummary.issues_by_file.length > 0) {
            this.outputChannel.appendLine('\n=== Files with Security Issues ===');
            scanSummary.issues_by_file.forEach(file => {
                const fileType = file.is_anchor_program ? '[Anchor]' : '[Rust]';
                this.outputChannel.appendLine(`${fileType} ${file.path}: ${file.issue_count} issues`);
            });
        }

        // Only show notification popups for manual scans
        if (scanSummary.is_manual_scan) {
            if (scanSummary.total_issues > 0) {
                window.showWarningMessage(
                    `Solana security scan found ${scanSummary.total_issues} issues in ${scanSummary.files_with_issues} files. Check the Security Server output for details.`,
                    'Show Output'
                ).then(selection => {
                    if (selection === 'Show Output') {
                        this.outputChannel.show();
                    }
                });
            } else {
                window.showInformationMessage(
                    `Solana security scan completed. No issues found in ${scanSummary.total_rust_files} Rust files.`
                );
            }
        }
    }

    private handleDetectorStatus(detectorStatus: DetectorStatus) {
        console.log('Received detector status notification:', detectorStatus);

        // Update status bar based on detector status
        if (this.statusBarUpdateCallback) {
            switch (detectorStatus.status) {
                case 'initializing':
                case 'building':
                case 'running':
                    this.statusBarUpdateCallback(StatusBarState.Running, detectorStatus.message);
                    break;
                case 'complete':
                case 'idle':
                    this.statusBarUpdateCallback(StatusBarState.Chill, detectorStatus.message);
                    break;
                default:
                    this.outputChannel.appendLine(`Unknown detector status: ${detectorStatus.status}`);
            }
        }

        // Log to output channel
        this.outputChannel.appendLine(`Detector Status: ${detectorStatus.message}`);
    }

    dispose() {
        this.client?.stop();
        this.outputChannel.dispose();
    }

    // Method to show the output channel
    showOutput() {
        this.outputChannel.show();
    }

    // Method to manually trigger a workspace scan
    async scanWorkspace(): Promise<void> {
        if (!this.client) {
            window.showErrorMessage('Language server not running');
            this.updateStatusBar(StatusBarState.Error, 'Language server not running');
            return;
        }

        this.outputChannel.appendLine('Manually triggering workspace scan...');
        this.updateStatusBar(StatusBarState.Running, 'Scanning workspace...');

        try {
            const result = await this.client.sendRequest('workspace/executeCommand', {
                command: 'solana.scanWorkspace',
                arguments: []
            });

            this.outputChannel.appendLine(`Scan request result: ${JSON.stringify(result)}`);
            this.updateStatusBar(StatusBarState.Chill, 'Workspace scan completed');
        } catch (error) {
            this.outputChannel.appendLine(`Error triggering scan: ${error}`);
            window.showErrorMessage(`Failed to scan workspace: ${error}`);
            this.updateStatusBar(StatusBarState.Error, `Failed to scan workspace: ${error}`);
        }
    }

    // Method to reload all detectors
    async reloadDetectors(): Promise<void> {
        if (!this.client) {
            window.showErrorMessage('Language server not running');
            this.updateStatusBar(StatusBarState.Error, 'Language server not running');
            return;
        }

        this.outputChannel.appendLine('Reloading security detectors...');
        this.updateStatusBar(StatusBarState.Running, 'Reloading security detectors...');

        try {
            const result = await this.client.sendRequest('workspace/executeCommand', {
                command: 'solana.reloadDetectors',
                arguments: []
            });

            this.outputChannel.appendLine(`Reload detectors result: ${JSON.stringify(result)}`);
            this.updateStatusBar(StatusBarState.Chill, 'Detectors reloaded successfully');
        } catch (error) {
            this.outputChannel.appendLine(`Error reloading detectors: ${error}`);
            window.showErrorMessage(`Failed to reload detectors: ${error}`);
            this.updateStatusBar(StatusBarState.Error, `Failed to reload detectors: ${error}`);
        }
    }

    /**
     * Set callback for status bar updates
     */
    setStatusBarUpdateCallback(callback: (state: StatusBarState, message?: string) => void): void {
        this.statusBarUpdateCallback = callback;
    }

    /**
     * Update status bar based on server state
     */
    private updateStatusBarFromServerState(state: State): void {
        if (!this.statusBarUpdateCallback) {
            return;
        }

        switch (state) {
            case State.Starting:
                this.statusBarUpdateCallback(StatusBarState.Running, 'Starting language server...');
                break;
            case State.Running:
                this.statusBarUpdateCallback(StatusBarState.Chill, 'Language server is ready');
                break;
            case State.Stopped:
                this.statusBarUpdateCallback(StatusBarState.Error, 'Language server stopped');
                break;
        }
    }

    /**
     * Update status bar
     */
    private updateStatusBar(state: StatusBarState, message?: string): void {
        if (this.statusBarUpdateCallback) {
            this.statusBarUpdateCallback(state, message);
        }
    }
}
