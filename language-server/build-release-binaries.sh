#!/bin/bash

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
    "win32" | "msys" | "cygwin" | "windows_nt")
        platform="win32"
        if [ "$arch" = "arm64" ]; then
            rust_target="aarch64-pc-windows-msvc"
        else
            rust_target="x86_64-pc-windows-msvc"
        fi
        ;;
    *)
        echo "Unsupported platform: $platform"
        exit 1
        ;;
esac

echo "Detected platform: $platform"
echo "Detected architecture: $arch"
echo "Using Rust target: $rust_target"

# Add the target if not present
rustup target add "$rust_target"

# Build for the current platform
cd language-server
echo "Building for $platform-$arch..."
cargo build --target "$rust_target" --release

# Clean and create bin directory
rm -rf ../extension/bin/*
mkdir -p ../extension/bin

# Copy the binary
if [ "$platform" = "win32" ]; then
    cp "target/$rust_target/release/language-server.exe" ../extension/bin/
else
    cp "target/$rust_target/release/language-server" ../extension/bin/
fi

echo "Build complete! Binary is available in extension/bin/"
