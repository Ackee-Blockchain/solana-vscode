import { OutputChannel, window, workspace } from 'vscode';
import { LanguageClient, LanguageClientOptions, RevealOutputChannelOn, ServerOptions, StateChangeEvent, TransportKind } from 'vscode-languageclient/node';

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
    }

    dispose() {
        this.client.stop();
        this.outputChannel.dispose();
    }
}
