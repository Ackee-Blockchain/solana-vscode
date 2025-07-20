import * as vscode from 'vscode';
import * as fs from 'fs';
import * as path from 'path';
import * as os from 'os';
import { exec } from 'child_process';
import { promisify } from 'util';

const execAsync = promisify(exec);

// Interface for AI detection result
interface AIDetectionResult {
    filePath: string;
    line: number;
    message: string;
    severity: 'error' | 'warning' | 'info';
    detectorId: string;
}

export class AIDetectorsManager implements vscode.Disposable {
    private outputChannel: vscode.OutputChannel;
    private diagnosticCollection: vscode.DiagnosticCollection;
    private detectionResultsPath: string;
    private detectorMarkdownPath: string;
    private fileWatcher: vscode.FileSystemWatcher | undefined;

    constructor() {
        this.outputChannel = vscode.window.createOutputChannel('AI Security Detectors');
        this.diagnosticCollection = vscode.languages.createDiagnosticCollection('aiSecurityDetectors');

        // Create temp directory for detection results if it doesn't exist
        const tempDir = path.join(os.tmpdir(), 'solana-vscode-ai-detectors');
        if (!fs.existsSync(tempDir)) {
            fs.mkdirSync(tempDir, { recursive: true });
        }

        this.detectionResultsPath = path.join(tempDir, 'detection-results.json');
        this.detectorMarkdownPath = path.join(tempDir, 'detector-description.md');

        // Initialize file watcher to monitor detection results
        this.initFileWatcher();
    }

    private initFileWatcher() {
        // Watch for changes to the detection results file
        this.fileWatcher = vscode.workspace.createFileSystemWatcher(this.detectionResultsPath);

        this.fileWatcher.onDidChange(() => {
            this.loadAndDisplayDetections();
        });

        this.fileWatcher.onDidCreate(() => {
            this.loadAndDisplayDetections();
        });
    }

    private loadAndDisplayDetections() {
        try {
            if (!fs.existsSync(this.detectionResultsPath)) {
                return;
            }

            const content = fs.readFileSync(this.detectionResultsPath, 'utf8');
            const detections: AIDetectionResult[] = JSON.parse(content);

            // Clear previous diagnostics
            this.diagnosticCollection.clear();

            // Group diagnostics by file
            const diagnosticMap = new Map<string, vscode.Diagnostic[]>();

            for (const detection of detections) {
                const range = new vscode.Range(
                    detection.line - 1, 0,
                    detection.line - 1, 100
                );

                const severity = this.mapSeverity(detection.severity);

                const diagnostic = new vscode.Diagnostic(
                    range,
                    detection.message,
                    severity
                );

                diagnostic.source = 'AI Security Detector';
                diagnostic.code = detection.detectorId;

                const diagnostics = diagnosticMap.get(detection.filePath) || [];
                diagnostics.push(diagnostic);
                diagnosticMap.set(detection.filePath, diagnostics);
            }

            // Set diagnostics for each file
            for (const [file, diagnostics] of diagnosticMap.entries()) {
                const uri = vscode.Uri.file(file);
                this.diagnosticCollection.set(uri, diagnostics);
            }

            // Log to output channel
            this.outputChannel.appendLine(`Loaded ${detections.length} AI-based detections`);
        } catch (error) {
            this.outputChannel.appendLine(`Error loading detections: ${error}`);
        }
    }

    private mapSeverity(severity: string): vscode.DiagnosticSeverity {
        switch (severity) {
            case 'error':
                return vscode.DiagnosticSeverity.Error;
            case 'warning':
                return vscode.DiagnosticSeverity.Warning;
            case 'info':
                return vscode.DiagnosticSeverity.Information;
            default:
                return vscode.DiagnosticSeverity.Warning;
        }
    }

