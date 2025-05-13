import * as vscode from "vscode";
import { ExtensionFeatureManagers } from "./extensionFeatureManagers";

function registerCommands(
  context: vscode.ExtensionContext,
  extensionFeatureManagers: ExtensionFeatureManagers
) {
  context.subscriptions.push(
    vscode.commands.registerCommand("tridentCoverage.show-coverage", async () => {
      await extensionFeatureManagers.coverageManager.showCoverage();
    })
  );
  context.subscriptions.push(
    vscode.commands.registerCommand("tridentCoverage.close-coverage", async () => {
      await extensionFeatureManagers.coverageManager.closeCoverage();
    })
  );
}

export default registerCommands;
