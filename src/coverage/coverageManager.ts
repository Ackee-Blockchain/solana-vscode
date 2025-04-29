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
} from "./utils";
import * as path from "path";
import { CoverageType, FuzzerType } from "./types";

const { COVERAGE_ID, COVERAGE_LABEL } = TestApiConstants;
const { IGNORE_FILE_NAME_REGEX } = TridentConstants;
const { DEFAULT_UPDATE_INTERVAL } = CoverageConstants;

class CoverageManager {
  private coverageDecorations: CoverageDecorations;
  private coverageReportLoader: CoverageReportLoader;
  private coverageTestController: vscode.TestController;
  private disposables: { dispose(): any }[];
  private fileSystemWatcher: vscode.FileSystemWatcher | undefined;
  private windowChangeListener: vscode.Disposable | undefined;
  private coverageType: CoverageType | undefined;
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

  dispose() {
    this.disposables.forEach((disposable) => disposable.dispose());
    this.disposables = [];
  }

  public async showCoverage() {
    this.coverageDecorations.clearCoverage(this.coverageTestController);
    await this.setupCoverage();

    switch (this.coverageType) {
      case CoverageType.Static:
        this.showStaticCoverage();
        break;
      case CoverageType.Dynamic:
        this.startDynamicCoverage();
        break;
    }
  }

  private async showStaticCoverage() {
    await this.coverageReportLoader.selectCoverageFile();

    if (this.coverageReportLoader.coverageReport) {
      this.coverageDecorations.displayCoverage(
        this.coverageReportLoader.coverageReport,
        this.coverageTestController
      );
    }
  }

  private async setupCoverage() {
    this.setupWindowChangeListener();

    await this.selectCoverageType();
    if (this.coverageType === CoverageType.Static) {
      return;
    }

    await this.selectFuzzerType();
    await this.setupDynamicCoverage();
  }

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

    try {
      await this.checkProfrawFiles();
    } catch (error) {
      coverageErrorLog(
        "No profraw files found in the target directory. Is the fuzzer running?"
      );
      throw error;
    }

    if (this.fileSystemWatcher) {
      this.fileSystemWatcher.dispose();
    }

    const profilesToWatch = await getTargetDirPath(this.fuzzerType);
    this.fileSystemWatcher =
      vscode.workspace.createFileSystemWatcher(profilesToWatch);
  }

  private async startDynamicCoverage() {
    const updateInterval = vscode.workspace
      .getConfiguration("tridentCoverage")
      .get("dynamicUpdateInterval", DEFAULT_UPDATE_INTERVAL);

    try {
      this.updateCoverage(updateInterval);
    } catch (error) {
      coverageErrorLog(`Coverage update failed: ${error}`);
      throw error;
    }
  }

  private async updateCoverage(updateInterval: number) {
    try {
      const hasProfrawFiles = await this.checkProfrawFiles();
      if (!hasProfrawFiles) {
        vscode.window.showInformationMessage(
          "No profraw files found - fuzzing has stopped."
        );
        return;
      }

      const generateReportCommand = await this.getGenerateReportCommand();
      await executeCommand(generateReportCommand);

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
    } catch (error) {
      throw error;
    }
  }

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

  private async getGenerateReportCommand(): Promise<string> {
    const workspaceRoot = getWorkspaceRoot();
    const fuzzerConstants = getFuzzerConstants(this.fuzzerType);
    const targetPath = await getTargetDirPath(this.fuzzerType);

    const profrawFilePath = path.join(targetPath, fuzzerConstants.PROFRAW_FILE);
    const liveReportFilePath = path.join(
      targetPath,
      fuzzerConstants.LIVE_REPORT_FILE
    );

    return `cd ${workspaceRoot} && LLVM_PROFILE_FILE="${profrawFilePath}" CARGO_LLVM_COV_TARGET_DIR="${targetPath}" cargo llvm-cov report --json --skip-functions --release --output-path ${liveReportFilePath} --ignore-filename-regex ${IGNORE_FILE_NAME_REGEX}`;
  }
}

export { CoverageManager };
