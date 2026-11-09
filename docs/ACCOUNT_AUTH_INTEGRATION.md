# Account and Microsoft Authentication

## Existing integration

The launcher already has separate Millida website-account and Microsoft gameplay sessions. Website registration and credential entry happen on the Millida website; the launcher starts a device-code flow and never asks for the website password. Microsoft login is a second, independent device-code flow. A Millida session is not evidence of Minecraft ownership, and selecting a Microsoft account does not sign the player into the Millida website.

### Website account

`src/state/login.ts::startWebLogin` calls the existing launcher API and opens the returned verification page. The site handles registration/login and confirmation. The launcher polls the device code through a Rust command; only the user summary returns to the webview.

The client relies on these existing API routes under `https://api.millida.net/v2`:

| Route | Client contract |
| --- | --- |
| `POST /auth/launcher/init` | Request body includes `clientName`. Response includes `deviceCode`, `userCode`, `verifyUrl`, `expiresInSec`, and `intervalSec`. The website owns account creation and confirmation. |
| `POST /auth/launcher/poll` | Rust sends `{ deviceCode }`. Responses are `pending` (optionally with an anonymous `login:<32 lowercase hex>` notification channel), `denied`, `expired`, or `ok`. On `ok`, the core requires `accessToken` and stores `refreshToken` when present; without it, the account can only remain signed in until its access token expires. The API must return a refresh token for the requested short-lived/rotating-session contract. |
| `POST /auth/refresh` | Rust sends `{ refreshToken }`; response JSON contains `accessToken` and the rotated refresh token is returned in the `rt2` `Set-Cookie` header. The new pair is stored together in the encrypted core vault. An HTTP 200 without a usable pair (missing/malformed access token or missing `rt2`) is retried once; unrecoverable rotation loss expires the local session rather than reusing a possibly spent token. |
| `GET /users/me` | Authenticated profile read used after website login. The core adds the bearer token. |

The login socket is only a wake-up hint to poll; it carries no token. The code flow continues to work by polling if the socket is unavailable. The webview API bridge restricts auth routes: only `/auth/launcher/init` is callable through the generic UI command; polling and refresh are Rust-only. Tokens are not put in URLs or localStorage. The webview gets only session-presence flags and account display data. Tokens are stored in `secrets.rs` encrypted storage.

The launcher currently implements **local logout**: `millida_logout` erases the website session from the local vault and the UI removes its Millida account row. No server-side refresh-token revocation route is referenced in this repository. Do not invent one in the client. A backend-provided revocation contract is needed if logout must invalidate stolen/other-device refresh tokens immediately.

### Microsoft gameplay account

The existing flow in `engine/accounts/microsoft.rs` uses Microsoft's official consumer device-code endpoints (`/oauth2/v2.0/devicecode` and `/oauth2/v2.0/token`), followed by Xbox Live, XSTS, Minecraft Services `login_with_xbox`, and `GET /minecraft/profile`. It requires a returned Minecraft profile before creating a licensed account row; a missing profile is an error, not an offline success. This is the current Java-profile gate; the code does not separately call `GET /entitlements/mcstore`. The Minecraft access token stays in the core vault and is only supplied to Minecraft/game-service operations.

The device-code grant is not a browser redirect flow and does not use PKCE parameters. It remains the appropriate existing native-app flow; no password is collected by the launcher. Cancellation stops the UI polling loop without converting the Millida website session into a Minecraft license. Microsoft refreshes are account-scoped and serialized; Xbox/Minecraft service outages do not erase a still-valid Microsoft refresh grant.

The UI account list keeps `millida` and `microsoft` rows separate. A Millida website sign-in does not set Microsoft tokens or satisfy the Minecraft profile check. Microsoft account switching selects another account ID and its separately stored core session.

## Server-side contracts not present here

This repository contains the launcher client, not the Millida website/API implementation. The routes above and their response fields are the contract the current client consumes; the client cannot independently verify the server's credential validation, access-token lifetime, refresh-token entropy/reuse policy, or cookie flags.

No website-to-Microsoft account-linking route or proof-of-ownership contract is referenced in the launcher. If the product requires a persistent link between those identities, the website backend must define an authenticated, consented linking/unlinking flow that proves Microsoft ownership server-side. The launcher must not infer ownership from a Millida login or send a Microsoft token to Millida. No route has been invented here.

The Microsoft flow uses `GET /minecraft/profile` as its current profile/Java-license gate. There is no separate `entitlements/mcstore` call in this code. If a distinct entitlement check is required, confirm the official response/schema and expected product entitlement before adding one; the current flow does not bypass Microsoft authentication or allow a missing profile to play online as licensed.

## Configuration

- `MILLIDA_MS_CLIENT_ID` is a compile-time public Microsoft Entra application client ID. It is not a secret. Fork maintainers should use their own registered client ID; the existing fallback is for the original Millida build.
- `MILLIDA_API` is currently a Rust constant (`https://api.millida.net/v2`), not a runtime environment variable. The frontend API root is also compiled in; the `m-api` localStorage override is development-only. A self-hosted website/API needs an intentional build configuration change, not a production localStorage override.
- No website password, private API key, or permanent bearer token is required or embedded by this integration.

## Verification

The repository has existing Millida refresh/path tests and Microsoft device-code/session-refresh tests. New Rust regression cases cover HTTP 200 refresh responses missing either `rt2` or `accessToken`: the client retries inside the rotation grace and treats unrecoverable rotation loss as an expired session. The Rust suite and rustfmt could not be run in this Codespace because Cargo and rustfmt are unavailable. The login state has no dedicated website-flow test file; the Millida website backend, real account registration/login/logout, Microsoft/Xbox/Minecraft Services, entitlement edge cases, and account linking were not integration-tested. No authentication behavior is claimed as end-to-end verified.