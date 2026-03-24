# DMG Packaging Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Package the HomeRun Tauri app as a `.dmg` with the `homerund` daemon bundled as a sidecar, and automate releases via release-please + GitHub Actions.

**Architecture:** Tauri's `externalBin` bundles `homerund` inside the `.app`. release-please creates versioned tags from conventional commits. A tag-triggered GitHub Actions workflow builds `.dmg` files for both arm64 and x86_64 and publishes them as GitHub Releases.

**Tech Stack:** Tauri 2.x, GitHub Actions, release-please v4, Rust cross-compilation

**Spec:** `docs/superpowers/specs/2026-03-24-dmg-packaging-design.md`

---

## File Structure

| File                                         | Action | Responsibility                                     |
| -------------------------------------------- | ------ | -------------------------------------------------- |
| `apps/desktop/src-tauri/tauri.conf.json`     | Modify | Add externalBin, DMG layout, macOS bundle settings |
| `apps/desktop/src-tauri/binaries/.gitignore` | Create | Ignore platform-specific sidecar binaries          |
| `.github/workflows/release-build.yml`        | Create | Tag-triggered matrix build + GitHub Release        |
| `.github/workflows/release-please.yml`       | Create | Automated versioning + Release PRs                 |
| `.release-please-config.json`                | Create | release-please package config                      |
| `.release-please-manifest.json`              | Create | Version tracking manifest                          |

---

## Task 1: Configure Tauri sidecar and DMG settings

**Files:**

- Modify: `apps/desktop/src-tauri/tauri.conf.json`
- Create: `apps/desktop/src-tauri/binaries/.gitignore`

- [ ] **Step 1: Create the binaries directory and .gitignore**

