import { CoverageManager } from "./coverage/coverageManager";
import { DetectorsManager } from "./detectors/detectorsManager";

function initExtensionFeatureManagers(): ExtensionFeatureManagers {
  console.log('Initializing extension feature managers');
  let extensionFeatureManagers: ExtensionFeatureManagers = {
    coverageManager: new CoverageManager(),
    detectorsManager: new DetectorsManager(),
    // add other managers here ...
  };

  return extensionFeatureManagers;
}

interface ExtensionFeatureManagers {
  coverageManager: CoverageManager;
  detectorsManager: DetectorsManager;
  // add other managers here ...
}

export { initExtensionFeatureManagers, ExtensionFeatureManagers };
