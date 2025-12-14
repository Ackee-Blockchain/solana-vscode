#!/bin/bash

# Build all Dylint lints and copy them to the extension directory
# This script should be run before packaging the extension

set -e

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
cd "$SCRIPT_DIR"

# Required nightly toolchain version
REQUIRED_NIGHTLY="nightly-2025-09-18"

echo "🔨 Building all Dylint lints with $REQUIRED_NIGHTLY..."
echo ""

# Check if required nightly is installed
if ! rustup toolchain list | grep -q "$REQUIRED_NIGHTLY"; then
    echo "⚠️  Required Rust toolchain not found: $REQUIRED_NIGHTLY"
    echo "📦 Installing $REQUIRED_NIGHTLY..."
    rustup toolchain install "$REQUIRED_NIGHTLY"
    echo "✅ Toolchain installed"
    echo ""
fi

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

# Find all detector directories
DETECTOR_DIRS=$(find "$SCRIPT_DIR/extension/detectors" -mindepth 1 -maxdepth 1 -type d 2>/dev/null || true)

if [ -z "$DETECTOR_DIRS" ]; then
    echo "⚠️  No detector directories found in extension/detectors"
    exit 1
fi

# Build each detector
for DETECTOR_DIR in $DETECTOR_DIRS; do
    DETECTOR_NAME=$(basename "$DETECTOR_DIR")
    echo "🔧 Building detector: $DETECTOR_NAME"

    cd "$DETECTOR_DIR"

    # Build in debug mode (faster for development)
    # Use --release for production builds
    # The rust-toolchain file in each detector directory will be used automatically
    cargo build

    # Find the built library
    LIB_FILE=$(find target/debug -name "lib${DETECTOR_NAME}@*.$LIB_EXT" -o -name "lib${DETECTOR_NAME}.$LIB_EXT" | head -n 1)

    if [ -z "$LIB_FILE" ]; then
        echo "⚠️  Warning: Could not find library for $DETECTOR_NAME"
        continue
    fi

    # Copy to output directory
    cp "$LIB_FILE" "$OUTPUT_DIR/"
    echo "✅ Copied $(basename "$LIB_FILE") to $OUTPUT_DIR"
    echo ""
done

cd "$SCRIPT_DIR"

echo ""
echo "🎉 All detectors built successfully!"
echo ""
echo "📁 Compiled detectors are in: $OUTPUT_DIR"
ls -lh "$OUTPUT_DIR"
echo ""
echo "💡 To use these detectors, make sure they are included in the extension package."
