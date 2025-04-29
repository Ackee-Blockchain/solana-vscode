type CoverageReport = {
  data: CoverageData[];
  type: string;
  version: string;
  cargo_llvm_cov: {
    version: string;
    manifest_path: string;
  };
};

type CoverageData = {
  files: CoverageFileData[];
  totals: CoverageSummary;
};

type CoverageFileData = {
  branches: any[];
  mcdc_records: any[];
  expansions: any[];
  filename: string;
  segments: CoverageSegment[];
  summary: CoverageSummary;
};

type CoverageSegment = {
  line: number;
  column: number;
  execution_count: number;
  has_count: boolean;
  is_region_entry: boolean;
  is_gap_region: boolean;
};

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

enum CoverageType {
  Static = "Static",
  Dynamic = "Dynamic",
}

enum FuzzerType {
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
