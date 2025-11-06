import * as vscode from "vscode";

// Shared output channel used across the extension
const SOLANA_OUTPUT_CHANNEL =
  vscode.window.createOutputChannel("Solana Extension");

export { SOLANA_OUTPUT_CHANNEL };
