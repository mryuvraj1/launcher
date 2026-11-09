# Multiplayer Audit

## Scope

Audited server address storage and parsing, ping/SRV resolution, Quick Play arguments, account selection and game-server tracking at checkout `f72ea35`. This audit did not connect a real account to a real server. No source changes were made.

## Connection ownership and flow

The launcher does not implement Minecraft's login or play-session TCP protocol. It starts the Mojang/loader client and, for Quick Play, supplies the server address as game arguments. Minecraft itself performs endpoint resolution, TCP, protocol negotiation, mod compatibility checks and online-mode authentication. Server hops after startup are observed from client log lines by `engine/game/gameserver.rs::hop_from_log`; that code does not establish or control the connection.

Quick Play is built in `engine/game/launch.rs::server_join_args`: versions with Quick Play support use `--quickPlayMultiplayer <address>`; older clients receive `--server <host> --port <port>`. `engine/game/worlds.rs::quick_play` validates command-line text before launch. The frontend chooses account mode in `src/lib/launch.ts::resolveAuth`; an expired Microsoft session offers reauthentication or an explicitly offline launch. The core resolves the selected account session. A Millida/Yggdrasil session needs authlib-injector metadata and agent preparation in `pipeline/mod.rs::skin_server`; failure there deliberately falls back to offline auth with a warning.

The launcher's separate status check runs through `commands/launch.rs::ping_server` → `engine/game/ping.rs::resolve_srv` → `ping`. It sends a Server List Ping handshake/status request, applies timeouts, rejects local/private targets, and extracts status JSON. This is a status probe, not the game client's multiplayer login.

## Confirmed finding at the audited revision

**P2: A server can be shown offline when an alternate DNS/SRV endpoint is reachable.** `resolve_srv` chooses one record with the lowest priority and returns only that endpoint; it does not try other records if that target fails. `ping` then takes only the first result from `ToSocketAddrs` and makes one TCP attempt. For a hostname with multiple A/AAAA addresses, or an SRV set with multiple usable targets, the probe can fail on its selected endpoint even though another advertised endpoint would answer. This can mislead players diagnosing connectivity, but it does not cause a normal or Quick Play join: the ping command is not called by `quick_play`, and the game client resolves/connects independently.

This finding was confirmed by source inspection at `f72ea35`; it was not reproduced against a live server.

## Follow-up status

Part 2, commit [`768ea78`](https://github.com/mryuvraj1/launcher/commit/768ea78313317908b9037c8901f60d87966085a0), changed the status probe to retain up to four SRV targets in priority order, retry targets, and try up to two public A/AAAA answers per target. Local/private answers remain blocked. Tests were added for target ordering, endpoint retry, and skipping a local answer before trying a public one. The Rust tests could not run in the available Codespace, so runtime behavior remains unverified.

Part 3 adds `engine/game/server_addr.rs::parse_server_address`, used by Quick Play and ping. It rejects malformed, zero, and out-of-range explicit ports instead of silently turning them into port 25565; valid DNS names, IPv4, bracketed IPv6, and bare IPv6 retain their default/explicit ports. Tests cover valid and invalid forms, but they also could not run here. `launch.rs::server_join_args` relies on the validation at the `worlds.rs::quick_play` entry point.

No confirmed bug remains in the launcher's own server-status endpoint selection or explicit-port parsing based on the source evidence available. That statement does not establish that real multiplayer joins work: the launcher delegates the game login and play protocol to the Minecraft Java client, and no failing server/account/log set was supplied.

## Address parsing and persistence

- `server_addr.rs::parse_server_address` is shared by the ping and Quick Play paths. It distinguishes a bare IPv6 address from a bracketed IPv6 address with a port and rejects malformed explicit ports.
- `launch.rs::server_join_args` covers Quick Play version cutover and IPv4/IPv6-style inputs. Quick Play validates the address before installation/preparation, so an invalid port cannot silently launch against the default port.
- `worlds.rs::pin_server_dat` and `serversdat.rs` preserve existing in-game server entries and unknown NBT tags when pinning Quick Play targets. `serversdat.rs` tests include round trips, unknown tags and byte-tag entries.
- `gameserver.rs::hop_from_log` records a server only when a recognized `Connecting to host, port` client log appears; `canon_addr` normalizes case and the default port. Tests cover known log formats and invalid lookalikes.

## Authentication and compatibility

Online-mode servers require credentials recognized by that server. A player choosing offline mode after Microsoft session expiry receives a clear warning that online servers will reject the session; this is expected behavior, not a launcher defect. Servers requiring Millida authentication require a Millida account/session. Modded servers can additionally reject incompatible game versions, loaders, mod sets, or server-side allowlist requirements. These server decisions are outside the launcher's status ping and cannot be diagnosed from source alone.

Microsoft handling was reviewed in `src/lib/launch.ts::resolveAuth` and `engine/accounts/session.rs::ms_session_refresh` / `resolve_launch_auth`. The frontend requires a live Microsoft session for online play and makes offline fallback explicit. The core serializes refreshes per account and saves a rotated refresh token before completing the Xbox/Minecraft exchange. Existing tests distinguish revoked grants from transient failures. No authentication bypass or confirmed session-handling defect was found, and these paths were not changed.

The ping request uses a status handshake and parses a 1.7+ status JSON response, but the client-to-server login/protocol exchange is delegated to the selected Minecraft version. No protocol compatibility matrix or successful real login was verified.

## Tests and unknowns

- Tests present: `server_addr.rs` (valid/invalid address forms), `ping.rs` (SRV eligibility, endpoint ordering/fallback, local-address refusal and DNS-answer fallback), `launch.rs` (Quick Play arguments), `serversdat.rs` (NBT read/write), and `gameserver.rs` (log tracking/address canonicalization).
- Rust tests and rustfmt could not run because `cargo` and `rustfmt` are absent. No live DNS, status ping, Microsoft/Xbox/Minecraft Services exchange, Yggdrasil authentication, or game-server join was performed.
- Without a failing server address, account type/session state, client version/mod list, and fresh `logs/latest.log` or launcher capture, the exact cause of a user's join failure is unverified. Common protocol/auth rejection categories above are possibilities, not findings about a specific incident.
- The status probe's A/AAAA lookup still uses the system `ToSocketAddrs` resolver inside the existing blocking task; its duration is not explicitly bounded by this code. SRV lookup has a three-second timeout, and each TCP connect/read/write uses a four-second timeout. Candidate counts are capped, but no slow or failing DNS environment was tested.
- Equal-priority SRV records retain resolver order; the probe does not implement SRV weight-based ordering. This affects selection order, not the Minecraft client's server join.
- This code path launches Minecraft Java Edition through a JVM. No Bedrock launch path was found in the audited engine. The repository has Android icon assets, but no Android game-launch or server-connection test was run; Java/Bedrock support on an Android device is unverified.

## Recommended fix order

1. **P1 for root-cause support:** for a reported join failure, inspect the fresh client log and server-side rejection before changing account, protocol, loader, or network behavior.
2. **P1 verification:** run the new parser/ping tests plus existing launch-argument and server-list suites with Cargo in CI; test multiple DNS/SRV answers and a local test server in a network-enabled environment.
3. **P2:** consider an explicit timeout around system A/AAAA resolution and add protocol-response fixtures for malformed/unsupported status replies. Keep these changes on the status-probe path unless a real game log shows a client-side defect.
