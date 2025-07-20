import { CoverageManager } from "./coverage/coverageManager";
import { DetectorsManager } from "./detectors/detectorsManager";
import { AIDetectorsManager } from "./detectors/ai/aiDetectorsManager";

function initExtensionFeatureManagers(): ExtensionFeatureManagers {
  console.log('Initializing extension feature managers');
  let extensionFeatureManagers: ExtensionFeatureManagers = {
    coverageManager: new CoverageManager(),
    detectorsManager: new DetectorsManager(),
    aiDetectorsManager: new AIDetectorsManager(),
    // add other managers here ...
  };

  return extensionFeatureManagers;
}

interface ExtensionFeatureManagers {
  coverageManager: CoverageManager;
  detectorsManager: DetectorsManager;
  aiDetectorsManager: AIDetectorsManager;
  // add other managers here ...
}

export { initExtensionFeatureManagers, ExtensionFeatureManagers };
