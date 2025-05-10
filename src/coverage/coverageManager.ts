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
const { DEFAULT_UPDATE_INTERVAL, NOTIFICATION_FILE } = CoverageConstants;

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
  /** Listens for active editor changes */
  private windowChangeListener: vscode.Disposable | undefined;
  /** Type of coverage analysis being performed */
  private coverageType: CoverageType | undefined;
  /** Type of fuzzer being used */
  private fuzzerType: FuzzerType | undefined;
  /** File watcher for notification file */
  private notificationWatcher: vscode.FileSystemWatcher;

  constructor() {
    this.coverageDecorations = new CoverageDecorations();
    this.coverageReportLoader = new CoverageReportLoader();
    this.coverageTestController = vscode.tests.createTestController(
      COVERAGE_ID,
      COVERAGE_LABEL
    );
    this.notificationWatcher = this.setupNotificationWatcher();
    this.disposables = [];

    this.disposables.push(this.coverageTestController);
    this.disposables.push(this.coverageReportLoader);
    this.disposables.push(this.coverageDecorations);
    this.disposables.push(this.notificationWatcher);
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
   * Sets up dynamic coverage monitoring by verifying directory structure and checking for profraw files
   * @private
   * @param {boolean} waitForFiles - If true, waits for profraw files to be created when none exist.
   *                                 If false, throws an error when no files exist.
   * @throws {Error} If:
   *  - trident-tests directory is not found in the workspace
   *  - no profraw files exist and waitForFiles is false
   */
  private async setupDynamicCoverage(waitForFiles: boolean = false) {
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
    if (!hasProfrawFiles && !waitForFiles) {
      const errorMessage =
        "No profraw files found in the target directory. Is the chosen fuzzer running?";
      coverageErrorLog(errorMessage);
      throw new Error(errorMessage);
    }

    if (!hasProfrawFiles && waitForFiles) {
      await this.waitForProfrawFiles();
    }
  }

  /**
   * Creates a file system watcher to wait for profraw files to be created
   * @private
   * @returns {Promise<void>} Resolves when:
   *  - profraw files are detected in the target directory
   *  - rejects after 30 seconds timeout if no files are created
   *  - rejects if there's an error checking for files
   * @throws {Error} If:
   *  - timeout occurs before files are created
   *  - error occurs while checking for files
   *  - error occurs while setting up the file watcher
   */
  private async waitForProfrawFiles() {
    return new Promise<void>(async (resolve, reject) => {
      try {
        const targetPath = await getTargetDirPath(this.fuzzerType);
        const watcher = vscode.workspace.createFileSystemWatcher(
          new vscode.RelativePattern(targetPath, "*.profraw")
        );

        const timeout = setTimeout(() => {
          watcher.dispose();
          reject(new Error("Timeout waiting for profraw files"));
        }, 30000); // 30 second timeout

        watcher.onDidCreate(async () => {
          try {
            if (await this.checkProfrawFiles()) {
              clearTimeout(timeout);
              watcher.dispose();
              resolve();
            }
          } catch (error) {
            clearTimeout(timeout);
            watcher.dispose();
            reject(error);
          }
        });
      } catch (error) {
        reject(error);
      }
    });
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
      " " +
      `CARGO_LLVM_COV_TARGET_DIR="${targetPath}"` +
      " " +
      `cargo llvm-cov report --json --skip-functions ${releaseFlag}` +
      " " +
      `--output-path ${liveReportFilePath}` +
      " " +
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

  /**
   * Sets up a file system watcher for the notification file
   * Watches for file creation events to trigger dynamic coverage setup
   * @private
   * @returns {vscode.FileSystemWatcher} The configured file watcher
   */
  private setupNotificationWatcher(): vscode.FileSystemWatcher {
    const notificationPath = NOTIFICATION_FILE.split("/");
    const completePath = path.join(getWorkspaceRoot(), ...notificationPath);

    const watcher = vscode.workspace.createFileSystemWatcher(completePath);

    watcher.onDidCreate(this.handleNotificationFile.bind(this));

    return watcher;
  }

  /**
   * Handles the creation of the notification file
   * Parses the file contents to determine fuzzer type and sets up dynamic coverage
   * @private
   * @throws {Error} If the notification file cannot be read or parsed
   */
  private async handleNotificationFile() {
    const notificationPath = NOTIFICATION_FILE.split("/");
    const completePath = path.join(getWorkspaceRoot(), ...notificationPath);

    try {
      const jsonContent = await vscode.workspace.fs.readFile(
        vscode.Uri.file(completePath)
      );
      const notification = JSON.parse(jsonContent.toString());

      this.coverageType = CoverageType.Dynamic;
      if (notification.fuzzer.toUpperCase() === "AFL") {
        this.fuzzerType = FuzzerType.Afl;
      } else {
        this.fuzzerType = FuzzerType.Honggfuzz;
      }

      this.setupWindowChangeListener();
      await this.setupDynamicCoverage(true);
      await this.startDynamicCoverage();
    } catch (error) {
      coverageErrorLog(`Error handling notification file: ${error}`);
    }
  }
}

export { CoverageManager };
