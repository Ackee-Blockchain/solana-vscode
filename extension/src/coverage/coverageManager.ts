import * as vscode from "vscode";
import { CoverageDecorations } from "./coverageDecoration";
import { CoverageReportLoader } from "./coverageReportLoader";
import {
  TestApiConstants,
  TridentConstants,
  CoverageServerConstants,
} from "./constants";
import {
  coverageErrorLog,
  executeCommand,
  getTargetDirPath,
  getWorkspaceRoot,
  extractCorruptedFiles,
  removeFiles,
  readProfrawList,
  verifyTridentTestsDirectory,
} from "./utils";
import * as path from "path";
import { CoverageType } from "./types";
import { CoverageServer } from "./coverageServer";

const { COVERAGE_ID, COVERAGE_LABEL } = TestApiConstants;
const { IGNORE_FILE_NAME_REGEX } = TridentConstants;
const { UPDATE_DECORATIONS, SETUP_DYNAMIC_COVERAGE, DISPLAY_FINAL_REPORT } = CoverageServerConstants;

/**
 * Manages code coverage functionality including static and dynamic coverage visualization
 * Coordinates between coverage decorations, report loading, and VS Code test integration
 * Handles both file-based static coverage and real-time dynamic coverage from running fuzzer
 */
class CoverageManager {
  /** Manages the visual representation of coverage in editors */
  private coverageDecorations: CoverageDecorations;
  /** Handles loading and parsing of coverage report files */
  private coverageReportLoader: CoverageReportLoader;
  /** Controls the test UI integration in VS Code */
  private coverageTestController: vscode.TestController;
  /** List of disposable resources to clean up */
  private disposables: { dispose(): any }[];
  /** Listens for active editor changes */
  private windowChangeListener: vscode.Disposable | undefined;
  /** Type of coverage analysis being performed */
  private coverageType: CoverageType | undefined;
  /** Coverage server for dynamic coverage (assigned using method within constructor) */ 
  private coverageServer!: CoverageServer;
  /** Flag to track if coverage update is in progress */
  private isUpdatingCoverage: boolean = false;

  /**
   * Creates a new CoverageManager instance and initializes all required components
   * Sets up coverage decorations, report loader, test controller, and coverage server
   */
  constructor() {
    this.coverageDecorations = new CoverageDecorations();
    this.coverageReportLoader = new CoverageReportLoader();
    this.coverageTestController = vscode.tests.createTestController(
      COVERAGE_ID,
      COVERAGE_LABEL
    );
    this.setupCoverageServer();
    this.disposables = [];

    this.disposables.push(this.coverageTestController);
    this.disposables.push(this.coverageReportLoader);
    this.disposables.push(this.coverageDecorations);
  }

  /**
   * Cleans up all resources used by the coverage manager
   */
  dispose() {
    this.disposables.forEach((disposable) => disposable.dispose());
    this.disposables = [];
  }

  /**
   * Sets up the coverage server and listens for events
   * @private
   */
  private setupCoverageServer() {
    this.coverageServer = new CoverageServer();

    this.coverageServer.on('any', (eventName: string, data: any) => {
      this.handleServerEvent(eventName, data);
    });
  }

  /**
   * Initiates coverage visualization based on selected coverage type
   * Handles both static coverage from files and dynamic coverage from running fuzzers
   */
  public async showCoverage() {
    this.coverageDecorations.clearCoverage(this.coverageTestController);
    await this.setupCoverage();

    switch (this.coverageType) {
      case CoverageType.Static: {
        await this.showStaticCoverage();
        break;
      }
      case CoverageType.Dynamic: {
        await this.setupDynamicCoverage();
        break;
      }
    }
  }

  /**
   * Displays static coverage from a selected coverage report file
   * @private
   */
  private async showStaticCoverage() {
    await this.coverageReportLoader.selectCoverageFile();

    if (this.coverageReportLoader.coverageReport) {
      this.coverageDecorations.displayCoverage(
        this.coverageReportLoader.coverageReport,
        this.coverageTestController
      );
    }
  }

