#!/usr/bin/env bash
set -euo pipefail

REPO_ROOT="$(cd "$(dirname "$0")/.." && pwd)"
cd "$REPO_ROOT"

OPEN_HTML=false
for arg in "$@"; do
  case "$arg" in
    --open) OPEN_HTML=true ;;
  esac
done

echo "=== React tests + coverage ==="
(cd apps/desktop && npm run test:coverage)

echo ""
echo "=== Rust daemon tests + coverage ==="
cargo llvm-cov -p homerund --all-features --lcov --output-path daemon-lcov.info

echo ""
echo "=== Rust TUI tests + coverage ==="
cargo llvm-cov -p homerun --all-features --lcov --output-path tui-lcov.info

echo ""
echo "=== Merging coverage reports ==="
sed 's|^SF:src/|SF:apps/desktop/src/|' apps/desktop/coverage/lcov.info > react-lcov-fixed.info
cat daemon-lcov.info tui-lcov.info react-lcov-fixed.info > lcov.info

# Cleanup intermediate files
rm -f daemon-lcov.info tui-lcov.info react-lcov-fixed.info

echo "Merged coverage written to lcov.info"

if command -v genhtml &>/dev/null; then
  genhtml lcov.info --output-directory coverage-html --quiet
  echo "HTML report: coverage-html/index.html"
  if $OPEN_HTML; then
    open coverage-html/index.html
  fi
else
  echo "Install lcov (brew install lcov) to generate an HTML report"
fi