    async runAIDetector(detectorName: string) {
        try {
            // Check if Claude CLI is installed
            try {
                await execAsync('claude --version');
            } catch (error) {
                vscode.window.showErrorMessage('Claude CLI is not installed. Please install it to use AI detectors.');
                return;
            }

            // Get active workspace folder
            const workspaceFolders = vscode.workspace.workspaceFolders;
            if (!workspaceFolders || workspaceFolders.length === 0) {
                vscode.window.showErrorMessage('No workspace folder is open');
                return;
            }

            // Use the active workspace or the first one if multiple are open
            let activeWorkspace = workspaceFolders[0].uri.fsPath;

            // If there's an active editor, use its workspace folder
            if (vscode.window.activeTextEditor) {
                const activeFile = vscode.window.activeTextEditor.document.uri;
                for (const folder of workspaceFolders) {
                    if (activeFile.fsPath.startsWith(folder.uri.fsPath)) {
                        activeWorkspace = folder.uri.fsPath;
                        break;
                    }
                }
            }

            this.outputChannel.appendLine(`Using workspace: ${activeWorkspace}`);

            // Get configured Claude model
            const config = vscode.workspace.getConfiguration('aiDetector');
            const claudeModel = config.get<string>('claudeModel') || 'claude-3-opus-20240229';

            // Show progress indicator
            await vscode.window.withProgress({
                location: vscode.ProgressLocation.Notification,
                title: `Running AI ${detectorName} detector`,
                cancellable: true
            }, async (progress, token) => {
                progress.report({ message: 'Analyzing code...' });

                // Read detector description from markdown file if it exists
                if (!fs.existsSync(this.detectorMarkdownPath)) {
                    vscode.window.showErrorMessage(`Detector description file not found: ${this.detectorMarkdownPath}`);
                    return;
                }

                const detectorDescription = fs.readFileSync(this.detectorMarkdownPath, 'utf8');

                // Create a temporary directory for Rust files if it doesn't exist
                const tempRustDir = path.join(os.tmpdir(), 'solana-vscode-ai-detectors', 'rust-files');
                if (!fs.existsSync(tempRustDir)) {
                    fs.mkdirSync(tempRustDir, { recursive: true });
                }

                // Find all Rust files in the workspace
                try {
                    this.outputChannel.appendLine(`Searching for Rust files in ${activeWorkspace}`);

                    // First, check if there's a programs directory
                    const programsDir = path.join(activeWorkspace, 'programs');
                    let rustFiles = [];

                    if (fs.existsSync(programsDir) && fs.statSync(programsDir).isDirectory()) {
                        // If programs directory exists, focus on it
                        this.outputChannel.appendLine(`Found Anchor programs directory: ${programsDir}`);
                        const { stdout } = await execAsync(`find "${programsDir}" -name "*.rs" -type f | grep -v "/target/" | grep -v "/node_modules/"`);
                        rustFiles = stdout.trim().split('\n').filter(file => file);
                        this.outputChannel.appendLine(`Found ${rustFiles.length} Rust files in programs directory`);
                    } else {
                        // If no programs directory, search the entire workspace
                        this.outputChannel.appendLine(`No programs directory found, searching entire workspace`);
                        const { stdout } = await execAsync(`find "${activeWorkspace}" -name "*.rs" -type f | grep -v "/target/" | grep -v "/node_modules/"`);
                        rustFiles = stdout.trim().split('\n').filter(file => file);
                    }

                    if (rustFiles.length === 0) {
                        vscode.window.showWarningMessage('No Rust files found in the workspace');
                        return;
                    }

                    // Prioritize files that are likely to be Anchor program files
                    // Look for files that might contain Accounts structs
                    const anchorProgramFiles = [];
                    const otherFiles = [];

                    for (const file of rustFiles) {
                        try {
                            const content = fs.readFileSync(file, 'utf8');
                            // Check if the file contains Anchor-specific code
                            if (content.includes('#[derive(Accounts)') ||
                                content.includes('use anchor_lang::prelude') ||
                                content.includes('use solana_program')) {
                                anchorProgramFiles.push(file);
                            } else {
                                otherFiles.push(file);
                            }
                        } catch (error) {
                            this.outputChannel.appendLine(`Error reading file ${file}: ${error}`);
                            otherFiles.push(file);
                        }
                    }

                    // Combine the lists, prioritizing Anchor program files
                    rustFiles = [...anchorProgramFiles, ...otherFiles];

                    this.outputChannel.appendLine(`Found ${rustFiles.length} Rust files to analyze (${anchorProgramFiles.length} potential Anchor program files)`);

                    // Prepare the prompt
                    const prompt = `You are a Solana security expert. Analyze the following Rust code for missing signers in Anchor programs according to these instructions:

${detectorDescription}

Output ONLY a JSON array of detections with format: [{filePath: string, line: number, message: string, severity: 'error' | 'warning' | 'info', detectorId: string}]. Do not include any explanatory text, just the JSON.

Here are the Rust files to analyze:`;

                    // Create a temporary file for the combined content
                    const tempFile = path.join(os.tmpdir(), 'solana-vscode-ai-detectors', 'combined-rust-files.txt');

                    // Combine the prompt and first few files (to avoid command line length limits)
                    let combinedContent = prompt + '\n\n';

                    // Limit to first 10 files to avoid overwhelming Claude
                    const filesToProcess = rustFiles.slice(0, 10);

                    for (const file of filesToProcess) {
                        try {
                            const content = fs.readFileSync(file, 'utf8');
                            combinedContent += `\n\nFile: ${file}\n\`\`\`rust\n${content}\n\`\`\`\n`;
                            this.outputChannel.appendLine(`Added file: ${file}`);
                        } catch (error) {
                            this.outputChannel.appendLine(`Error reading file ${file}: ${error}`);
                        }
                    }

                    fs.writeFileSync(tempFile, combinedContent, 'utf8');
                    this.outputChannel.appendLine(`Combined content written to: ${tempFile}`);

                    // Prepare Claude CLI command using the correct syntax
                    const claudeCommand = `claude --print --output-format json < "${tempFile}" > "${this.detectionResultsPath}"`;

                    this.outputChannel.appendLine(`Running command: ${claudeCommand}`);

                    try {
                        // Execute Claude CLI
                        const { stdout, stderr } = await execAsync(claudeCommand, { shell: '/bin/bash' });

                        if (stderr) {
                            this.outputChannel.appendLine(`Error: ${stderr}`);
                        }

                        this.outputChannel.appendLine(`Claude command completed successfully`);
                        this.outputChannel.show();

                        // Load and display detections
                        this.loadAndDisplayDetections();

                    } catch (error) {
                        this.outputChannel.appendLine(`Error executing Claude CLI: ${error}`);
                        vscode.window.showErrorMessage(`Failed to run AI detector: ${error}`);
                    }
                } catch (error) {
                    this.outputChannel.appendLine(`Error finding Rust files: ${error}`);
                    vscode.window.showErrorMessage(`Failed to find Rust files: ${error}`);
                }
            });

        } catch (error) {
            this.outputChannel.appendLine(`Error: ${error}`);
            vscode.window.showErrorMessage(`Error running AI detector: ${error}`);
        }
    }

    async setDetectorDescription(markdownContent: string) {
        try {
            fs.writeFileSync(this.detectorMarkdownPath, markdownContent, 'utf8');
            this.outputChannel.appendLine(`Detector description saved to ${this.detectorMarkdownPath}`);
        } catch (error) {
            this.outputChannel.appendLine(`Error saving detector description: ${error}`);
            vscode.window.showErrorMessage(`Failed to save detector description: ${error}`);
        }
    }

    showOutput() {
        this.outputChannel.show();
    }

    dispose() {
        this.outputChannel.dispose();
        this.diagnosticCollection.dispose();
        this.fileWatcher?.dispose();
    }
}
