import * as assert from "assert";
import * as vscode from "vscode";
import { CoverageManager } from "../coverageManager";
import { CoverageType, FuzzerType } from "../types";
import {
  TridentConstants,
  CoverageConstants,
} from "../constants";

const { DEFAULT_UPDATE_INTERVAL } = CoverageConstants;

// Test-only subclass to access private methods
class TestCoverageManager extends CoverageManager {
  public testSetupCoverage(): Promise<void> {
    return this["setupCoverage"]();
  }

  public testStartDynamicCoverage(): Promise<void> {
    return this["startDynamicCoverage"]();
  }

  public testGetGenerateReportCommand(): Promise<string> {
    return this["getGenerateReportCommand"]();
  }

  public testHandleProfdata(): Promise<void> {
    return this["handleProfdata"]();
  }

  public async testRemoveLeftOverProfrawFiles(): Promise<void> {
    await this["removeLeftOverProfrawFiles"]();
  }

  public async testCheckProfrawFiles(): Promise<boolean> {
    return await this["checkProfrawFiles"]();
  }

  public async testGenerateReport(): Promise<void> {
    await this["generateReport"]();
  }

  public getCoverageType(): CoverageType | undefined {
    return this["coverageType"];
  }

  public getFuzzerType(): FuzzerType | undefined {
    return this["fuzzerType"];
  }

  public setCoverageType(type: CoverageType): void {
    this["coverageType"] = type;
  }

  public setFuzzerType(type: FuzzerType): void {
    this["fuzzerType"] = type;
  }
}

