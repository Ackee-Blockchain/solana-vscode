import { CoverageManager } from "./coverage/coverageManager";

function initExtensionFeatureManagers(): ExtensionFeatureManagers {
  let extensionFeatureManagers: ExtensionFeatureManagers = {
    coverageManager: new CoverageManager(),
    // add other managers here ...
  };

  return extensionFeatureManagers;
}

interface ExtensionFeatureManagers {
  coverageManager: CoverageManager;
  // add other managers here ...
}

export { initExtensionFeatureManagers, ExtensionFeatureManagers };
