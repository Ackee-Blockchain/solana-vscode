import * as assert from "assert";
import * as vscode from "vscode";
import { CoverageDecorations } from "../coverageDecoration";
import { CoverageReport, CoverageFileData, CoverageSegment } from "../types";
import { DecorationConstants } from "../constants";

// Test-only subclass to access private methods
class TestCoverageDecorations extends CoverageDecorations {
  public testCalculateCoverageLevel(count: number): number {
    return this["calculateCoverageLevel"](count);
  }

  public testCreateSegmentDecoration(
    segment: CoverageSegment,
    coverageFileData: CoverageFileData,
    editor: vscode.TextEditor
  ): vscode.DecorationOptions {
    return this["createSegmentDecoration"](segment, coverageFileData, editor);
  }

  public testPrepareAndDisplaySegments(
    coverageFileData: CoverageFileData,
    editor: vscode.TextEditor
  ): void {
    return this["prepareAndDisplaySegments"](coverageFileData, editor);
  }

  public testShouldDisplaySegment(segment: CoverageSegment, editor: vscode.TextEditor): boolean {
    return this["shouldDisplaySegment"](segment, editor);
  }
}

suite("Coverage Decoration Test Suite", () => {
  let coverageDecorations: TestCoverageDecorations;
  let mockTestController: vscode.TestController;

  setup(() => {
    coverageDecorations = new TestCoverageDecorations();

    mockTestController = {
      createRunProfile: () => ({
        dispose: () => {},
      }),
      createTestRun: () => ({
        addCoverage: () => {},
        end: () => {},
      }),
    } as any;
  });

  teardown(() => {
    coverageDecorations.dispose();
  });

  test("should initialize line coverage decorations", () => {
    assert.strictEqual(
      coverageDecorations.lineCoverageDecorations.length,
      Object.keys(DecorationConstants.levelColors).length,
      "Should create decoration for each color level"
    );
  });

  test("should dispose line coverage decorations", () => {
    let disposeCalled = 0;
    coverageDecorations.lineCoverageDecorations = [
      {
        dispose: () => {
          disposeCalled++;
        },
      },
      {
        dispose: () => {
          disposeCalled++;
        },
      },
    ] as any[];

    coverageDecorations.dispose();

    assert.strictEqual(disposeCalled, 2, "Should dispose all decorations");
    assert.strictEqual(
      coverageDecorations.lineCoverageDecorations.length,
      0,
      "Should clear decorations array"
    );
  });

  suite("displayCoverage", () => {
    test("should display coverage for visible editors", () => {
      let decorationsSet = false;
      const mockEditor = {
        document: {
          uri: { fsPath: "/test/file.rs" },
          lineAt: (line: number) => ({
            text: "test line content",
            range: new vscode.Range(
              new vscode.Position(line, 0),
              new vscode.Position(line, 10)
            ),
            lineNumber: line,
            firstNonWhitespaceCharacterIndex: 0,
            isEmptyOrWhitespace: false,
          }),
        },
        setDecorations: () => {
          decorationsSet = true;
        },
      } as any;

      const originalVisibleEditors = Object.getOwnPropertyDescriptor(
        vscode.window,
        "visibleTextEditors"
      );
      Object.defineProperty(vscode.window, "visibleTextEditors", {
        get: () => [mockEditor],
        configurable: true,
      });

      const mockCoverageReport: CoverageReport = {
        data: [
          {
            files: [
              {
                filename: "/test/file.rs",
                segments: [
                  {
                    line: 1,
                    column: 1,
                    has_count: true,
                    execution_count: 5,
                    is_region_entry: true,
                    is_gap_region: false,
                  },
                ],
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
                  lines: {
                    covered: 1,
                    count: 1,
                    percent: 100,
                  },
                  branches: { covered: 0, count: 0, notcovered: 0, percent: 0 },
                  mcdc: { covered: 0, count: 0, notcovered: 0, percent: 0 },
                  instantiations: {
                    covered: 0,
                    count: 0,
                    percent: 0,
                  },
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

      coverageDecorations.displayCoverage(
        mockCoverageReport,
        mockTestController
      );

      assert.strictEqual(
        decorationsSet,
        true,
        "Should set decorations for visible editor"
      );

      if (originalVisibleEditors) {
        Object.defineProperty(
          vscode.window,
          "visibleTextEditors",
          originalVisibleEditors
        );
      }
    });
  });

  suite("clearCoverage", () => {
    test("should clear decorations from all visible editors", () => {
      let decorationsClearedCount = 0;
      const mockEditor = {
        setDecorations: () => {
          decorationsClearedCount++;
        },
      };

      const originalVisibleEditors = Object.getOwnPropertyDescriptor(
        vscode.window,
        "visibleTextEditors"
      );
      Object.defineProperty(vscode.window, "visibleTextEditors", {
        get: () => [mockEditor],
        configurable: true,
      });

      coverageDecorations.clearCoverage(mockTestController);

      assert.strictEqual(
        decorationsClearedCount,
        coverageDecorations.lineCoverageDecorations.length,
        "Should clear all decoration types"
      );

      if (originalVisibleEditors) {
        Object.defineProperty(
          vscode.window,
          "visibleTextEditors",
          originalVisibleEditors
        );
      }
    });
  });

  suite("calculateCoverageLevel", () => {
    test("should return appropriate coverage level based on execution count", () => {
      const testCases = [
        { count: 0, expected: 0 },
        { count: 1, expected: 1 },
        { count: 10, expected: 1 },
        { count: 100, expected: 1 },
        { count: 500, expected: 2 },
        { count: 1000, expected: 2 },
        { count: 10000, expected: 3 },
        { count: 10001, expected: 4 },
      ];

      testCases.forEach(({ count, expected }) => {
        const level = coverageDecorations.testCalculateCoverageLevel(count);
        assert.strictEqual(
          level,
          expected,
          `Execution count ${count} should result in level ${expected}`
        );
      });
    });
  });

  suite("createSegmentDecoration", () => {
    test("should create decoration with correct range and hover message", () => {
      const mockEditor = {
        document: {
          uri: { fsPath: "/test/file.rs" },
          lineAt: (line: number) => ({
            text: "test line content",
            range: new vscode.Range(
              new vscode.Position(line, 0),
              new vscode.Position(line, 10)
            ),
            lineNumber: line,
            firstNonWhitespaceCharacterIndex: 0,
            isEmptyOrWhitespace: false,
          }),
        },
      } as any;

      const segment: CoverageSegment = {
        line: 1,
        column: 1,
        has_count: true,
        execution_count: 5,
        is_region_entry: true,
        is_gap_region: false,
      };

      const coverageFileData: CoverageFileData = {
        filename: "/test/file.rs",
        segments: [segment],
        branches: [],
        mcdc_records: [],
        expansions: [],
        summary: {
          regions: { covered: 1, count: 1, notcovered: 0, percent: 100 },
          functions: { covered: 1, count: 1, percent: 100 },
          lines: { covered: 1, count: 1, percent: 100 },
          branches: { covered: 0, count: 0, notcovered: 0, percent: 0 },
          mcdc: { covered: 0, count: 0, notcovered: 0, percent: 0 },
          instantiations: { covered: 0, count: 0, percent: 0 },
        },
      };

      const decoration = coverageDecorations.testCreateSegmentDecoration(
        segment,
        coverageFileData,
        mockEditor
      );

      assert.ok(
        decoration.range instanceof vscode.Range,
        "Should create a valid range"
      );
      assert.strictEqual(
        decoration.range.start.line,
        segment.line - 1,
        "Should adjust line number to 0-based"
      );
      assert.ok(decoration.hoverMessage, "Should include hover message");
    });
  });

  suite("prepareAndDisplaySegments", () => {
    test("should prepare and set decorations for segments", () => {
      let decorationsSet = false;
      const mockEditor = {
        document: {
          uri: { fsPath: "/test/file.rs" },
          lineAt: (line: number) => ({
            text: "test line content",
            range: new vscode.Range(
              new vscode.Position(line, 0),
              new vscode.Position(line, 10)
            ),
            lineNumber: line,
            firstNonWhitespaceCharacterIndex: 0,
            isEmptyOrWhitespace: false,
          }),
        },
        setDecorations: () => {
          decorationsSet = true;
        },
      } as any;

      const coverageFileData: CoverageFileData = {
        filename: "/test/file.rs",
        segments: [
          {
            line: 1,
            column: 1,
            has_count: true,
            execution_count: 5,
            is_region_entry: true,
            is_gap_region: false,
          },
        ],
        branches: [],
        mcdc_records: [],
        expansions: [],
        summary: {
          regions: { covered: 1, count: 1, notcovered: 0, percent: 100 },
          functions: { covered: 1, count: 1, percent: 100 },
          lines: { covered: 1, count: 1, percent: 100 },
          branches: { covered: 0, count: 0, notcovered: 0, percent: 0 },
          mcdc: { covered: 0, count: 0, notcovered: 0, percent: 0 },
          instantiations: { covered: 0, count: 0, percent: 0 },
        },
      };

      coverageDecorations.testPrepareAndDisplaySegments(
        coverageFileData,
        mockEditor
      );
      assert.strictEqual(
        decorationsSet,
        true,
        "Should set decorations on editor"
      );
    });
  });

  suite("Macro Filtering", () => {
    test("should filter out derive macro lines", () => {
      // Create a mock editor with a derive macro line
      const mockEditor = {
        document: {
          uri: { fsPath: "/test/file.rs" },
          lineCount: 100,
          lineAt: (line: number) => ({
            text: line === 24 ? "#[derive(Account)]" : "normal code line", // Line 25 (0-indexed 24) has derive macro
            range: new vscode.Range(
              new vscode.Position(line, 0),
              new vscode.Position(line, 20)
            ),
            lineNumber: line,
            firstNonWhitespaceCharacterIndex: 0,
            isEmptyOrWhitespace: false,
          }),
        },
        setDecorations: () => {},
      } as any;

      // Test segment on derive macro line (line 25, 0-indexed 24)
      const macroSegment: CoverageSegment = {
        line: 25,
        column: 10,
        execution_count: 1956,
        has_count: true,
        is_region_entry: true,
        is_gap_region: false,
      };

      // Test segment on normal code line
      const normalSegment: CoverageSegment = {
        line: 10,
        column: 5,
        execution_count: 100,
        has_count: true,
        is_region_entry: true,
        is_gap_region: false,
      };

      const macroResult = coverageDecorations.testShouldDisplaySegment(macroSegment, mockEditor);
      const normalResult = coverageDecorations.testShouldDisplaySegment(normalSegment, mockEditor);

      assert.strictEqual(macroResult, false, "Should filter out derive macro line");
      assert.strictEqual(normalResult, true, "Should display normal code line");
    });

    test("should filter out attribute macro lines", () => {
      const mockEditor = {
        document: {
          uri: { fsPath: "/test/file.rs" },
          lineCount: 100,
          lineAt: (line: number) => {
            switch (line) {
              case 9: return { text: "#[account]", lineNumber: line };
              case 14: return { text: "#[program]", lineNumber: line };
              default: return { text: "normal code", lineNumber: line };
            }
          },
        },
        setDecorations: () => {},
      } as any;

      const accountMacroSegment: CoverageSegment = {
        line: 10, // 0-indexed 9
        column: 5,
        execution_count: 500,
        has_count: true,
        is_region_entry: true,
        is_gap_region: false,
      };

      const programMacroSegment: CoverageSegment = {
        line: 15, // 0-indexed 14
        column: 5,
        execution_count: 200,
        has_count: true,
        is_region_entry: true,
        is_gap_region: false,
      };

      const accountResult = coverageDecorations.testShouldDisplaySegment(accountMacroSegment, mockEditor);
      const programResult = coverageDecorations.testShouldDisplaySegment(programMacroSegment, mockEditor);

      assert.strictEqual(accountResult, false, "Should filter out #[account] line");
      assert.strictEqual(programResult, false, "Should filter out #[program] line");
    });

    test("should still filter basic patterns", () => {
      const mockEditor = {
        document: {
          uri: { fsPath: "/test/file.rs" },
          lineCount: 100,
          lineAt: (line: number) => ({ text: "normal code", lineNumber: line }),
        },
        setDecorations: () => {},
      } as any;

      // Test segment without has_count
      const noCountSegment: CoverageSegment = {
        line: 10,
        column: 5,
        execution_count: 0,
        has_count: false,
        is_region_entry: false,
        is_gap_region: false,
      };

      // Test gap region
      const gapSegment: CoverageSegment = {
        line: 15,
        column: 5,
        execution_count: 0,
        has_count: true,
        is_region_entry: false,
        is_gap_region: true,
      };

      const noCountResult = coverageDecorations.testShouldDisplaySegment(noCountSegment, mockEditor);
      const gapResult = coverageDecorations.testShouldDisplaySegment(gapSegment, mockEditor);

      assert.strictEqual(noCountResult, false, "Should filter out segments without has_count");
      assert.strictEqual(gapResult, false, "Should filter out gap regions");
    });
  });
});
