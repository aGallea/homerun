# HomeRun .dmg Packaging Design

**Issue:** #18 — Package HomeRun as .dmg for macOS distribution
**Date:** 2026-03-24
**Status:** Approved

## Overview

Package the Tauri desktop app as a `.dmg` installer with the `homerund` daemon bundled as a sidecar binary. Automate releases via release-please (versioning) and a GitHub Actions workflow (build + publish).

## Scope

**In scope:**

- Tauri sidecar configuration for `homerund`
- DMG build configuration
- GitHub Actions release workflow (arm64 + x86_64)
- release-please for automated versioning and changelog
- Code-signing infrastructure (secrets wired, unsigned until certificate obtained — see #49)

**Out of scope:**

- Homebrew formula (separate issue)
- CLI install prompt on first launch
- Apple code-signing certificate (separate issue #49)

## 1. Tauri Sidecar Configuration

### externalBin

Add `homerund` to `tauri.conf.json` under `bundle.externalBin`:

```json
{
  "bundle": {
    "externalBin": ["binaries/homerund"]
  }
}
```

Tauri expects platform-specific binaries at:

- `src-tauri/binaries/homerund-aarch64-apple-darwin`
- `src-tauri/binaries/homerund-x86_64-apple-darwin`

These are placed by the build pipeline before `tauri build` runs. The `binaries/` directory is gitignored.

### Build pipeline (local + CI)

1. `cargo build --release -p homerund --target <triple>`
2. Copy `target/<triple>/release/homerund` → `apps/desktop/src-tauri/binaries/homerund-<triple>`
3. `npm run tauri build -- --target <triple>` (from `apps/desktop/`)

Tauri bundles the sidecar into `.app/Contents/MacOS/homerund`.

### Launchd integration update

The existing `install_service` command in `commands.rs` uses `std::env::current_exe()` to find the daemon binary path for the launchd plist. This must change to resolve the sidecar binary path.

**New logic:**

- Compute sidecar path relative to the app bundle: the Tauri app executable lives at `.app/Contents/MacOS/HomeRun`, and the sidecar is at `.app/Contents/MacOS/homerund`
- Use `std::env::current_exe()` to get the app binary path, then resolve `../MacOS/homerund` relative to it
- The launchd plist `ProgramArguments` points to this resolved path

**Edge case — app moved after launchd registration:**

- The plist path becomes stale
- The daemon won't start on next login
- User can re-register via Settings > Startup > "Launch at login" (existing UI)
- No special handling needed — this is self-healing via the existing flow

## 2. DMG Configuration

### tauri.conf.json additions

```json
{
  "bundle": {
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
}
```

- Standard drag-to-Applications layout
- No custom background image (Tauri default)
- Unsigned for now (`signingIdentity: null`)
- macOS Ventura minimum (13.0)

## 3. GitHub Actions Release Workflow

### release-build.yml

**Trigger:** Tag push matching `v*`

**Matrix strategy — two parallel jobs:**

| Variant | Runner         | Target triple          |
| ------- | -------------- | ---------------------- |
| arm64   | `macos-latest` | `aarch64-apple-darwin` |
| x86_64  | `macos-13`     | `x86_64-apple-darwin`  |

**Steps per matrix job:**

1. Checkout code
2. Install Rust toolchain + add target triple
3. Setup Node.js, run `npm ci` in `apps/desktop/`
4. Build homerund: `cargo build --release -p homerund --target <triple>`
5. Copy binary to `apps/desktop/src-tauri/binaries/homerund-<triple>`
6. Run `npm run tauri build -- --target <triple>` from `apps/desktop/`
7. Upload `.dmg` as workflow artifact

**Code-signing environment variables (optional, for future use):**

- `APPLE_CERTIFICATE` — base64-encoded .p12
- `APPLE_CERTIFICATE_PASSWORD`
- `APPLE_SIGNING_IDENTITY`
- `APPLE_ID`
- `APPLE_TEAM_ID`
- `APPLE_PASSWORD`

Tauri automatically picks these up when present. No workflow changes needed to enable signing.

**Final job — `create-release` (needs both matrix jobs):**

1. Download both `.dmg` artifacts
2. Create GitHub Release from the tag (using `softprops/action-gh-release` or `gh release create`)
3. Attach files: `HomeRun_<version>_aarch64.dmg`, `HomeRun_<version>_x86_64.dmg`
4. Use release-please's generated release notes

### Runner flexibility

Default: GitHub-hosted (`macos-latest`, `macos-13`). Can switch to self-hosted by changing `runs-on` values.

## 4. Release Versioning with release-please

### Workflow — release-please.yml

**Trigger:** Push to `master`

**Action:** `googleapis/release-please-action@v4`

**Flow:**

```
push to master
  → release-please analyzes conventional commits
  → creates/updates Release PR (version bumps + CHANGELOG.md)
  → merge Release PR
  → release-please creates v* tag
  → triggers release-build.yml
```

### Configuration files

**`.release-please-config.json`:**

```json
{
  "$schema": "https://raw.githubusercontent.com/googleapis/release-please/main/schemas/config.json",
  "packages": {
    ".": {
      "release-type": "rust",
      "component": "homerun",
      "changelog-path": "CHANGELOG.md",
      "bump-minor-pre-major": true,
      "extra-files": [
        "apps/desktop/package.json",
        "apps/desktop/src-tauri/tauri.conf.json",
        "apps/desktop/src-tauri/Cargo.toml"
      ]
    }
  }
}
```

**`.release-please-manifest.json`:**

```json
{
  ".": "0.1.0"
}
```

### Commit type mapping (conventional commits)

- `feat` → minor bump
- `fix` → patch bump
- `feat!` / `BREAKING CHANGE` → major bump
- `docs`, `style`, `refactor`, `test`, `chore`, `ci` → no bump (included in next release's changelog)

## 5. Files Changed / Created

| File                                         | Action   | Purpose                                      |
| -------------------------------------------- | -------- | -------------------------------------------- |
| `apps/desktop/src-tauri/tauri.conf.json`     | Modified | Add externalBin, DMG config, macOS settings  |
| `apps/desktop/src-tauri/binaries/.gitignore` | Created  | Ignore sidecar binaries                      |
| `apps/desktop/src-tauri/src/commands.rs`     | Modified | Update launchd plist path to resolve sidecar |
| `.github/workflows/release-build.yml`        | Created  | Tag-triggered DMG build + GitHub Release     |
| `.github/workflows/release-please.yml`       | Created  | Automated versioning + Release PRs           |
| `.release-please-config.json`                | Created  | release-please monorepo config               |
| `.release-please-manifest.json`              | Created  | Version tracking                             |

## 6. Testing Strategy

- **Local build:** Run the build pipeline manually (`cargo build -p homerund`, copy, `tauri build`) and verify the `.dmg` mounts, app launches, and daemon sidecar is present in `.app/Contents/MacOS/`
- **Launchd:** Verify "Launch at login" in Settings creates a plist pointing to the sidecar path, and that `launchctl load` works
- **CI dry-run:** Push a test tag to verify the workflow builds both architectures and creates a draft release
- **Unsigned install:** Verify the app can be opened via right-click → Open on a fresh macOS install
