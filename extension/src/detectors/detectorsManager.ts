import { OutputChannel, window, workspace } from 'vscode';
import { LanguageClient, LanguageClientOptions, RevealOutputChannelOn, ServerOptions, StateChangeEvent, TransportKind } from 'vscode-languageclient/node';
import * as path from 'path';
import * as fs from 'fs';
import * as os from 'os';
import * as vscode from 'vscode';

// Interface for scan summary data from language server
interface ScanSummary {
    total_rust_files: number;
    anchor_program_files: number;
    files_with_issues: number;
    total_issues: number;
    anchor_configs: number;
    cargo_files: number;
    issues_by_file: FileIssueInfo[];
}

interface FileIssueInfo {
    path: string;
    issue_count: number;
    is_anchor_program: boolean;
    is_test_file: boolean;
}

export class DetectorsManager {
    private client?: LanguageClient;
    private outputChannel: OutputChannel;

    constructor() {
        console.log('Security Server initialized');
        this.outputChannel = window.createOutputChannel('Security Server');
        this.outputChannel.show(true);

        // Improved server path resolution
        const serverPath = this.resolveServerPath();

        if (!serverPath) {
            this.handleServerNotFound();
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
        const arch = process.arch;

        // Determine binary name
        const binaryName = platform === 'win32' ? 'language-server.exe' : 'language-server';

        // Determine platform directory with architecture support
        let platformDir: string;

        if (platform === 'darwin') {
            // Handle Apple Silicon vs Intel Mac
            platformDir = arch === 'arm64' ? 'darwin-arm64' : 'darwin-x64';
        } else if (platform === 'win32') {
            platformDir = arch === 'x64' ? 'win32-x64' : 'win32';
        } else if (platform === 'linux') {
            platformDir = arch === 'x64' ? 'linux-x64' : `linux-${arch}`;
        } else {
            platformDir = platform;
        }

        // Only check the standard bundled location
        const bundledPath = path.join(extensionPath, 'bin', platformDir, binaryName);

        if (fs.existsSync(bundledPath)) {
            return bundledPath;
        }

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
            run: { command: serverPath, transport: TransportKind.stdio },
            debug: { command: serverPath, transport: TransportKind.stdio }
        };

        // Options to control the language client
        const clientOptions: LanguageClientOptions = {
            // Register the server for Rust files
            documentSelector: [{ scheme: 'file', language: 'rust' }],
		    diagnosticCollectionName: 'securityServer',
		    revealOutputChannelOn: RevealOutputChannelOn.Debug,
		    progressOnInitialization: true,
            synchronize: {
               // Notify the server about file changes to '.clientrc files contained in the workspace
            fileEvents: workspace.createFileSystemWatcher('**/*.rs')
            }
        };

        // Create the language client and start the client.
        this.client = new LanguageClient(
            'securityServer',
            'Security Server',
            serverOptions,
            clientOptions
        );

        this.client.start();

        this.client.onDidChangeState((e: StateChangeEvent) => {
		    this.outputChannel.appendLine(`Server state changed: ${e.newState}`);
	    });

	    this.client.onNotification('window/logMessage', (params) => {
		    this.outputChannel.appendLine(params.message);
	    });

        // Listen for scan complete notifications
        this.client.onNotification('solana/scanComplete', (scanSummary: ScanSummary) => {
            this.handleScanComplete(scanSummary);
        });
    }

    private handleScanComplete(scanSummary: ScanSummary) {
        console.log('Received scan complete notification:', scanSummary);

        // Log scan results to output channel
        this.outputChannel.appendLine('=== Workspace Scan Complete ===');
        this.outputChannel.appendLine(`Total Rust files: ${scanSummary.total_rust_files}`);
        this.outputChannel.appendLine(`Anchor programs: ${scanSummary.anchor_program_files}`);
        this.outputChannel.appendLine(`Files with issues: ${scanSummary.files_with_issues}`);
        this.outputChannel.appendLine(`Total security issues: ${scanSummary.total_issues}`);
        this.outputChannel.appendLine(`Anchor.toml files: ${scanSummary.anchor_configs}`);
        this.outputChannel.appendLine(`Cargo.toml files: ${scanSummary.cargo_files}`);

        if (scanSummary.issues_by_file.length > 0) {
            this.outputChannel.appendLine('\n=== Files with Security Issues ===');
            scanSummary.issues_by_file.forEach(file => {
                const fileType = file.is_anchor_program ? '[Anchor]' : '[Rust]';
                const testFlag = file.is_test_file ? '[Test]' : '';
                this.outputChannel.appendLine(`${fileType}${testFlag} ${file.path}: ${file.issue_count} issues`);
            });
        }

        // Show information message to user
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

    dispose() {
        this.client?.stop();
        this.outputChannel.dispose();
    }

    // Add method to trigger manual workspace scan
    async triggerWorkspaceScan() {
        this.outputChannel.appendLine('Scan request sent to language server\n\n\n');
        this.outputChannel.appendLine('=== Manual Workspace Scan Triggered ===');
        try {
            // Send a custom request to trigger workspace scan
            await this.client?.sendRequest('workspace/executeCommand', {
                command: 'workspace.scan',
                arguments: []
            });
        } catch (error) {
            this.outputChannel.appendLine(`Failed to trigger scan: ${error}`);
        }
    }

    // Method to show the output channel
    showOutput() {
        this.outputChannel.show();
    }
}