  /**
   * Sets up coverage visualization by configuring listeners and selecting coverage options
   * @private
   * @throws {Error} If required selections are not made
   */
  private async setupCoverage() {
    this.setupWindowChangeListener();

    await this.selectCoverageType();
    if (this.coverageType === CoverageType.Static) {
      return;
    }

    await verifyTridentTestsDirectory();
  }

  /**
   * Prompts user to select coverage type (Static or Dynamic)
   * @private
   * @throws {Error} If no coverage type is selected
   */
  private async selectCoverageType() {
    const coverageType = await vscode.window.showQuickPick(
      [CoverageType.Static, CoverageType.Dynamic],
      {
        placeHolder: "Select coverage type",
        title: "Coverage Type",
      }
    );

    if (!coverageType) {
      const errorMessage = "No coverage type selected.";
      coverageErrorLog(errorMessage);
      throw new Error(errorMessage);
    }

    this.coverageType = coverageType as CoverageType;
  }

  /**
   * Generates a coverage report and updates the coverage decorations
   * @private
   */
  private async updateCoverage() {

    await this.generateReport();

    // Load and display the coverage report
    const targetPath = await getTargetDirPath();
    const reportUri = vscode.Uri.file(
      path.join(targetPath, TridentConstants.LIVE_REPORT_FILE)
    );
    await this.coverageReportLoader.loadCoverageReport(reportUri);
    
    if (this.coverageReportLoader.coverageReport) {
      this.coverageDecorations.displayCoverage(
        this.coverageReportLoader.coverageReport,
        this.coverageTestController
      );
    }
    
    // Remove used live report file
    const liveReportFilePath = path.join(
      targetPath,
      TridentConstants.LIVE_REPORT_FILE
    );
    await removeFiles([liveReportFilePath]);
  }

  /**
   * Closes coverage visualization and cleans up resources
   * Disposes of listeners and watchers, and clears coverage display
   */
  public async closeCoverage() {
    if (this.windowChangeListener) {
      this.windowChangeListener.dispose();
      this.windowChangeListener = undefined;
    }

    this.coverageType = undefined;

    this.coverageDecorations.clearCoverage(this.coverageTestController);
  }

  /**
   * Sets up a listener for active editor changes to update coverage decorations
   * @private
   */
  private setupWindowChangeListener() {
    if (this.windowChangeListener) {
      this.windowChangeListener.dispose();
    }

    this.windowChangeListener = vscode.window.onDidChangeActiveTextEditor(
      (editor) => {
        if (editor && this.coverageReportLoader.coverageReport) {
          this.coverageDecorations.displayLineCoverageDecorations(
            this.coverageReportLoader.coverageReport
          );
        }
      }
    );
  }

  /**
   * Handles profdata file management by converting old profdata to profraw
   * @private
   */
  private async handleProfdata(): Promise<void> {
    const targetPath = await getTargetDirPath();
    const workspaceName = path.basename(getWorkspaceRoot());
    const profDataPath = path.join(targetPath, `${workspaceName}.profdata`);
    const oldProfrawPath = path.join(
      targetPath,
      `${workspaceName}-old.profraw`
    );

    try {
      await executeCommand(`mv ${profDataPath} ${oldProfrawPath}`);
    } catch {
      console.log("No existing profdata file to convert.");
    }
  }

  /**
   * Generates a coverage report from profraw files
   * @private
   * @throws {Error} If report generation fails
   */
  private async generateReport(): Promise<void> {
    const generateReportCommand = await this.getGenerateReportCommand();

    try {
      await executeCommand(generateReportCommand);
    } catch (error: any) {
      const errorMessage = error.toString();
      const corruptedFiles = extractCorruptedFiles(errorMessage);
      if (corruptedFiles.length > 0) {
        await removeFiles(corruptedFiles);
        await executeCommand(generateReportCommand);
      } else {
        await this.removeLeftOverProfrawFiles();
        throw error;
      }
    }

    // Remove used profraw files because all the data
    // is combined and stored in a .profdata file
    const workspaceRoot = getWorkspaceRoot();
    const targetDirPath = await getTargetDirPath();
    const workspaceName = path.basename(workspaceRoot);
    const profrawListPath = path.join(
      targetDirPath,
      `${workspaceName}-profraw-list`
    );
    const profrawFiles = await readProfrawList(profrawListPath);
    await removeFiles(profrawFiles);
    await this.handleProfdata();
  }