Create `apps/desktop/src-tauri/binaries/.gitignore` to ignore sidecar binaries (they're placed here by the build pipeline, not checked in):

```
*
!.gitignore
```

- [ ] **Step 2: Update tauri.conf.json — add externalBin and DMG config**

Edit `apps/desktop/src-tauri/tauri.conf.json`. The `bundle` section currently looks like:

```json
"bundle": {
  "active": true,
  "targets": "all",
  "icon": [
    "icons/icon.icns",
    "icons/32x32.png",
    "icons/128x128.png",
    "icons/128x128@2x.png",
    "icons/icon.png"
  ]
}
```

Replace the entire `bundle` section with:

```json
"bundle": {
  "active": true,
  "targets": "all",
  "icon": [
    "icons/icon.icns",
    "icons/32x32.png",
    "icons/128x128.png",
    "icons/128x128@2x.png",
    "icons/icon.png"
  ],
  "category": "DeveloperTool",
  "copyright": "© 2026 HomeRun contributors",
  "externalBin": ["binaries/homerund"],
  "macOS": {
    "dmg": {
      "appPosition": { "x": 180, "y": 170 },
      "applicationFolderPosition": { "x": 480, "y": 170 },
      "windowSize": { "width": 660, "height": 400 }
    },
    "minimumSystemVersion": "13.0",
    "signingIdentity": null,
    "entitlements": null
  }
}
```

- [ ] **Step 3: Verify the config is valid JSON**

```bash
python3 -c "import json; json.load(open('apps/desktop/src-tauri/tauri.conf.json')); print('tauri.conf.json OK')"
```

Expected: `tauri.conf.json OK`

- [ ] **Step 4: Test local sidecar build pipeline**

Run from repo root (replace `<triple>` with your host triple, e.g., `aarch64-apple-darwin`):

```bash
# Build the daemon
cargo build --release -p homerund --target aarch64-apple-darwin

# Copy to sidecar location
mkdir -p apps/desktop/src-tauri/binaries
cp target/aarch64-apple-darwin/release/homerund apps/desktop/src-tauri/binaries/homerund-aarch64-apple-darwin

# Build the Tauri app (this produces the .dmg)
cd apps/desktop && npx tauri build --target aarch64-apple-darwin
```

Expected: Build succeeds and produces a `.dmg` in `apps/desktop/src-tauri/target/aarch64-apple-darwin/release/bundle/dmg/`.

- [ ] **Step 5: Verify the sidecar is bundled**

Mount the `.dmg` and inspect the `.app` bundle:

```bash
ls -la /Volumes/HomeRun/HomeRun.app/Contents/MacOS/
```

Expected: Both `HomeRun` (the app binary) and `homerund` (the sidecar) are present.

- [ ] **Step 6: Commit**

```bash
git add apps/desktop/src-tauri/tauri.conf.json apps/desktop/src-tauri/binaries/.gitignore
git commit -m "feat: configure Tauri sidecar and DMG settings for homerund bundling"
```

---

## Task 2: Create release-please configuration

**Files:**

- Create: `.release-please-config.json`
- Create: `.release-please-manifest.json`

- [ ] **Step 1: Create `.release-please-config.json`**

```json
{
  "$schema": "https://raw.githubusercontent.com/googleapis/release-please/main/schemas/config.json",
  "packages": {
    ".": {
      "release-type": "simple",
      "component": "homerun",
      "changelog-path": "CHANGELOG.md",
      "bump-minor-pre-major": true,
      "extra-files": [
        {
          "type": "toml",
          "path": "Cargo.toml",
          "jsonpath": "$.workspace.package.version"
        },
        {
          "type": "json",
          "path": "apps/desktop/package.json",
          "jsonpath": "$.version"
        },
        {
          "type": "json",
          "path": "apps/desktop/src-tauri/tauri.conf.json",
          "jsonpath": "$.version"
        },
        {
          "type": "toml",
          "path": "apps/desktop/src-tauri/Cargo.toml",
          "jsonpath": "$.package.version"
        }
      ]
    }
  }
}
```

- [ ] **Step 2: Create `.release-please-manifest.json`**

```json
{
  ".": "0.1.0"
}
```

- [ ] **Step 3: Validate JSON files**

```bash
python3 -c "import json; json.load(open('.release-please-config.json')); print('config OK')"
python3 -c "import json; json.load(open('.release-please-manifest.json')); print('manifest OK')"
```

Expected: Both print OK.

- [ ] **Step 4: Commit**

```bash
git add .release-please-config.json .release-please-manifest.json
git commit -m "chore: add release-please configuration for automated versioning"
```

---

## Task 3: Create release-please GitHub Actions workflow

**Files:**

- Create: `.github/workflows/release-please.yml`

- [ ] **Step 1: Create `.github/workflows/release-please.yml`**

```yaml
name: Release Please

on:
  push:
    branches: [master]

permissions:
  contents: write
  pull-requests: write

jobs:
  release-please:
    runs-on: ubuntu-latest
    steps:
      - uses: googleapis/release-please-action@v4
        with:
          token: ${{ secrets.GITHUB_TOKEN }}
```

The action reads `.release-please-config.json` and `.release-please-manifest.json` automatically from the repo root.

- [ ] **Step 2: Validate YAML syntax**

```bash
python3 -c "import yaml; yaml.safe_load(open('.github/workflows/release-please.yml')); print('YAML OK')"
```

Expected: `YAML OK`

- [ ] **Step 3: Commit**

```bash
git add .github/workflows/release-please.yml
git commit -m "ci: add release-please workflow for automated version bumps"
```

---

## Task 4: Create release build GitHub Actions workflow

**Files:**

- Create: `.github/workflows/release-build.yml`

- [ ] **Step 1: Create `.github/workflows/release-build.yml`**

```yaml
name: Release Build

on:
  push:
    tags: ["v*"]

permissions:
  contents: write

concurrency:
  group: release-${{ github.ref }}
  cancel-in-progress: true

env:
  CARGO_TERM_COLOR: always

jobs:
  build:
    name: Build ${{ matrix.target }}
    runs-on: macos-latest
    strategy:
      fail-fast: false
      matrix:
        include:
          - target: aarch64-apple-darwin
            arch: aarch64
          - target: x86_64-apple-darwin
            arch: x86_64

    steps:
      - uses: actions/checkout@v6

      - name: Install Rust toolchain
        uses: dtolnay/rust-toolchain@stable
        with:
          targets: ${{ matrix.target }}

      - name: Rust cache
        uses: swatinem/rust-cache@v2
        with:
          workspaces: |
            . -> target
            apps/desktop/src-tauri -> target

      - name: Setup Node.js
        uses: actions/setup-node@v4
        with:
          node-version: 22
          cache: npm
          cache-dependency-path: apps/desktop/package-lock.json

      - name: Install frontend dependencies
        working-directory: apps/desktop
        run: npm ci

      - name: Build homerund sidecar
        run: |
          cargo build --release -p homerund --target ${{ matrix.target }}
          mkdir -p apps/desktop/src-tauri/binaries
          cp target/${{ matrix.target }}/release/homerund \
            apps/desktop/src-tauri/binaries/homerund-${{ matrix.target }}

      - name: Build Tauri app
        working-directory: apps/desktop
        env:
          # Code-signing (optional — only active when secrets are set)
          APPLE_CERTIFICATE: ${{ secrets.APPLE_CERTIFICATE }}
          APPLE_CERTIFICATE_PASSWORD: ${{ secrets.APPLE_CERTIFICATE_PASSWORD }}
          APPLE_SIGNING_IDENTITY: ${{ secrets.APPLE_SIGNING_IDENTITY }}
          APPLE_ID: ${{ secrets.APPLE_ID }}
          APPLE_TEAM_ID: ${{ secrets.APPLE_TEAM_ID }}
          APPLE_PASSWORD: ${{ secrets.APPLE_PASSWORD }}
          KEYCHAIN_PASSWORD: ${{ secrets.KEYCHAIN_PASSWORD }}
        run: npx tauri build --target ${{ matrix.target }}

      - name: Upload DMG artifact
        uses: actions/upload-artifact@v4
        with:
          name: HomeRun-${{ matrix.arch }}
          path: apps/desktop/src-tauri/target/${{ matrix.target }}/release/bundle/dmg/*.dmg
          if-no-files-found: error

  release:
    name: Create GitHub Release
    needs: build
    runs-on: ubuntu-latest
    steps:
      - uses: actions/checkout@v6

      - name: Download all artifacts
        uses: actions/download-artifact@v4
        with:
          path: artifacts
          merge-multiple: true

      - name: List artifacts
        run: ls -la artifacts/

      - name: Create Release
        uses: softprops/action-gh-release@v2
        with:
          files: artifacts/*.dmg
          draft: false
```

- [ ] **Step 2: Validate YAML syntax**

```bash
python3 -c "import yaml; yaml.safe_load(open('.github/workflows/release-build.yml')); print('YAML OK')"
```

Expected: `YAML OK`

- [ ] **Step 3: Commit**

```bash
git add .github/workflows/release-build.yml
git commit -m "ci: add release build workflow for .dmg packaging (#18)"
```

---

## Task 5: End-to-end verification

- [ ] **Step 1: Verify all files are in place**

```bash
# Check all new/modified files
ls -la .release-please-config.json .release-please-manifest.json
ls -la .github/workflows/release-please.yml .github/workflows/release-build.yml
ls -la apps/desktop/src-tauri/binaries/.gitignore
cat apps/desktop/src-tauri/tauri.conf.json | python3 -m json.tool > /dev/null && echo "tauri.conf.json valid"
```

Expected: All files exist, JSON is valid.

- [ ] **Step 2: Run existing CI checks to make sure nothing is broken**

```bash
# Rust checks
cargo fmt --check
cargo clippy --all-targets --all-features -- -D warnings
cargo test

# TypeScript checks
cd apps/desktop && npx tsc --noEmit && npm run build
```

Expected: All pass.

- [ ] **Step 3: Local DMG build test (arm64)**

If not already done in Task 1 Step 4:

```bash
cargo build --release -p homerund --target aarch64-apple-darwin
mkdir -p apps/desktop/src-tauri/binaries
cp target/aarch64-apple-darwin/release/homerund apps/desktop/src-tauri/binaries/homerund-aarch64-apple-darwin
cd apps/desktop && npx tauri build --target aarch64-apple-darwin
```

Expected: `.dmg` produced, mountable, app launches, `homerund` sidecar present in `.app/Contents/MacOS/`.

- [ ] **Step 4: Final commit message for PR**

No additional commit needed — all changes are committed in Tasks 1-4.
