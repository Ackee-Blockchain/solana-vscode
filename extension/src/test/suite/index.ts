import * as path from 'path';
import Mocha from 'mocha';
import { glob } from 'glob';

export function run(): Promise<void> {
  // Create the mocha test
  const mocha = new Mocha({
    ui: 'tdd',
    color: true,
    timeout: 10000 // Increase timeout to 10 seconds
  });

  const testsRoot = path.resolve(__dirname, '../..');

  return new Promise<void>((resolve, reject) => {
    // Find all test files in any tests directory
    glob('**/tests/**/*.test.js', { cwd: testsRoot }).then((files) => {
      // Add files to the test suite
      files.forEach((f: string) => mocha.addFile(path.resolve(testsRoot, f)));

      try {
        // Run the mocha test
        mocha.run((failures: number) => {
          if (failures > 0) {
            reject(new Error(`${failures} tests failed.`));
          } else {
            resolve();
          }
        });
      } catch (err) {
        console.error(err);
        reject(err);
      }
    }).catch(reject);
  });
}
