# Changelog

All notable changes to HomeRun will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.0.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

This file is auto-generated from [Conventional Commits](https://www.conventionalcommits.org/).

---

## [0.8.2](https://github.com/aGallea/homerun/compare/v0.8.1...v0.8.2) (2026-04-03)


### Bug Fixes

* CI badge, release version caching, and sidebar logo size ([eace8ad](https://github.com/aGallea/homerun/commit/eace8ad085f3ba9a21d9a891022dc1b9364376e8))
* CI badge, release version caching, and sidebar logo size ([f783cd0](https://github.com/aGallea/homerun/commit/f783cd0361bc6b4075490ae0fb27561f087a8097))
* **ci:** skip no-commit-to-branch hook on push-to-master CI runs ([cf38754](https://github.com/aGallea/homerun/commit/cf387541b6f8335a42332683091f6356fce38b68))
* **ci:** skip no-commit-to-branch hook on push-to-master CI runs ([c39fa50](https://github.com/aGallea/homerun/commit/c39fa5052a0ec58bcbedd746244abd72144e6991))

## [0.8.1](https://github.com/aGallea/homerun/compare/v0.8.0...v0.8.1) (2026-04-03)


### Bug Fixes

* **ci:** add pip scripts dir to PATH for macOS self-hosted runners ([9f49b67](https://github.com/aGallea/homerun/commit/9f49b67d71df8c8915c64fbe6ac645b2c8cd4fb3))
* **ci:** fall back to python3 -m pre_commit when binary not in PATH ([9eaa285](https://github.com/aGallea/homerun/commit/9eaa2859fc1bddf072ca2b04f4c7658e1de650fc))
* **ci:** use python3 fallback for pre-commit on macOS self-hosted runners ([3d90011](https://github.com/aGallea/homerun/commit/3d9001192b10f332e9da71b61fbb9146a0bfae4d))
* restrict macOS release build to macOS self-hosted runners ([d334fb3](https://github.com/aGallea/homerun/commit/d334fb38629c0d886605ab015d96a454b5de38c0))

## [0.8.0](https://github.com/aGallea/homerun/compare/v0.7.0...v0.8.0) (2026-04-03)


### Features

* add Windows MSI build to release workflow ([#112](https://github.com/aGallea/homerun/issues/112)) ([4dd5082](https://github.com/aGallea/homerun/commit/4dd508236061d07fa92dc2cfbf4381296e2c3dc5))
* cross-platform binary download with .zip extraction for Windows ([#112](https://github.com/aGallea/homerun/issues/112)) ([b8bfc62](https://github.com/aGallea/homerun/commit/b8bfc623a4f33c77f78678a4558f9d4393f11060))
* named pipe IPC for Windows daemon server ([#112](https://github.com/aGallea/homerun/issues/112)) ([0044323](https://github.com/aGallea/homerun/commit/00443232d985c3054a91cbe455fec44b72da50f2))
* show job progress in runner overview and add Windows build docs ([35d7f1c](https://github.com/aGallea/homerun/commit/35d7f1c1f6375c7508d186c48875fb94256ecdb0))
* Windows named pipe support for desktop client ([#112](https://github.com/aGallea/homerun/issues/112)) ([355b987](https://github.com/aGallea/homerun/commit/355b9877c91e38f12b206339e120c052312a91ca))
* Windows named pipe support for TUI client ([#112](https://github.com/aGallea/homerun/issues/112)) ([1e920d2](https://github.com/aGallea/homerun/commit/1e920d27ca1403d663930832e6422c00ff86d5f1))
* Windows platform support ([c8f0fb0](https://github.com/aGallea/homerun/commit/c8f0fb091846a8bdcb1f6095e563f9bd9b417000))


### Bug Fixes

* add .gitattributes for LF normalization and fix XML end-of-file ([#113](https://github.com/aGallea/homerun/issues/113)) ([211814a](https://github.com/aGallea/homerun/commit/211814aa7635f81f55b1aaa6a37db60507d46203))
* add essential dirs to runner PATH and use cmd for Python setup ([#113](https://github.com/aGallea/homerun/issues/113)) ([b7a1010](https://github.com/aGallea/homerun/commit/b7a10103f5c12b105cc5b6a0b30ffb49aad609a7))
* add setup-node and rust-toolchain to coverage-badge workflow ([57d2450](https://github.com/aGallea/homerun/commit/57d2450c8046b999fb27d9972575107f0695b10e))
* add shell: bash to all self-hosted workflow jobs ([8bc86a9](https://github.com/aGallea/homerun/commit/8bc86a99cbb0d16b5f39c04e83c9c3b21dbd60c4))
* cargo fmt and use PowerShell for Python setup in CI ([#113](https://github.com/aGallea/homerun/issues/113)) ([19b2d9e](https://github.com/aGallea/homerun/commit/19b2d9e59a07e367acaa2733d59b953e264e7d8d))
* CI PATH resolution for Windows Git Bash ([#112](https://github.com/aGallea/homerun/issues/112)) ([f1c095c](https://github.com/aGallea/homerun/commit/f1c095c5ea9a273c5b91e1325c553f939472ad6b))
* CI rustup shim and python issues on Windows runner ([#112](https://github.com/aGallea/homerun/issues/112)) ([aef6775](https://github.com/aGallea/homerun/commit/aef6775d30c7f40f703dc3c35ed114617b536fbc))
* clippy warnings and Windows test client TCP support ([#112](https://github.com/aGallea/homerun/issues/112)) ([bd68b00](https://github.com/aGallea/homerun/commit/bd68b00bb4e46fd388a925402d05ba17d1aeffdf))
* derive Python tool cache path from GITHUB_WORKSPACE ([#113](https://github.com/aGallea/homerun/issues/113)) ([3a9591b](https://github.com/aGallea/homerun/commit/3a9591b57dd41be98d83ce72d1580e5be273983e))
* dynamically discover Git install path for runner PATH ([5700d00](https://github.com/aGallea/homerun/commit/5700d002bfa4f54e4525520840fa89d1a4e95daa))
* enable diag log tailing for freshly spawned runners on Windows ([942ec47](https://github.com/aGallea/homerun/commit/942ec47e894828e8e6647a2ec88da97d3d4be140))
* ensure Git Bash is on PATH for Windows runners ([#113](https://github.com/aGallea/homerun/issues/113)) ([869294e](https://github.com/aGallea/homerun/commit/869294e2110e6f593a27b27c3206b91ba18b6284))
* fallback to any runner's Python cache when setup-python fails ([#113](https://github.com/aGallea/homerun/issues/113)) ([8706baa](https://github.com/aGallea/homerun/commit/8706baaf60a086c317af679821734c3d05643772))
* improve tray panel positioning for bottom taskbar ([#112](https://github.com/aGallea/homerun/issues/112)) ([c80b8ad](https://github.com/aGallea/homerun/commit/c80b8adf8d0214ccc9dc0dd86045ec26391435cc))
* logout on bad credentials and add bash shell to coverage workflow ([5f1ddef](https://github.com/aGallea/homerun/commit/5f1ddef76931e6e6b772ea89ed780ccca5d17858))
* make all shutdown tests resilient to daemon service state ([13e415f](https://github.com/aGallea/homerun/commit/13e415fb1337521ef865df010beef6d48df1a06c))
* make CI workflow cross-platform for Windows self-hosted runner ([#112](https://github.com/aGallea/homerun/issues/112)) ([b71c957](https://github.com/aGallea/homerun/commit/b71c957abd37645b0aef13bcbd33f750dd6378ea))
* make shutdown test resilient to daemon service state ([f6f0d47](https://github.com/aGallea/homerun/commit/f6f0d47bb29df23d278548212e2062cbbd8bfc68))
* make shutdown test resilient to daemon service state ([3809747](https://github.com/aGallea/homerun/commit/3809747495cfaa0f00c94e6c7dacc8f1c057bac9))
* platform-aware shutdown error message ([#113](https://github.com/aGallea/homerun/issues/113)) ([df841a4](https://github.com/aGallea/homerun/commit/df841a4121b5c21327711e4a0f7891314fce097f))
* position tray panel above taskbar on Windows ([#112](https://github.com/aGallea/homerun/issues/112)) ([0c25876](https://github.com/aGallea/homerun/commit/0c258764778753142d614ea346d8933150c0f2ba))
* remove dtolnay/rust-toolchain from CI to avoid concurrent rustup conflicts ([5bc4bef](https://github.com/aGallea/homerun/commit/5bc4bef14960ce1a2742e9d5bfd4e1bbb850c896))
* remove invalid NSIS config from tauri.conf.json ([#112](https://github.com/aGallea/homerun/issues/112)) ([f42ffaa](https://github.com/aGallea/homerun/commit/f42ffaace8ec3d3f3b6007c8544adc0b1587bfca))
* remove jq dependency from coverage badge workflow ([adf552d](https://github.com/aGallea/homerun/commit/adf552d2858e9f4c3f9c73dd53fd38a9cc14927e))
* remove jq dependency from coverage badge workflow ([54a091d](https://github.com/aGallea/homerun/commit/54a091d5b052ab4b8e8bc391327c87befa79e116))
* remove unused CommandExt import in unix process group ([#113](https://github.com/aGallea/homerun/issues/113)) ([a276410](https://github.com/aGallea/homerun/commit/a27641067a769ceb6414f285a382c594a7242cad))
* resolve Windows compilation blockers in runner/mod.rs and test-utils ([#112](https://github.com/aGallea/homerun/issues/112)) ([a453984](https://github.com/aGallea/homerun/commit/a4539840580431d24bebaf68ddf4755101d35319))
* rewrite diag log tailing to use poll-based file reading ([83c72eb](https://github.com/aGallea/homerun/commit/83c72eb0d411c93f649e83591c8afc1a343c4368))
* skip setup-python on Windows self-hosted runner ([#113](https://github.com/aGallea/homerun/issues/113)) ([90410b9](https://github.com/aGallea/homerun/commit/90410b9349a0a1b697cb12fcfbc6b80985ac42fe))
* update pre-commit stage names for v4 compatibility ([#113](https://github.com/aGallea/homerun/issues/113)) ([b6b6417](https://github.com/aGallea/homerun/commit/b6b64179f24719e1dce0d0096ab7973427bcaefe))
* use existing Rust installation instead of dtolnay/rust-toolchain ([96b2709](https://github.com/aGallea/homerun/commit/96b2709eedea1d9ee4a6ff06a4df0506ebf17688))
* use platform-aware default runner labels ([#112](https://github.com/aGallea/homerun/issues/112)) ([36918de](https://github.com/aGallea/homerun/commit/36918de44cf434f379b82a1114a911b0c4d136ce))
* use Registry Run key instead of schtasks for Windows auto-start ([#112](https://github.com/aGallea/homerun/issues/112)) ([e239d61](https://github.com/aGallea/homerun/commit/e239d6117e029ba29bad8923f3bc1b5b413b2f5e))
* Windows compilation fixes and release workflow ([#112](https://github.com/aGallea/homerun/issues/112)) ([d5cfa59](https://github.com/aGallea/homerun/commit/d5cfa59daf04640d906caf27ce33711c5a697f5f))
* Windows launch-at-login, remove macOS-specific UI text ([#112](https://github.com/aGallea/homerun/issues/112)) ([cf42d68](https://github.com/aGallea/homerun/commit/cf42d684deb6df6f0b26217c8d40a7a87cc54241))

## [0.7.0](https://github.com/aGallea/homerun/compare/v0.6.0...v0.7.0) (2026-03-28)


### Features

* add --version flag to homerun and homerund binaries ([f59b128](https://github.com/aGallea/homerun/commit/f59b12841c07d9b8a5551e86194e560ea5fbea7b))
* add About section to desktop app and CLI about command ([0515987](https://github.com/aGallea/homerun/commit/0515987f1520ff6dcaec552b360d2f5bc401d9ff)), closes [#88](https://github.com/aGallea/homerun/issues/88)
* add Hide View option to tray menu ([0fd2e7e](https://github.com/aGallea/homerun/commit/0fd2e7edec29cd672371dbdce5a6df3b76f1289b))
* add Homebrew tap support and update README ([b2a0347](https://github.com/aGallea/homerun/commit/b2a03478340e77ca0b29e8020c0edf459c9e2ddc))
* add macOS native notifications for runner status changes and job completions ([b35c52f](https://github.com/aGallea/homerun/commit/b35c52fc1edcd58638fc8edf5c511c90e2842cfa))
* macOS native notifications and tray Hide View ([2f3705f](https://github.com/aGallea/homerun/commit/2f3705f81d4322c64852330f8922d2131106b4ee))
* notify when a runner is deleted ([280bfa3](https://github.com/aGallea/homerun/commit/280bfa3e801e7f521f2731665bcdd7afc54ee88f))


### Bug Fixes

* add FAQ for background items notification and improve CLI test coverage ([bb4e773](https://github.com/aGallea/homerun/commit/bb4e773fb9d0d12086991b5cca9ee475b64141a2))
* use monotonic job counter for completion notifications ([46f23d1](https://github.com/aGallea/homerun/commit/46f23d117c13e1fb827a403dce150e5ce86cd07b))
* use monotonic job counter for completion notifications ([72ea28e](https://github.com/aGallea/homerun/commit/72ea28e0afc1f9a010c14bcceeec5c64f59d6a34))

## [0.6.0](https://github.com/aGallea/homerun/compare/v0.5.2...v0.6.0) (2026-03-28)


### Features

* add matched_labels to TUI DiscoveredRepo type ([ef950b5](https://github.com/aGallea/homerun/commit/ef950b52c40e77c45a04225dd9fa714d69538a59))
* add merge_results to daemon scanner ([589d881](https://github.com/aGallea/homerun/commit/589d8810315a43d79ba0b032e5ac8ce4b9a8ff02))
* add React/TypeScript test coverage with Vitest ([54552e3](https://github.com/aGallea/homerun/commit/54552e31d486b117b60d761e7be6f81ef808f0da))
* add remote SSE scan stream with progress ([4372188](https://github.com/aGallea/homerun/commit/43721884c04dbcd2b48258e4013596748b2001d4))
* add Repository Scanning section to Settings page ([1cdf12b](https://github.com/aGallea/homerun/commit/1cdf12b96844c0b94526c082af6195b002f94a08))
* add scan button, filters, and enriched cards to Repositories page ([5a43595](https://github.com/aGallea/homerun/commit/5a4359583816e38727276c2540d4dfefeab83dbc))
* add scan progress display, view toggle, and persisted timestamp ([faf9218](https://github.com/aGallea/homerun/commit/faf92181f85ebd9212f5164275be1da037b398c6))
* add scan results persistence module ([dcbf650](https://github.com/aGallea/homerun/commit/dcbf650cc9c5f5238080ce2311ec5c8d65740b28))
* add scan types and commands to Tauri bridge ([ada9712](https://github.com/aGallea/homerun/commit/ada9712f146f4034b7ac053f270e3b184f0285c2))
* add scan_labels, workspace_path, auto_scan to Preferences ([7903767](https://github.com/aGallea/homerun/commit/7903767032253c2152b4ea717a04ea25727b42ea))
* add SSE scan stream endpoints with cancellation and persistence ([62fa958](https://github.com/aGallea/homerun/commit/62fa9586a61c3198175f55e6774a8c59241bd2ea))
* add stable job_number to history entries ([#61](https://github.com/aGallea/homerun/issues/61)) ([c1716d5](https://github.com/aGallea/homerun/commit/c1716d5ef5fe991e678507f7acb79889227d76f8))
* add Tauri bridge for SSE scan progress and persistence ([0855efb](https://github.com/aGallea/homerun/commit/0855efb78bcec9b0f1154098f603a1b8ee7ddfa1))
* add two-phase local scanning with progress callback ([28e01fa](https://github.com/aGallea/homerun/commit/28e01fa85ab37d1a7faaa07261c16148d3663e19))
* add useScan hook for scan state management ([67bbea0](https://github.com/aGallea/homerun/commit/67bbea0d24f742934b520397fed1c9b02c47fc0e))
* **desktop:** add /mini and /tray routes with stub components ([2651be3](https://github.com/aGallea/homerun/commit/2651be3d2ca39e200244d40d620a0bed2d212197))
* **desktop:** add compact mini-view and menu bar tray icon ([36cc311](https://github.com/aGallea/homerun/commit/36cc311ed34cb4ddedff7454b812859197606319))
* **desktop:** add custom macOS menu bar with About metadata and Help links ([62a743a](https://github.com/aGallea/homerun/commit/62a743ad3e632afc8be6cd4e49554fae52e07972))
* **desktop:** add mini window and main window toggle commands ([643f422](https://github.com/aGallea/homerun/commit/643f422e0d2402b2c2dcbacc3efdd50838932373))
* **desktop:** add re-run attempt badge to history and runner list ([#61](https://github.com/aGallea/homerun/issues/61)) ([b8b11cf](https://github.com/aGallea/homerun/commit/b8b11cfd0e3c1b77705f93506d364b3c39ae8a52))
* **desktop:** add resized tray icon assets ([7149007](https://github.com/aGallea/homerun/commit/7149007baa567d8259ca1094eb053326fd9f85a9))
* **desktop:** add Toggle Mini View to Window menu with Cmd+Shift+M ([64868c0](https://github.com/aGallea/homerun/commit/64868c0f3b3d37875f15db78facc798672639170))
* **desktop:** add tray-icon and positioner dependencies ([c4471bf](https://github.com/aGallea/homerun/commit/c4471bfd0f50b631c1f294c9c067bd9c30047f02))
* **desktop:** add update_tray_icon command ([81c7f61](https://github.com/aGallea/homerun/commit/81c7f613ea7bb5da15dcbad75abd80a8071bbfad))
* **desktop:** add useTrayIcon hook for automatic tray icon state updates ([2dfb2f5](https://github.com/aGallea/homerun/commit/2dfb2f505ceae9302de3b6055ad39c8d8692b729))
* **desktop:** custom macOS menu bar with About and Help ([ac7153b](https://github.com/aGallea/homerun/commit/ac7153b9d77d05df4750b68188ca35e3216e68c1))
* **desktop:** implement MiniView component with runner cards ([cb09baa](https://github.com/aGallea/homerun/commit/cb09baa2dcbf78cc051d47ed47d0bf288816524a))
* **desktop:** implement TrayPanel component with runner list and actions ([603b37e](https://github.com/aGallea/homerun/commit/603b37e400a1787c67cc5d25e39365f862e98222))
* **desktop:** initialize system tray with idle icon ([5fffef0](https://github.com/aGallea/homerun/commit/5fffef008e8bf1ea8853726631c8b3343f7b5991))
* enhance runner page with ID and last job details ([3b67f2b](https://github.com/aGallea/homerun/commit/3b67f2b602e722af433c230bd0b9872e02c8a4f2))
* fall back to group history for job duration estimate ([e28ca17](https://github.com/aGallea/homerun/commit/e28ca178533559d46251cc7437c4f7904fd59aff))
* fall back to group history for job duration estimate ([8e8a125](https://github.com/aGallea/homerun/commit/8e8a12521d99f7371b944cd4161b64e8a792776d)), closes [#53](https://github.com/aGallea/homerun/issues/53)
* **github:** return run_attempt and runner_name from job status ([#61](https://github.com/aGallea/homerun/issues/61)) ([019b87a](https://github.com/aGallea/homerun/commit/019b87af262ef750184b85e8dd20abb82418b6af))
* **poller:** add rerun detection poller that annotates history entries ([#61](https://github.com/aGallea/homerun/issues/61)) ([10320ab](https://github.com/aGallea/homerun/commit/10320ab0752b1f182ebab0bd04e2db5aca0faec3))
* refactor scanner to accept custom labels ([11c5bed](https://github.com/aGallea/homerun/commit/11c5bed297b5d99a70e003d3213c053afe27f539))
* rename list_self_hosted_workflows to list_workflows_with_labels ([54d79c5](https://github.com/aGallea/homerun/commit/54d79c51a811e1c1ea03ee220cbe654fd0d6f67f))
* restore runner labels UI, fix shutdown timeout, fix daemon log card layout ([96be8e8](https://github.com/aGallea/homerun/commit/96be8e87b6ea1d7b98eb0ad9638acb354e96ff69))
* rewrite useScan hook for event-driven scanning with persistence ([7aceb84](https://github.com/aGallea/homerun/commit/7aceb8413d951c9bdfe9578cc59174fef448a66d))
* scan endpoints accept optional labels override ([ab8085e](https://github.com/aGallea/homerun/commit/ab8085ec58fea736b77fa1b31f058ea1aa42ccf2))
* show last job info in runner list when idle ([#83](https://github.com/aGallea/homerun/issues/83)) ([035e477](https://github.com/aGallea/homerun/commit/035e4777a208adc2c2b97eea3f7c1a1f9d346a3a))
* show runner ID in detail page header ([#83](https://github.com/aGallea/homerun/issues/83)) ([8226e74](https://github.com/aGallea/homerun/commit/8226e74ac45b68b8cb6a7d93d663c7cf9e7f2480))
* smart repo discovery with custom label scanning ([5570e63](https://github.com/aGallea/homerun/commit/5570e638a092f83d1facde0a2a0a725ce471b08a))
* **test-utils:** create test-utils crate with MockDaemon skeleton ([f5ee20f](https://github.com/aGallea/homerun/commit/f5ee20fd1b47380f034bff01b11ab71ee4d7cf0e))
* **test-utils:** implement mock daemon route handlers ([8fd904d](https://github.com/aGallea/homerun/commit/8fd904d692150f1a6be0433464322519b3563d59))
* **tui:** add context-sensitive key_hints method to App ([b837a5e](https://github.com/aGallea/homerun/commit/b837a5e986d7271c4eff2efa4f12bbdfbc3db7b2))
* **tui:** add device flow client methods for login ([5030161](https://github.com/aGallea/homerun/commit/5030161fadbc24eb7040154d7d145e80ddaa1299))
* **tui:** add header widget with info bar and key grid ([4dc8f47](https://github.com/aGallea/homerun/commit/4dc8f47fd9036952516c40a81c400d42f7b5993b))
* **tui:** add job progress bar and duration to job history ([daf7618](https://github.com/aGallea/homerun/commit/daf76182a6aa501e35c9ce56a6b0d4da23fefded))
* **tui:** add login popup overlay for device flow ([085785f](https://github.com/aGallea/homerun/commit/085785f02afe6cc37121d398ddd13b1a97483318))
* **tui:** add LoginState, L key handler, and login key hints ([88c9e0b](https://github.com/aGallea/homerun/commit/88c9e0b36f8b6cc95f5f3f1be7edd7709b23937a))
* **tui:** add search/filter to Repos tab ([6a02698](https://github.com/aGallea/homerun/commit/6a02698d83088fe8bd4ac685d30e952cde865a13))
* **tui:** k9s-inspired layout redesign with login, job history, and test suite ([fe87ce6](https://github.com/aGallea/homerun/commit/fe87ce6583868d8e1251b758a1bcf2743fffccf2))
* **tui:** k9s-inspired layout with bordered frame, header, and key grid ([e04fcb3](https://github.com/aGallea/homerun/commit/e04fcb307c1af48097dc16daaa182864a576a1d4))
* **tui:** show job history in runner detail panel ([851e63b](https://github.com/aGallea/homerun/commit/851e63b7db21195b98bc6c2b0bb35bdfe2fb319e))
* **tui:** wire up device flow login in main event loop ([e3a9c98](https://github.com/aGallea/homerun/commit/e3a9c9833b08c399945a929f8684938e9a5bd2bf))
* **types:** add RunAttempt struct and latest_attempt field ([#61](https://github.com/aGallea/homerun/issues/61)) ([4931f7e](https://github.com/aGallea/homerun/commit/4931f7e6d37f5f3c9468c739d246b5d34828997c))
* **types:** add RunAttempt struct and latest_attempt field ([#61](https://github.com/aGallea/homerun/issues/61)) ([c7afaeb](https://github.com/aGallea/homerun/commit/c7afaeb823b2b084c83915eb60e2fcd380964d7c))


### Bug Fixes

* auto-collapse sidebar when window width drops below 900px ([df33fc2](https://github.com/aGallea/homerun/commit/df33fc2fcd3acda2ab3d339e1c79877c22975bc5))
* backfill job_number on existing history entries at startup ([#61](https://github.com/aGallea/homerun/issues/61)) ([3f3442a](https://github.com/aGallea/homerun/commit/3f3442acb0f2933b1e7404d942607c3a0f3ade59))
* cap status column width so action buttons stay visible ([f4d18c0](https://github.com/aGallea/homerun/commit/f4d18c0d6d2a27aae94b55df3f830df7f1ca0c28))
* **ci:** prefix React lcov paths with apps/desktop/ for coverage report ([0b8d943](https://github.com/aGallea/homerun/commit/0b8d9430011e27adf599881cef6a335ee01ed9a1))
* **desktop:** add missing changes from compact-mini-view ([78d6ca2](https://github.com/aGallea/homerun/commit/78d6ca29ed97d79a71ba146aee1c41a7a525f8d3))
* **desktop:** add missing changes from compact-mini-view feature ([d179808](https://github.com/aGallea/homerun/commit/d1798082f06f70a1445506c52883486575033d38))
* **desktop:** address code review issues for mini-view and tray ([f6eb7bb](https://github.com/aGallea/homerun/commit/f6eb7bbfb3f506fea7480345454aedc2ed3035a3))
* **desktop:** dynamically resize mini window to fit content ([3c6f14f](https://github.com/aGallea/homerun/commit/3c6f14f3c8affb04cd83a7b1e181e7ff18ef8820))
* **desktop:** dynamically resize tray panel to fit content ([ebf6759](https://github.com/aGallea/homerun/commit/ebf67597b570e54a0045d101f115333a6179363a))
* **desktop:** enable transparent windows for tray panel and mini view ([c0da74b](https://github.com/aGallea/homerun/commit/c0da74b4636b9816307114c436507e9b78cde21d))
* **desktop:** feed tray events to positioner plugin to prevent crash ([3eecd32](https://github.com/aGallea/homerun/commit/3eecd32b1ba9446bf2c189794388fe0e6931eca8))
* **desktop:** fix blurry sidebar logo by removing scale transform ([b3a00b2](https://github.com/aGallea/homerun/commit/b3a00b2e54a94823f3295669195609239bee23d7))
* **desktop:** replace positioner plugin with direct tray position ([784a0b1](https://github.com/aGallea/homerun/commit/784a0b14ab50150fcdc13dca6cbad3cfeddbdbdf))
* **desktop:** resize mini/tray windows on any runner state change ([66e3e7d](https://github.com/aGallea/homerun/commit/66e3e7dedd3ac8b6e02cf8d1d378c77ab686414d))
* job history and last run status not updated on workflow re-run ([#61](https://github.com/aGallea/homerun/issues/61)) ([f80cf8e](https://github.com/aGallea/homerun/commit/f80cf8ecada43dfb2419cc741833d65ef82710b8))
* make runner row columns shrinkable for narrow windows ([9e55245](https://github.com/aGallea/homerun/commit/9e5524593886c7d80b838de16a80200d85e4b553))
* make runner row job name responsive to available width ([76e3847](https://github.com/aGallea/homerun/commit/76e384709d94542cc875ab5bea4f51559c313285))
* preserve previous attempt info on same-runner re-runs ([#61](https://github.com/aGallea/homerun/issues/61)) ([cec65fa](https://github.com/aGallea/homerun/commit/cec65fa6731e7b7233a29f4bfb2e689251c27e33))
* protect action buttons from shrinking at narrow widths ([b2042f0](https://github.com/aGallea/homerun/commit/b2042f0106d0358bd145fa265eb51424c7759b02))
* remove unused variable in useAuth test ([f4f2d6a](https://github.com/aGallea/homerun/commit/f4f2d6ae0837ce76e199a1058bd4dba3b7832193))
* responsive runner row layout — buttons always visible at all widths ([e93e647](https://github.com/aGallea/homerun/commit/e93e6475be8c526b7e6f3d720d8911d0f6dae3c3))
* show job name instead of branch in last job summary ([#83](https://github.com/aGallea/homerun/issues/83)) ([054bea4](https://github.com/aGallea/homerun/commit/054bea457b9c54ef10346da88c13c6059ed8d16b))
* **tauri:** add RunAttempt and latest_attempt to Tauri IPC types ([#61](https://github.com/aGallea/homerun/issues/61)) ([112f736](https://github.com/aGallea/homerun/commit/112f736d05aaae7cfca19bbfced9b441c2fa2b9d))
* **test:** resolve flaky colored test and React act() warnings ([1cccfc2](https://github.com/aGallea/homerun/commit/1cccfc2801a5d26faed877dd5fce2f67d1c5f9d1))
* **test:** resolve flaky colored test and React act() warnings ([1279e23](https://github.com/aGallea/homerun/commit/1279e23a00156a95f47f24bf82bc8793c698ba1d))
* **test:** use tempdir in keychain test to avoid CI race condition ([a6f3e10](https://github.com/aGallea/homerun/commit/a6f3e10f215cab80be19d1c9c56690152fd55d03))
* truncate busy status text so action buttons stay visible ([1d1c846](https://github.com/aGallea/homerun/commit/1d1c8462b1dc3d54b94514c4e683c9cde18bb4c9))
* **tui:** account for indicator line in step skip calculation ([408deb7](https://github.com/aGallea/homerun/commit/408deb73a12ad7d1ca5bc8c634eeaf0af4117057))
* **tui:** ensure clean process exit on quit ([6c5f2ff](https://github.com/aGallea/homerun/commit/6c5f2ffdb126d4a08322b0d7b1d3d666eb169e4b))
* **tui:** show progress bar above steps, show latest steps first ([54b4b24](https://github.com/aGallea/homerun/commit/54b4b247f18c6aa6a1273ccc5245374b336d1899))
* **tui:** switch tab navigation from 1-4 to Ctrl+1-4 ([9e48918](https://github.com/aGallea/homerun/commit/9e48918609e7af851d6a27f95e7dea451485b452))
* **tui:** use F1-F4 for tab switching instead of Ctrl+1-4 ([63a23ed](https://github.com/aGallea/homerun/commit/63a23ed5edaf74bc631d28105cf28717ea5847cb))
* **tui:** use percentage-based layout for runner detail panels ([6c9c094](https://github.com/aGallea/homerun/commit/6c9c0945d52ddcd4cb65af64f93d29cd10521bc3))

## [0.5.2](https://github.com/aGallea/homerun/compare/v0.5.1...v0.5.2) (2026-03-26)


### Bug Fixes

* auto-uninstall launchd service when stopping daemon from app ([91d7cba](https://github.com/aGallea/homerun/commit/91d7cba467bd7a8d1b49b6df982055de107e92a0))
* **ci:** use python3 -m pre_commit to avoid PATH issues ([e1f7ca5](https://github.com/aGallea/homerun/commit/e1f7ca55078f13ab0c510de7806916f882bfe05e))
* improve desktop app UX and daemon reliability ([8338601](https://github.com/aGallea/homerun/commit/8338601e2ab3a12d345c32ccd8df7599c6ed8bb5))
* resolve user's shell PATH for runner processes ([b5a7e44](https://github.com/aGallea/homerun/commit/b5a7e440558cb5c5d2dabbba69e119c8caf198e8))
* resolve user's shell PATH in launchd plist ([1764c31](https://github.com/aGallea/homerun/commit/1764c313b1d8b52f77bb9e1b0bce5a6c2b712344))

## [0.5.1](https://github.com/aGallea/homerun/compare/v0.5.0...v0.5.1) (2026-03-25)


### Bug Fixes

* add PATH env to launchd plist and deduplicate re-run history ent… ([9b2d735](https://github.com/aGallea/homerun/commit/9b2d735b1e15435e26626349b955c765af2af9ea))
* add PATH env to launchd plist and deduplicate re-run history entries ([d4c82bc](https://github.com/aGallea/homerun/commit/d4c82bcc940bf586303425039f230451b95290a7))

## [0.5.0](https://github.com/aGallea/homerun/compare/v0.4.0...v0.5.0) (2026-03-25)


### Features

* add ActiveRunners component for sidebar ([#71](https://github.com/aGallea/homerun/issues/71)) ([d4de51d](https://github.com/aGallea/homerun/commit/d4de51dfd9709323bc3d0186bee63136e781d9dd))
* add formatElapsed utility for sidebar active runners ([#71](https://github.com/aGallea/homerun/issues/71)) ([fc658a1](https://github.com/aGallea/homerun/commit/fc658a19f7ad792ca4d4fe79aeec3c452180077e))
* add job progress bar to sidebar runner entries ([#71](https://github.com/aGallea/homerun/issues/71)) ([6a4e217](https://github.com/aGallea/homerun/commit/6a4e217cb29f28ad7d3a01fc550fd6e37ce6edb4))
* show active/busy runners in sidebar ([#71](https://github.com/aGallea/homerun/issues/71)) ([21a8716](https://github.com/aGallea/homerun/commit/21a8716b7d4e1a8e23eac1b716e3925ca1707772))
* wire ActiveRunners into sidebar via shared useRunners context ([#71](https://github.com/aGallea/homerun/issues/71)) ([a560861](https://github.com/aGallea/homerun/commit/a5608618fb94ee3a5f07ff42416d1c26f17ffa91))


### Bug Fixes

* Runners tab active state by pointing to /dashboard route ([#71](https://github.com/aGallea/homerun/issues/71)) ([7a07452](https://github.com/aGallea/homerun/commit/7a074525007adba720756ae46c93f944c1819472))
* sort runners with null job_started_at last in sidebar ([#71](https://github.com/aGallea/homerun/issues/71)) ([17aa7db](https://github.com/aGallea/homerun/commit/17aa7dbd5ba5f76b6d5050378eb4bcd1c7e383ba))

## [0.4.0](https://github.com/aGallea/homerun/compare/v0.3.2...v0.4.0) (2026-03-25)


### Features

* add copy-to-clipboard button for device flow auth code ([176dba7](https://github.com/aGallea/homerun/commit/176dba75061c6e2c9081e58d60126b622861018f))
* add run status endpoint and fix parse_run_id for job URLs ([edf7bee](https://github.com/aGallea/homerun/commit/edf7beeb0e85122c2e567880288ab8e06ead4e96))


### Bug Fixes

* improve resize handle visibility and rerun job feedback ([e06fab7](https://github.com/aGallea/homerun/commit/e06fab75670659f2a88a8927435cbc574f00617c))
* runner restart with migrated config, scale-up count, and delete UX ([78be891](https://github.com/aGallea/homerun/commit/78be8916d2b7ed2be5bd6625d179a9887be9a415))
* runner restart with migrated config, scale-up count, and delete UX ([3b5583e](https://github.com/aGallea/homerun/commit/3b5583e4ef2b04e2de07510ba0d64447a73288cf))
* use job_id directly for annotation fetch ([63825f5](https://github.com/aGallea/homerun/commit/63825f59a4f3e1cc8d6fa0dccf8c675fbd6db1e0))
* use job_id directly for annotation fetch to avoid wrong-run matching ([263e0ac](https://github.com/aGallea/homerun/commit/263e0ac8a2ac25163e1588464d6ae9bb304b008b))

## [0.3.2](https://github.com/aGallea/homerun/compare/v0.3.1...v0.3.2) (2026-03-25)


### Bug Fixes

* add Cancelled variant to StepStatus ([#52](https://github.com/aGallea/homerun/issues/52)) ([0da3629](https://github.com/aGallea/homerun/commit/0da3629de25eeaa5e9a8d9b5ffe24639eb47861c))
* allow multiple job history entries to be expanded simultaneously ([32083c5](https://github.com/aGallea/homerun/commit/32083c5a8da7767e150c429e75e3767e73c66810))
* defer Worker log discovery to poll() for reliable step tracking ([#52](https://github.com/aGallea/homerun/issues/52)) ([dc320f6](https://github.com/aGallea/homerun/commit/dc320f657c04da7733217da88dff21f4a7dd069b))
* resolve missing job steps in dashboard ([#52](https://github.com/aGallea/homerun/issues/52)) ([f380d56](https://github.com/aGallea/homerun/commit/f380d560603a5f95f331cd90f0b0f2bbe0071b4a))
* UI improvements and icon regeneration ([9b21698](https://github.com/aGallea/homerun/commit/9b21698d4b2eb0293951c7005ee85c8df7fe042b))

## [0.3.1](https://github.com/aGallea/homerun/compare/v0.3.0...v0.3.1) (2026-03-24)


### Bug Fixes

* add shell:allow-execute permission for sidecar spawning ([9aa08d5](https://github.com/aGallea/homerun/commit/9aa08d56e74ad7c97c807ac7c6a1454c23d74227))
* add shell:allow-execute permission for sidecar spawning ([069bd0e](https://github.com/aGallea/homerun/commit/069bd0ed4159af4e086bb0db22a3675bc56a8c92))
* correct sidecar name and add shell:allow-spawn permission ([14de160](https://github.com/aGallea/homerun/commit/14de1600ec801f9c8cea16ae73acb6d4dfd9dd3f))

## [0.3.0](https://github.com/aGallea/homerun/compare/v0.2.3...v0.3.0) (2026-03-24)


### Features

* add daemon control buttons to desktop UI and error banner ([7aa14f0](https://github.com/aGallea/homerun/commit/7aa14f04a703d453fb8d89c04e2f1b1cd540811c))
* add daemon startup guard to prevent duplicate instances ([d785006](https://github.com/aGallea/homerun/commit/d785006efe104e9fc6d6dd76ecb8f6e006d14325))
* add daemon_lifecycle module for start/stop/restart ([bd6e689](https://github.com/aGallea/homerun/commit/bd6e689c17d49b49b770a17207d9e8042d12e2f5))
* add homerun daemon start|stop|restart CLI commands ([540abbe](https://github.com/aGallea/homerun/commit/540abbe36fa68a28656f6073f4ff0b7c80746e85))
* add POST /daemon/shutdown endpoint with graceful teardown ([8fec156](https://github.com/aGallea/homerun/commit/8fec156f54771cfaeb85e98b0211dcd80ffd33df))
* add SIGTERM/SIGINT signal handling for graceful shutdown ([4a82a08](https://github.com/aGallea/homerun/commit/4a82a08485145c24bfdafa08aa0e2c37ddb30ac7))
* add Tauri IPC commands for daemon start/stop/restart ([30f97da](https://github.com/aGallea/homerun/commit/30f97da6f403f9b8e17c5289d20dab801d45691f))
* add TUI disconnected mode and daemon start/stop/restart keybindings ([f598fe6](https://github.com/aGallea/homerun/commit/f598fe6b0f95ddd9aa6d91a8826fa381e4c38d04))
* auto-start daemon sidecar on Tauri app launch ([8476303](https://github.com/aGallea/homerun/commit/8476303c56af6d58b912e06042f72fcfbaa7928f))
* daemon lifecycle controls (start/stop/restart) ([4e8e38d](https://github.com/aGallea/homerun/commit/4e8e38d473bd80192e79f33f3945e2dac9581e44))
* include PID in /health response ([34ff210](https://github.com/aGallea/homerun/commit/34ff210fe32dd036e6ae983f93632f5befb4eb8a))

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
