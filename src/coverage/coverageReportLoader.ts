import * as vscode from "vscode";
import { CoverageReport, CoverageSegment, FuzzerType } from "./types";
import { coverageErrorLog, getDirContents, getTargetDirPath } from "./utils";
import * as path from "path";

/**
 * Handles loading and parsing of coverage report files
 * Manages the discovery, selection, and parsing of coverage report files
 * from different fuzzer outputs (AFL and Honggfuzz).
 */
class CoverageReportLoader {
  /**
   * The currently loaded coverage report
   * @type {CoverageReport | undefined}
   */
  public coverageReport: CoverageReport | undefined;

  constructor() {
    this.coverageReport = undefined;
  }

  /**
   * Cleans up resources by clearing the current coverage report
   */
  dispose() {
    this.coverageReport = undefined;
  }

  /**
   * Finds all coverage report files in the fuzzer target directories
   * Searches in both AFL and Honggfuzz target directories for coverage files
   * @returns {Promise<vscode.Uri[]>} Array of URIs to coverage report files
   */
  public async findCoverageFiles(): Promise<vscode.Uri[]> {
    let coverageFiles: vscode.Uri[] = [];
    if (!vscode.workspace.workspaceFolders) {
      return coverageFiles;
    }

    const hfuzzTargetPath = await getTargetDirPath(FuzzerType.Honggfuzz);
    const aflTargetPath = await getTargetDirPath(FuzzerType.Afl);
    const getHfuzzTargetContents = await getDirContents(hfuzzTargetPath);
    const getAflTargetContents = await getDirContents(aflTargetPath);

    coverageFiles.push(
      ...this.getFilteredCoverageFiles(hfuzzTargetPath, getHfuzzTargetContents)
    );
    coverageFiles.push(
      ...this.getFilteredCoverageFiles(aflTargetPath, getAflTargetContents)
    );

    return coverageFiles;
  }

  /**
   * Loads and parses a coverage report from a file
   * @param {vscode.Uri} filePath - URI of the coverage report file to load
   * @throws {Error} If the file cannot be read or parsed
   */
  public async loadCoverageReport(filePath: vscode.Uri) {
    try {
      const data = await vscode.workspace.fs.readFile(filePath);
      this.parseCoverageReport(data);
    } catch {
      const errorMessage = "Failed to read coverage report file.";
      coverageErrorLog(errorMessage);
      throw new Error(errorMessage);
    }
  }

  /**
   * Handles the selection of a coverage file based on available files
   * If no files are found, shows a file picker
   * If one file is found, loads it directly
   * If multiple files are found, shows a quick pick menu
   */
  public async selectCoverageFile() {
    const coverageFiles = await this.findCoverageFiles();

    switch (coverageFiles.length) {
      case 0: {
        await this.showCoverageFilePicker();
        break;
      }
      case 1: {
        await this.loadCoverageReport(coverageFiles[0]);
        break;
      }
      default: {
        await this.showCoverageQuickPick(coverageFiles);
      }
    }
  }

  /**
   * Shows a file picker dialog for selecting a coverage report file
   * @private
   * @throws {Error} If no file is selected
   */
  private async showCoverageFilePicker() {
    const optionSettings: vscode.OpenDialogOptions = {
      defaultUri:
        vscode.workspace.workspaceFolders !== undefined
          ? vscode.workspace.workspaceFolders[0].uri
          : undefined,
      canSelectFiles: true,
      canSelectFolders: false,
      canSelectMany: false,
      filters: {
        "Coverage Report": ["json"],
      },
    };

    const filePath = await vscode.window.showOpenDialog(optionSettings);
    if (filePath !== undefined) {
      await this.loadCoverageReport(filePath[0]);
    } else {
      const errorMessage = "No coverage report file selected.";
      coverageErrorLog(errorMessage);
      throw new Error(errorMessage);
    }
  }

  /**
   * Shows a quick pick menu for selecting from multiple coverage report files
   * @private
   * @param {vscode.Uri[]} coverageFiles - Array of coverage file URIs to choose from
   * @throws {Error} If no file is selected
   */
  private async showCoverageQuickPick(coverageFiles: vscode.Uri[]) {
    const files = coverageFiles.map((file) => ({
      label: path.basename(file.fsPath),
      uri: file,
    }));

    const selectedFile = await vscode.window.showQuickPick(files, {
      placeHolder: "Select a coverage report",
    });

    if (selectedFile) {
      await this.loadCoverageReport(selectedFile.uri);
    } else {
      const errorMessage = "No coverage report file selected.";
      coverageErrorLog(errorMessage);
      throw new Error(errorMessage);
    }
  }

  /**
   * Transforms a raw segment array into a CoverageSegment object
   * @private
   * @param {any[]} segmentArray - Raw array containing segment data
   * @returns {CoverageSegment} Structured coverage segment object
   */
  private transformSegment(segmentArray: any[]): CoverageSegment {
    return {
      line: segmentArray[0],
      column: segmentArray[1],
      execution_count: segmentArray[2],
      has_count: segmentArray[3],
      is_region_entry: segmentArray[4],
      is_gap_region: segmentArray[5],
    };
  }

  /**
   * Parses raw coverage report data and updates the current coverage report
   * @private
   * @param {Uint8Array} data - Raw binary data from the coverage report file
   * @throws {Error} When the data cannot be parsed as valid JSON or does not match the expected coverage report format
   */
  private parseCoverageReport(data: Uint8Array) {
    try {
      const stringData = Buffer.from(data).toString("utf-8");
      const parsedData = JSON.parse(stringData);

      // segments are stored as arrays
      parsedData.data[0].files.forEach((file: any) => {
        file.segments = file.segments.map((segment: any[]) =>
          this.transformSegment(segment)
        );
      });

      this.coverageReport = parsedData as CoverageReport;
    } catch {
      const errorMessage = "Failed to parse coverage report";
      coverageErrorLog(errorMessage);
      throw new Error(errorMessage);
    }
  }

  /**
   * Filters directory contents to find coverage report files
   * @private
   * @param {string} dirPath - Path to the directory to search
   * @param {[string, vscode.FileType][]} dirContents - Array of directory entries
   * @returns {vscode.Uri[]} Array of URIs to coverage report files
   */
  private getFilteredCoverageFiles(
    dirPath: string,
    dirContents: [string, vscode.FileType][]
  ): vscode.Uri[] {
    const coverageFiles: vscode.Uri[] = [];

    for (const [name, type] of dirContents) {
      if (
        type === vscode.FileType.File &&
        name.toLowerCase().includes("cov") &&
        name.toLowerCase().endsWith(".json")
      ) {
        coverageFiles.push(vscode.Uri.file(path.join(dirPath, name)));
      }
    }

    return coverageFiles;
  }
}

export { CoverageReportLoader };
