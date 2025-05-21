import { defineConfig } from "@vscode/test-cli";

export default defineConfig({
  files: "out/coverage/tests/*.test.js",
});
