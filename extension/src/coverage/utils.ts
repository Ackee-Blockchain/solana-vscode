import * as vscode from "vscode";
import { FuzzerType } from "./types";
import { TridentConstants } from "./constants";
import * as path from "path";

/**
 * Logs an error message both to the VS Code error notification and console
 * @param {string} errorMessage - The error message to display
 */
function coverageErrorLog(errorMessage: string) {
  vscode.window.showErrorMessage(errorMessage);
  console.error(errorMessage);
}

/**
 * Gets the root path of the current workspace
 * @returns {string} The absolute path to the workspace root
 * @throws {Error} If no workspace folder is found
 */
function getWorkspaceRoot(): string {
  const workspaceRoot = vscode.workspace.workspaceFolders?.[0].uri.fsPath;
  if (!workspaceRoot) {
    const errorMessage = "No workspace folder found.";
    coverageErrorLog(errorMessage);
    throw new Error(errorMessage);
  }
  return workspaceRoot;
}

/**
 * Retrieves the constants associated with a specific fuzzer type
 * @param {FuzzerType} type - The type of fuzzer
 * @returns {Object} Constants specific to the fuzzer type
 * @throws {Error} If no fuzzer type is provided
 */
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

/**
 * Gets the target directory path for a specific fuzzer type
 * @param {FuzzerType} type - The type of fuzzer
 * @returns {Promise<string>} The absolute path to the target directory
 * @throws {Error} If no fuzzer type is provided or if there's an error getting the directory
 */
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

/**
 * Gets the contents of a directory
 * @param {string} dirPath - The path to the directory
 * @returns {Promise<[string, vscode.FileType][]>} Array of tuples containing file names and their types, or empty array if reading fails
 */
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

/**
 * Gets the OS-specific directory from the target directory contents
 * @param {[string, vscode.FileType][]} targetDirContents - Array of directory entries
 * @returns {string} The name of the OS-specific directory
 * @throws {Error} If the OS directory cannot be found
 */
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

/**
 * Extracts paths of corrupted profraw files from error output
 * @param {string} stderr - Error output containing file corruption messages
 * @returns {string[]} Array of paths to corrupted profraw files
 */
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

/**
 * Removes specified files from the filesystem
 * @param {string[]} files - Array of file paths to remove
 * @returns {Promise<void>}
 * @throws {Error} If there's an error deleting the files
 */
async function removeFiles(files: string[]) {
  if (files.length === 0) {
    return;
  }

  try {
    await executeCommand(`rm -f ${files.join(" ")}`);
  } catch (error) {
    console.error(`Failed to delete files: ${error}`);
    throw error;
  }
}

/**
 * Executes a shell command
 * @param {string} command - The command to execute
 * @returns {Promise<void>}
 * @throws {Error} If the command execution fails
 */
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

/**
 * Reads the list of profraw files from a list file
 * @param {string} profrawListPath - Path to the file containing the list of profraw files
 * @returns {Promise<string[]>} Array of profraw file paths, or empty array if reading fails
 */
async function readProfrawList(profrawListPath: string): Promise<string[]> {
  try {
    const content = await vscode.workspace.fs.readFile(
      vscode.Uri.file(profrawListPath)
    );
    const fileList = content
      .toString()
      .split("\n")
      .filter((line) => line.trim().length > 0);
    return fileList;
  } catch (error) {
    console.error(`Failed to read profraw list: ${error}`);
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