suite("Coverage Manager Test Suite", () => {
  let coverageManager: TestCoverageManager;
  let mockWorkspaceFolder: vscode.WorkspaceFolder;
  let mockTestController: vscode.TestController;
  let disposeCalled: number;

  setup(() => {
    disposeCalled = 0;
    mockWorkspaceFolder = {
      uri: vscode.Uri.file("/test/workspace"),
      name: "test",
      index: 0,
    };

    Object.defineProperty(vscode.workspace, "workspaceFolders", {
      get: () => [mockWorkspaceFolder],
      configurable: true,
    });

    mockTestController = {
      createRunProfile: () => ({
        dispose: () => {
          disposeCalled++;
        },
      }),
      createTestRun: () => ({
        end: () => {},
        addCoverage: () => {},
      }),
      dispose: () => {
        disposeCalled++;
      },
    } as any;

    Object.defineProperty(vscode.tests, "createTestController", {
      value: () => mockTestController,
      configurable: true,
    });

    coverageManager = new TestCoverageManager();
  });

  teardown(() => {
    coverageManager.dispose();
    Object.defineProperty(vscode.workspace, "workspaceFolders", {
      get: () => undefined,
      configurable: true,
    });
  });

  test("should initialize with required components", () => {
    assert.ok(coverageManager, "Coverage manager should be created");
  });

  test("should dispose all components", () => {
    coverageManager.dispose();
    assert.ok(
      disposeCalled > 0,
      "Should dispose test controller and other components"
    );
  });

  suite("showCoverage", () => {
    test("should handle static coverage", async () => {
      let coverageDisplayed = false;
      let coverageCleared = false;

      const originalSetupCoverage = (coverageManager as any).setupCoverage;
      (coverageManager as any).setupCoverage = async () => {
        (coverageManager as any).coverageType = CoverageType.Static;
      };

      (coverageManager as any).coverageDecorations = {
        clearCoverage: () => {
          coverageCleared = true;
        },
        displayCoverage: () => {
          coverageDisplayed = true;
        },
      };

      (coverageManager as any).coverageReportLoader = {
        selectCoverageFile: async () => {},
        coverageReport: {
          data: [],
          type: "llvm",
          version: "2.0.0",
          cargo_llvm_cov: {
            version: "0.5.0",
            manifest_path: "/test/Cargo.toml",
          },
        },
      };

      try {
        await coverageManager.showCoverage();
        assert.ok(coverageCleared, "Should clear existing coverage");
        assert.ok(coverageDisplayed, "Should display new coverage");
      } finally {
        (coverageManager as any).setupCoverage = originalSetupCoverage;
      }
    });

    test("should handle dynamic coverage", async () => {
      let dynamicCoverageStarted = false;
      let coverageCleared = false;

      const originalSetupCoverage = (coverageManager as any).setupCoverage;
      const originalStartDynamicCoverage = (coverageManager as any)
        .startDynamicCoverage;

      (coverageManager as any).setupCoverage = async () => {
        (coverageManager as any).coverageType = CoverageType.Dynamic;
      };

      (coverageManager as any).startDynamicCoverage = async () => {
        dynamicCoverageStarted = true;
      };

      (coverageManager as any).coverageDecorations = {
        clearCoverage: () => {
          coverageCleared = true;
        },
      };

      try {
        await coverageManager.showCoverage();
        assert.ok(coverageCleared, "Should clear existing coverage");
        assert.ok(dynamicCoverageStarted, "Should start dynamic coverage");
      } finally {
        (coverageManager as any).setupCoverage = originalSetupCoverage;
        (coverageManager as any).startDynamicCoverage =
          originalStartDynamicCoverage;
      }
    });
  });

  suite("Window Change Listener", () => {
    test("should setup window change listener and handle editor changes", () => {
      let listenerCallback:
        | ((editor: vscode.TextEditor | undefined) => void)
        | undefined;
      let coverageUpdated = false;

      const originalOnDidChangeActiveTextEditor =
        Object.getOwnPropertyDescriptor(
          vscode.window,
          "onDidChangeActiveTextEditor"
        );

      Object.defineProperty(vscode.window, "onDidChangeActiveTextEditor", {
        get:
          () => (callback: (editor: vscode.TextEditor | undefined) => void) => {
            listenerCallback = callback;
            return { dispose: () => {} };
          },
        configurable: true,
      });

      (coverageManager as any).coverageDecorations = {
        displayLineCoverageDecorations: () => {
          coverageUpdated = true;
        },
      };

      (coverageManager as any).coverageReportLoader = {
        coverageReport: {
          data: [],
        },
      };

      try {
        (coverageManager as any).setupWindowChangeListener();
        assert.ok(
          listenerCallback,
          "Window change listener callback should be registered"
        );

        listenerCallback({
          document: { uri: vscode.Uri.file("/test/file.rs") },
        } as any);

        assert.ok(
          coverageUpdated,
          "Should update coverage decorations on editor change"
        );
      } finally {
        if (originalOnDidChangeActiveTextEditor) {
          Object.defineProperty(
            vscode.window,
            "onDidChangeActiveTextEditor",
            originalOnDidChangeActiveTextEditor
          );
        }
      }
    });
  });

  suite("Coverage Report Generation", () => {
    test("should generate coverage report", async () => {
      let workspaceRootCalled = false;
      let targetDirPathCalled = false;
      let readProfrawListCalled = false;
      let removeFilesCalled = false;
      let handleProfdataCalled = false;
      let commandExecuted = false;

      const originalUtils = {
        getWorkspaceRoot: require("../utils").getWorkspaceRoot,
        getTargetDirPath: require("../utils").getTargetDirPath,
        readProfrawList: require("../utils").readProfrawList,
        removeFiles: require("../utils").removeFiles,
        executeCommand: require("../utils").executeCommand,
      };

      require("../utils").getWorkspaceRoot = () => {
        workspaceRootCalled = true;
        return "/test/workspace";
      };

      require("../utils").getTargetDirPath = async () => {
        targetDirPathCalled = true;
        return "/test/target/dir";
      };

      require("../utils").readProfrawList = async () => {
        readProfrawListCalled = true;
        return ["file1.profraw", "file2.profraw"];
      };

      require("../utils").removeFiles = async (files: string[]) => {
        removeFilesCalled = true;
        assert.deepStrictEqual(
          files,
          ["file1.profraw", "file2.profraw"],
          "Should remove correct profraw files"
        );
      };

      require("../utils").executeCommand = async () => {
        commandExecuted = true;
      };

      const originalHandleProfdata = coverageManager["handleProfdata"];
      coverageManager["handleProfdata"] = async function (
        this: TestCoverageManager
      ) {
        handleProfdataCalled = true;
      };

      try {
        coverageManager.setFuzzerType(FuzzerType.Afl);
        await coverageManager.testGenerateReport();

        assert.ok(workspaceRootCalled, "Should get workspace root");
        assert.ok(targetDirPathCalled, "Should get target directory path");
        assert.ok(readProfrawListCalled, "Should read profraw list");
        assert.ok(removeFilesCalled, "Should remove profraw files");
        assert.ok(handleProfdataCalled, "Should handle profdata");
        assert.ok(
          commandExecuted,
          "Should execute coverage report generation command"
        );
      } finally {
        Object.assign(require("../utils"), originalUtils);
        coverageManager["handleProfdata"] = originalHandleProfdata;
      }
    });

    test("should handle corrupted profraw files", async () => {
      let filesRemoved = false;
      let retryExecuted = false;
      let executionCount = 0;

      const originalExecuteCommand = require("../utils").executeCommand;
      const originalRemoveFiles = require("../utils").removeFiles;

      require("../utils").executeCommand = async () => {
        executionCount++;
        if (executionCount === 1) {
          throw new Error(
            "warning: /path/to/file.profraw: invalid instrumentation profile data"
          );
        }
        retryExecuted = true;
      };

      require("../utils").removeFiles = async () => {
        filesRemoved = true;
      };

      try {
        coverageManager.setFuzzerType(FuzzerType.Afl);
        await coverageManager.testGenerateReport();
        assert.ok(filesRemoved, "Should remove corrupted files");
        assert.ok(retryExecuted, "Should retry report generation");
      } finally {
        require("../utils").executeCommand = originalExecuteCommand;
        require("../utils").removeFiles = originalRemoveFiles;
      }
    });
  });

  suite("checkProfrawFiles", () => {
    test("should detect profraw files in target directory", async () => {
      let targetDirPathCalled = false;
      let getDirContentsCalled = false;

      const originalUtils = {
        getTargetDirPath: require("../utils").getTargetDirPath,
        getDirContents: require("../utils").getDirContents,
      };

      require("../utils").getTargetDirPath = async () => {
        targetDirPathCalled = true;
        return "/test/target/dir";
      };

      require("../utils").getDirContents = async () => {
        getDirContentsCalled = true;
        return [
          ["test.profraw", vscode.FileType.File],
          ["other.txt", vscode.FileType.File],
        ];
      };

      try {
        coverageManager.setFuzzerType(FuzzerType.Afl);
        const result = await coverageManager.testCheckProfrawFiles();
        assert.ok(targetDirPathCalled, "Should get target directory path");
        assert.ok(getDirContentsCalled, "Should get directory contents");
        assert.ok(result, "Should detect profraw files");
      } finally {
        Object.assign(require("../utils"), originalUtils);
      }
    });

    test("should return false when no profraw files exist", async () => {
      require("../utils").getDirContents = async () => {
        return [
          ["test.txt", vscode.FileType.File],
          ["other.log", vscode.FileType.File],
        ];
      };

      try {
        coverageManager.setFuzzerType(FuzzerType.Afl);
        const result = await coverageManager.testCheckProfrawFiles();
        assert.strictEqual(
          result,
          false,
          "Should not detect any profraw files"
        );
      } finally {
        require("../utils").getDirContents = require("../utils").getDirContents;
      }
    });

    test("should handle errors when checking files", async () => {
      require("../utils").getDirContents = async () => {
        throw new Error("Failed to read directory");
      };

      try {
        coverageManager.setFuzzerType(FuzzerType.Afl);
        await assert.rejects(
          () => coverageManager.testCheckProfrawFiles(),
          Error,
          "Should throw error when directory read fails"
        );
      } finally {
        require("../utils").getDirContents = require("../utils").getDirContents;
      }
    });
  });

  suite("removeLeftOverProfrawFiles", () => {
    test("should remove leftover profraw files", async () => {
      let filesRemoved: string[] = [];
      const originalUtils = {
        getWorkspaceRoot: require("../utils").getWorkspaceRoot,
        getTargetDirPath: require("../utils").getTargetDirPath,
        removeFiles: require("../utils").removeFiles,
      };

      require("../utils").getWorkspaceRoot = () => "/test/workspace";
      require("../utils").getTargetDirPath = async () => "/test/target/dir";
      require("../utils").removeFiles = async (files: string[]) => {
        filesRemoved = files;
      };

      try {
        coverageManager.setFuzzerType(FuzzerType.Afl);
        await coverageManager.testRemoveLeftOverProfrawFiles();

        assert.deepStrictEqual(
          filesRemoved,
          [
            "/test/target/dir/workspace-profraw-list",
            "/test/target/dir/workspace.profdata",
          ],
          "Should remove profraw list and profdata files"
        );
      } finally {
        Object.assign(require("../utils"), originalUtils);
      }
    });

    test("should handle errors during file removal", async () => {
      const originalUtils = {
        getWorkspaceRoot: require("../utils").getWorkspaceRoot,
        getTargetDirPath: require("../utils").getTargetDirPath,
        removeFiles: require("../utils").removeFiles,
      };

      require("../utils").removeFiles = async () => {
        throw new Error("Failed to remove files");
      };

      try {
        coverageManager.setFuzzerType(FuzzerType.Afl);
        await assert.rejects(
          () => coverageManager.testRemoveLeftOverProfrawFiles(),
          Error,
          "Should throw error when file removal fails"
        );
      } finally {
        Object.assign(require("../utils"), originalUtils);
      }
    });
  });

  suite("setupCoverage", () => {
    test("should prompt for coverage type selection", async () => {
      let quickPickShown = false;
      const originalShowQuickPick = Object.getOwnPropertyDescriptor(
        vscode.window,
        "showQuickPick"
      );

      Object.defineProperty(vscode.window, "showQuickPick", {
        value: (
          items: readonly string[] | Thenable<readonly string[]>,
          options?: vscode.QuickPickOptions,
          token?: vscode.CancellationToken
        ): Thenable<string | undefined> => {
          quickPickShown = true;
          assert.deepStrictEqual(
            [...(items as readonly string[])],
            [CoverageType.Static, CoverageType.Dynamic],
            "Should show correct coverage type options"
          );
          return Promise.resolve(CoverageType.Static);
        },
        configurable: true,
      });

      try {
        await coverageManager.testSetupCoverage();
        assert.ok(quickPickShown, "Should show quick pick for coverage type");
        assert.strictEqual(
          coverageManager.getCoverageType(),
          CoverageType.Static,
          "Should set selected coverage type"
        );
      } finally {
        if (originalShowQuickPick) {
          Object.defineProperty(
            vscode.window,
            "showQuickPick",
            originalShowQuickPick
          );
        }
      }
    });

    test("should prompt for fuzzer type selection for dynamic coverage", async () => {
      let quickPickCallCount = 0;
      const originalShowQuickPick = Object.getOwnPropertyDescriptor(
        vscode.window,
        "showQuickPick"
      );
      const originalUtils = {
        getWorkspaceRoot: require("../utils").getWorkspaceRoot,
        getTargetDirPath: require("../utils").getTargetDirPath,
        getDirContents: require("../utils").getDirContents,
      };
      const originalStat = Object.getOwnPropertyDescriptor(
        vscode.workspace.fs,
        "stat"
      );
      const originalCreateFileSystemWatcher = Object.getOwnPropertyDescriptor(
        vscode.workspace,
        "createFileSystemWatcher"
      );

      Object.defineProperty(vscode.window, "showQuickPick", {
        value: (
          items: readonly string[] | Thenable<readonly string[]>,
          options?: vscode.QuickPickOptions,
          token?: vscode.CancellationToken
        ): Thenable<string | undefined> => {
          quickPickCallCount++;
          if (quickPickCallCount === 1) {
            return Promise.resolve(CoverageType.Dynamic);
          }
          assert.deepStrictEqual(
            [...(items as readonly string[])],
            [FuzzerType.Afl, FuzzerType.Honggfuzz],
            "Should show correct fuzzer type options"
          );
          return Promise.resolve(FuzzerType.Afl);
        },
        configurable: true,
      });

      require("../utils").getWorkspaceRoot = () => "/test/workspace";
      require("../utils").getTargetDirPath = async () => "/test/target/dir";
      require("../utils").getDirContents = async () => [
        ["test.profraw", vscode.FileType.File],
      ];

      Object.defineProperty(vscode.workspace.fs, "stat", {
        value: async () => ({} as vscode.FileStat),
        configurable: true,
      });

      Object.defineProperty(vscode.workspace, "createFileSystemWatcher", {
        value: () => ({
          dispose: () => {},
          onDidChange: () => ({ dispose: () => {} }),
          onDidCreate: () => ({ dispose: () => {} }),
          onDidDelete: () => ({ dispose: () => {} }),
        }),
        configurable: true,
      });

      try {
        await coverageManager.testSetupCoverage();
        assert.strictEqual(
          quickPickCallCount,
          2,
          "Should show quick pick twice"
        );
        assert.strictEqual(
          coverageManager.getFuzzerType(),
          FuzzerType.Afl,
          "Should set selected fuzzer type"
        );
      } finally {
        if (originalShowQuickPick) {
          Object.defineProperty(
            vscode.window,
            "showQuickPick",
            originalShowQuickPick
          );
        }
        if (originalStat) {
          Object.defineProperty(vscode.workspace.fs, "stat", originalStat);
        }
        if (originalCreateFileSystemWatcher) {
          Object.defineProperty(
            vscode.workspace,
            "createFileSystemWatcher",
            originalCreateFileSystemWatcher
          );
        }
        Object.assign(require("../utils"), originalUtils);
      }
    });

    test("should throw error when no coverage type selected", async () => {
      const originalShowQuickPick = Object.getOwnPropertyDescriptor(
        vscode.window,
        "showQuickPick"
      );

      Object.defineProperty(vscode.window, "showQuickPick", {
        value: () => Promise.resolve(undefined),
        configurable: true,
      });

      try {
        await assert.rejects(
          () => coverageManager.testSetupCoverage(),
          Error,
          "Should throw error when no coverage type selected"
        );
      } finally {
        if (originalShowQuickPick) {
          Object.defineProperty(
            vscode.window,
            "showQuickPick",
            originalShowQuickPick
          );
        }
      }
    });

    test("should throw error when trident-tests directory not found", async () => {
      const originalShowQuickPick = Object.getOwnPropertyDescriptor(
        vscode.window,
        "showQuickPick"
      );
      const originalStat = Object.getOwnPropertyDescriptor(
        vscode.workspace.fs,
        "stat"
      );
      const originalUtils = {
        getWorkspaceRoot: require("../utils").getWorkspaceRoot,
      };

      Object.defineProperty(vscode.window, "showQuickPick", {
        value: () => Promise.resolve(CoverageType.Dynamic),
        configurable: true,
      });

      require("../utils").getWorkspaceRoot = () => "/test/workspace";

      Object.defineProperty(vscode.workspace.fs, "stat", {
        value: async () => {
          throw new Error("Directory not found");
        },
        configurable: true,
      });

      try {
        await assert.rejects(
          () => coverageManager.testSetupCoverage(),
          /Trident tests directory not found/,
          "Should throw error when trident-tests directory not found"
        );
      } finally {
        if (originalShowQuickPick) {
          Object.defineProperty(
            vscode.window,
            "showQuickPick",
            originalShowQuickPick
          );
        }
        if (originalStat) {
          Object.defineProperty(vscode.workspace.fs, "stat", originalStat);
        }
        Object.assign(require("../utils"), originalUtils);
      }
    });

    test("should throw error when no profraw files found", async () => {
      const originalShowQuickPick = Object.getOwnPropertyDescriptor(
        vscode.window,
        "showQuickPick"
      );
      const originalStat = Object.getOwnPropertyDescriptor(
        vscode.workspace.fs,
        "stat"
      );
      const originalUtils = {
        getWorkspaceRoot: require("../utils").getWorkspaceRoot,
        getTargetDirPath: require("../utils").getTargetDirPath,
        getDirContents: require("../utils").getDirContents,
      };

      Object.defineProperty(vscode.window, "showQuickPick", {
        value: () => Promise.resolve(CoverageType.Dynamic),
        configurable: true,
      });

      require("../utils").getWorkspaceRoot = () => "/test/workspace";
      require("../utils").getTargetDirPath = async () => "/test/target/dir";
      require("../utils").getDirContents = async () => [
        ["test.txt", vscode.FileType.File],
      ];

      Object.defineProperty(vscode.workspace.fs, "stat", {
        value: async () => ({} as vscode.FileStat),
        configurable: true,
      });

      try {
        await assert.rejects(
          () => coverageManager.testSetupCoverage(),
          /No profraw files found/,
          "Should throw error when no profraw files found"
        );
      } finally {
        if (originalShowQuickPick) {
          Object.defineProperty(
            vscode.window,
            "showQuickPick",
            originalShowQuickPick
          );
        }
        if (originalStat) {
          Object.defineProperty(vscode.workspace.fs, "stat", originalStat);
        }
        Object.assign(require("../utils"), originalUtils);
      }
    });
  });

  suite("startDynamicCoverage", () => {
    test("should setup dynamic coverage updates", async () => {
      let updateIntervalCalled = false;
      let infoMessageShown = false;
      let updateCoverageStarted = false;

      const originalGetConfiguration = Object.getOwnPropertyDescriptor(
        vscode.workspace,
        "getConfiguration"
      );
      const originalShowInfoMessage = Object.getOwnPropertyDescriptor(
        vscode.window,
        "showInformationMessage"
      );
      const originalUpdateCoverage = (coverageManager as any)["updateCoverage"];

      Object.defineProperty(vscode.workspace, "getConfiguration", {
        value: () => ({
          get: (key: string, defaultValue: any) => {
            updateIntervalCalled = true;
            return defaultValue;
          },
        }),
        configurable: true,
      });

      Object.defineProperty(vscode.window, "showInformationMessage", {
        value: async (message: string) => {
          infoMessageShown = true;
          assert.strictEqual(
            message,
            "Starting dynamic coverage generation. This could take a while...",
            "Should show correct info message"
          );
          return undefined;
        },
        configurable: true,
      });

      (coverageManager as any)["updateCoverage"] = async (interval: number) => {
        updateCoverageStarted = true;
        assert.strictEqual(
          interval,
          DEFAULT_UPDATE_INTERVAL,
          "Should use default update interval"
        );
      };

      coverageManager.setFuzzerType(FuzzerType.Afl);

      try {
        await coverageManager.testStartDynamicCoverage();
        assert.ok(
          updateIntervalCalled,
          "Should get update interval from configuration"
        );
        assert.ok(infoMessageShown, "Should show info message");
        assert.ok(updateCoverageStarted, "Should start coverage updates");
      } finally {
        if (originalGetConfiguration) {
          Object.defineProperty(
            vscode.workspace,
            "getConfiguration",
            originalGetConfiguration
          );
        }
        if (originalShowInfoMessage) {
          Object.defineProperty(
            vscode.window,
            "showInformationMessage",
            originalShowInfoMessage
          );
        }
        (coverageManager as any)["updateCoverage"] = originalUpdateCoverage;
      }
    });
  });

  suite("getGenerateReportCommand", () => {
    test("should generate correct command for AFL fuzzer", async () => {
      const originalUtils = {
        getWorkspaceRoot: require("../utils").getWorkspaceRoot,
        getTargetDirPath: require("../utils").getTargetDirPath,
        getFuzzerConstants: require("../utils").getFuzzerConstants,
      };

      require("../utils").getWorkspaceRoot = () => "/test/workspace";
      require("../utils").getTargetDirPath = async () => "/test/target/dir";
      require("../utils").getFuzzerConstants = (fuzzerType: FuzzerType) => ({
        PROFRAW_FILE: "test.profraw",
        LIVE_REPORT_FILE: "coverage.json",
      });

      try {
        coverageManager.setFuzzerType(FuzzerType.Afl);
        const command = await coverageManager.testGetGenerateReportCommand();

        assert.ok(
          command.includes("cargo llvm-cov"),
          "Should include base command"
        );
        assert.ok(
          command.includes("--json"),
          "Should include JSON output flag"
        );
        assert.ok(
          command.includes(TridentConstants.IGNORE_FILE_NAME_REGEX),
          "Should include ignore regex"
        );
      } finally {
        Object.assign(require("../utils"), originalUtils);
      }
    });

    test("should generate correct command for Honggfuzz fuzzer", async () => {
      const originalUtils = {
        getWorkspaceRoot: require("../utils").getWorkspaceRoot,
        getTargetDirPath: require("../utils").getTargetDirPath,
        getFuzzerConstants: require("../utils").getFuzzerConstants,
      };

      require("../utils").getWorkspaceRoot = () => "/test/workspace";
      require("../utils").getTargetDirPath = async () => "/test/target/dir";
      require("../utils").getFuzzerConstants = (fuzzerType: FuzzerType) => ({
        PROFRAW_FILE: "test.profraw",
        LIVE_REPORT_FILE: "coverage.json",
      });

      try {
        coverageManager.setFuzzerType(FuzzerType.Honggfuzz);
        const command = await coverageManager.testGetGenerateReportCommand();

        assert.ok(
          command.includes("--release"),
          "Should include release flag for Honggfuzz"
        );
      } finally {
        Object.assign(require("../utils"), originalUtils);
      }
    });

    test("should throw error when fuzzer type not set", async () => {
      const originalUtils = {
        getFuzzerConstants: require("../utils").getFuzzerConstants,
      };

      require("../utils").getFuzzerConstants = (
        fuzzerType: FuzzerType | undefined
      ) => {
        if (!fuzzerType) {
          throw new Error("Fuzzer type not set");
        }
        return {
          PROFRAW_FILE: "test.profraw",
          LIVE_REPORT_FILE: "coverage.json",
        };
      };

      try {
        await assert.rejects(
          () => coverageManager.testGetGenerateReportCommand(),
          Error,
          "Should throw error when fuzzer type not set"
        );
      } finally {
        Object.assign(require("../utils"), originalUtils);
      }
    });
  });

  suite("handleProfdata", () => {
    test("should handle profdata file operations", async () => {
      const originalUtils = {
        getWorkspaceRoot: require("../utils").getWorkspaceRoot,
        getTargetDirPath: require("../utils").getTargetDirPath,
        executeCommand: require("../utils").executeCommand,
      };

      require("../utils").getWorkspaceRoot = () => "/test/workspace";
      require("../utils").getTargetDirPath = async () => "/test/target/dir";
      require("../utils").executeCommand = async (command: string) => {
        assert.ok(
          command.includes("mv") &&
            command.includes("/test/target/dir/workspace.profdata") &&
            command.includes("/test/target/dir/workspace-old.profraw"),
          "Should execute correct mv command"
        );
      };

      try {
        coverageManager.setFuzzerType(FuzzerType.Afl);
        await coverageManager.testHandleProfdata();
      } finally {
        Object.assign(require("../utils"), originalUtils);
      }
    });

    test("should handle case when no profdata file exists", async () => {
      const originalUtils = {
        getWorkspaceRoot: require("../utils").getWorkspaceRoot,
        getTargetDirPath: require("../utils").getTargetDirPath,
        executeCommand: require("../utils").executeCommand,
      };

      let consoleLogCalled = false;
      const originalConsoleLog = Object.getOwnPropertyDescriptor(
        console,
        "log"
      );

      require("../utils").getWorkspaceRoot = () => "/test/workspace";
      require("../utils").getTargetDirPath = async () => "/test/target/dir";
      require("../utils").executeCommand = async () => {
        throw new Error("File not found");
      };

      Object.defineProperty(console, "log", {
        value: (message: string) => {
          consoleLogCalled = true;
          assert.strictEqual(
            message,
            "No existing profdata file to convert.",
            "Should log correct message"
          );
        },
        configurable: true,
      });

      try {
        coverageManager.setFuzzerType(FuzzerType.Afl);
        await coverageManager.testHandleProfdata();
        assert.ok(
          consoleLogCalled,
          "Should log message when file doesn't exist"
        );
      } finally {
        Object.assign(require("../utils"), originalUtils);
        if (originalConsoleLog) {
          Object.defineProperty(console, "log", originalConsoleLog);
        }
      }
    });

    test("should throw error when fuzzer type not set", async () => {
      const originalUtils = {
        getTargetDirPath: require("../utils").getTargetDirPath,
      };

      require("../utils").getTargetDirPath = async (
        fuzzerType: FuzzerType | undefined
      ) => {
        if (!fuzzerType) {
          throw new Error("Fuzzer type not set");
        }
        return "/test/target/dir";
      };

      try {
        await assert.rejects(
          () => coverageManager.testHandleProfdata(),
          Error,
          "Should throw error when fuzzer type not set"
        );
      } finally {
        Object.assign(require("../utils"), originalUtils);
      }
    });
  });
});
