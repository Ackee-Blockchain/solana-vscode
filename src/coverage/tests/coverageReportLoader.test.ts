import * as assert from "assert";
import * as vscode from "vscode";
import { CoverageReportLoader } from "../coverageReportLoader";
import { FuzzerType } from "../types";
import * as path from "path";

// Test-only subclass to access private methods
class TestCoverageReportLoader extends CoverageReportLoader {
  public testGetFilteredCoverageFiles(
    dirPath: string,
    dirContents: [string, vscode.FileType][]
  ): vscode.Uri[] {
    return this["getFilteredCoverageFiles"](dirPath, dirContents);
  }

  public testParseCoverageReport(data: Uint8Array): void {
    return this["parseCoverageReport"](data);
  }

  public testTransformSegment(segmentArray: any[]): any {
    return this["transformSegment"](segmentArray);
  }
}

suite("Coverage Report Loader Test Suite", () => {
  let coverageReportLoader: TestCoverageReportLoader;
  let mockWorkspaceFolder: vscode.WorkspaceFolder;

  setup(() => {
    coverageReportLoader = new TestCoverageReportLoader();
    mockWorkspaceFolder = {
      uri: vscode.Uri.file("/test/workspace"),
      name: "test",
      index: 0,
    };

    Object.defineProperty(vscode.workspace, "workspaceFolders", {
      get: () => [mockWorkspaceFolder],
      configurable: true,
    });
  });

  teardown(() => {
    coverageReportLoader.dispose();
    Object.defineProperty(vscode.workspace, "workspaceFolders", {
      get: () => undefined,
      configurable: true,
    });
  });

  test("should initialize with undefined coverage report", () => {
    assert.strictEqual(
      coverageReportLoader.coverageReport,
      undefined,
      "Coverage report should be undefined initially"
    );
  });

  test("should clear coverage report on dispose", () => {
    coverageReportLoader.coverageReport = {
      data: [],
      type: "llvm",
      version: "2.0.0",
      cargo_llvm_cov: {
        version: "0.5.0",
        manifest_path: "/test/Cargo.toml",
      },
    };

    coverageReportLoader.dispose();

    assert.strictEqual(
      coverageReportLoader.coverageReport,
      undefined,
      "Coverage report should be cleared after dispose"
    );
  });

  suite("findCoverageFiles", () => {
    test("should return empty array when no workspace folders", async () => {
      Object.defineProperty(vscode.workspace, "workspaceFolders", {
        get: () => undefined,
        configurable: true,
      });

      const files = await coverageReportLoader.findCoverageFiles();
      assert.deepStrictEqual(files, [], "Should return empty array");
    });

    test("should find coverage files in fuzzer directories", async () => {
      const hfuzzPath = "/test/hfuzz/target";
      const aflPath = "/test/afl/target";

      const hfuzzContents: [string, vscode.FileType][] = [
        ["coverage.json", vscode.FileType.File],
        ["other.txt", vscode.FileType.File],
      ];

      const aflContents: [string, vscode.FileType][] = [
        ["cov_report.json", vscode.FileType.File],
        ["temp.json", vscode.FileType.File],
      ];

      const originalGetTargetDirPath = require("../utils").getTargetDirPath;
      const originalGetDirContents = require("../utils").getDirContents;

      require("../utils").getTargetDirPath = async (fuzzerType: FuzzerType) => {
        return fuzzerType === FuzzerType.Honggfuzz ? hfuzzPath : aflPath;
      };

      require("../utils").getDirContents = async (path: string) => {
        return path === hfuzzPath ? hfuzzContents : aflContents;
      };

      try {
        const files = await coverageReportLoader.findCoverageFiles();
        assert.strictEqual(files.length, 2, "Should find two coverage files");
        assert.ok(
          files.some((f) => f.fsPath.endsWith("coverage.json")),
          "Should include Honggfuzz coverage file"
        );
        assert.ok(
          files.some((f) => f.fsPath.endsWith("cov_report.json")),
          "Should include AFL coverage file"
        );
      } finally {
        require("../utils").getTargetDirPath = originalGetTargetDirPath;
        require("../utils").getDirContents = originalGetDirContents;
      }
    });
  });

  suite("loadCoverageReport", () => {
    test("should load and parse coverage report", async () => {
      const mockData = {
        data: [
          {
            files: [
              {
                filename: "/test/file.rs",
                segments: [[1, 0, 5, true, true, false]],
                branches: [],
                mcdc_records: [],
                expansions: [],
                summary: {
                  regions: {
                    covered: 1,
                    count: 1,
                    notcovered: 0,
                    percent: 100,
                  },
                  functions: { covered: 1, count: 1, percent: 100 },
                  lines: { covered: 1, count: 1, percent: 100 },
                  branches: { covered: 0, count: 0, notcovered: 0, percent: 0 },
                  mcdc: { covered: 0, count: 0, notcovered: 0, percent: 0 },
                  instantiations: { covered: 0, count: 0, percent: 0 },
                },
              },
            ],
            totals: {
              regions: { covered: 1, count: 1, notcovered: 0, percent: 100 },
              functions: { covered: 1, count: 1, percent: 100 },
              lines: { covered: 1, count: 1, percent: 100 },
              branches: { covered: 0, count: 0, notcovered: 0, percent: 0 },
              mcdc: { covered: 0, count: 0, notcovered: 0, percent: 0 },
              instantiations: { covered: 0, count: 0, percent: 0 },
            },
          },
        ],
        type: "llvm",
        version: "2.0.0",
        cargo_llvm_cov: {
          version: "0.5.0",
          manifest_path: "/test/Cargo.toml",
        },
      };

      const mockFs = {
        readFile: async () => Buffer.from(JSON.stringify(mockData)),
      };

      Object.defineProperty(vscode.workspace, "fs", {
        value: mockFs,
        configurable: true,
      });

      await coverageReportLoader.loadCoverageReport(
        vscode.Uri.file("/test/coverage.json")
      );

      assert.ok(
        coverageReportLoader.coverageReport,
        "Coverage report should be loaded"
      );
      assert.strictEqual(
        coverageReportLoader.coverageReport.type,
        "llvm",
        "Should parse report type"
      );
      assert.strictEqual(
        coverageReportLoader.coverageReport.data[0].files[0].segments[0].line,
        1,
        "Should transform segments"
      );
    });

    test("should throw error on file read failure", async () => {
      const mockFs = {
        readFile: async () => {
          throw new Error("File read error");
        },
      };

      Object.defineProperty(vscode.workspace, "fs", {
        value: mockFs,
        configurable: true,
      });

      await assert.rejects(
        () =>
          coverageReportLoader.loadCoverageReport(
            vscode.Uri.file("/test/coverage.json")
          ),
        Error,
        "Should throw error on file read failure"
      );
    });
  });

  suite("selectCoverageFile", () => {
    test("should show file picker when no coverage files found", async () => {
      let showOpenDialogCalled = false;
      const originalShowOpenDialog = vscode.window.showOpenDialog;
      const originalFindCoverageFiles = coverageReportLoader.findCoverageFiles;

      coverageReportLoader.findCoverageFiles = async () => [];

      vscode.window.showOpenDialog = async () => {
        showOpenDialogCalled = true;
        return undefined;
      };

      try {
        await assert.rejects(
          () => coverageReportLoader.selectCoverageFile(),
          Error,
          "Should throw error when no file selected"
        );
        assert.strictEqual(
          showOpenDialogCalled,
          true,
          "Should show open dialog"
        );
      } finally {
        vscode.window.showOpenDialog = originalShowOpenDialog;
        coverageReportLoader.findCoverageFiles = originalFindCoverageFiles;
      }
    });

    test("should load single coverage file directly", async () => {
      let loadCalled = false;
      const originalLoadCoverageReport =
        coverageReportLoader.loadCoverageReport;
      const originalFindCoverageFiles = coverageReportLoader.findCoverageFiles;
      const singleFile = vscode.Uri.file("/test/coverage.json");

      coverageReportLoader.findCoverageFiles = async () => [singleFile];
      coverageReportLoader.loadCoverageReport = async () => {
        loadCalled = true;
      };

      try {
        await coverageReportLoader.selectCoverageFile();
        assert.strictEqual(
          loadCalled,
          true,
          "Should load single file directly"
        );
      } finally {
        coverageReportLoader.loadCoverageReport = originalLoadCoverageReport;
        coverageReportLoader.findCoverageFiles = originalFindCoverageFiles;
      }
    });

    test("should show quick pick for multiple coverage files", async () => {
      let quickPickShown = false;
      const originalShowQuickPick = vscode.window.showQuickPick;
      const originalFindCoverageFiles = coverageReportLoader.findCoverageFiles;
      const multipleFiles = [
        vscode.Uri.file("/test/coverage1.json"),
        vscode.Uri.file("/test/coverage2.json"),
      ];

      coverageReportLoader.findCoverageFiles = async () => multipleFiles;

      vscode.window.showQuickPick = async () => {
        quickPickShown = true;
        return undefined;
      };

      try {
        await assert.rejects(
          () => coverageReportLoader.selectCoverageFile(),
          Error,
          "Should throw error when no file selected"
        );
        assert.strictEqual(quickPickShown, true, "Should show quick pick");
      } finally {
        vscode.window.showQuickPick = originalShowQuickPick;
        coverageReportLoader.findCoverageFiles = originalFindCoverageFiles;
      }
    });
  });

  suite("getFilteredCoverageFiles", () => {
    test("should filter coverage JSON files", () => {
      const dirPath = "/test/path";
      const dirContents: [string, vscode.FileType][] = [
        ["coverage.json", vscode.FileType.File],
        ["not_coverage.json", vscode.FileType.File],
        ["cov_report.json", vscode.FileType.File],
        ["test.txt", vscode.FileType.File],
        ["subfolder", vscode.FileType.Directory],
      ];

      const files = coverageReportLoader.testGetFilteredCoverageFiles(
        dirPath,
        dirContents
      );

      assert.strictEqual(files.length, 3, "Should find three coverage files");
      assert.ok(
        files.some((f) => f.fsPath.endsWith("coverage.json")),
        "Should include coverage.json"
      );
      assert.ok(
        files.some((f) => f.fsPath.endsWith("cov_report.json")),
        "Should include cov_report.json"
      );
      assert.ok(
        files.some((f) => f.fsPath.endsWith("not_coverage.json")),
        "Should include not_coverage.json"
      );
    });

    test("should handle empty directory contents", () => {
      const files = coverageReportLoader.testGetFilteredCoverageFiles(
        "/test/path",
        []
      );
      assert.deepStrictEqual(files, [], "Should return empty array");
    });

    test("should ignore non-JSON files with 'cov' in name", () => {
      const dirContents: [string, vscode.FileType][] = [
        ["coverage.txt", vscode.FileType.File],
        ["cov_report.log", vscode.FileType.File],
      ];

      const files = coverageReportLoader.testGetFilteredCoverageFiles(
        "/test/path",
        dirContents
      );
      assert.deepStrictEqual(files, [], "Should not include non-JSON files");
    });
  });

  suite("parseCoverageReport", () => {
    test("should parse valid coverage report data", () => {
      const mockData = {
        data: [
          {
            files: [
              {
                filename: "/test/file.rs",
                segments: [[1, 0, 5, true, true, false]],
                branches: [],
                mcdc_records: [],
                expansions: [],
                summary: {
                  regions: {
                    covered: 1,
                    count: 1,
                    notcovered: 0,
                    percent: 100,
                  },
                  functions: { covered: 1, count: 1, percent: 100 },
                  lines: { covered: 1, count: 1, percent: 100 },
                  branches: {
                    covered: 0,
                    count: 0,
                    notcovered: 0,
                    percent: 0,
                  },
                  mcdc: { covered: 0, count: 0, notcovered: 0, percent: 0 },
                  instantiations: { covered: 0, count: 0, percent: 0 },
                },
              },
            ],
            totals: {
              regions: { covered: 1, count: 1, notcovered: 0, percent: 100 },
              functions: { covered: 1, count: 1, percent: 100 },
              lines: { covered: 1, count: 1, percent: 100 },
              branches: { covered: 0, count: 0, notcovered: 0, percent: 0 },
              mcdc: { covered: 0, count: 0, notcovered: 0, percent: 0 },
              instantiations: { covered: 0, count: 0, percent: 0 },
            },
          },
        ],
        type: "llvm",
        version: "2.0.0",
        cargo_llvm_cov: {
          version: "0.5.0",
          manifest_path: "/test/Cargo.toml",
        },
      };

      const data = Buffer.from(JSON.stringify(mockData));
      coverageReportLoader.testParseCoverageReport(data);

      assert.ok(
        coverageReportLoader.coverageReport,
        "Coverage report should be set"
      );
      assert.strictEqual(
        coverageReportLoader.coverageReport.type,
        "llvm",
        "Should parse report type"
      );
      const segment =
        coverageReportLoader.coverageReport.data[0].files[0].segments[0];
      assert.strictEqual(segment.line, 1, "Should transform segment line");
      assert.strictEqual(segment.column, 0, "Should transform segment column");
      assert.strictEqual(
        segment.execution_count,
        5,
        "Should transform segment execution count"
      );
    });

    test("should handle invalid JSON data", () => {
      const invalidData = Buffer.from("invalid json");
      assert.throws(
        () => coverageReportLoader.testParseCoverageReport(invalidData),
        Error,
        "Failed to parse coverage report"
      );
    });
  });

  suite("transformSegment", () => {
    test("should transform segment array to object", () => {
      const segmentArray = [1, 2, 5, true, false, true];
      const segment = coverageReportLoader.testTransformSegment(segmentArray);

      assert.deepStrictEqual(
        segment,
        {
          line: 1,
          column: 2,
          execution_count: 5,
          has_count: true,
          is_region_entry: false,
          is_gap_region: true,
        },
        "Should correctly transform segment array to object"
      );
    });

    test("should handle segment array with different values", () => {
      const segmentArray = [10, 0, 100, false, true, false];
      const segment = coverageReportLoader.testTransformSegment(segmentArray);

      assert.deepStrictEqual(
        segment,
        {
          line: 10,
          column: 0,
          execution_count: 100,
          has_count: false,
          is_region_entry: true,
          is_gap_region: false,
        },
        "Should correctly transform segment array with different values"
      );
    });
  });
});
