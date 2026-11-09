# Game Engine Audit

## Scope and method

Audited the current checkout at `f72ea35` (`Release 2.3.0`, 2026-11-08). This was a source and test-inventory audit, not a live Minecraft session. No implementation or UI changes were made.

## Architecture

The app is a Tauri 2 desktop launcher with a React 19/TypeScript webview and Rust core. The webview calls typed wrappers in [`src/ipc/commands.ts`](../src/ipc/commands.ts); Tauri command wrappers in [`src-tauri/src/commands`](../src-tauri/src/commands) delegate to [`src-tauri/src/engine`](../src-tauri/src/engine). The core owns filesystem access, processes, network requests and account secrets. `src-tauri/src/lib.rs` configures the app and registers IPC commands. Frontend stores/screens are under `src/state` and `src/screens`.

Relevant engine areas include `engine/core` (HTTP/download integrity, safe paths, game/data paths, jobs, archive handling), `engine/game` (Minecraft metadata, install, Java, launch, logs/crash handling, worlds and ping), `engine/accounts` (Microsoft and Millida sessions), `engine/content` (catalog and mod installation), and `engine/profiles` (instances/import/export). The more detailed repository map is in [`ARCHITECTURE.md`](../ARCHITECTURE.md).

The Minecraft launch route is `commands::launch_game` / `launch_profile` / `quick_play` → `engine/game/launch.rs::install_and_launch_into` → `engine/game/pipeline/mod.rs::launch`. The pipeline plans version requirements and arguments in `pipeline/plan.rs`, prepares metadata, Java, client, libraries, natives, assets and loader files in parallel through `pipeline/graph.rs`, then calls `launch.rs::start_game` and `watch_game`. Vanilla/Mojang files are described by `pipeline/plan.rs::version_plan`; loaders are handled in `install.rs`. The process starts in the instance directory, captures stdout/stderr, streams game log events, and gathers fresh launcher/game/crash/JVM-fatal logs for crash diagnosis.

## Installation and launch review

- Minecraft version metadata drives Java major selection (`install.rs::java_major_of`, `mcmeta.rs`). Java selection checks configured/system/managed runtimes in `java.rs`; managed archives are downloaded with vendor SHA-256 metadata, unpacked, and checked for executable, `jvm.cfg`, class files, and JVM native libraries (`java_usable`).
- The install plan downloads the client, platform-allowed libraries, native classifiers and assets. Mojang hashes and sizes are checked where supplied; loader metadata may lack hashes and has separate verification/fallback behavior. Native extraction is cached by a stamp derived from native artifact names and hashes. Old virtual asset indexes receive a real-name layout.
- Loader paths cover Fabric, Quilt, Legacy Fabric, Forge and NeoForge. Forge-family installers wait on Java, client and library preparation. Launch argument construction merges loader/version metadata, substitutes Mojang placeholders, adds heap/security/tuning options, filters user JVM settings, and selects Quick Play arguments by game version.
- Runtime logs and crash reports are scoped to the current launch where possible. The launcher includes specific diagnosis paths for JVM fatal logs, early exits, mod faults and graphics-start failures; a discrete-GPU fallback is applied only when its crash classifier matches.

### Confirmed bugs

No Minecraft installation, JVM argument, native extraction, asset layout, graphics, or game-process defect was confirmed from this source audit. The repository has focused tests for Java selection/integrity predicates, installer planning and loader handling, launch argument snapshots, early-exit/crash classification, and server-list NBT. Those tests could not be executed here because Rust tooling is unavailable. A live crash, black screen, or failed launch cannot be attributed to a root cause without the affected platform, profile/version/loader, Java selection, and fresh log evidence.

The confirmed server-status diagnostic limitation is documented in [`MULTIPLAYER_AUDIT.md`](MULTIPLAYER_AUDIT.md). It affects the launcher's server-list ping result, not the Minecraft client's actual connection attempt.

## Tests and build checks

- `./node_modules/.bin/tsc --noEmit`: passed.
- `./node_modules/.bin/vite build`: passed; Vite transformed 983 modules and generated the production frontend bundle.
- `bun run test`: not run; `bun` is not installed. The repository harness is `scripts/test.mjs` and discovers Bun tests under `src/**/*.test.ts(x)` and `scripts/**/*.test.mjs`.
- `cargo test --manifest-path src-tauri/Cargo.toml`: not run; `cargo` is not installed. Rust tests are primarily inline `#[cfg(test)]` modules, with launch argument reference data in `src-tauri/tests/data/launch`.
- Full Tauri packaging and live game launch were not run. The Codespace has no Rust toolchain available, and no Minecraft account/server session was provided.

Build/test entry points are declared in [`package.json`](../package.json), [`BUILDING.md`](../BUILDING.md), and `src-tauri/Cargo.toml`. Frontend release scripts add the frontend gate and startup-budget checks after the web build; those Bun-dependent gates were not run.

## Licenses

The project declares GPL-3.0-only in [`LICENSE`](../LICENSE), `package.json`, and `src-tauri/Cargo.toml`. [`THIRD-PARTY.md`](../THIRD-PARTY.md) records dependency and asset notices; [`deny.toml`](../deny.toml) blocks unlisted Rust dependency licenses and unknown dependency sources. The vendored `src/vendor/mine3d` is identified there as MIT, fonts as SIL OFL 1.1, and music/resources have their own notices. These files were reviewed but the Rust dependency/license check could not be regenerated without Cargo/cargo-deny. No dependency or license files were changed.

## Recommended next actions

1. **P1: Re-run Rust tests and clippy in a Rust-enabled CI/Codespace.** This is the lowest-cost way to detect regressions in the launcher’s existing install/launch safeguards; it is not evidence that a specific game defect exists.
2. **P1: Diagnose individual failures from fresh artifacts.** For each report, retain OS/architecture, Minecraft and loader versions, selected Java version/path, whether the process created a window, `logs/launcher-latest.log`, fresh `logs/latest.log`, crash report, and `hs_err_pid` file. Do not prescribe a fix until the failure reproduces or the log identifies its cause.
3. **P2: Address multi-endpoint status-ping behavior.** See the multiplayer audit; add deterministic mocked DNS/connect tests before changing endpoint selection.
4. **P2: Re-run `cargo deny ... check licenses` and Bun release gates in the normal build environment.** The current repository notices are present, but this audit could not independently regenerate the dependency inventory.