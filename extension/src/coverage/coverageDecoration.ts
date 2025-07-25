import * as vscode from "vscode";
import { TestApiConstants, DecorationConstants } from "./constants";
import { ExtensionConfigurationConstants } from "../constants";
import { CoverageFileData, CoverageReport, CoverageSegment } from "./types";

const { COVERAGE_LABEL, COVERAGE_TEST_RUN_NAME } = TestApiConstants;
const { executionCountProperties } = DecorationConstants;
const { CONFIG_ID, SHOW_EXECUTION_COUNT, EXECUTION_COUNT_COLOR } = ExtensionConfigurationConstants;

/**
 * Manages code coverage decorations in the editor, including line coverage highlighting
 * and execution count display.
 */
class CoverageDecorations {
  /**
   * Array of text editor decoration types used for different coverage levels
   */
  public lineCoverageDecorations: vscode.TextEditorDecorationType[];

  /**
   * Creates a new CoverageDecorations instance and initializes line coverage decorations
   */
  constructor() {
    this.lineCoverageDecorations = [];
    this.initLineCoverageDecorations();
  }

  /**
   * Disposes all line coverage decorations
   */
  dispose() {
    this.lineCoverageDecorations.forEach((decoration) => decoration.dispose());
    this.lineCoverageDecorations = [];
  }

  /**
   * Initializes line coverage decorations based on predefined color levels
   * @private
   */
  private initLineCoverageDecorations() {
    for (const color of Object.values(DecorationConstants.levelColors)) {
      let decorationOption = { backgroundColor: color };
      let decorationType =
        vscode.window.createTextEditorDecorationType(decorationOption);

      this.lineCoverageDecorations.push(decorationType);
    }
  }

  /**
   * Displays code coverage information in the editor
   * @param {CoverageReport} coverageReport - The coverage report to display
   * @param {vscode.TestController} coverageTestController - The test controller instance
   */
  displayCoverage(
    coverageReport: CoverageReport,
    coverageTestController: vscode.TestController
  ) {
    this.displayLineCoverageDecorations(coverageReport);
    this.displayCoverageStatusBars(coverageReport, coverageTestController);
  }

  /**
   * Clears all coverage information from the editor
   * @param {vscode.TestController} coverageTestController - The test controller instance
   */
  clearCoverage(coverageTestController: vscode.TestController) {
    this.clearLineCoverageDecorations();
    this.clearCoverageStatusBars(coverageTestController);
  }

  /**
   * Clears all line coverage decorations from visible editors
   * @private
   */
  private clearLineCoverageDecorations() {
    let visibleEditors = vscode.window.visibleTextEditors;
    for (const editor of visibleEditors) {
      for (const decoration of this.lineCoverageDecorations) {
        editor.setDecorations(decoration, []);
      }
    }
  }

  /**
   * Displays line coverage decorations in all visible editors
   * @param {CoverageReport} coverageReport - The coverage report to display
   */
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

  /**
   * Displays coverage information in the status bars next to the file names in the sidebar
   * @param {CoverageReport} coverageReport - The coverage report to display
   * @param {vscode.TestController} coverageTestController - The test controller instance
   * @private
   */
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

  /**
   * Removes the status bars next to the file names in the sidebar
   * @param {vscode.TestController} coverageTestController - The test controller instance
   * @private
   */
  private clearCoverageStatusBars(
    coverageTestController: vscode.TestController
  ) {
    const run = coverageTestController.createTestRun(
      new vscode.TestRunRequest()
    );
    run.end();
  }

  /**
   * Prepares and displays coverage segments for a specific file
   * @param {CoverageFileData} coverageFileData - Coverage data for a specific file
   * @param {vscode.TextEditor} editor - The text editor instance
   * @private
   */
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
      (segment) => this.shouldDisplaySegment(segment, editor)
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

  /**
   * Determines whether a coverage segment should be displayed based on filtering rules
   * This helps avoid displaying misleading coverage information from Rust macros
   * @param {CoverageSegment} segment - The coverage segment to evaluate
   * @param {vscode.TextEditor} editor - The text editor to examine line content
   * @returns {boolean} True if the segment should be displayed, false otherwise
   * @private
   */
  private shouldDisplaySegment(segment: CoverageSegment, editor: vscode.TextEditor): boolean {
    if (!segment.has_count) {
      return false;
    }

    // Check the actual line content for macro patterns
    try {
      const lineIndex = segment.line - 1;
      if (lineIndex >= 0 && lineIndex < editor.document.lineCount) {
        const lineContent = editor.document.lineAt(lineIndex).text.trim();
        
        // Filter out lines that start with #[ (attribute macros like #[derive], #[account], etc.)
        if (lineContent.startsWith('#[')) {
          return false;
        }
        
        if (lineContent.includes('declare_id!')) {
          return false;
        }
      }
    } catch (error) {
      console.error('Failed to read line content for macro detection:', error);
    }

    return true;
  }

  /**
   * Calculates the coverage level based on execution count
   * @param {number} count - The execution count
   * @returns {number} The calculated coverage level
   * @private
   */
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

  /**
   * Creates a decoration for a specific code segment
   * @param {CoverageSegment} segment - The coverage segment
   * @param {CoverageFileData} coverageFileData - Coverage data for the file
   * @param {vscode.TextEditor} editor - The text editor instance
   * @returns {vscode.DecorationOptions} The created decoration options
   * @private
   */
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

    const config = vscode.workspace.getConfiguration(CONFIG_ID);
    const showExecutionCount = config.get(SHOW_EXECUTION_COUNT, true);
    const selectedColor = config.get(
      EXECUTION_COUNT_COLOR,
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
