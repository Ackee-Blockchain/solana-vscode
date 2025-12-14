import * as vscode from "vscode";
import { ExtensionFeatureManagers } from "./extensionFeatureManagers";
import { CLOSE_COVERAGE, SHOW_COVERAGE } from "./coverage/commands";
import { RELOAD_DETECTORS, SCAN_WORKSPACE, SHOW_SCAN_OUTPUT } from "./detectors/commands";
import { INSTALL_NIGHTLY, SHOW_STATUS_DETAILS, INSTALL_DYLINT_DRIVER } from "./statusBar/commands";
import { StatusBarState } from "./statusBar/statusBarManager";

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

  // Add command to show status details
  context.subscriptions.push(
    vscode.commands.registerCommand(SHOW_STATUS_DETAILS, async () => {
      const state = extensionFeatureManagers.statusBarManager.getCurrentState();
      const isNightly = extensionFeatureManagers.statusBarManager.isNightlyRustAvailable();
      const isDylintDriver = extensionFeatureManagers.statusBarManager.isDylintDriverInstalled();

      let message = `Solana Extension Status\n\n`;
      message += `State: ${state}\n`;
      message += `Nightly Rust: ${isNightly ? 'Available' : 'Not available'}\n`;
      message += `dylint-driver: ${isDylintDriver ? 'Available' : 'Not available'}\n`;

      if (state === 'error') {
        message += `\nThe language server encountered an error. Check the output channel for details.`;
        extensionFeatureManagers.detectorsManager.showOutput();
      }

      vscode.window.showInformationMessage(message);
    })
  );

  // Add command to install nightly Rust
  context.subscriptions.push(
    vscode.commands.registerCommand(INSTALL_NIGHTLY, async () => {
      const requiredToolchain = 'nightly-2025-09-18';
      const action = await vscode.window.showInformationMessage(
        `Install Rust ${requiredToolchain} toolchain? This will run: rustup toolchain install ${requiredToolchain}`,
        'Install',
        'Cancel'
      );

      if (action === 'Install') {
        const terminal = vscode.window.createTerminal('Install Nightly Rust');
        terminal.sendText(`rustup toolchain install ${requiredToolchain}`);
        terminal.show();

        // Recheck toolchain after a delay
        setTimeout(async () => {
          await extensionFeatureManagers.statusBarManager.recheckRustToolchain();
          if (extensionFeatureManagers.statusBarManager.isNightlyRustAvailable()) {
            const currentState = extensionFeatureManagers.statusBarManager.getCurrentState();
            extensionFeatureManagers.statusBarManager.updateStatus(
              currentState === StatusBarState.Warn
                ? StatusBarState.Chill
                : currentState,
              `Rust ${requiredToolchain} is now available`
            );
          }
        }, 5000);
      }
    })
  );

  // Add command to install dylint-driver
  context.subscriptions.push(
    vscode.commands.registerCommand(INSTALL_DYLINT_DRIVER, async () => {
      const action = await vscode.window.showInformationMessage(
        `Install dylint-driver? This will run: cargo install cargo-dylint dylint-link`,
        'Install',
        'Cancel'
      );

      if (action === 'Install') {
        const terminal = vscode.window.createTerminal('Install dylint-driver');
        terminal.sendText(`cargo install cargo-dylint dylint-link`);
        terminal.show();

        vscode.window.showInformationMessage(
          'Installing dylint-driver... This may take a few minutes. After installation completes, the driver will be initialized automatically.'
        );

        // Recheck after a longer delay (dylint installation takes time)
        setTimeout(async () => {
          await extensionFeatureManagers.statusBarManager.recheckRustToolchain();
          if (extensionFeatureManagers.statusBarManager.isDylintDriverInstalled()) {
            const currentState = extensionFeatureManagers.statusBarManager.getCurrentState();
            extensionFeatureManagers.statusBarManager.updateStatus(
              currentState === StatusBarState.Warn
                ? StatusBarState.Chill
                : currentState,
              'dylint-driver is now available'
            );
          }
        }, 10000);
      }
    })
  );
}

export default registerCommands;
