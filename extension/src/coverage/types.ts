/**
 * Represents a complete coverage report containing test coverage data
 * @typedef {Object} CoverageReport
 * @property {CoverageData[]} data - Array of coverage data
 * @property {string} type - The type of coverage report
 * @property {string} version - Version of the coverage report format
 * @property {Object} cargo_llvm_cov - Information about the coverage tool
 * @property {string} cargo_llvm_cov.version - Version of cargo-llvm-cov
 * @property {string} cargo_llvm_cov.manifest_path - Path to the Cargo.toml file
 */
type CoverageReport = {
  data: CoverageData[];
  type: string;
  version: string;
  cargo_llvm_cov: {
    version: string;
    manifest_path: string;
  };
};

/**
 * Generated coverage data
 * @typedef {Object} CoverageData
 * @property {CoverageFileData[]} files - Coverage data for individual files
 * @property {CoverageSummary} totals - Overall coverage statistics
 */
type CoverageData = {
  files: CoverageFileData[];
  totals: CoverageSummary;
};

/**
 * Coverage information for a specific file
 * @typedef {Object} CoverageFileData
 * @property {any[]} branches - Branch coverage information
 * @property {any[]} mcdc_records - MC/DC coverage records
 * @property {any[]} expansions - Coverage expansion data
 * @property {string} filename - Path to the source file
 * @property {CoverageSegment[]} segments - Code segments with coverage data
 * @property {CoverageSummary} summary - Coverage statistics for this file
 */
type CoverageFileData = {
  branches: any[];
  mcdc_records: any[];
  expansions: any[];
  filename: string;
  segments: CoverageSegment[];
  summary: CoverageSummary;
};

/**
 * Represents a segment of code with coverage information
 * @typedef {Object} CoverageSegment
 * @property {number} line - Line number in the source file
 * @property {number} column - Column number in the source file
 * @property {number} execution_count - Number of times this segment was executed
 * @property {boolean} has_count - Whether this segment has execution count data
 * @property {boolean} is_region_entry - Whether this segment is an entry point
 * @property {boolean} is_gap_region - Whether this segment represents a gap in coverage
 */
type CoverageSegment = {
  line: number;
  column: number;
  execution_count: number;
  has_count: boolean;
  is_region_entry: boolean;
  is_gap_region: boolean;
};

/**
 * Summary of coverage statistics
 * @typedef {Object} CoverageSummary
 * @property {Object} branches - Branch coverage statistics
 * @property {Object} mcdc - MC/DC coverage statistics
 * @property {Object} functions - Function coverage statistics
 * @property {Object} instantiations - Template instantiation coverage
 * @property {Object} lines - Line coverage statistics
 * @property {Object} regions - Region coverage statistics
 */
type CoverageSummary = {
  branches: {
    count: number;
    covered: number;
    notcovered: number;
    percent: number;
  };
  mcdc: {
    count: number;
    covered: number;
    notcovered: number;
    percent: number;
  };
  functions: {
    count: number;
    covered: number;
    percent: number;
  };
  instantiations: {
    count: number;
    covered: number;
    percent: number;
  };
  lines: {
    count: number;
    covered: number;
    percent: number;
  };
  regions: {
    count: number;
    covered: number;
    notcovered: number;
    percent: number;
  };
};

/**
 * Type of coverage analysis being performed
 * @enum {string}
 */
const enum CoverageType {
  Static = "Load generated JSON report",
  Dynamic = "Attach to active fuzzing session",
}

/**
 * Type of fuzzer being used for testing
 * @enum {string}
 */
const enum FuzzerType {
  Afl = "AFL",
  Honggfuzz = "Honggfuzz",
}

export {
  CoverageReport,
  CoverageSegment,
  CoverageSummary,
  CoverageFileData,
  CoverageData,
  CoverageType,
  FuzzerType,
};
