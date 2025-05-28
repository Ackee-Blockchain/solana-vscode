import { OutputChannel, window, workspace } from 'vscode';
import { LanguageClient, LanguageClientOptions, RevealOutputChannelOn, ServerOptions, StateChangeEvent, TransportKind } from 'vscode-languageclient/node';

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
    private client: LanguageClient;
    private outputChannel: OutputChannel;

    constructor() {
        console.log('Security Server initialized');
        this.outputChannel = window.createOutputChannel('Security Server');
        this.outputChannel.show(true);

        const config = workspace.getConfiguration('server');
        const serverPath = config.get<string>('path');

        if (!serverPath) {
            this.outputChannel.appendLine('No server path found in settings');
            throw new Error('Server path not configured');
        }

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
        this.client.stop();
        this.outputChannel.dispose();
    }

    // Add method to trigger manual workspace scan
    async triggerWorkspaceScan() {
        this.outputChannel.appendLine('Scan request sent to language server\n\n\n');
        this.outputChannel.appendLine('=== Manual Workspace Scan Triggered ===');
        try {
            // Send a custom request to trigger workspace scan
            await this.client.sendRequest('workspace/executeCommand', {
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
