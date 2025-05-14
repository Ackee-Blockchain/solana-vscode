import * as assert from "assert";
import * as vscode from "vscode";
import * as path from "path";
import {
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
} from "../utils";
import { FuzzerType } from "../types";
import { TridentConstants } from "../constants";

suite("Utils Test Suite", () => {
  let mockWorkspaceFolder: vscode.WorkspaceFolder;
  let mockErrorMessage: string | undefined;

  setup(() => {
    mockWorkspaceFolder = {
      uri: vscode.Uri.file("/test/workspace"),
      name: "test",
      index: 0,
    };

    Object.defineProperty(vscode.workspace, "workspaceFolders", {
      get: () => [mockWorkspaceFolder],
      configurable: true,
    });

    mockErrorMessage = undefined;
    vscode.window.showErrorMessage = (message: string) => {
      mockErrorMessage = message;
      return Promise.resolve(undefined);
    };
  });

  teardown(() => {
    Object.defineProperty(vscode.workspace, "workspaceFolders", {
      get: () => undefined,
      configurable: true,
    });
    mockErrorMessage = undefined;
  });

  suite("coverageErrorLog", () => {
    test("should display error message in VS Code and console", () => {
      const errorMessage = "Test error message";
      let consoleErrorCalled = false;
      let consoleErrorMessage: string | undefined;

      const originalDescriptor = Object.getOwnPropertyDescriptor(
        console,
        "error"
      );

      try {
        // Define new console.error
        Object.defineProperty(console, "error", {
          value: function (message: string) {
            consoleErrorCalled = true;
            consoleErrorMessage = message;
          },
          configurable: true,
          writable: true,
        });

        coverageErrorLog(errorMessage);

        assert.strictEqual(
          mockErrorMessage,
          errorMessage,
          "VS Code error message should match"
        );
        assert.strictEqual(
          consoleErrorCalled,
          true,
          "console.error should have been called"
        );
        assert.strictEqual(
          consoleErrorMessage,
          errorMessage,
          "console.error message should match"
        );
      } finally {
        // Restore original console.error
        if (originalDescriptor) {
          Object.defineProperty(console, "error", originalDescriptor);
        }
      }
    });
  });

  suite("getWorkspaceRoot", () => {
    test("should return workspace root path when available", () => {
      const root = getWorkspaceRoot();
      assert.strictEqual(root, mockWorkspaceFolder.uri.fsPath);
    });

    test("should throw error when no workspace folder is found", () => {
      const originalWorkspaceFolders = Object.getOwnPropertyDescriptor(
        vscode.workspace,
        "workspaceFolders"
      );
      Object.defineProperty(vscode.workspace, "workspaceFolders", {
        get: () => undefined,
        configurable: true,
      });

      assert.throws(() => getWorkspaceRoot(), Error);
      assert.strictEqual(mockErrorMessage, "No workspace folder found.");

      if (originalWorkspaceFolders) {
        Object.defineProperty(
          vscode.workspace,
          "workspaceFolders",
          originalWorkspaceFolders
        );
      }
    });
  });

  suite("getFuzzerConstants", () => {
    test("should return AFL constants for AFL fuzzer type", () => {
      const constants = getFuzzerConstants(FuzzerType.Afl);
      assert.deepStrictEqual(constants, TridentConstants.afl);
    });

    test("should return Honggfuzz constants for Honggfuzz fuzzer type", () => {
      const constants = getFuzzerConstants(FuzzerType.Honggfuzz);
      assert.deepStrictEqual(constants, TridentConstants.hfuzz);
    });

    test("should throw error when no fuzzer type is provided", () => {
      assert.throws(() => getFuzzerConstants(undefined), Error);
    });
  });

  suite("getTargetDirPath", () => {
    test("should return correct path for AFL fuzzer", async () => {
      const expectedPath = path.join(
        mockWorkspaceFolder.uri.fsPath,
        ...TridentConstants.afl.TARGET_PATH.split("/")
      );
      const result = await getTargetDirPath(FuzzerType.Afl);
      assert.strictEqual(result, expectedPath);
    });

    test("should throw error when no fuzzer type is provided", async () => {
      await assert.rejects(() => getTargetDirPath(undefined), Error);
    });
  });

  suite("getDirContents", () => {
    test("should return empty array on error", async () => {
      const result = await getDirContents("/nonexistent/path");
      assert.deepStrictEqual(result, []);
    });

    test("should return directory contents when available", async () => {
      const mockContents: [string, vscode.FileType][] = [
        ["file1.txt", vscode.FileType.File],
        ["dir1", vscode.FileType.Directory],
      ];

      const mockFs = {
        readDirectory: async () => mockContents,
      };
      const originalFs = Object.getOwnPropertyDescriptor(
        vscode.workspace,
        "fs"
      );
      Object.defineProperty(vscode.workspace, "fs", {
        value: mockFs,
        configurable: true,
      });

      const result = await getDirContents("/test/path");
      assert.deepStrictEqual(result, mockContents);

      if (originalFs) {
        Object.defineProperty(vscode.workspace, "fs", originalFs);
      }
    });
  });

  suite("getOsDir", () => {
    test("should find OS directory excluding debug and release", () => {
      const contents: [string, vscode.FileType][] = [
        ["debug", vscode.FileType.Directory],
        ["release", vscode.FileType.Directory],
        ["custom-target-triple", vscode.FileType.Directory],
      ];

      const result = getOsDir(contents);
      assert.notStrictEqual(result, "debug");
      assert.notStrictEqual(result, "release");
      assert.strictEqual(result, "custom-target-triple");
    });

    test("should throw error when OS directory is not found", () => {
      const contents: [string, vscode.FileType][] = [
        ["debug", vscode.FileType.Directory],
        ["release", vscode.FileType.Directory],
      ];
      assert.throws(
        () => getOsDir(contents),
        /Could not find os directory in honggfuzz target path/
      );
    });
  });

  suite("extractCorruptedFiles", () => {
    test("should extract profraw file paths from stderr", () => {
      const stderr = `
          warning: /path/to/file1.profraw: invalid instrumentation profile data
          Some other message
          warning: /path/to/file2.profraw: invalid instrumentation profile data
        `;
      const result = extractCorruptedFiles(stderr);
      assert.deepStrictEqual(result, [
        "/path/to/file1.profraw",
        "/path/to/file2.profraw",
      ]);
    });

    test("should return empty array when no corrupted files found", () => {
      const stderr = "Some other error message";
      const result = extractCorruptedFiles(stderr);
      assert.deepStrictEqual(result, []);
    });
  });

  suite("executeCommand", () => {
    test("should execute command successfully", async () => {
      // Mock child_process.exec
      const mockExec = (
        command: string,
        callback: (error: Error | null) => void
      ) => {
        callback(null);
      };
      require("child_process").exec = mockExec;

      await assert.doesNotReject(() => executeCommand("test command"));
    });

    test("should reject on command failure", async () => {
      // Mock child_process.exec with error
      const mockExec = (
        command: string,
        callback: (error: Error | null) => void
      ) => {
        callback(new Error("Command failed"));
      };
      require("child_process").exec = mockExec;

      await assert.rejects(() => executeCommand("test command"));
    });
  });

  suite("readProfrawList", () => {
    test("should read and parse profraw list file", async () => {
      const mockContent = Buffer.from(
        "file1.profraw\nfile2.profraw\n\nfile3.profraw"
      );
      const mockFs = {
        readFile: async () => mockContent,
      };
      const originalFs = Object.getOwnPropertyDescriptor(
        vscode.workspace,
        "fs"
      );
      Object.defineProperty(vscode.workspace, "fs", {
        value: mockFs,
        configurable: true,
      });

      const result = await readProfrawList("/test/path/list");
      assert.deepStrictEqual(result, [
        "file1.profraw",
        "file2.profraw",
        "file3.profraw",
      ]);

      if (originalFs) {
        Object.defineProperty(vscode.workspace, "fs", originalFs);
      }
    });

    test("should return empty array on error", async () => {
      const mockFs = {
        readFile: async () => {
          throw new Error("File read error");
        },
      };
      const originalFs = Object.getOwnPropertyDescriptor(
        vscode.workspace,
        "fs"
      );
      Object.defineProperty(vscode.workspace, "fs", {
        value: mockFs,
        configurable: true,
      });

      const result = await readProfrawList("/test/path/list");
      assert.deepStrictEqual(result, []);

      if (originalFs) {
        Object.defineProperty(vscode.workspace, "fs", originalFs);
      }
    });
  });

  suite("removeFiles", () => {
    test("should not execute command for empty file list", async () => {
      let commandExecuted = false;
      require("child_process").exec = () => {
        commandExecuted = true;
      };

      await removeFiles([]);
      assert.strictEqual(commandExecuted, false);
    });

    test("should execute rm command for file list", async () => {
      let executedCommand: string | undefined;
      require("child_process").exec = (
        command: string,
        callback: (error: Error | null) => void
      ) => {
        executedCommand = command;
        callback(null);
      };

      await removeFiles(["file1.txt", "file2.txt"]);
      assert.strictEqual(executedCommand, "rm -f file1.txt file2.txt");
    });
  });
});
