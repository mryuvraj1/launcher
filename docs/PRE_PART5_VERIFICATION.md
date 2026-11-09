# Pre-Part-5 Verification

**Date:** 2026-11-09
**Branch:** `main`
**Verification result:** **PARTIALLY VERIFIED**. Part 5 has not started.

This is a source/history audit plus checks that the current Codespace can run. It is not a certification of a working Minecraft install, game launch, multiplayer join, website backend, or Windows/Android build. The Rust and Bun test suites could not run because their toolchains are absent.

## Part Status

| Phase | Status | Evidence and limits |
| --- | --- | --- |
| Part 1: engine/multiplayer audit | PARTIALLY VERIFIED | Reports were created and source-reviewed. The audit was not a live game or server test. Hyper's supplied repository matched the audited base commit at that time. |
| Part 2: install/launch engine | PARTIALLY VERIFIED | No confirmed install, Java, JVM-argument, graphics, or game-process bug was found to fix. The only code change was the status-ping fallback described in Part 3; no Minecraft installation or launch was executed. |
| Part 3: multiplayer | PARTIALLY VERIFIED | Status ping now tries bounded alternate SRV/DNS candidates; malformed explicit ports are rejected before Quick Play or ping. Regression tests exist but were not executable here. Actual game-server joins remain untested. |
| Part 4: account/Microsoft auth | PARTIALLY VERIFIED | Website device-code login and Microsoft device-code/Xbox/XSTS/Minecraft flows already existed and were inspected. A refresh-token rotation failure was hardened. The website backend is not in this repository, so service behavior and integration were not verified. |
| Part 4.5: verification | PARTIALLY VERIFIED | TypeScript and Vite checks pass; the startup-budget check fails. Rust, Bun, Tauri packaging, and device checks are blocked. |

The reviewed commits are `2949bbe` (audit documents), `768ea78` (status-ping endpoint retries), `d081d86` (server address validation), and `5f69977` (account refresh rotation). The pre-existing untracked `package-lock.json` was not modified.

### Phase Files Reviewed

- Part 1: `docs/GAME_ENGINE_AUDIT.md`, `docs/MULTIPLAYER_AUDIT.md`, `docs/HYPER_INTEGRATION_PLAN.md`.
- Part 2: `src-tauri/src/commands/launch.rs`, `src-tauri/src/engine/game/ping.rs`.
- Part 3: `docs/MULTIPLAYER_AUDIT.md`, `src-tauri/src/engine/game/server_addr.rs`, `src-tauri/src/engine/game/mod.rs`, `src-tauri/src/engine/game/ping.rs`, `src-tauri/src/engine/game/launch.rs`, `src-tauri/src/engine/game/worlds.rs`.
- Part 4: `docs/ACCOUNT_AUTH_INTEGRATION.md`, `src-tauri/src/engine/accounts/millida.rs`.
- Part 4.5 verification changes: `src-tauri/src/engine/accounts/millida.rs`, `docs/ACCOUNT_AUTH_INTEGRATION.md`, `docs/PRE_PART5_VERIFICATION.md`.

## Functionality Matrix

| Area | Status | Verification |
| --- | --- | --- |
| Minecraft metadata/install, libraries, assets, natives, Java runtime detection | NOT TESTED | Source and existing inline tests were reviewed. No Rust toolchain, game install, or Minecraft files were available for execution. |
| JVM args, classpath, game directories, loader launch, crash/log handling | NOT TESTED | Existing launch snapshot and crash tests were identified, but no JVM or Tauri app was run. |
| Server address parsing | PARTIALLY VERIFIED | Shared parser handles DNS names, ports, bracketed/bare IPv6 and rejects malformed/zero/out-of-range explicit ports. Unit tests exist but did not run. |
| Status ping DNS/SRV fallback | PARTIALLY VERIFIED | Code retries up to four SRV targets and two public addresses per target while rejecting local/private targets. Tests exist but did not run; no live probe was made. |
| Real multiplayer joins/protocol compatibility | NOT TESTED | The launcher passes Quick Play/legacy arguments to the Minecraft Java client; it does not implement the client login protocol. No server, account, version/mod list, or fresh game log was supplied. |
| Website login/session renewal | PARTIALLY VERIFIED | Existing device-code flow and Rust-vault token handling were reviewed. Mock Rust tests cover refresh rotation, but no website backend or real account was available. |
| Website logout/account linking | BLOCKED | Logout clears the local vault; no server revocation or website-to-Microsoft linking route is referenced by this client. The website backend/API specification is absent. |
| Microsoft sign-in/ownership | PARTIALLY VERIFIED | Official device-code OAuth, Xbox/XSTS, Minecraft Services and profile gating are present. No live OAuth or entitlement test was performed. The code uses `/minecraft/profile` as its Java-profile gate and does not separately call `/entitlements/mcstore`. |
| Bedrock/Android | NOT TESTED | The inspected game path launches Java Edition through a JVM. No Bedrock launch path or Android game/server run was verified; Android icon assets alone do not prove runtime support. |

## Automated Checks

