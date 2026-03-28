#!/usr/bin/env bash
set -euo pipefail

echo "=== HomeRun Development Setup ==="
echo ""

# Check Rust
if command -v rustc &>/dev/null; then
  RUST_VERSION=$(rustc --version | awk '{print $2}')
  echo "Rust: $RUST_VERSION"
else
  echo "Rust not found. Install it with:"
  echo "  curl https://sh.rustup.rs -sSf | sh"
  exit 1
fi

# Check Node
if command -v node &>/dev/null; then
  NODE_VERSION=$(node --version)
  echo "Node: $NODE_VERSION"
else
  echo "Node.js not found. Install it from https://nodejs.org or:"
  echo "  brew install node"
  exit 1
fi

# Check Xcode CLI tools
if xcode-select -p &>/dev/null; then
  echo "Xcode CLI: installed"
else
  echo "Xcode Command Line Tools not found. Install with:"
  echo "  xcode-select --install"
  exit 1
fi

echo ""

# Build
echo "=== Building daemon + TUI ==="
cargo build --release -p homerund -p homerun

# Install frontend deps
echo ""
echo "=== Installing frontend dependencies ==="
(cd apps/desktop && npm install)

# Default config
CONFIG_DIR="$HOME/.homerun"
CONFIG_FILE="$CONFIG_DIR/config.toml"
if [ ! -f "$CONFIG_FILE" ]; then
  echo ""
  echo "=== Creating default config ==="
  mkdir -p "$CONFIG_DIR"
  cat > "$CONFIG_FILE" << 'TOML'
# HomeRun configuration
# See https://github.com/aGallea/homerun for documentation
TOML
  echo "Created $CONFIG_FILE"
fi

echo ""
echo "=== Setup complete! ==="
echo ""
echo "Next steps:"
echo "  1. Start the daemon:    ./target/release/homerund"
echo "  2. Launch the TUI:      ./target/release/homerun"
echo "  3. Or use make:         make dev    (daemon)"
echo "                          make tui    (TUI)"
echo "                          make desktop (desktop app)"
echo ""
echo "Run 'make help' to see all available commands."
