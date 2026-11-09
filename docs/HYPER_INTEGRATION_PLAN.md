# Hyper Integration Plan

## Comparison result

The supplied comparison URL is [`mryuvraj1/launcher`](https://github.com/mryuvraj1/launcher). Its remote `HEAD` resolved to `f72ea35dc679bcc3a3f8b585a6add45acc99c399`; the current checkout is the same commit, dated 2026-11-08, titled `Release 2.3.0`. Matching Git commit IDs mean both references identify the same committed tree. The fetched comparison excerpts also matched local paths and implementations in `engine/game/install.rs`, `launch.rs`, `java.rs`, `pipeline`, and launch snapshots.

This target is therefore not an independent implementation from which to transplant Minecraft fixes at the audited revision. No external code was copied, no dependency was added, and no third-party license notice needed updating.

## Existing compatible components already present

The compared tree already has the same capabilities as this checkout: Java runtime discovery/download and structural validation; hash/size-checked game downloads; loader installation; launch argument planning and snapshots; log/crash evidence collection; Quick Play; server-list NBT preservation; Microsoft/Millida authentication; and SRV-aware server status probing. These should be treated as existing Millida Launcher behavior, not new Hyper integrations.

The comparison surfaced no distinct upstream fix at the shared HEAD. In particular, both copies have the same limitation where server status ping selects one SRV target and one resolved socket rather than retrying alternate endpoints. It is an appropriate small standalone improvement, but it is not an integration from this repository.

## Safe follow-up comparison procedure

1. Re-check the comparison remote's commit ID when an integration is requested. Compare only commits or files that differ from the shared base; do not infer improvements from repository branding or duplicated source.
2. For each changed component, identify its direct dependencies, IPC/API contracts, platform assumptions, tests, and data/license obligations before proposing adoption.
3. Prefer a narrow port of a demonstrably useful behavior with a local regression test. Preserve the current engine boundaries and existing GPL-3.0-only and third-party notices; do not replace the launcher or copy a subsystem wholesale.
4. Run targeted Rust tests, full Rust tests/clippy, frontend tests/build gates, and relevant platform builds before claiming compatibility. These gates were not all available in this audit environment.

## Current recommendation

**No Hyper code integration is recommended at this revision because the comparison target and checkout are the same Git commit.** Keep the multiplayer ping endpoint-retry issue as an independent local fix candidate, subject to the focused tests described in [`MULTIPLAYER_AUDIT.md`](MULTIPLAYER_AUDIT.md). Revisit the comparison only when the remote has distinct commits or the user identifies a specific feature/fix to evaluate.