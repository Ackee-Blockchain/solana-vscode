#!/bin/bash

# Function to print usage
print_usage() {
    echo "Usage: $0 [--ci PLATFORM ARCH] [--debug|--release]"
    echo ""
    echo "Options:"
    echo "  --ci PLATFORM ARCH    Run in CI mode with specified platform and architecture"
    echo "                        Platforms: darwin (macOS), linux, alpine, win32"
    echo "                        Architectures: x64, arm64, armhf"
    echo "  --debug              Build in debug mode (default is release)"
    echo "  --release            Build in release mode (default)"
    echo ""
    echo "Examples:"
    echo "  $0                             # Local development build (auto-detect)"
    echo "  $0 --ci darwin arm64           # CI build for macOS ARM64"
    echo "  $0 --ci linux x64 --debug      # CI build for Linux x64 in debug mode"
}

# Parse arguments
CI_MODE=false
BUILD_TYPE="release"

while [[ $# -gt 0 ]]; do
    case $1 in
        --ci)
            CI_MODE=true
            platform="$2"
            arch="$3"
            shift 3
            ;;
        --debug)
            BUILD_TYPE="debug"
            shift
            ;;
        --release)
            BUILD_TYPE="release"
            shift
            ;;
        -h|--help)
            print_usage
            exit 0
            ;;
        *)
            echo "Unknown argument: $1"
            print_usage
            exit 1
            ;;
    esac
done

if [ "$CI_MODE" = false ]; then
    echo "Building language server for local development..."
    # Detect current platform and architecture
    platform=$(uname -s | tr '[:upper:]' '[:lower:]')
    arch=$(uname -m)

    # Convert architecture names to our standard
    case "$arch" in
        "x86_64")
            arch="x64"
            ;;
        "aarch64" | "arm64")
            arch="arm64"
            ;;
        "armv7l")
            arch="armhf"
            ;;
    esac

    # Determine if we're on Alpine Linux
    if [ "$platform" = "linux" ] && [ -f "/etc/alpine-release" ]; then
        platform="alpine"
    fi
else
    echo "Building language server in CI mode..."
    echo "Platform: $platform"
    echo "Architecture: $arch"
fi

# Validate platform and architecture
case "$platform" in
    "darwin"|"linux"|"alpine"|"win32") ;;
    *)
        echo "Unsupported platform: $platform"
        exit 1
        ;;
esac

case "$arch" in
    "x64"|"arm64"|"armhf") ;;
    *)
        echo "Unsupported architecture: $arch"
        exit 1
        ;;
esac

# Set Rust target based on platform and architecture
case "$platform" in
    "darwin")
        if [ "$arch" = "arm64" ]; then
            rust_target="aarch64-apple-darwin"
        else
            rust_target="x86_64-apple-darwin"
        fi
        ;;
    "linux" | "alpine")
        if [ "$arch" = "arm64" ]; then
            rust_target="aarch64-unknown-linux-gnu"
        elif [ "$arch" = "armhf" ]; then
            rust_target="armv7-unknown-linux-gnueabihf"
        else
            rust_target="x86_64-unknown-linux-gnu"
        fi
        if [ "$platform" = "alpine" ]; then
            rust_target="${rust_target/gnu/musl}"
        fi
        ;;
    "win32")
        if [ "$arch" = "arm64" ]; then
            rust_target="aarch64-pc-windows-msvc"
        else
            rust_target="x86_64-pc-windows-msvc"
        fi
        ;;
esac

echo "Using Rust target: $rust_target"
echo "Build type: $BUILD_TYPE"

# Add the target if not present
rustup target add "$rust_target"

# Build for the target platform
echo "Building for $platform-$arch..."
if [ "$BUILD_TYPE" = "debug" ]; then
    cargo build --target "$rust_target"
else
    cargo build --target "$rust_target" --release
fi

# Clean and create bin directory
rm -rf ../extension/bin/*
mkdir -p ../extension/bin

# Copy the binary
if [ "$platform" = "win32" ]; then
    cp "target/$rust_target/$BUILD_TYPE/language-server.exe" ../extension/bin/
else
    cp "target/$rust_target/$BUILD_TYPE/language-server" ../extension/bin/
fi

echo "Build complete! Binary is available in extension/bin/"
