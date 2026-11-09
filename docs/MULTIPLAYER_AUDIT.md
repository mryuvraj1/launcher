# Multiplayer Audit

## Scope

Audited server address storage and parsing, ping/SRV resolution, Quick Play arguments, account selection and game-server tracking at checkout `f72ea35`. This audit did not connect a real account to a real server. No source changes were made.

## Connection ownership and flow

The launcher does not implement Minecraft's login or play-session TCP protocol. It starts the Mojang/loader client and, for Quick Play, supplies the server address as game arguments. Minecraft itself performs endpoint resolution, TCP, protocol negotiation, mod compatibility checks and online-mode authentication. Server hops after startup are observed from client log lines by `engine/game/gameserver.rs::hop_from_log`; that code does not establish or control the connection.

Quick Play is built in `engine/game/launch.rs::server_join_args`: versions with Quick Play support use `--quickPlayMultiplayer <address>`; older clients receive `--server <host> --port <port>`. `engine/game/worlds.rs::quick_play` validates command-line text before launch. The frontend chooses account mode in `src/lib/launch.ts::resolveAuth`; an expired Microsoft session offers reauthentication or an explicitly offline launch. The core resolves the selected account session. A Millida/Yggdrasil session needs authlib-injector metadata and agent preparation in `pipeline/mod.rs::skin_server`; failure there deliberately falls back to offline auth with a warning.

The launcher's separate status check runs through `commands/launch.rs::ping_server` → `engine/game/ping.rs::resolve_srv` → `ping`. It sends a Server List Ping handshake/status request, applies timeouts, rejects local/private targets, and extracts status JSON. This is a status probe, not the game client's multiplayer login.

## Confirmed finding

**P2: A server can be shown offline when an alternate DNS/SRV endpoint is reachable.** `resolve_srv` chooses one record with the lowest priority and returns only that endpoint; it does not try other records if that target fails. `ping` then takes only the first result from `ToSocketAddrs` and makes one TCP attempt. For a hostname with multiple A/AAAA addresses, or an SRV set with multiple usable targets, the probe can fail on its selected endpoint even though another advertised endpoint would answer. This can mislead players diagnosing connectivity, but it does not cause a normal or Quick Play join: the ping command is not called by `quick_play`, and the game client resolves/connects independently.

The existing ping tests cover which inputs trigger an SRV name and refusal of local targets. They do not cover multiple SRV records, multiple A/AAAA answers, endpoint retry, protocol response validation, or a status response from a live server. No runtime reproduction was possible here.

## Address parsing and persistence

- `ping.rs::split_host_port` handles bracketed IPv6 and bare IPv6; invalid or absent ports fall back to 25565. `srv_name` only requests SRV for an eligible bare hostname.
- `launch.rs::split_server_addr` handles host/port, bracketed IPv6, bare IPv6 and defaults malformed/zero ports to 25565. `server_join_args` tests cover Quick Play version cutover and IPv4/IPv6-style inputs.
- `worlds.rs::pin_server_dat` and `serversdat.rs` preserve existing in-game server entries and unknown NBT tags when pinning Quick Play targets. `serversdat.rs` tests include round trips, unknown tags and byte-tag entries.
- `gameserver.rs::hop_from_log` records a server only when a recognized `Connecting to host, port` client log appears; `canon_addr` normalizes case and the default port. Tests cover known log formats and invalid lookalikes.

## Authentication and compatibility

Online-mode servers require credentials recognized by that server. A player choosing offline mode after Microsoft session expiry receives a clear warning that online servers will reject the session; this is expected behavior, not a launcher defect. Servers requiring Millida authentication require a Millida account/session. Modded servers can additionally reject incompatible game versions, loaders, mod sets, or server-side allowlist requirements. These server decisions are outside the launcher's status ping and cannot be diagnosed from source alone.

The ping request uses a status handshake and parses a 1.7+ status JSON response, but the client-to-server login/protocol exchange is delegated to the selected Minecraft version. No protocol compatibility matrix or successful real login was verified.

## Tests and unknowns

- Source-reviewed tests: `ping.rs` (SRV eligibility and local-address refusal), `launch.rs` (Quick Play arguments), `serversdat.rs` (NBT read/write), and `gameserver.rs` (log tracking/address canonicalization).
- Rust tests could not run because `cargo` is absent; no live DNS, status ping, Microsoft/Xbox/Minecraft Services exchange, Yggdrasil authentication, or game-server join was performed.
- Without a failing server address, account type/session state, client version/mod list, and fresh `logs/latest.log` or launcher capture, the exact cause of a user's join failure is unverified. Common protocol/auth rejection categories above are possibilities, not findings about a specific incident.

## Recommended fix order

1. **P1 for diagnosis accuracy:** change ping resolution to retain endpoint candidates, honor SRV priority/weight policy, and make bounded attempts across candidates and resolved IPs. Add deterministic resolver/connector tests for fallback and all-failed cases. Keep this isolated from the Minecraft launch route.
2. **P1 for root-cause support:** when a join failure is reported, inspect the fresh client log and server-side rejection text before changing address, auth, or protocol behavior.
3. **P2:** run the existing Rust parsing, launch-argument and server-list suites in CI and add protocol-response fixtures for malformed/unsupported status replies.