| Check | Status | Result |
| --- | --- | --- |
| `./node_modules/.bin/tsc --noEmit` | VERIFIED | Passed. |
| `./node_modules/.bin/vite build` | VERIFIED | Passed; Vite transformed 983 modules and produced the frontend bundle. |
| `node scripts/check-startup-budget.mjs` | FAILED | Reproduced after the build: entry JS 378 KB / 350 KB; startup JS 862 KB / 833 KB; gzip 321 KB / 316 KB; CSS 408 KB / 415 KB. The phase commits changed no frontend files. |
| `git diff --check` | VERIFIED | Passed for the current uncommitted verification changes. |
| `cargo test --manifest-path src-tauri/Cargo.toml` | BLOCKED | `cargo` is unavailable. No Rust unit or integration tests ran. |
| `cargo clippy --all-targets -- -D warnings` | BLOCKED | `cargo` is unavailable. |
| `rustfmt --check` | BLOCKED | `rustfmt` is unavailable. |
| `bun run test` / frontend gate | BLOCKED | `bun` is unavailable; `scripts/test.mjs` and the frontend gate use Bun APIs. |
| `cargo deny` / `cargo audit` | BLOCKED | The Cargo toolchain and security tools are unavailable. Dependency/license notices were not regenerated. |

Editor diagnostics reported no errors in the changed Rust/documentation files, but that is not a substitute for Rust compilation. No automated test result is claimed for the new Rust cases.

## Findings and Remaining Risks

### Critical

No confirmed critical defect was identified in the reviewed source. Real installation, launch, multiplayer, and authentication outcomes remain unverified, so this is not a statement that those workflows work in production.

### Major

1. **Startup budget gate fails.** Component: `scripts/check-startup-budget.mjs` and the existing frontend bundle. Root cause: measured entry/startup JS exceeds configured limits. Reproduction: run `vite build`, then `node scripts/check-startup-budget.mjs`. Recommended fix: inspect startup imports and split/defer non-critical code, or have the owner approve a justified budget change. Priority: **P1 before a release/Part 5 baseline**. No UI change was made during this audit.
2. **Server-side logout revocation is absent from the client contract.** Component: `engine/accounts/millida.rs::millida_sign_out`. Root cause: sign-out erases local vault keys but makes no API request to revoke the server refresh token. Reproduction: inspect the function; it only calls local session deletion. Recommended fix: obtain a documented backend revocation contract, then integrate and test it; no endpoint was invented. Priority: **P1 security/integration gap**.
3. **Persistent website/Microsoft account linking is not present in the client contract.** Components: `src/state/accounts.ts`, `src-tauri/src/commands/accounts.rs`, and `engine/accounts`. Root cause: Millida and Microsoft accounts are intentionally stored as separate local identities and no link/unlink API is referenced. Reproduction: add both account types; they remain separate rows with no server association. Recommended fix: backend must define an authenticated, consented linking flow that proves Microsoft ownership before the launcher can integrate it. Priority: **P1 if persistent linking is a product requirement; otherwise BLOCKED on backend scope**.
4. **No real engine test was possible.** Components: install/launch/ping/auth paths. Root cause: this Codespace lacks Cargo/Bun and no Minecraft account/server or Windows/Android device was supplied. Recommended fix: run the existing Rust suites and platform smoke tests in CI/device environments before claiming installation, launch, or joining works. Priority: **P1 verification gate**.

### Minor / Incomplete

1. **Status ping has bounded candidate coverage and unbounded system DNS duration.** Component: `engine/game/ping.rs`. Root cause: at most four SRV targets and two public A/AAAA answers per target are tried; `ToSocketAddrs` has no explicit timeout, and equal-priority SRV records are not weight-ordered. Reproduction: a mocked DNS set with a reachable target after the caps, or a stalled system resolver. Recommended fix: consider a total deadline and deterministic resolver tests; keep this separate from actual Minecraft joins. Priority: **P2**. Not reproduced against live DNS.
2. **No explicit Minecraft Store entitlement call.** Component: `engine/accounts/microsoft.rs::xbox_chain`. Root cause: the existing ownership gate requires a Minecraft profile response but does not call `/entitlements/mcstore`. The current code refuses a missing profile; no ownership bypass was found. Recommended fix: confirm Microsoft's current entitlement schema and Millida policy before adding a distinct check. Priority: **P2 verification**, not a confirmed bypass.
3. **Refresh recovery deliberately fails closed.** Component: `engine/accounts/millida.rs::refresh_at`. Root cause: after an unanswered or malformed successful response, the backend may have rotated the token; if the immediate recovery attempt also fails, the launcher clears the local session to avoid replaying a possibly spent token. Reproduction: covered by the new mock HTTP tests, which were not run here. Recommended fix: if false reauthentication during outages is observed, request a backend recovery/idempotency contract; do not retry an uncertain token beyond its rotation grace. Priority: **P2 reliability/security tradeoff**.

## Security and Platform Notes

- Website and Microsoft tokens are held by the Rust core; the webview receives account/session facts, not bearer tokens. Website tokens use the encrypted `secrets.rs` vault. The Microsoft client ID is public configuration, not a secret.
- The website auth API's server-side credential validation, token TTL, cookie flags, refresh replay policy, logout revocation, and account-linking behavior cannot be verified without the backend/OpenAPI contract.
- The OAuth implementation is Microsoft's official device-code flow. PKCE parameters are not used in that flow; no alternate OAuth flow was introduced.
- Minecraft Java Edition profile presence is checked before a licensed account is accepted. A separate entitlement endpoint is not called, and no live ownership edge case was tested.
- Current environment is Linux in Codespaces. Windows and Android builds/devices, real Tauri packaging, local Minecraft installation, game launch, and actual multiplayer servers were not tested.

## Part 5 Readiness

**Status: PARTIALLY VERIFIED; not ready to claim full pre-Part-5 verification.** The UI redesign has not started. Before treating this gate as complete, run Rust tests/clippy/format and auth tests in a Rust-enabled environment, resolve or explicitly accept the startup-budget failure, and obtain backend contracts for remote logout/account linking if those are required. Real Windows/game/server tests are still required for platform claims.