#!/bin/zsh

echo "Building language server for all platforms..."

# Add targets if not present
rustup target add x86_64-unknown-linux-gnu
rustup target add x86_64-apple-darwin
rustup target add aarch64-apple-darwin  # ARM Mac
rustup target add x86_64-pc-windows-msvc

# Build for all platforms
cd language-server

echo "Building for Linux x64..."
cargo build --target x86_64-unknown-linux-gnu --release

echo "Building for Intel Mac..."
cargo build --target x86_64-apple-darwin --release

echo "Building for ARM Mac..."
cargo build --target aarch64-apple-darwin --release

echo "Building for Windows..."
cargo build --target x86_64-pc-windows-msvc --release

# Organize binaries for extension
mkdir -p ../extension/bin/{linux,darwin-x64,darwin-arm64,win32}

cp target/x86_64-unknown-linux-gnu/release/language-server ../extension/bin/linux/
cp target/x86_64-apple-darwin/release/language-server ../extension/bin/darwin-x64/
cp target/aarch64-apple-darwin/release/language-server ../extension/bin/darwin-arm64/
cp target/x86_64-pc-windows-msvc/release/language-server.exe ../extension/bin/win32/

echo "All builds complete!"
