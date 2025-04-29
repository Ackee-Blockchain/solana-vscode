import * as vscode from "vscode";
import { TestApiConstants, DecorationConstants } from "./constants";
import { CoverageFileData, CoverageReport, CoverageSegment } from "./types";

const { COVERAGE_LABEL, COVERAGE_TEST_RUN_NAME } = TestApiConstants;
const { executionCountProperties } = DecorationConstants;

class CoverageDecorations {
  public lineCoverageDecorations: vscode.TextEditorDecorationType[];

  constructor() {
    this.lineCoverageDecorations = [];
    this.initLineCoverageDecorations();
  }

  dispose() {
    this.lineCoverageDecorations.forEach((decoration) => decoration.dispose());
    this.lineCoverageDecorations = [];
  }

  private initLineCoverageDecorations() {
    for (const color of Object.values(DecorationConstants.levelColors)) {
      let decorationOption = { backgroundColor: color };
      let decorationType =
        vscode.window.createTextEditorDecorationType(decorationOption);

      this.lineCoverageDecorations.push(decorationType);
    }
  }

  displayCoverage(
    coverageReport: CoverageReport,
    coverageTestController: vscode.TestController
  ) {
    this.displayLineCoverageDecorations(coverageReport);
    this.displayCoverageStatusBars(coverageReport, coverageTestController);
  }

  clearCoverage(coverageTestController: vscode.TestController) {
    this.clearLineCoverageDecorations();
    this.clearCoverageStatusBars(coverageTestController);
  }

  private clearLineCoverageDecorations() {
    let visibleEditors = vscode.window.visibleTextEditors;
    for (const editor of visibleEditors) {
      for (const decoration of this.lineCoverageDecorations) {
        editor.setDecorations(decoration, []);
      }
    }
  }

  displayLineCoverageDecorations(coverageReport: CoverageReport) {
    let visibleEditors = vscode.window.visibleTextEditors;
    for (const editor of visibleEditors) {
      const coverageFileData = coverageReport.data[0].files.find(
        (file) => file.filename === editor.document.uri.fsPath
      );
      if (coverageFileData) {
        this.prepareAndDisplaySegments(coverageFileData, editor);
      }
    }
  }

  private displayCoverageStatusBars(
    coverageReport: CoverageReport,
    coverageTestController: vscode.TestController
  ) {
    const coverageProfile = coverageTestController.createRunProfile(
      COVERAGE_LABEL,
      vscode.TestRunProfileKind.Coverage,
      () => {},
      true,
      undefined,
      true
    );

    const run = coverageTestController.createTestRun(
      new vscode.TestRunRequest([], [], coverageProfile),
      COVERAGE_TEST_RUN_NAME,
      false
    );

    for (const file of coverageReport.data[0].files) {
      run.addCoverage(
        new vscode.FileCoverage(
          vscode.Uri.file(file.filename),
          new vscode.TestCoverageCount(
            file.summary.regions.covered,
            file.summary.regions.count
          ),
          undefined,
          new vscode.TestCoverageCount(
            file.summary.functions.covered,
            file.summary.functions.count
          )
        )
      );
    }

    run.end();
    coverageProfile.dispose();
  }

  private clearCoverageStatusBars(
    coverageTestController: vscode.TestController
  ) {
    const run = coverageTestController.createTestRun(
      new vscode.TestRunRequest()
    );
    run.end();
  }

  private prepareAndDisplaySegments(
    coverageFileData: CoverageFileData,
    editor: vscode.TextEditor
  ) {
    const decorationBackgroundTypes = new Array(
      this.lineCoverageDecorations.length
    )
      .fill(null)
      .map(() => [] as vscode.DecorationOptions[]);

    const filteredSegments = coverageFileData.segments.filter(
      (segment) => segment.has_count
    );
    for (const segment of filteredSegments) {
      const decoration = this.createSegmentDecoration(
        segment,
        coverageFileData,
        editor
      );

      const coverageLevel = this.calculateCoverageLevel(
        segment.execution_count
      );
      decorationBackgroundTypes[coverageLevel].push(decoration);
    }

    decorationBackgroundTypes.forEach(
      (decorations: vscode.DecorationOptions[], index: number) => {
        editor.setDecorations(this.lineCoverageDecorations[index], decorations);
      }
    );
  }

  private calculateCoverageLevel(count: number) {
    let coverageLevel = 0;
    for (const levelThreshold of Object.values(
      DecorationConstants.levelThresholds
    )) {
      if (count <= levelThreshold) {
        break;
      }
      coverageLevel++;
    }

    return coverageLevel;
  }

  private createSegmentDecoration(
    segment: CoverageSegment,
    coverageFileData: CoverageFileData,
    editor: vscode.TextEditor
  ) {
    const nextSegmentOnLine = coverageFileData.segments.find(
      (nextSegment) =>
        nextSegment.line === segment.line && nextSegment.column > segment.column
    );

    const line = segment.line - 1;
    const startColumn = segment.column - 1;
    const endColumn = nextSegmentOnLine
      ? nextSegmentOnLine.column - 1
      : editor.document.lineAt(line).text.length + 1;

    const decoration: vscode.DecorationOptions = {
      range: new vscode.Range(
        new vscode.Position(line, startColumn),
        new vscode.Position(line, endColumn)
      ),
      hoverMessage: `Executed ${segment.execution_count} times`,
    };

    const config = vscode.workspace.getConfiguration("tridentCoverage");
    const showExecutionCount = config.get("showExecutionCount", true);
    const selectedColor = config.get(
      "executionCountColor",
      executionCountProperties.DEFAULT_COLOR
    ) as keyof typeof DecorationConstants.configColorOptions;
    const executionCountColor =
      DecorationConstants.configColorOptions[selectedColor];

    if (showExecutionCount) {
      decoration.renderOptions = {
        after: {
          contentText: ` (${segment.execution_count}x)`,
          color: executionCountColor,
          margin: executionCountProperties.MARGIN,
        },
      };
    }
    return decoration;
  }
}

export { CoverageDecorations };
