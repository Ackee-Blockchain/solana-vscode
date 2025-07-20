import * as vscode from "vscode";
import * as fs from "fs";
import * as path from "path";
import { ExtensionFeatureManagers } from "./extensionFeatureManagers";
import { CLOSE_COVERAGE, SHOW_COVERAGE } from "./coverage/commands";
import { SCAN_WORKSPACE, SHOW_SCAN_OUTPUT } from "./detectors/commands";
import {
  RUN_AI_MISSING_SIGNER_DETECTOR,
  SET_AI_DETECTOR_DESCRIPTION,
  SHOW_AI_DETECTOR_OUTPUT
} from "./detectors/ai/aiCommands";

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
    vscode.commands.registerCommand(SCAN_WORKSPACE, async () => {
      await extensionFeatureManagers.detectorsManager.triggerWorkspaceScan();
    })
  );
  context.subscriptions.push(
    vscode.commands.registerCommand(SHOW_SCAN_OUTPUT, async () => {
      extensionFeatureManagers.detectorsManager.showOutput();
    })
  );

  // AI detector commands
  context.subscriptions.push(
    vscode.commands.registerCommand(RUN_AI_MISSING_SIGNER_DETECTOR, async () => {
      await extensionFeatureManagers.aiDetectorsManager.runAIDetector("Missing Signer");
    })
  );

  context.subscriptions.push(
    vscode.commands.registerCommand(SET_AI_DETECTOR_DESCRIPTION, async () => {
      // Allow user to select a markdown file with detector description
      const fileUris = await vscode.window.showOpenDialog({
        canSelectMany: false,
        filters: {
          'Markdown': ['md']
        },
        title: 'Select Detector Description File'
      });

      if (fileUris && fileUris.length > 0) {
        const fileContent = fs.readFileSync(fileUris[0].fsPath, 'utf8');
        await extensionFeatureManagers.aiDetectorsManager.setDetectorDescription(fileContent);
        vscode.window.showInformationMessage('Detector description updated');
      }
    })
  );

  context.subscriptions.push(
    vscode.commands.registerCommand(SHOW_AI_DETECTOR_OUTPUT, () => {
      extensionFeatureManagers.aiDetectorsManager.showOutput();
    })
  );

  // Initialize default detector description from bundled file
  const extensionPath = context.extensionPath;
  const defaultDescriptionPath = path.join(extensionPath, 'src', 'detectors', 'ai', 'missingSignerDetector.md');

  if (fs.existsSync(defaultDescriptionPath)) {
    const defaultDescription = fs.readFileSync(defaultDescriptionPath, 'utf8');
    extensionFeatureManagers.aiDetectorsManager.setDetectorDescription(defaultDescription)
      .then(() => {
        console.log('Default detector description loaded');
      })
      .catch(error => {
        console.error('Failed to load default detector description', error);
      });
  }
}

export default registerCommands;