  /**
   * Constructs the command for generating a coverage report
   * @private
   * @returns {Promise<string>} The complete shell command string for generating the coverage report
   */
  private async getGenerateReportCommand(): Promise<string> {
    const workspaceRoot = getWorkspaceRoot();
    const targetPath = await getTargetDirPath();
    const liveReportFilePath = path.join(
      targetPath,
      TridentConstants.LIVE_REPORT_FILE
    );

    return (
      `cd ${workspaceRoot} &&` +
      " " +
      `CARGO_LLVM_COV_TARGET_DIR="${targetPath}"` +
      " " +
      `cargo llvm-cov report --json --skip-functions` +
      " " +
      `--output-path ${liveReportFilePath}` +
      " " +
      `--ignore-filename-regex "${IGNORE_FILE_NAME_REGEX}"`
    );
  }

  /**
   * Removes leftover profraw files and related artifacts
   * @private
   */
  private async removeLeftOverProfrawFiles(): Promise<void> {
    const workspaceRoot = getWorkspaceRoot();
    const workspaceName = path.basename(workspaceRoot);
    const targetDirPath = await getTargetDirPath();
    const profrawListPath = path.join(
      targetDirPath,
      `${workspaceName}-profraw-list`
    );
    const profDataPath = path.join(targetDirPath, `${workspaceName}.profdata`);

    await removeFiles([profrawListPath, profDataPath]);
  }

  /**
   * Handles events from the coverage server
   * @private
   */
  private async handleServerEvent(event: string, data: any = {}) {
    console.error(`Received event: ${event} with data:`, data);
    try {
      switch (event) {
        case SETUP_DYNAMIC_COVERAGE:
          await this.setupDynamicCoverage();
          break;
        case UPDATE_DECORATIONS:
          await this.handleUpdateDecorations();
          break;
        case DISPLAY_FINAL_REPORT:
          await this.displayFinalReport(data);
          break;
        default:
          console.error(`Invalid event: ${event}`);
      }
    } catch (error) {
      console.error(`Error handling server event: ${error}`);
    }
  }

  /**
   * Handles update decorations event from the coverage server
   * @private
   */
  private async handleUpdateDecorations() {
    if (this.coverageType !== CoverageType.Dynamic) {
      return;
    }

    // Ignore request if already updating
    if (this.isUpdatingCoverage) {
      console.log('Coverage update already in progress, ignoring request');
      return;
    }

    try {
      this.isUpdatingCoverage = true;
      await this.updateCoverage();
    } finally {
      this.isUpdatingCoverage = false;
    }
  }


  /**
   * Handles display final report event from the coverage server
   * @private
   */
  private async displayFinalReport(data: any = {}) {
    console.error('Display final report with data:', data);
    if (this.coverageType !== CoverageType.Dynamic) {
      return;
    }

    const targetPath = await getTargetDirPath();
    const parentPath = path.dirname(targetPath);
    const reportFileName = `${data.target}-coverage-report.json`;
    const reportUri = vscode.Uri.file(
      path.join(parentPath, reportFileName)
    );
    await this.coverageReportLoader.loadCoverageReport(reportUri);

    if (this.coverageReportLoader.coverageReport) {
      this.coverageDecorations.displayCoverage(
        this.coverageReportLoader.coverageReport,
        this.coverageTestController
      );
    }

    this.coverageType = undefined;
  }

  /**
   * Sets up dynamic coverage by setting the coverage type 
   * to dynamic and verifying the trident tests directory
   * @private
   */
  private async setupDynamicCoverage() {
    this.coverageType = CoverageType.Dynamic;
    
    if (!this.windowChangeListener) {
      this.setupWindowChangeListener();
      await verifyTridentTestsDirectory();
    }
  }
}

export { CoverageManager };
