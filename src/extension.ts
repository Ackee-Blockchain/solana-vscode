import * as vscode from "vscode";
import registerCommands from "./commands";
import { initExtensionFeatureManagers } from "./extensionFeatureManagers";

export function activate(context: vscode.ExtensionContext) {
  let extensionFeatureManagers = initExtensionFeatureManagers();
  Object.values(extensionFeatureManagers).forEach((manager) => {
    context.subscriptions.push(manager);
  });

  registerCommands(context, extensionFeatureManagers);
}

export function deactivate() {}
