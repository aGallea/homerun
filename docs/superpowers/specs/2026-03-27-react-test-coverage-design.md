# React/TypeScript Test Coverage with Vitest

**Issue:** aGallea/homerun#15 (adapted to current app state)
**Date:** 2026-03-27

## Overview

Add Vitest test coverage for the React/TypeScript desktop app. Covers high-value components (pure/prop-driven) and medium-value items (hooks requiring api mocks). Integrates coverage into CI and the coverage badge.

## Mock Strategy

**Single mock boundary:** `src/api/commands.ts` — the `api` object that wraps Tauri `invoke()` calls.

- **Pure components** (StatusBadge, RunnerTable, NewRunnerWizard): receive data as props, zero mocking needed.
- **Hooks** (useRunners, useAuth, useMetrics): mock `api` module via `vi.mock("../api/commands")`. Hooks run real code; only the invoke-backed api calls are stubbed.
- **Sidebar**: uses `useAuth` internally. Wrap in `AuthProvider` with mocked `api.getAuthStatus`.

Why this boundary: `invoke()` requires the Tauri runtime which doesn't exist in jsdom. The `api` object is the thinnest possible mock layer. Everything above it (hooks, components, state management) runs as real code.

The project has a shared `crates/test-utils` crate with `MockDaemon` (Axum on Unix socket). This is used for Rust-level integration tests (TUI). It cannot be used from jsdom but test data shapes should stay consistent with `MockDaemon`'s types.

## Setup Changes

### vitest.config.ts

- Add `@vitest/coverage-v8` provider
- Configure lcov + text output
- Add `setupFiles: ["./src/test/setup.ts"]`
- Set coverage include/exclude paths

### package.json

- Add `"test": "vitest run"` script
- Add `"test:watch": "vitest"` script
- Add `"test:coverage": "vitest run --coverage"` script
- Install `@vitest/coverage-v8` as devDependency

### src/test/setup.ts

Global test setup:

- Mock `@tauri-apps/api/core` module (invoke returns undefined by default)
- Import `@testing-library/jest-dom` matchers
- Any global afterEach cleanup (e.g., `vi.restoreAllMocks()`)

### src/test/factories.ts

Shared test data factory functions (aligned with MockDaemon types):

- `makeRunner(overrides)` — creates `RunnerInfo` with sensible defaults
- `makeRepo(overrides)` — creates `RepoInfo`
- `makeMetrics(overrides)` — creates `MetricsResponse`
- `makeAuth(overrides)` — creates `AuthStatus`

The existing `ActiveRunners.test.tsx` has an inline `makeRunner`; refactor to use the shared factory.

## Test Files

### 1. StatusBadge.test.tsx

No mocking. Pure prop-driven component.

Tests:

- Each of the 8 RunnerState values renders the correct label text
- Transient states (creating, registering, stopping, deleting, busy) render an SVG spinner
- Non-transient states (online, offline, error) render a dot element
- `busy` state with `currentJob` renders "Busy: <jobName>"
- `busy` state without `currentJob` renders "Busy"

### 2. RunnerTable.test.tsx

Props only — callbacks are passed as props.

Tests:

- Renders empty state when runners array is empty
- Renders a row for each runner with name and status
- Groups runners sharing the same name-prefix + repo into collapsible groups
- Group row shows runner count
- Expanding a group shows individual runner rows
- Action callbacks (onDelete, onStart, onStop, onRestart) fire with correct runner ID
- `readOnly` mode hides action buttons
- `pendingActions` set applies loading state to matching runners
- Service mode badge displayed for service runners

### 3. NewRunnerWizard.test.tsx

Props only — `onCreate`, `onCreateBatch`, `onClose` are callback props. Needs mock for `api.listRepos` since the wizard fetches repos.

Tests:

- Renders step 1 (repository selection) initially
- Repo search/filter narrows the list
- Selecting a repo enables "Next" and advances to step 2
- Step 2 shows name input, labels, mode toggle
- Single mode: name is required, "Create" calls `onCreate`
- Batch mode: count stepper works (2-10 range), "Create" calls `onCreateBatch`
- Back button returns to previous step
- Close button calls `onClose`
- Error state shown on failed creation

### 4. Sidebar.test.tsx

Requires `AuthProvider` with mocked api for `useAuth`.

Tests:

- Renders 4 navigation links (Runners, Repositories, Daemon, Settings)
- Authenticated: shows avatar and username
- Unauthenticated: shows "Sign in" button
- Collapsed: hides labels, shows only icons
- Collapsed + unauthenticated: sign-in button is icon-only
- Passes runners to ActiveRunners component

### 5. useRunners.test.ts

Mock `api` module.

Tests:

- Returns `loading: true` on initial render, then `false` after first fetch
- `runners` populated from `api.listRunners` response
- Polls `api.listRunners` every 2 seconds (use `vi.useFakeTimers`)
- `createRunner` calls `api.createRunner` and triggers refresh
- `deleteRunner` calls `api.deleteRunner` and triggers refresh
- `startRunner/stopRunner/restartRunner` call correct api methods
- `pendingActions` contains runner ID during in-flight action, removed after completion
- `error` set when `api.listRunners` rejects
- Batch/group operations: `createBatch`, `startGroup`, `stopGroup`, `restartGroup`, `deleteGroup`, `scaleGroup`

### 6. useAuth.test.ts

Mock `api` module.

Tests:

- Returns `loading: true` initially, `false` after first fetch
- `auth` populated from `api.getAuthStatus` response
- `loginWithToken` calls `api.loginWithToken` and updates state
- `logout` calls `api.logout` and clears auth
- Polls `api.getAuthStatus` every 5 seconds
- `handleUnauthorized` triggers a refresh

Note: `useAuth` is exported from `AuthContext.tsx` which provides context. Tests wrap in `AuthProvider`.

### 7. useMetrics.test.ts

Mock `api` module.

Tests:

- Returns `loading: true` initially, `false` after first fetch
- `metrics` populated from `api.getMetrics` response
- Polls at default 2-second interval
- `refresh` manually triggers a fetch
- `error` set when `api.getMetrics` rejects

## CI Integration

### ci.yml changes

Add a `react-test` job (parallel with existing `rust` and `typescript` jobs):

```yaml
react-test:
  name: React (test + coverage)
  runs-on: self-hosted
  defaults:
    run:
      working-directory: apps/desktop
  steps:
    - uses: actions/checkout@v6
    - name: Install dependencies
      run: npm ci
    - name: Run tests with coverage
      run: npm run test:coverage
    - name: Upload React coverage
      uses: actions/upload-artifact@v4
      with:
        name: react-lcov
        path: apps/desktop/coverage/lcov.info
```

Modify the `rust` job to merge React coverage:

```yaml
- name: Download React coverage
  uses: actions/download-artifact@v4
  with:
    name: react-lcov
    path: react-coverage/

- name: Merge coverage
  run: cat daemon-lcov.info tui-lcov.info react-coverage/lcov.info > lcov.info
```

The `rust` job needs `needs: [react-test]` added so the React lcov artifact is available before merge.

### coverage-badge.yml changes

Add steps to:

1. Install Node + run React tests with coverage
2. Merge React lcov with Rust lcov before computing the badge percentage

## Out of Scope

- Settings page tests (complex device flow UI — separate issue)
- RunnerDetail page tests (54KB, many sub-features — separate issue)
- Dashboard, Daemon, Repositories page tests
- E2E/Playwright tests
- Tauri-level integration tests using MockDaemon
