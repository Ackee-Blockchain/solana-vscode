import * as vscode from "vscode";
import { ExtensionFeatureManagers } from "./extensionFeatureManagers";
import { CLOSE_COVERAGE, SHOW_COVERAGE } from "./coverage/commands";
import { RELOAD_DETECTORS, SCAN_WORKSPACE, SHOW_SCAN_OUTPUT } from "./detectors/commands";

function registerCommands(
  context: vscode.ExtensionContext,
  extensionFeatureManagers: ExtensionFeatureManagers
) {
  context.subscriptions.push(
    vscode.commands.registerCommand(SHOW_COVERAGE, async () => {
      await extensionFeatureManagers.coverageManager.showCoverage();
    })
  );
  context.subscriptions.push(
    vscode.commands.registerCommand(CLOSE_COVERAGE, async () => {
      await extensionFeatureManagers.coverageManager.closeCoverage();
    })
  );

  context.subscriptions.push(
    vscode.commands.registerCommand(SHOW_SCAN_OUTPUT, async () => {
      extensionFeatureManagers.detectorsManager.showOutput();
    })
  );

  // Add command to manually scan workspace
  context.subscriptions.push(
    vscode.commands.registerCommand(SCAN_WORKSPACE, async () => {
      vscode.window.showInformationMessage("Scanning workspace for security issues...");
      await extensionFeatureManagers.detectorsManager.scanWorkspace();
    })
  );

  // Add command to reload detectors
  context.subscriptions.push(
    vscode.commands.registerCommand(RELOAD_DETECTORS, async () => {
      vscode.window.showInformationMessage("Reloading security detectors...");
      await extensionFeatureManagers.detectorsManager.reloadDetectors();
    })
  );
}

export default registerCommands;
