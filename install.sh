#!/bin/sh
set -e

# Detect OS and Architecture
OS="$(uname -s)"
ARCH="$(uname -m)"

echo "Installing Nyx for $OS ($ARCH)..."

URL=""

case "$OS" in
    Linux)
        if [ "$ARCH" = "x86_64" ]; then
            URL="https://github.com/nyx-lang/nyx/releases/latest/download/nyx-linux-x64.tar.gz"
        elif [ "$ARCH" = "aarch64" ] || [ "$ARCH" = "arm64" ]; then
            URL="https://github.com/nyx-lang/nyx/releases/latest/download/nyx-linux-arm64.tar.gz"
        else
            echo "Unsupported architecture: $ARCH"
            exit 1
        fi
        ;;
    Darwin)
        if [ "$ARCH" = "x86_64" ]; then
            URL="https://github.com/nyx-lang/nyx/releases/latest/download/nyx-macos-x64.tar.gz"
        elif [ "$ARCH" = "arm64" ]; then
            URL="https://github.com/nyx-lang/nyx/releases/latest/download/nyx-macos-arm64.tar.gz"
        else
            echo "Unsupported architecture: $ARCH"
            exit 1
        fi
        ;;
    *)
        echo "Unsupported OS: $OS"
        exit 1
        ;;
esac

# Download and extract
echo "Downloading from $URL..."
TMP_DIR=$(mktemp -d)
curl -sL "$URL" -o "$TMP_DIR/nyx.tar.gz"
tar -xzf "$TMP_DIR/nyx.tar.gz" -C "$TMP_DIR"

# Install to standard location
INSTALL_DIR="/usr/local/bin"
if [ ! -w "$INSTALL_DIR" ]; then
    INSTALL_DIR="$HOME/.local/bin"
    mkdir -p "$INSTALL_DIR"
fi

# Find the extracted binary (it could be named nyx-linux-x64, etc.)
BIN_NAME=$(basename -s .tar.gz "$URL")
mv "$TMP_DIR/$BIN_NAME" "$INSTALL_DIR/nyx"
chmod +x "$INSTALL_DIR/nyx"
rm -rf "$TMP_DIR"

echo "Nyx was successfully installed to $INSTALL_DIR/nyx."
if ! command -v nyx >/dev/null 2>&1; then
    echo "Please ensure $INSTALL_DIR is in your PATH."
fi
echo "\nCheck installation with: nyx --version"
