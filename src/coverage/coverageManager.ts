import * as vscode from "vscode";
import { CoverageDecorations } from "./coverageDecoration";
import { CoverageReportLoader } from "./coverageReportLoader";
import {
  TestApiConstants,
  TridentConstants,
  CoverageConstants,
} from "./constants";
import {
  coverageErrorLog,
  executeCommand,
  getDirContents,
  getFuzzerConstants,
  getTargetDirPath,
  getWorkspaceRoot,
  extractCorruptedFiles,
  removeFiles,
  readProfrawList,
} from "./utils";
import * as path from "path";
import { CoverageType, FuzzerType } from "./types";

const { COVERAGE_ID, COVERAGE_LABEL } = TestApiConstants;
const { IGNORE_FILE_NAME_REGEX } = TridentConstants;
const { DEFAULT_UPDATE_INTERVAL } = CoverageConstants;

/**
 * Manages code coverage visualization and updates in VS Code
 * Handles both static and dynamic coverage reporting for AFL and Honggfuzz fuzzers.
 * Coordinates between coverage decorations, report loading, and test controller components.
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
  /** Watches for changes in coverage files */
  private fileSystemWatcher: vscode.FileSystemWatcher | undefined;
  /** Listens for active editor changes */
  private windowChangeListener: vscode.Disposable | undefined;
  /** Type of coverage analysis being performed */
  private coverageType: CoverageType | undefined;
  /** Type of fuzzer being used */
  private fuzzerType: FuzzerType | undefined;

  constructor() {
    this.coverageDecorations = new CoverageDecorations();
    this.coverageReportLoader = new CoverageReportLoader();
    this.coverageTestController = vscode.tests.createTestController(
      COVERAGE_ID,
      COVERAGE_LABEL
    );
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
   * Initiates coverage visualization based on selected coverage type
   * Handles both static coverage from files and dynamic coverage from running fuzzers
   * @throws {Error} If setup fails or required selections are not made
   */
  public async showCoverage() {
    try {
      this.coverageDecorations.clearCoverage(this.coverageTestController);
      await this.setupCoverage();

      switch (this.coverageType) {
        case CoverageType.Static: {
          this.showStaticCoverage();
          break;
        }
        case CoverageType.Dynamic: {
          this.startDynamicCoverage();
          break;
        }
      }
    } catch (error) {
      return;
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

    await this.selectFuzzerType();
    await this.setupDynamicCoverage();
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
   * Prompts user to select fuzzer type (AFL or Honggfuzz)
   * @private
   * @throws {Error} If no fuzzer type is selected
   */
  private async selectFuzzerType() {
    const fuzzerType = await vscode.window.showQuickPick(
      [FuzzerType.Afl, FuzzerType.Honggfuzz],
      {
        placeHolder: "Select fuzzer type",
        title: "Fuzzer Type",
      }
    );

    if (!fuzzerType) {
      const errorMessage = "No fuzzer type selected.";
      coverageErrorLog(errorMessage);
      throw new Error(errorMessage);
    }

    this.fuzzerType = fuzzerType as FuzzerType;
  }

  /**
   * Sets up dynamic coverage monitoring by verifying directory structure and watching for file changes
   * @private
   * @throws {Error} If trident-tests directory is not found or no profraw files exist
   */
  private async setupDynamicCoverage() {
    const workspaceRoot = getWorkspaceRoot();
    try {
      // Check if trident-tests directory exists
      const tridentTestsPath = path.join(workspaceRoot, "trident-tests");
      await vscode.workspace.fs.stat(vscode.Uri.file(tridentTestsPath));
    } catch (error) {
      const errorMessage =
        "Trident tests directory not found in the current workspace. Please navigate to the project's root directory.";
      coverageErrorLog(errorMessage);
      throw new Error(errorMessage);
    }

    const hasProfrawFiles = await this.checkProfrawFiles();
    if (!hasProfrawFiles) {
      const errorMessage =
        "No profraw files found in the target directory. Is the chosen fuzzer running?";
      coverageErrorLog(errorMessage);
      throw new Error(errorMessage);
    }

    if (this.fileSystemWatcher) {
      this.fileSystemWatcher.dispose();
    }

    const profilesToWatch = await getTargetDirPath(this.fuzzerType);
    this.fileSystemWatcher =
      vscode.workspace.createFileSystemWatcher(profilesToWatch);
  }

  /**
   * Initiates dynamic coverage monitoring with periodic updates
   * @private
   * @throws {Error} If coverage update fails
   */
  private async startDynamicCoverage() {
    const updateInterval = vscode.workspace
      .getConfiguration("tridentCoverage")
      .get("dynamicUpdateInterval", DEFAULT_UPDATE_INTERVAL);

    try {
      vscode.window.showInformationMessage(
        "Starting dynamic coverage generation. This could take a while..."
      );
      this.updateCoverage(updateInterval);
    } catch (error) {
      coverageErrorLog(`Coverage update failed: ${error}`);
      throw error;
    }
  }

  /**
   * Updates coverage information periodically for dynamic coverage
   * @private
   * @param {number} updateInterval - Time in milliseconds between updates
   */
  private async updateCoverage(updateInterval: number) {
    const hasProfrawFiles = await this.checkProfrawFiles();
    if (!hasProfrawFiles) {
      vscode.window.showInformationMessage(
        "No profraw files found - fuzzing has stopped."
      );
      await this.removeLeftOverProfrawFiles();
      return;
    }

    await this.generateReport();

    // Load and display the coverage report
    const liveReportFilePath = getFuzzerConstants(
      this.fuzzerType
    ).LIVE_REPORT_FILE;
    const targetPath = await getTargetDirPath(this.fuzzerType);
    const reportUri = vscode.Uri.file(
      path.join(targetPath, liveReportFilePath)
    );
    await this.coverageReportLoader.loadCoverageReport(reportUri);

    if (this.coverageReportLoader.coverageReport) {
      this.coverageDecorations.displayCoverage(
        this.coverageReportLoader.coverageReport,
        this.coverageTestController
      );
    }

    // Wait before next update
    await new Promise((resolve) => setTimeout(resolve, updateInterval));
    this.updateCoverage(updateInterval);
  }

  /**
   * Checks if profraw files exist in the target directory
   * @private
   * @returns {Promise<boolean>} True if profraw files exist
   * @throws {Error} If checking for files fails
   */
  private async checkProfrawFiles(): Promise<boolean> {
    try {
      const targetPath = await getTargetDirPath(this.fuzzerType);
      const profrawFiles = await getDirContents(targetPath);
      const hasProfrawFiles = profrawFiles.some(
        ([name, type]) =>
          type === vscode.FileType.File && name.endsWith(".profraw")
      );

      return hasProfrawFiles;
    } catch (error) {
      console.error(`Failed to check for profraw files: ${error}`);
      throw error;
    }
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

    if (this.fileSystemWatcher) {
      this.fileSystemWatcher.dispose();
      this.fileSystemWatcher = undefined;
    }

    this.coverageType = undefined;
    this.fuzzerType = undefined;

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

    this.disposables.push(this.windowChangeListener);
  }

  /**
   * Handles profdata file management by converting old profdata to profraw
   * @private
   * @throws {Error} If file operations fail
   */
  private async handleProfdata(): Promise<void> {
    const targetPath = await getTargetDirPath(this.fuzzerType);
    const workspaceName = path.basename(getWorkspaceRoot());
    const profDataPath = path.join(targetPath, `${workspaceName}.profdata`);
    const oldProfrawPath = path.join(
      targetPath,
      `${workspaceName}-old.profraw`
    );

    try {
      await executeCommand(`mv ${profDataPath} ${oldProfrawPath}`);
    } catch (error) {
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
    const targetDirPath = await getTargetDirPath(this.fuzzerType);
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
   * @returns {Promise<string>} The complete command string
   */
  private async getGenerateReportCommand(): Promise<string> {
    const workspaceRoot = getWorkspaceRoot();
    const fuzzerConstants = getFuzzerConstants(this.fuzzerType);
    const targetPath = await getTargetDirPath(this.fuzzerType);
    const releaseFlag =
      this.fuzzerType === FuzzerType.Honggfuzz ? "--release" : "";
    const profrawFilePath = path.join(targetPath, fuzzerConstants.PROFRAW_FILE);
    const liveReportFilePath = path.join(
      targetPath,
      fuzzerConstants.LIVE_REPORT_FILE
    );

    return (
      `cd ${workspaceRoot} && LLVM_PROFILE_FILE="${profrawFilePath}"` +
      `CARGO_LLVM_COV_TARGET_DIR="${targetPath}"` +
      `cargo llvm-cov report --json --skip-functions ${releaseFlag}` +
      `--output-path ${liveReportFilePath}` +
      `--ignore-filename-regex ${IGNORE_FILE_NAME_REGEX}`
    );
  }

  /**
   * Removes leftover profraw files and related artifacts
   * @private
   */
  private async removeLeftOverProfrawFiles(): Promise<void> {
    const workspaceRoot = getWorkspaceRoot();
    const workspaceName = path.basename(workspaceRoot);
    const targetDirPath = await getTargetDirPath(this.fuzzerType);
    const profrawListPath = path.join(
      targetDirPath,
      `${workspaceName}-profraw-list`
    );
    const profDataPath = path.join(targetDirPath, `${workspaceName}.profdata`);

    await removeFiles([profrawListPath, profDataPath]);
  }
}

export { CoverageManager };
