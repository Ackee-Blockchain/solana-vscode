import { CoverageManager } from "./coverage/coverageManager";
import { DetectorsManager } from "./detectors/detectorsManager";
import { StatusBarManager, StatusBarState } from "./statusBar/statusBarManager";

function initExtensionFeatureManagers(): ExtensionFeatureManagers {
  console.log('Initializing extension feature managers');

  // Create status bar manager first
  const statusBarManager = new StatusBarManager();

  // Create detectors manager
  const detectorsManager = new DetectorsManager();

  // Connect status bar manager to detectors manager
  detectorsManager.setStatusBarUpdateCallback((state: StatusBarState, message?: string) => {
    statusBarManager.updateStatus(state, message);
  });

  // After connecting, check if we need to update status based on current state
  // This handles the case where DetectorsManager set an error state before callback was connected
  // The server state will be updated via onDidChangeState event, so we don't need to manually check here

  let extensionFeatureManagers: ExtensionFeatureManagers = {
    coverageManager: new CoverageManager(),
    detectorsManager: detectorsManager,
    statusBarManager: statusBarManager,
    // add other managers here ...
  };

  return extensionFeatureManagers;
}

interface ExtensionFeatureManagers {
  coverageManager: CoverageManager;
  detectorsManager: DetectorsManager;
  statusBarManager: StatusBarManager;
  // add other managers here ...
}

export { initExtensionFeatureManagers, ExtensionFeatureManagers };
