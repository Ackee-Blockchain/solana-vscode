#!/bin/bash

# Build all Dylint lints and copy them to the extension directory
# This script should be run before packaging the extension

set -e

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
cd "$SCRIPT_DIR"

echo "🔨 Building all Dylint lints..."
echo ""

# Detect platform
OS=$(uname -s)
ARCH=$(uname -m)

if [ "$OS" = "Darwin" ]; then
    if [ "$ARCH" = "arm64" ]; then
        PLATFORM="macos-arm64"
        LIB_EXT="dylib"
    else
        PLATFORM="macos-x64"
        LIB_EXT="dylib"
    fi
elif [ "$OS" = "Linux" ]; then
    if [ "$ARCH" = "x86_64" ]; then
        PLATFORM="linux-x64"
        LIB_EXT="so"
    elif [ "$ARCH" = "aarch64" ]; then
        PLATFORM="linux-arm64"
        LIB_EXT="so"
    else
        echo "❌ Unsupported architecture: $ARCH"
        exit 1
    fi
else
    echo "❌ Unsupported OS: $OS"
    exit 1
fi

echo "📦 Platform: $PLATFORM"
echo ""

# Create output directory
OUTPUT_DIR="$SCRIPT_DIR/lints_compiled/$PLATFORM"
mkdir -p "$OUTPUT_DIR"

# Find all lint directories
LINT_DIRS=$(find "$SCRIPT_DIR/lints" -mindepth 1 -maxdepth 1 -type d)

# Build each lint
for LINT_DIR in $LINT_DIRS; do
    LINT_NAME=$(basename "$LINT_DIR")
    echo "🔧 Building lint: $LINT_NAME"

    cd "$LINT_DIR"

    # Build in debug mode (faster for development)
    # Use --release for production builds
    cargo build

    # Find the built library
    LIB_FILE=$(find target/debug -name "lib${LINT_NAME}@*.$LIB_EXT" -o -name "lib${LINT_NAME}.$LIB_EXT" | head -n 1)

    if [ -z "$LIB_FILE" ]; then
        echo "⚠️  Warning: Could not find library for $LINT_NAME"
        continue
    fi

    # Copy to output directory
    cp "$LIB_FILE" "$OUTPUT_DIR/"
    echo "✅ Copied $(basename "$LIB_FILE") to $OUTPUT_DIR"
    echo ""
done

cd "$SCRIPT_DIR"

echo ""
echo "🎉 All lints built successfully!"
echo ""
echo "📁 Compiled lints are in: $OUTPUT_DIR"
ls -lh "$OUTPUT_DIR"
echo ""
echo "💡 To use these lints, make sure they are included in the extension package."
