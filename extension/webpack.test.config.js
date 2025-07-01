const path = require('path');

/** @type {import('webpack').Configuration} */
module.exports = {
  target: 'node',
  mode: 'development', // Using development mode for better debugging
  entry: {
    runTest: './src/test/runTest.ts',
    'suite/index': './src/test/suite/index.ts'
  },
  output: {
    path: path.resolve(__dirname, 'out', 'test'),
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
