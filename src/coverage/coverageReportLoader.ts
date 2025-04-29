import * as vscode from "vscode";
import { CoverageReport, CoverageSegment, FuzzerType } from "./types";
import { coverageErrorLog, getDirContents, getTargetDirPath } from "./utils";
import * as path from "path";

class CoverageReportLoader {
  public coverageReport: CoverageReport | undefined;

  constructor() {
    this.coverageReport = undefined;
  }

  dispose() {
    this.coverageReport = undefined;
  }

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
      coverageErrorLog("Failed to parse coverage report");
    }
  }

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
