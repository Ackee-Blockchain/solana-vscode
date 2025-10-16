#!/usr/bin/env bash
# Build all Solana lints for CURRENT platform only

set -e

echo "🔨 Building all Solana lints for current platform..."
echo ""

# Get the toolchain from rust-toolchain file (should be same for all lints)
# Find the first lint directory that has a rust-toolchain file
TOOLCHAIN=""
for lint_dir in */; do
    if [[ -f "$lint_dir/rust-toolchain" ]]; then
        TOOLCHAIN=$(grep 'channel' "$lint_dir/rust-toolchain" | cut -d'"' -f2)
        break
    fi
done

# Fallback to default if no rust-toolchain found
if [[ -z "$TOOLCHAIN" ]]; then
    echo "⚠️  No rust-toolchain file found in any lint directory, using default nightly-2025-08-07"
    TOOLCHAIN="nightly-2025-08-07"
fi

echo "🦀 Toolchain: $TOOLCHAIN"

# Install the required toolchain if not present
if ! rustup toolchain list | grep -q "$TOOLCHAIN"; then
    echo "📥 Installing toolchain $TOOLCHAIN..."
    rustup toolchain install "$TOOLCHAIN"
fi

# Script directory (use absolute path)
SCRIPT_DIR="$( cd "$( dirname "${BASH_SOURCE[0]}" )" && pwd )"
BASE_OUTPUT_DIR="$SCRIPT_DIR/../lints_compiled"

# Detect current platform
HOST_OS=$(uname -s | tr '[:upper:]' '[:lower:]')
HOST_ARCH=$(uname -m)

# Map to our platform naming
if [[ "$HOST_OS" == "darwin" ]]; then
    if [[ "$HOST_ARCH" == "arm64" ]]; then
        PLATFORM="macos-arm64"
        TARGET="aarch64-apple-darwin"
        LIB_EXT="dylib"
    else
        PLATFORM="macos-x64"
        TARGET="x86_64-apple-darwin"
        LIB_EXT="dylib"
    fi
elif [[ "$HOST_OS" == "linux" ]]; then
    if [[ "$HOST_ARCH" == "aarch64" ]]; then
        PLATFORM="linux-arm64"
        TARGET="aarch64-unknown-linux-gnu"
        LIB_EXT="so"
    else
        PLATFORM="linux-x64"
        TARGET="x86_64-unknown-linux-gnu"
        LIB_EXT="so"
    fi
else
    echo "❌ Unsupported platform: $HOST_OS $HOST_ARCH"
    exit 1
fi

echo "🖥️  Platform: $PLATFORM ($TARGET)"
echo ""

# Add target if not installed
if ! rustup target list --toolchain "$TOOLCHAIN" 2>/dev/null | grep -q "^$TARGET (installed)$"; then
    echo "📥 Installing target $TARGET..."
    rustup target add "$TARGET" --toolchain "$TOOLCHAIN"
    echo ""
fi

# Clean previous builds
echo "🧹 Cleaning previous builds..."
for lint_dir in */; do
    if [[ -f "$lint_dir/Cargo.toml" ]]; then
        lint_name=$(basename "$lint_dir")
        echo "  Cleaning $lint_name..."
        (cd "$lint_dir" && cargo clean 2>/dev/null || true)
    fi
done
echo ""

# Clean output directory for current platform
echo "🧹 Cleaning output directory..."
OUTPUT_DIR="$BASE_OUTPUT_DIR/$PLATFORM"
rm -rf "$OUTPUT_DIR"
mkdir -p "$OUTPUT_DIR"
echo ""

echo "━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━"
echo "🏗️  Building for: $PLATFORM ($TARGET)"
echo "━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━"
echo ""

# Build each lint directory that has a Cargo.toml
for lint_dir in */; do
    if [[ -f "$lint_dir/Cargo.toml" ]]; then
        lint_name=$(basename "$lint_dir")
        
        echo "📦 Building: $lint_name"
        
        cd "$lint_dir"
        
        # Build for the target platform
        if cargo +$TOOLCHAIN build --release --target "$TARGET" 2>&1; then
            echo "✅ Build successful"
            
            # Find and copy the built library WITH toolchain suffix
            TARGET_DIR="target/$TARGET/release"
            
            # Look for the library file with toolchain suffix (@toolchain-target)
            FOUND=false
            for lib_file in "$TARGET_DIR"/lib${lint_name}@*.${LIB_EXT}; do
                if [[ -f "$lib_file" ]]; then
                    lib_name=$(basename "$lib_file")
                    cp "$lib_file" "$OUTPUT_DIR/"
                    echo "📋 Copied: $lib_name"
                    FOUND=true
                    break
                fi
            done
            
            if [[ "$FOUND" == "false" ]]; then
                echo "⚠️  Warning: No library file with toolchain suffix found in $TARGET_DIR"
                echo "    Looking for: lib${lint_name}@*.${LIB_EXT}"
            fi
        else
            echo "❌ Build failed for $lint_name"
            exit 1
        fi
        
        cd ..
        echo ""
    fi
done

echo "━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━"
echo "✅ Build complete!"
echo "━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━"
echo ""
echo "📂 Built libraries for $PLATFORM:"
ls -lh "$OUTPUT_DIR" | tail -n +2 | awk '{print "   " $9 " (" $5 ")"}'
echo ""
echo "💡 To build for other platforms, run this script on those platforms."
echo "   Or use CI/CD with multiple platform runners."
