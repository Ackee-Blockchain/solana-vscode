const DecorationConstants = {
  configColorOptions: {
    CYAN: "rgb(0, 255, 255)",
    DEEP_PINK: "rgb(255, 20, 147)",
    MAGENTA: "rgb(255, 0, 255)",
    DODGER_BLUE: "rgb(30, 144, 255)",
    WHITE: "rgb(255, 255, 255)",
    BLACK: "rgb(0, 0, 0)",
    ELECTRIC_PURPLE: "rgb(191, 0, 255)",
    NEON_ORANGE: "rgb(255, 96, 0)",
    LIME_GREEN: "rgb(50, 255, 50)",
    GOLDEN_YELLOW: "rgb(255, 223, 0)",
    VIVID_RED: "rgb(255, 0, 42)",
  },
  levelColors: {
    UNCOVERED: "rgba(250, 0, 0, 0.20)",
    LOW: "rgba(255, 140, 0, 0.20)",
    MEDIUM: "rgba(255, 242, 0, 0.20)",
    HIGH: "rgba(144, 238, 144, 0.20)",
    VERY_HIGH: "rgba(0, 255, 0, 0.20)",
  },
  levelThresholds: {
    UNCOVERED: 0,
    LOW: 100,
    MEDIUM: 1000,
    HIGH: 10000,
  },
  executionCountProperties: {
    DEFAULT_COLOR: "CYAN",
    MARGIN: "0 0.2em 0 0.2em",
  },
};

const TestApiConstants = {
  COVERAGE_LABEL: "Trident Coverage",
  COVERAGE_ID: "trident-coverage",
  COVERAGE_TEST_RUN_NAME: "Trident Coverage Test Run",
};

const TridentConstants = {
  IGNORE_FILE_NAME_REGEX: "trident-tests|trident/crates",
  TARGET_PATH: "trident-tests/target",
  LIVE_REPORT_FILE: "trident-live-coverage-report.json",
};

const CoverageServerConstants = {
  DEFAULT_COVERAGE_SERVER_PORT: 58432,
  UPDATE_DECORATIONS: "/update-decorations",
  SETUP_DYNAMIC_COVERAGE: "/setup-dynamic-coverage",
  DISPLAY_FINAL_REPORT: "/display-final-report",
};

export {
  DecorationConstants,
  TestApiConstants,
  TridentConstants,
  CoverageServerConstants,
};
