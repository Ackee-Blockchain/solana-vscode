const path = require('path');
const glob = require('glob');

// Find all test files in the project
const testFiles = glob.sync('./src/**/tests/**/*.test.ts');
const testEntries = {
  runTest: './src/test/runTest.ts',
  'suite/index': './src/test/suite/index.ts'
};

// Add each test file to the entries
testFiles.forEach(file => {
  // Convert the file path to an entry name
  // e.g., './src/coverage/tests/utils.test.ts' -> 'coverage/tests/utils.test'
  const entryName = file
    .replace('./src/', '')
    .replace('.ts', '');
  // Keep the ./ prefix for the file path
  testEntries[entryName] = `./${file}`;
});

/** @type {import('webpack').Configuration} */
module.exports = {
  target: 'node',
  mode: 'development', // Using development mode for better debugging
  entry: testEntries,
  output: {
    path: path.resolve(__dirname, 'out'),
    filename: '[name].js',
    libraryTarget: 'commonjs2'
  },
  externals: {
    vscode: 'commonjs vscode',
    mocha: 'commonjs mocha'
  },
  resolve: {
    extensions: ['.ts', '.js']
  },
  module: {
    rules: [
      {
        test: /\.ts$/,
        exclude: /node_modules/,
        use: [
          {
            loader: 'ts-loader'
          }
        ]
      }
    ]
  },
  devtool: 'source-map'
}
