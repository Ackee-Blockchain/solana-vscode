import * as vscode from "vscode";
import { FuzzerType } from "./types";
import { TridentConstants } from "./constants";
import * as path from "path";

function coverageErrorLog(errorMessage: string) {
  vscode.window.showErrorMessage(errorMessage);
  console.error(errorMessage);
}

function getWorkspaceRoot(): string {
  const workspaceRoot = vscode.workspace.workspaceFolders?.[0].uri.fsPath;
  if (!workspaceRoot) {
    const errorMessage = "No workspace folder found.";
    coverageErrorLog(errorMessage);
    throw new Error(errorMessage);
  }
  return workspaceRoot;
}

function getFuzzerConstants(type?: FuzzerType) {
  if (!type) {
    throw new Error("Fuzzer type is required.");
  }

  const constantsMap = {
    [FuzzerType.Afl]: TridentConstants.afl,
    [FuzzerType.Honggfuzz]: TridentConstants.hfuzz,
  };

  return constantsMap[type];
}

async function getTargetDirPath(type?: FuzzerType): Promise<string> {
  if (!type) {
    throw new Error("Fuzzer type is required.");
  }

  try {
    const workspaceRoot = getWorkspaceRoot();
    const fuzzerConstants = getFuzzerConstants(type);
    const targetDirPathParts = fuzzerConstants.TARGET_PATH.split("/");

    switch (type) {
      case FuzzerType.Afl: {
        return path.join(workspaceRoot, ...targetDirPathParts);
      }
      case FuzzerType.Honggfuzz: {
        let targetDirPath = path.join(workspaceRoot, ...targetDirPathParts);
        const targetContents = await getDirContents(targetDirPath);
        const osDir = getOsDir(targetContents);

        if (osDir) {
          targetDirPath = path.join(targetDirPath, osDir);
        }
        return targetDirPath;
      }
    }
  } catch (error) {
    console.error(`Error getting target directory: ${error}`);
    throw error;
  }
}

async function getDirContents(
  dirPath: string
): Promise<[string, vscode.FileType][]> {
  try {
    const contents = await vscode.workspace.fs.readDirectory(
      vscode.Uri.file(dirPath)
    );
    return contents;
  } catch (error) {
    console.error(`Error getting directory contents: ${error}`);
    return [];
  }
}

function getOsDir(targetDirContents: [string, vscode.FileType][]) {
  // There should only be 3 directories in Honggfuzz target.
  // We are looking for the target triple directory.
  const osDir = targetDirContents.find(
    ([name, type]) =>
      type === vscode.FileType.Directory &&
      name !== "debug" &&
      name !== "release"
  );

  if (!osDir) {
    throw new Error("Could not find os directory in honggfuzz target path.");
  }

  return osDir[0];
}

function extractCorruptedFiles(stderr: string): string[] {
  const corruptedFiles: string[] = [];
  const lines = stderr.split("\n");

  for (const line of lines) {
    if (line.includes(".profraw: invalid instrumentation profile data")) {
      // Extract the file path from warning messages like:
      // "warning: /path/to/file.profraw: invalid instrumentation profile data"
      const [, filePath] = line.match(/warning: ([^:]+\.profraw)/) || [];
      if (filePath) {
        corruptedFiles.push(filePath);
      }
    }
  }

  return corruptedFiles;
}

async function removeFiles(files: string[]) {
  if (files.length === 0) return;

  try {
    await executeCommand(`rm -f ${files.join(" ")}`);
  } catch (error) {
    console.error(`Failed to delete files: ${error}`);
    throw error;
  }
}

async function executeCommand(command: string) {
  await new Promise<void>((resolve, reject) => {
    const exec = require("child_process").exec;
    exec(command, (error: Error | null) => {
      if (error) {
        reject(`Failed to execute command: ${error}`);
      } else {
        resolve();
      }
    });
  });
}

async function readProfrawList(listFilePath: string): Promise<string[]> {
  try {
    const content = await vscode.workspace.fs.readFile(
      vscode.Uri.file(listFilePath)
    );
    const fileList = content
      .toString()
      .split("\n")
      .filter((line) => line.trim().length > 0);
    return fileList;
  } catch (error) {
    console.error(`Failed to read profraw list file: ${error}`);
    return [];
  }
}

export {
  coverageErrorLog,
  getWorkspaceRoot,
  getFuzzerConstants,
  getTargetDirPath,
  getOsDir,
  getDirContents,
  executeCommand,
  extractCorruptedFiles,
  removeFiles,
  readProfrawList,
};
