# Changelog

All notable changes to HomeRun will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.0.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

This file is auto-generated from [Conventional Commits](https://www.conventionalcommits.org/).

---

## [0.2.3](https://github.com/aGallea/homerun/compare/v0.2.2...v0.2.3) (2026-03-24)


### Bug Fixes

* remove caching from release build and update Cargo.lock on release ([2aeaa92](https://github.com/aGallea/homerun/commit/2aeaa929937d30de73e4773abb410d2598b0852b))
* remove caching from release build workflow ([a3dcb45](https://github.com/aGallea/homerun/commit/a3dcb45a8dc922644a12c306721220d47ac5c7e2))
* use dtolnay/rust-toolchain instead of $HOME/.cargo/env in CI ([be9b695](https://github.com/aGallea/homerun/commit/be9b69534461a42a87a598b63a3896a503710dac))

## [0.2.2](https://github.com/aGallea/homerun/compare/v0.2.1...v0.2.2) (2026-03-24)


### Bug Fixes

* remove signing env vars to allow unsigned DMG builds ([ba5b53c](https://github.com/aGallea/homerun/commit/ba5b53c0f45108d5daddb08724fd37641ac344d9))
* remove signing env vars to allow unsigned DMG builds ([688001d](https://github.com/aGallea/homerun/commit/688001d95bf7a615ee15f0c57049f6626215d832))

## [0.2.1](https://github.com/aGallea/homerun/compare/v0.2.0...v0.2.1) (2026-03-24)


### Bug Fixes

* also exclude CHANGELOG.md from markdownlint ([048c9d2](https://github.com/aGallea/homerun/commit/048c9d2757ae81383195549ae0008b8ad4700497))
* exclude CHANGELOG.md from prettier ([5fd76d5](https://github.com/aGallea/homerun/commit/5fd76d5fe3356a3b4a91fc7659e478a5826028a4))
* exclude CHANGELOG.md from prettier ([392ce5b](https://github.com/aGallea/homerun/commit/392ce5b38272806762da3652808260318d80efeb))
* use PAT for release-please to trigger release build workflow ([417d3ec](https://github.com/aGallea/homerun/commit/417d3ecab778caf1a4d8b31cb52ccbf65777d077))
* use PAT for release-please to trigger release build workflow ([6bdaaff](https://github.com/aGallea/homerun/commit/6bdaaff678bd28e6451a28c8af69b6e230692954))

## [0.2.0](https://github.com/aGallea/homerun/compare/v0.1.0...v0.2.0) (2026-03-24)


### Features

* add /steps and /steps/{n}/logs daemon API endpoints ([452014e](https://github.com/aGallea/homerun/commit/452014e428ddcbd5887886f7b5d69c8413a5f463))
* add app shell with sidebar navigation and page routing ([404e8b9](https://github.com/aGallea/homerun/commit/404e8b944ed0d1e84b747b60d1ffd15cbc517dce))
* add batch/group/scale API types and commands to desktop app ([9507cae](https://github.com/aGallea/homerun/commit/9507cae2b5d29ec4f265e05acf9619934fc30e98))
* add collapse toggle to Runner Process Logs panel ([a8ad2c7](https://github.com/aGallea/homerun/commit/a8ad2c7612608a9720775dae73fa8eb33fa3650c))
* add collapse toggles to all panels, remove log search ([122f38e](https://github.com/aGallea/homerun/commit/122f38ed1ca382e11e03e766db1ee4c28b13f8de))
* add collapsible group rows with batch actions to desktop RunnerTable ([56030a5](https://github.com/aGallea/homerun/commit/56030a545e1a4f0563b4bcf7cd29ee30b901b397))
* add config module with defaults and serialization ([b350f23](https://github.com/aGallea/homerun/commit/b350f230e5a6007f099d5196a52d6b298ee0840a))
* add daemon log SSE and recent logs API endpoints ([d6aea0d](https://github.com/aGallea/homerun/commit/d6aea0d8738aae699f16bd35a3340f6e2b25fd49))
* add daemon page to Tauri frontend with types, hook, and UI ([340f50c](https://github.com/aGallea/homerun/commit/340f50c96a32db917ff3d9ab174ddbb2ba9493c5))
* add Daemon tab to TUI with log viewer and process metrics ([2579927](https://github.com/aGallea/homerun/commit/2579927b8e743b8561bbf570f7597763239208c7))
* add DaemonLogEntry type and DaemonLogState for daemon log capture ([9b7c923](https://github.com/aGallea/homerun/commit/9b7c92308411e4a8c243f20efc968dfb0ac37a77))
* add delete options to job history (clear all and per entry) ([2d9d3cf](https://github.com/aGallea/homerun/commit/2d9d3cf9b42eee33cb7273f2e3477ddebef685ad))
* add estimated_job_duration_secs to RunnerInfo type ([02da7e4](https://github.com/aGallea/homerun/commit/02da7e41b6676b1ebbf006e79cfce936bd4b6d7e))
* add frontend types, API client, and job history hook ([bd6fd3b](https://github.com/aGallea/homerun/commit/bd6fd3b1c2ee17e887fade438a9d78ba4c20acf4))
* add GET /runners/{id}/history API endpoint ([b755882](https://github.com/aGallea/homerun/commit/b755882cecb734954b79cfccaf8c9bfb1502af50))
* add GET/PUT /preferences daemon endpoints ([2992ca7](https://github.com/aGallea/homerun/commit/2992ca7f28b54b18a3173fb0a80ab61c73a76495))
* add GitHub API job log fetching and section parser ([1d1c957](https://github.com/aGallea/homerun/commit/1d1c95726a79d4dfad28de6ddf116b4ed4e71520))
* add GitHub Device Flow authentication (no PAT needed) ([b837753](https://github.com/aGallea/homerun/commit/b837753f9fe87c5e9bd5332c4a811dfce9a793fd))
* add group action endpoints (start/stop/restart/delete) ([e51a8f0](https://github.com/aGallea/homerun/commit/e51a8f01c7b52b5d85d6a517ad2aad271c654357))
* add group display with expand/collapse and batch actions to TUI ([c4628b1](https://github.com/aGallea/homerun/commit/c4628b143f01ba1de133563992124e032c297923))
* add group_id and batch/group API methods to TUI client ([e2de655](https://github.com/aGallea/homerun/commit/e2de6553c19147461aa22c3fbd15b69d870bc8dd))
* add group_id parameter to create() and repo-scoped name counter ([cc75ec5](https://github.com/aGallea/homerun/commit/cc75ec58ca68d9eeb12850e5756466bae8ecc0c2))
* add group_id to RunnerConfig and batch/group types ([15630f8](https://github.com/aGallea/homerun/commit/15630f84bd27b72afc7dbed084aea105a4c739ae))
* add header with job count and resize handle to Job History ([e13a71a](https://github.com/aGallea/homerun/commit/e13a71ad9531edc2a86cb018073e6aa3d45d0201))
* add history persistence module for job history file I/O ([ac55829](https://github.com/aGallea/homerun/commit/ac55829db91620c75fd23102fad6f8cf0665f337))
* add history_dir to Config and ensure it is created on startup ([6f7afba](https://github.com/aGallea/homerun/commit/6f7afba09511191a82819c3d98b9b2ee264a8b3e))
* add in-memory job log cache with TTL for step logs ([b0dac42](https://github.com/aGallea/homerun/commit/b0dac42ba08818b7111e6b1f6b139a0a05d78545))
* add job history types and Tauri command for runner history ([fdcc384](https://github.com/aGallea/homerun/commit/fdcc384b9c5b1dae1acb55fd281011151c6ac39b))
* add job step progress display to TUI runner detail view ([6eded77](https://github.com/aGallea/homerun/commit/6eded77e7e7b715a9dfd88bca3ab8ce1bd87e1fb))
* add job_id to JobContext for step log fetching ([a9948b0](https://github.com/aGallea/homerun/commit/a9948b07de09f8bf2a4134a6e950f4a0e0b23870))
* add JobHistoryEntry, CompletedJob types and new RunnerInfo fields ([becbd84](https://github.com/aGallea/homerun/commit/becbd84600beb2a949890cee5642a4a74fc9ebae))
* add JobProgress component to runner detail page ([06fce06](https://github.com/aGallea/homerun/commit/06fce065d982a697c66ddf956b2fd3fdb55a2e63))
* add last completed job display and job history section to RunnerDetail ([c801e2e](https://github.com/aGallea/homerun/commit/c801e2e4abe7bcb3160d2cb438f7a0de2d0ae2c2))
* add loading states and inline action buttons for runners ([404423c](https://github.com/aGallea/homerun/commit/404423c6c163721b7abb53c4c83923c8d27fe650))
* add median_duration_secs to history module ([3b4cf48](https://github.com/aGallea/homerun/commit/3b4cf48c3e4c0905612229f338d3cce9ddf45193))
* add mini progress bar to runner table rows ([51857d0](https://github.com/aGallea/homerun/commit/51857d0ee3974f0201c559fea662ccbaa19cfb64))
* add POST /runners/batch endpoint for batch runner creation ([d058bfd](https://github.com/aGallea/homerun/commit/d058bfd7466a71c48d9d36dbaa7d476aefd3b812))
* add Preferences struct to daemon Config ([b426941](https://github.com/aGallea/homerun/commit/b426941d198800f0b7f5d85da3fbe3d70bc74f2d))
* add resize handles to Runner Process Logs and Job Progress ([0b1b65e](https://github.com/aGallea/homerun/commit/0b1b65e0ea8e734a89c9c301a3086ac676940dc2))
* add runner API endpoints (Task 9) ([888a1ea](https://github.com/aGallea/homerun/commit/888a1eaa07493b5a7257806ade2a64d9938b7efe))
* add runner process management module (Task 10) ([1ab565f](https://github.com/aGallea/homerun/commit/1ab565fcfb2dffd59af130099e2bbf27f6ee39fe))
* add runner state machine and types ([f74900b](https://github.com/aGallea/homerun/commit/f74900bb7e89d4f93f8d8b01a109c3d41db28e4a))
* add RunnerManager and wire into AppState ([aea1cac](https://github.com/aGallea/homerun/commit/aea1cac207fc2d1abc36b546a233aca9a0d1deb1))
* add scale endpoint and group_id filter on list_runners ([50c8596](https://github.com/aGallea/homerun/commit/50c8596d17a9a27f3315dedc86d8b23e882aef68))
* add spinner and dimmed row for loading state on actions ([11194d5](https://github.com/aGallea/homerun/commit/11194d5b393aebf85f23d7a324291b0e5131ed14))
* add SSE log streaming endpoint (Task 11) ([7196daa](https://github.com/aGallea/homerun/commit/7196daa5874b1471779b00a0b945d311ec138268))
* add Tauri commands for preferences ([e5a09f2](https://github.com/aGallea/homerun/commit/e5a09f2a2052d3b5f017c3c16d94a84fe311044a))
* add Tauri IPC commands for step progress and step logs ([03f112b](https://github.com/aGallea/homerun/commit/03f112bca4ad2cdebf538d8cf02b9bd48846f727))
* add TypeScript API layer and React hooks for daemon communication ([4bfb3bf](https://github.com/aGallea/homerun/commit/4bfb3bf9db601650d7745951f313c749df752f96))
* add TypeScript types, API commands, and useJobSteps hook ([fb21b78](https://github.com/aGallea/homerun/commit/fb21b78833fad5f2648aced2e77922c2da09ef96))
* add Worker log step event parser with types and tests ([1f8ef88](https://github.com/aGallea/homerun/commit/1f8ef883384b9b41d3547d2dabf0e825eff33451))
* add WorkerLogWatcher for tracking step progress from Worker logs ([eb25a2a](https://github.com/aGallea/homerun/commit/eb25a2aae1f34f9432f28b40f4954bc63ced5c0b))
* capture and display job failure reason in history ([9c2901d](https://github.com/aGallea/homerun/commit/9c2901d9893c03ff61ddd63439756f59bbc23b42))
* capture job history on completion in both event paths ([8711f47](https://github.com/aGallea/homerun/commit/8711f47ecba1f34fee8adfd0af56235da2a3cb03))
* configure Tauri sidecar and DMG settings for homerund bundling ([18d365c](https://github.com/aGallea/homerun/commit/18d365c29fde6ebd62f4bdad03db356a59eab495))
* **daemon:** add GitHub API client and repos endpoint ([092523d](https://github.com/aGallea/homerun/commit/092523da8f43b4a8b728a3b0306db310ce2ce1ae))
* **daemon:** add launchd auto-start service management ([16b59f7](https://github.com/aGallea/homerun/commit/16b59f776da5a49cc61dff41635e2aa3b0be1a68))
* **daemon:** add macOS notifications via notify-rust ([146fdd7](https://github.com/aGallea/homerun/commit/146fdd7478d206bd57a9d3c5229ac76e6a784c3a))
* **daemon:** add runner binary downloader module ([2584c09](https://github.com/aGallea/homerun/commit/2584c0903f3c4637af64b53fb2c6bbce7817eceb))
* **daemon:** add runner binary update checker ([9fad5b7](https://github.com/aGallea/homerun/commit/9fad5b78c8fef4df5b0d6f562b4e2960a32f49ec))
* **daemon:** aggregate process tree for runner metrics ([1ccc725](https://github.com/aGallea/homerun/commit/1ccc725099b66671999325b67722728eca19afca))
* **daemon:** aggregate process tree for runner metrics ([c5490e0](https://github.com/aGallea/homerun/commit/c5490e0e5cddd7929588f5a756c2fbd9a3588d28))
* **daemon:** implement Auth module with PAT login and macOS Keychain storage ([631c693](https://github.com/aGallea/homerun/commit/631c69301178c2bb3e020fd64ef2e2b636621d85))
* **daemon:** implement Axum server on Unix socket with health endpoint ([4b3a03e](https://github.com/aGallea/homerun/commit/4b3a03e42395105a85bb803c28731725f91b830f))
* **desktop:** add htop-style resource bars and faster polling ([a0cf4be](https://github.com/aGallea/homerun/commit/a0cf4be5e3431122abcd8ce9a5069f7393f5818f))
* **desktop:** implement Tasks 6-10 — wizard, runner detail, repos, monitoring, settings ([4580b4e](https://github.com/aGallea/homerun/commit/4580b4e1ed26213603a10502c5a921126c6055fd))
* **desktop:** scaffold Tauri 2.0 + React 19 desktop app with Rust daemon commands ([8bca310](https://github.com/aGallea/homerun/commit/8bca3106b5ab13037b582401fb72db083cc40e78))
* display daemon logs and process info in TUI and Tauri ([#36](https://github.com/aGallea/homerun/issues/36)) ([9195edb](https://github.com/aGallea/homerun/commit/9195edbf89c3d3d77491b88b1ef40ab29def9642))
* extend metrics endpoint with daemon process info ([b2d0327](https://github.com/aGallea/homerun/commit/b2d032787462928b1a55a52c34186d5a01199700))
* fetch full error message from GitHub annotations ([f0186f2](https://github.com/aGallea/homerun/commit/f0186f2f1ef6a206bd1192c937e045a9ae738e9d))
* fetch missing job context at completion + add re-run button ([b7a796a](https://github.com/aGallea/homerun/commit/b7a796af72b80517cf8ec7cd544e1e355f93f064))
* group multi-instance runners for batch actions ([a503038](https://github.com/aGallea/homerun/commit/a503038149296e44307857d1b9d7f82cb959a116))
* implement dashboard with stats cards and runners table ([01bde57](https://github.com/aGallea/homerun/commit/01bde57c284f79e73fa9baf5658c503e51274f18))
* implement GitHub Device Flow (RFC 8628) authentication ([3c2ce1d](https://github.com/aGallea/homerun/commit/3c2ce1dda9f4fef7024eda24c739a5586bbc8bdd))
* implement Smart Repo Discovery (scanner module + API + TUI) ([7b4cf54](https://github.com/aGallea/homerun/commit/7b4cf54247ffc4fd14513bdbbc9ac43a07b7503d))
* integrate WorkerLogWatcher into RunnerManager with polling ([6bd0535](https://github.com/aGallea/homerun/commit/6bd0535f99b5256a4ec7ca1a8b8906106739b4ce))
* live log streaming in runner detail page ([c697d96](https://github.com/aGallea/homerun/commit/c697d961121a3eaa565f586e8dd3d5f785b236eb))
* live log streaming in runner detail page via SSE + Tauri events ([b1b74ab](https://github.com/aGallea/homerun/commit/b1b74ab55469ff99d30e6da429f1f8109d51f0c7))
* live log viewer + job tracking with Busy state ([6ac3a0e](https://github.com/aGallea/homerun/commit/6ac3a0e24f331765def0c27371da05a70ac1b129))
* make job history rows expandable to show step details ([90168c3](https://github.com/aGallea/homerun/commit/90168c34e8f5177ae3eef8ea1a9aab566e7a89bf))
* package HomeRun as .dmg for macOS distribution ([47e88b3](https://github.com/aGallea/homerun/commit/47e88b333e2ae26a4557257f97bf4e4be40100bd))
* populate estimated_job_duration_secs on RunnerInfo ([5d8a25a](https://github.com/aGallea/homerun/commit/5d8a25ad2d240517867fe90290e77b9ccd82dbb4))
* redesign dashboard runner table layout ([6a0e0cf](https://github.com/aGallea/homerun/commit/6a0e0cf0e5487c8ef9fcf919b027a4a5524d0337))
* redesign desktop app dashboard UX ([0498e52](https://github.com/aGallea/homerun/commit/0498e52147d44da6f16a9581b46298631c4c2b5c))
* redesign desktop app dashboard UX ([318539a](https://github.com/aGallea/homerun/commit/318539a50133cec410586e5e67f87fa5c44b1900))
* redesign runner detail layout and add diag log tailing ([0ef7f0f](https://github.com/aGallea/homerun/commit/0ef7f0f52ee31a144ac7b8102e3acefd577bf608))
* redesign runner detail page, add current job column, update icon ([9a8357b](https://github.com/aGallea/homerun/commit/9a8357be0c806abd63accb39de20f1f3cabc57b9))
* refresh history after delete, link to job instead of run ([f2784bb](https://github.com/aGallea/homerun/commit/f2784bbbf91fe02d2f61d6145728263992e2227a))
* replace "Not signed in" text with Sign in button in sidebar ([430bc50](https://github.com/aGallea/homerun/commit/430bc50fb4d8462be66883a48dc626cc29ae81e9))
* replace SSE log streaming with polling and add job tracking ([b51f833](https://github.com/aGallea/homerun/commit/b51f83334338b4a6e53b97cee7f3b7912026f47d))
* save workflow run history per runner ([e3cf349](https://github.com/aGallea/homerun/commit/e3cf3491b0ffb9f816714ac2d927bdcf9d18c830))
* show branch and PR number in job history rows ([879a0c2](https://github.com/aGallea/homerun/commit/879a0c23d840f21a53798b2dd8edbb0235b85f09))
* show branch name and PR number for running jobs ([fe520ea](https://github.com/aGallea/homerun/commit/fe520eaece7fa3b93d12ce14f96378066ae02a78))
* show branch name and PR number for running jobs ([aa316e3](https://github.com/aGallea/homerun/commit/aa316e30f151df1b5e2d298d6d42e331651a89d3)), closes [#12](https://github.com/aGallea/homerun/issues/12)
* show daemon disconnected banner on all pages ([ce53a18](https://github.com/aGallea/homerun/commit/ce53a18ed9ef380ac42723b5ed7664b9de4a29c4))
* show error message in Last Job card ([3b13f91](https://github.com/aGallea/homerun/commit/3b13f9118d909f5916eba4895fd27f499a01215c))
* show estimated job progress based on historical durations ([8d7cf50](https://github.com/aGallea/homerun/commit/8d7cf50e9381b5c06423b67078d96fdec96f5129))
* show loading state when deleting history entries ([f68e13e](https://github.com/aGallea/homerun/commit/f68e13e323b06fec688eb9f0ee795d4f862b47bc))
* show real job progress bar in runner detail ([4b25d33](https://github.com/aGallea/homerun/commit/4b25d33e2dbea245f17f8b028d8bc5f4484e22e9))
* show repo name on group rows ([d3b50ab](https://github.com/aGallea/homerun/commit/d3b50ab17ecee8218c22dbac9bd179abaa4101ba))
* show workflow job step progress on runner detail page ([5077f57](https://github.com/aGallea/homerun/commit/5077f57d899705181c3a234bb8bc558517dc11d4))
* stream runner logs via SSE + compute uptime dynamically (closes [#4](https://github.com/aGallea/homerun/issues/4), closes [#5](https://github.com/aGallea/homerun/issues/5)) ([0c4cbce](https://github.com/aGallea/homerun/commit/0c4cbce05849378a6e2e9e5eeaa0b5b6eba68c74))
* support creating multiple runners at once ([67e2dab](https://github.com/aGallea/homerun/commit/67e2dab615615f1c2104a0866d8d343226b69d5a))
* support creating multiple runners at once in NewRunnerWizard ([7219823](https://github.com/aGallea/homerun/commit/721982312a50752c81e9df7b65894671511d0707))
* **task-12:** add MetricsCollector, RingBuffer, and metrics API endpoint ([484e306](https://github.com/aGallea/homerun/commit/484e306fed3e5bf0574dd2c4efe8fbec70b1593b))
* **task-13:** add WebSocket events endpoint for real-time runner state broadcasting ([c97f18f](https://github.com/aGallea/homerun/commit/c97f18fbef6305425fb7fe77aa4d365979faf0ee))
* **tasks-14-15:** add integration tests and apply clippy/fmt cleanup ([e687761](https://github.com/aGallea/homerun/commit/e68776142c0bdca7dcc6dd1e7a4d9bc6410dffa9))
* **tauri:** add daemon log backend commands ([4df62d1](https://github.com/aGallea/homerun/commit/4df62d17040d2fe36c1da94d52fa73c626bcafeb))
* **tui:** add App state struct and event loop skeleton ([cf8947a](https://github.com/aGallea/homerun/commit/cf8947a0b3e3202f70231ba33f0143437b0c07d0))
* **tui:** add DaemonClient with Unix socket HTTP transport ([7c2d681](https://github.com/aGallea/homerun/commit/7c2d681effa6784a2a75ba0da0b73323be276730))
* **tui:** add keyboard handling with Action enum and handle_key method ([0d288e7](https://github.com/aGallea/homerun/commit/0d288e7d3bf9cdbba19e42b96c47ee63f2cc12c2))
* **tui:** add Runners tab with split-pane list/detail, tabs, and status bar ([33da352](https://github.com/aGallea/homerun/commit/33da3528c4643f5e7742c6908463058dc7949660))
* **tui:** implement plain CLI mode for --no-tui list and status commands ([89ebd46](https://github.com/aGallea/homerun/commit/89ebd46539310fb05624213a41bba0f594598523))
* **tui:** wire main TUI loop with 2s polling and WebSocket event forwarding ([b778297](https://github.com/aGallea/homerun/commit/b7782975a91c55271d12f60abf9deb60d3299800))
* UI polish — compact runner detail, current job column, new icon ([123fd65](https://github.com/aGallea/homerun/commit/123fd65bedb42d49c01598aca97cf8d6ce1ba98e))
* **ui:** add spinner for transient runner states (creating, registering, stopping) ([0e66a6b](https://github.com/aGallea/homerun/commit/0e66a6b635e4e92e3864956005690b5252932ccb))
* update NotificationManager with per-category toggles ([23ac762](https://github.com/aGallea/homerun/commit/23ac762473c9bc22ac3d765bfdb0cfbfade99583))
* update README with new features and add multi-runner integration test ([28ded9c](https://github.com/aGallea/homerun/commit/28ded9c3a427bf5f598f90f9779824ac91d1a4d9))
* wire all Settings toggles to daemon preferences API ([0a106b8](https://github.com/aGallea/homerun/commit/0a106b8ea45b16fa2f7fe105ea85845a9f49b12d))
* wire DaemonLogLayer into daemon startup and AppState ([32012af](https://github.com/aGallea/homerun/commit/32012af2878720ddfce48086f1adff02c34a90ce))
* wire full runner lifecycle — create/register/start/stop/delete + disk persistence (closes [#2](https://github.com/aGallea/homerun/issues/2)) ([84f3f0d](https://github.com/aGallea/homerun/commit/84f3f0d97cd6072b3849ae7a015a81821b27b6a1))
* wire job history into RunnerManager ([4f3625e](https://github.com/aGallea/homerun/commit/4f3625e5bd137c382e1ec7a6a2254ce591e612a8))


### Bug Fixes

* add estimated_job_duration_secs to Tauri RunnerInfo bridge ([c401d95](https://github.com/aGallea/homerun/commit/c401d953baf3a4963e62f47f82ebe9fdd2fdadd4))
* add include-component-in-tag to release-please config ([5205627](https://github.com/aGallea/homerun/commit/5205627c7cd054a2c882170656d998fd09a23680))
* add keychain debug logging for device flow token storage ([b088ff4](https://github.com/aGallea/homerun/commit/b088ff47326cbbaae33edbffe923804336c5c6d7))
* address code review issues and add comprehensive tests ([6e2bbd1](https://github.com/aGallea/homerun/commit/6e2bbd18df5b4f09939ea5e7001950fc19bedb33))
* align status badge text by using fixed-width icon container ([8d300f0](https://github.com/aGallea/homerun/commit/8d300f08d0e4885b4ee3733d7d12769197ba9c59))
* align status badges vertically in group row ([52eb73b](https://github.com/aGallea/homerun/commit/52eb73b3c016c1ac237e3bcf67e97f1bf874c73a))
* allow Stopping → Registering transition for runner restart ([f70c1d7](https://github.com/aGallea/homerun/commit/f70c1d7fc764e930ff21fe51ddb7c8e28b1b3f6c))
* always show action buttons, use disabled+opacity for loading state ([c4463d9](https://github.com/aGallea/homerun/commit/c4463d91a485ea2d48aaefbaafa3c6c6b7effcda))
* always show View link on current job, upgrade URL as context arrives ([4062c93](https://github.com/aGallea/homerun/commit/4062c93d118f8e93d4287266f404170ee05ae73e))
* apply resize height to history panel card, not inner list ([95f64d3](https://github.com/aGallea/homerun/commit/95f64d3254f3e561f8b1bdca180b5d28652763cf))
* auth logout clears state before keychain, stop polling flicker ([517939c](https://github.com/aGallea/homerun/commit/517939c67aa868a28c962b68c7029291522b73e3))
* auto-build homerund sidecar before tauri dev ([675ebff](https://github.com/aGallea/homerun/commit/675ebfff2bddda05c8982f70fe149cf5f10a10ac))
* auto-build homerund sidecar before tauri dev ([53983a3](https://github.com/aGallea/homerun/commit/53983a3752c8b5dc665d5bfe79d0742f7c809d65))
* **ci:** fix coverage badge extraction from llvm-cov output ([8e0b006](https://github.com/aGallea/homerun/commit/8e0b0061b493bc54dacf1ba4fbc8b950ccab7fa4))
* clean up job history on runner deletion ([4c1d76c](https://github.com/aGallea/homerun/commit/4c1d76c828b7e6299322ea1014514f279ed92cf5))
* clear GIT_DIR/GIT_WORK_TREE in scanner git commands ([b4964f7](https://github.com/aGallea/homerun/commit/b4964f70da4b82d3e0b6a555b46143c3688ca199))
* clear stale data on all pages when daemon is unreachable ([654b933](https://github.com/aGallea/homerun/commit/654b933b3970bd14437c1dbb18c886c62294f2c8))
* clippy bool_assert_comparison + make audit non-blocking ([280f01b](https://github.com/aGallea/homerun/commit/280f01b4fa00a3df79ea78dc661255c5595e1002))
* current job card takes 50% width, view link moved to top-right ([e391373](https://github.com/aGallea/homerun/commit/e391373c1f477d4a596c1b08af463df9fd650adf))
* **daemon:** refresh process list once per metrics request ([40d04fc](https://github.com/aGallea/homerun/commit/40d04fc0cbcda60aade6a8c41b3c74930ef8c603))
* deduplicate runner labels, update README with current features ([8e43a89](https://github.com/aGallea/homerun/commit/8e43a896f51954dce182c4c3ee1507530900adfb))
* delay "taking longer than usual" by 5s past estimate ([f3a1372](https://github.com/aGallea/homerun/commit/f3a13720b49be62d9c52917fc5605b159e023ece))
* **desktop:** use Tauri shell open for external links ([74172e4](https://github.com/aGallea/homerun/commit/74172e4fb0f183d46d1e553ea9b267d824be54a5))
* disable individual runner actions when group action is pending ([0991aca](https://github.com/aGallea/homerun/commit/0991acaf09eeadb6a9b065775703ec57e55ee49b))
* disable real macOS notifications in tests to prevent spurious popups ([608bc00](https://github.com/aGallea/homerun/commit/608bc00a5d9e6d4081df004b76d53c3ccfb2eb8a))
* disable row clicks during pending actions ([77195ae](https://github.com/aGallea/homerun/commit/77195ae94a46101f2c51d1de811c4bc01d3d0dc9))
* don't delete keychain token on transient validation failure ([a043ef5](https://github.com/aGallea/homerun/commit/a043ef550a300fa83efbd921b9932cb9dd077cd3))
* don't delete keychain token on transient validation failure ([2ca8d3e](https://github.com/aGallea/homerun/commit/2ca8d3e7451453951ebfdcf1c9c47167d3cd6270))
* exclude build artifacts from pre-commit, install deps in CI ([3e4bc36](https://github.com/aGallea/homerun/commit/3e4bc365f33a31c550b58acf9120b60fb2ee829e))
* fetch job annotations by searching recent runs directly ([b193ad2](https://github.com/aGallea/homerun/commit/b193ad2c4b49651423ffdf30ef01fff5ec8336db))
* improve job context fetch logging to diagnose missing branch/PR ([b362d08](https://github.com/aGallea/homerun/commit/b362d088dbb256247e7162bd3084b4e722bdce92))
* include /job/{id} in run_url for both completion handlers ([3fbf761](https://github.com/aGallea/homerun/commit/3fbf76190cfa2340850ecf9f42b689130c1aef28))
* job context poller for branch/PR info on busy runners ([56f52a4](https://github.com/aGallea/homerun/commit/56f52a45050b22d63b44a84c5d43e155b9d9bad4))
* keep "Not signed in" text and add small sign in button below it ([66002a4](https://github.com/aGallea/homerun/commit/66002a46cbe0cae0e8beef6a800bbcbe074afdf3))
* kill old runner process before spawning new one on restart ([9a2503e](https://github.com/aGallea/homerun/commit/9a2503e62af1aec19518b32e3df398acd08d7432))
* left-align status icon in container for consistent text alignment ([c6e96ea](https://github.com/aGallea/homerun/commit/c6e96ea81a640cb4678b6cd1d9c85098a66953e2))
* link to in-progress actions, add 12 tests for log+job tracking ([13b2909](https://github.com/aGallea/homerun/commit/13b2909d9dd85a11a7db84f20c08750c1ba2887b))
* load config.toml on startup and improve runner restore logic ([8c64029](https://github.com/aGallea/homerun/commit/8c640294123a64427fbca06723de7fd48e84d680))
* make group restart non-blocking by spawning stop+restart in background ([0b8211d](https://github.com/aGallea/homerun/commit/0b8211d5dcc26389d437191cd09f417474c22fba))
* make spinner same size as status dot (8px) for consistent alignment ([293e495](https://github.com/aGallea/homerun/commit/293e495d14e5105fcec296e97b9f22b612b72aa8))
* make tsc always_run to prevent skipping after prettier ([2b77282](https://github.com/aGallea/homerun/commit/2b77282ff0557aced5d5e94578db524a76abb4f2))
* match jobs by name instead of runner_name for in-progress runs ([06c6c18](https://github.com/aGallea/homerun/commit/06c6c18e71b484d8d76ce703de507d8f91e5bf43))
* only scroll expanded history entry into view once ([c7bc06a](https://github.com/aGallea/homerun/commit/c7bc06aa35d716b0638c40e64296119fd87036f0))
* only show "View" link when job context is available ([7530032](https://github.com/aGallea/homerun/commit/7530032bc7c0ce8732cbdf0d883f62707fe10613))
* only show per-status count in group row when statuses are mixed ([c7cfab8](https://github.com/aGallea/homerun/commit/c7cfab8cf6e095a288cd4b8b04c24bac139506e0))
* override flex: 1 on history panel so height is respected ([9be2a97](https://github.com/aGallea/homerun/commit/9be2a971f50f53da5a2696371d221f7807565641))
* parse job name from timestamped runner output, add workflow link ([a290857](https://github.com/aGallea/homerun/commit/a290857ba7b05c1de4872da9573aa862caab7b89))
* parse service_status JSON object instead of bare bool ([d5ef35c](https://github.com/aGallea/homerun/commit/d5ef35c6487fb7c2a69c23dcdca4a1a85925bbed))
* polish dashboard UX - responsive actions, compact stats, aligned grid ([205bcb9](https://github.com/aGallea/homerun/commit/205bcb95349d30385e511c2f7e52466c30524e2d))
* prevent name wrapping and remove status counts from group row ([ba678a7](https://github.com/aGallea/homerun/commit/ba678a7aaba17f68399339dfd9380d08481212cf))
* properly fix stop_process deadlock by removing process handle first ([185cf8d](https://github.com/aGallea/homerun/commit/185cf8d3f4d05a5754a3a3ff81048101ee83b3a2))
* remove committed target/ directory, fix gitignore to catch all target/ dirs ([bb74cf5](https://github.com/aGallea/homerun/commit/bb74cf5d02683711120c5a12d08548979b546128))
* remove tracked Tauri gen/ schemas from git ([5e1d009](https://github.com/aGallea/homerun/commit/5e1d00925016a15e4e5fb73c2466d0bff4bbb4ba))
* remove unused MetricBar component ([a45bdfa](https://github.com/aGallea/homerun/commit/a45bdfa962fa33fb893927a40c129ac7cf876e01))
* rename release-please config to match action default ([28de52a](https://github.com/aGallea/homerun/commit/28de52a457b0d4733833d125c0d93dde70670c07))
* rename release-please config to match action default ([c9afc90](https://github.com/aGallea/homerun/commit/c9afc9017550f2979d0f109c16b3b61f2fe89d23))
* replace one-shot job context fetch with background poller ([e41e56a](https://github.com/aGallea/homerun/commit/e41e56a1830d025202c030ccfdc5351e5505428b))
* resolve auth token loss and improve unauthenticated UX ([2180876](https://github.com/aGallea/homerun/commit/218087673a72b3e47dc6806f91e51da3d727369f))
* resolve auth token loss and improve unauthenticated UX ([#29](https://github.com/aGallea/homerun/issues/29)) ([ad3c5a0](https://github.com/aGallea/homerun/commit/ad3c5a0cb3af28ebbeef3721b058d626705bea45))
* resolve deadlock in stop_process by not waiting for child exit ([ace2fdf](https://github.com/aGallea/homerun/commit/ace2fdfe5d77b9bb99c816d7da63ddb1357e6511))
* resolve device flow login stuck on "Starting..." and broken logout ([#30](https://github.com/aGallea/homerun/issues/30)) ([eaeab45](https://github.com/aGallea/homerun/commit/eaeab45b2e5105a0abaa065490754f188387aaca))
* resolve device flow stuck on Starting and broken logout ([01bb3fa](https://github.com/aGallea/homerun/commit/01bb3faf7c430f5042457df9d455b362788c0ffa))
* resolve runner restart "session already exists" by graceful process group shutdown ([#31](https://github.com/aGallea/homerun/issues/31)) ([bf18e8b](https://github.com/aGallea/homerun/commit/bf18e8b8fe95f8371863bd513b818271e5fce650))
* resolve runner restart session conflict ([#31](https://github.com/aGallea/homerun/issues/31)) ([4d7b935](https://github.com/aGallea/homerun/commit/4d7b93574a7e125a1157f2a3b9a2c06b67852b38))
* resolve RwLock deadlock in job context fetch and add JobContext to Tauri client ([e598d6d](https://github.com/aGallea/homerun/commit/e598d6d6cd882fb4485b9584dc54dc59c7a21986))
* restore auth token from keychain on daemon startup ([00843c1](https://github.com/aGallea/homerun/commit/00843c1d548727490b9a8eb1d003506fbb6fed6b))
* restore completion time in job history rows ([a2b427f](https://github.com/aGallea/homerun/commit/a2b427f8554e404259dcc278c5d9bc58b6193e3b))
* runner lifecycle, auth persistence, and UI improvements ([1ace38b](https://github.com/aGallea/homerun/commit/1ace38b2d3f2eab893356f75ddf76e3d2b6b4c5d))
* settings page toggles and preferences persistence ([9b49730](https://github.com/aGallea/homerun/commit/9b49730f66b0061865a571f4d9b34272b4c65b34))
* show "Last Job" title when runner is not busy ([5f4142a](https://github.com/aGallea/homerun/commit/5f4142abd0368a6b5144baab9b19280eaa178d42))
* skip config.sh for already-registered runners, just start run.sh ([5d6f6e6](https://github.com/aGallea/homerun/commit/5d6f6e6a697d3c104b1690b5ab738c2a63f82c41))
* skip keychain restore in integration tests via env var ([1db79a2](https://github.com/aGallea/homerun/commit/1db79a2ec07cb165f213471eb0575d0d96009229))
* skip lazy keychain restore in tests ([6a3de01](https://github.com/aGallea/homerun/commit/6a3de013aaf154face735ebe93b412fc02b143a4))
* spawn step-watcher polling task in stdout reader path ([bb0d407](https://github.com/aGallea/homerun/commit/bb0d407b8c46a319d449455c7f6cb3831a23f937))
* sync auth token to runner manager on startup and login ([9f79ff1](https://github.com/aGallea/homerun/commit/9f79ff16d3525ab6d06312f1afff1ee8c9b218c3))
* transition Stopping runners to Offline when process exits ([9bd4eab](https://github.com/aGallea/homerun/commit/9bd4eab495c8d2fc2b8cbcc061763eb65e1c234f))
* use healthCheck instead of daemonAvailable for connectivity ([41cbe19](https://github.com/aGallea/homerun/commit/41cbe19a5dafe54631b3069a121887bcbe3c6ec1))
* use independent client with timeout for health check ([8a6dca0](https://github.com/aGallea/homerun/commit/8a6dca09b19aee6e2a4ed365546aec5be1a7bbc1))
* use next-available naming instead of monotonic counter for runners ([99ba1c8](https://github.com/aGallea/homerun/commit/99ba1c86e40177f7a2c7611eca256668bf485278))
* use percentage-based column widths for consistent runner list alignment ([c34e8d2](https://github.com/aGallea/homerun/commit/c34e8d20c4735d1803c1573d6c8db34d47f097bd))
* use PID-based kill to avoid Child write lock deadlock ([9975c03](https://github.com/aGallea/homerun/commit/9975c0350cd767599bcca1a042d977a27fec35cf))
* wait for runners to reach Offline before restarting ([b923b0b](https://github.com/aGallea/homerun/commit/b923b0b22f132df4642ee4a6b31f1427ec2fe066))

## [0.1.0] — 2026-03-21

Initial release of HomeRun.

### Added

#### Daemon (`homerund`)

- Rust + Axum daemon exposing REST/SSE/WebSocket API over Unix socket at `~/.homerun/daemon.sock`
- GitHub OAuth flow with temporary localhost callback listener
- Personal Access Token (PAT) authentication as fallback
- Token storage in macOS Keychain via `security-framework`
- Runner lifecycle management: create, register, start, stop, restart, delete
- App-managed runner mode (daemon child process)
- Background service runner mode (macOS launchd plist)
- Runner state machine: Creating → Registering → Online ⇄ Busy → Offline → Deleting
- Auto-restart on crash: up to 3 attempts with 10s backoff
- GitHub runner binary download and caching at `~/.homerun/cache/`
- Per-runner isolated working directories at `~/.homerun/runners/<name>/`
- Real-time log streaming via Server-Sent Events (SSE)
- CPU/RAM/disk metrics collection via `sysinfo`
- In-memory metrics ring buffer (last 24h per runner)
- WebSocket `/events` endpoint for real-time status updates
- Config stored at `~/.homerun/config.toml`
- Structured logging to `~/.homerun/logs/`

#### TUI / CLI (`homerun`)

- Ratatui-based terminal UI with split-pane layout (runner list + detail)
- Tab bar: Runners, Repos, Workflows, Monitoring
- Full keyboard navigation: `↑↓`, `Enter`, `a`, `d`, `s`, `r`, `l`, `e`, `1-4`, `q`, `?`
- Live log view per runner
- Plain CLI mode via `homerun --no-tui <command>`
- CLI commands: `list`, `add`, `remove`, `status`, `scan`, `login`
- `homerun scan <path>` — local workspace scan for `runs-on: self-hosted`
- `homerun scan --remote` — GitHub API scan
- Clap-based argument parsing

#### Desktop App (Tauri)

- Tauri 2.0 desktop app for macOS (ARM64 + Intel)
- React + TypeScript frontend
- Dashboard with runner stats cards and runners table
- Repositories view with runner counts and quick-add
- Runners view with filtering and bulk actions
- Monitoring view with CPU/RAM/disk graphs
- Workflow Runs view with status across all repos
- New Runner wizard: pick repo → configure → launch
- Runner detail view: live logs, resource graphs, controls
- Smart repo discovery: local workspace scan + GitHub API scan
- Actions menu: start, stop, restart, delete (with confirmation)
- React Router v7 for navigation

#### Infrastructure

- Rust workspace with `resolver = "2"`
- Shared workspace dependencies and version management
- MIT license

[0.1.0]: https://github.com/aGallea/homerun/releases/tag/v0.1.0
