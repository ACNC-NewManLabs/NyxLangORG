#!/bin/bash
set -e

echo "--- Nyx DB v0.1-beta Installer ---"

# 1. Build the production binary
echo "[Build] Compiling Nyx in release mode..."
cargo build --release

# 2. Identify the install location
INSTALL_DIR="$HOME/.local/bin"
mkdir -p "$INSTALL_DIR"

# 3. Copy the binary
echo "[Install] Copying 'nyx' to $INSTALL_DIR/nyx..."
cp target/release/nyx "$INSTALL_DIR/nyx"

# 4. Copy the shell binary (optional but good practice)
echo "[Install] Copying 'nyx-shell' to $INSTALL_DIR/nyx-shell..."
cp target/release/nyx-shell "$INSTALL_DIR/nyx-shell"

echo "[Success] Nyx v0.1-beta is now updated!"
echo "You can now run:"
echo "  nyx db server --port 9090"
echo "  nyx db shell --port 9090"
echo ""
echo "Note: If you use 'nyx -- db shell', it will also work now!"
