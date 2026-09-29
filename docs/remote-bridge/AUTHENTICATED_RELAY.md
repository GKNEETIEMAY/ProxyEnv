# Remote relay authentication boundaries

**Status: implemented in `v0.2.0dev`; shared-host acceptance remains pending.** General Proxy and AI Route share the SSH transport but intentionally use different authentication boundaries.

## Capability matrix

| Capability | Remote endpoint | Authentication | Purpose |
| --- | --- | --- | --- |
| General Proxy | loopback HTTP / SOCKS5 | none | Ordinary network access through the selected local proxy client |
| AI Route | loopback HTTP | `X-ProxyEnv-Session` | Codex and Claude traffic forwarded to the fixed local CC Switch route |
| SSH connection | OpenSSH | SSH key, Agent, password or host confirmation | Establish and own the bridge transport |

SSH login authentication and AI Route session authentication are not proxy usernames or passwords. Removing General Proxy credentials must not weaken either boundary.

## General Proxy

The General Proxy listener is lifecycle-bound to the active bridge and forwards only to the backend-selected local proxy endpoint. HTTP and Mixed accept requests without `Proxy-Authorization`; SOCKS5 negotiates the no-authentication method. ProxyEnv strips any supplied `Proxy-Authorization` and removes `X-ProxyEnv-Session` from unrelated proxy requests before forwarding them upstream.

No General Proxy username or password is generated, copied, stored, displayed, passed through IPC or written to the remote environment. The trade-off is explicit: another account on the same remote host that can connect to this loopback port may use the General Proxy while the bridge is active. ProxyEnv does not claim per-UID isolation for this capability. The listener remains limited to remote loopback, the upstream destination is fixed, and disconnect closes the relay and tracked streams.

## AI Route

The AI Route remains session-authenticated. Each explicit bridge session creates a fresh CSPRNG token held in backend-owned zeroized memory. Codex and Claude receive it through managed configuration as `X-ProxyEnv-Session`; the relay validates it before contacting the fixed local AI upstream and strips it before forwarding. Missing, invalid, stale and cross-session tokens remain rejected.

## Current implementation

- General proxy forwarding terminates at a loopback-only relay. HTTP uses no proxy authentication and SOCKS5 negotiates method `0x00`; supplied proxy authorization headers are stripped before connecting to the fixed Active Proxy endpoint.
- AI forwarding uses `X-ProxyEnv-Session`. Codex receives it through provider `http_headers`; Claude receives it through `ANTHROPIC_CUSTOM_HEADERS`. Claude versions older than `2.1.227` fail closed.
- The AI relay owns a 256-bit random token and a non-secret random session id. Tokens are compared in constant time and retained in zeroizing backend buffers where practical.
- Managed proxy terminals source `~/.proxyenv/sessions/<session-id>/env.sh`. The file is created through SSH stdin with mode `0600` and contains credential-free loopback proxy URLs.
- Reconnect rotates tokens. Existing ProxyEnv-owned Codex or Claude projections are updated through their established preview/apply transaction without a second user decision. Unowned configurations are not modified.
- Disconnect revokes both local capabilities before best-effort removal of the remote session environment. Relay drop also shuts down tracked active streams.
- Production AI forwarding cannot fall back to an unauthenticated route.

## External client authentication and independent capabilities

- The General Proxy and AI Route remain independently selectable. Both use SSH transport, but AI routing does not require the General Proxy or proxy environment variables. General Proxy staleness does not invalidate AI verification; an unavailable AI upstream does not block General Proxy testing or managed terminals.
- Managed terminals automatically receive uppercase and lowercase credential-free proxy variables. Stale variants are removed; both `NO_PROXY` and `no_proxy` contain loopback and preserve the user's existing bypass entries.
- Advanced offers a token-free command to source the current session file in an external remote terminal. Only that terminal and new descendants receive it. Existing VS Code Server / extension processes do not inherit it, and opening a new window is not evidence that those processes restarted.
- There is no General Proxy credential command, password copy action or Advanced username/password fallback. The AI token has no export command.
- VS Code Chromium/extension networking and a Codex child process do not share one universal proxy configuration source. ProxyEnv therefore isolates the remote server from inherited local proxy settings and keeps loopback AI routes direct; see below. It does not modify third-party extension files, inject into running processes, or use an undocumented SSH startup hook.

### Automatic VS Code network setup

Open in VS Code updates only the detected remote account's VS Code Server `data/Machine/settings.json`. The local Windows `Code/User/settings.json` remains untouched, whether it contains `http.proxy` or not. The transaction temporarily prevents inherited proxy settings from routing the loopback AI endpoint through the General Proxy. Multiple remote server roots are rejected rather than guessed; the server and Node runtime must already be discoverable.

The editor launch command runs before the remote configuration transaction, so missing runtimes, permission failures or SSH timeouts cannot prevent opening the editor. A successful launch with failed setup returns a separate localized warning; it does not claim proxy readiness or disable authentication. A failed editor launch performs no remote configuration write. Reload the remote window after setup completes so extension processes use the current settings.

Network-only setup selects the remote Server settings directory without scanning AI extension manifests or server version directories. An explicit in-home `VSCODE_AGENT_FOLDER` takes precedence; otherwise exactly one default server root must exist. The helper discovers existing Node 20+ through system paths, SSH PATH, native user paths, nvm and bundled VS Code Server runtimes. Package-manager symlinks are canonicalized before validating executable and ancestor ownership/write permissions. No shell profiles are sourced and no runtime is installed. Missing/old runtime, unsafe runtime paths, absent server and ambiguous server roots have distinct safe error codes instead of an AI-extension compatibility error.

Four managed fields form one reversible isolation boundary:

- `http.useLocalProxyConfiguration = false` separates the remote setting from the user's local network proxy.
- `http.proxy` is temporarily removed instead of storing a General Proxy credential in VS Code.
- `http.proxyAuthorization` is temporarily removed because Chromium and extension processes do not consume one shared credential source reliably.
- `http.noProxy` preserves existing exclusions and adds loopback. Codex and Claude reach the separately authenticated AI Route directly.

These settings and their ownership journal are written atomically as user-owned mode 0600 files over SSH stdin, never shell arguments. The journal retains original managed fields, not an entire unrelated settings backup. Apply and restore preserve unrelated JSONC fields and comments, reject edits to managed fields, and roll back write failures without overwriting concurrent changes. The transaction contains no proxy password or AI session token.

Normal disconnect revokes both relays first, then restores the original remote fields. Failed cleanup is reported. Exit/crash revokes the relays but does not wait for SSH cleanup; a token-free local ownership ledger lets the next connection or Open in VS Code recover the previous session before applying the current isolation state. Only previously managed targets receive this reconnect update; unrelated remote settings are not automatically enrolled. Fingerprint changes, unknown session owners and conflicting user edits fail closed.

An already-running extension process may need the remote window reloaded. This is not automatic injection into arbitrary external applications. HTTP, Mixed and SOCKS General Proxy access remains available without credentials through the managed terminal. AI-only and combined mode use the same VS Code isolation transaction and the separately authenticated AI Route.

Acceptance: retain the user's local `http.proxy`, rebuild ProxyEnv, reconnect, open the remote target through ProxyEnv, reload an existing window, and test Codex CLI plus the extension without a proxy authentication dialog. Repeat with no local proxy setting. Verify the remote Machine settings contain no ProxyEnv credential, loopback is bypassed, original fields restore on disconnect, and invalid AI session headers are rejected. Local automated tests do not replace this real VS Code/shared-server acceptance.

## Acceptance coverage

### Codex VS Code loopback requests through `http.proxy`

The inspected Codex extension (`26.901.22334`) copies VS Code's `http.proxy` into its spawned Codex process's `HTTP_PROXY`/`HTTPS_PROXY` without adding a loopback bypass in that function. An AI request can therefore arrive at the General Proxy even though the provider configuration already contains valid AI session authentication.

When both routes exist, the General Proxy recognizes only an absolute-form HTTP request addressed to the current AI route's exact remote port on `127.0.0.1`, `localhost` or `[::1]`. It dispatches that request to the existing AI parser/authenticator with the fixed AI upstream captured for this session. Missing or wrong AI headers still receive 401; ordinary General Proxy access does not authorize AI use. Revoked AI routes fail closed.

Both session revocations track and close routed connections. The shared AI handler strips the session header and preserves the body, model and streamed response/tool events. On unrelated General Proxy requests the AI session header is removed to avoid credential leakage.

This dispatch remains a fail-closed compatibility path for a stale consumer with a missing loopback bypass. It does **not** authenticate external Git/telemetry traffic, inject into a running process, or repair an absent SSH forward. The automatic remote VS Code setup above now removes inherited proxy routing instead of supplying a General Proxy credential. Live remote-extension acceptance is still required after rebuilding and reconnecting.

Acceptance must prove: General HTTP and SOCKS work without credentials; supplied proxy credentials are stripped; the AI header never leaks to unrelated destinations; AI requests still reject missing, wrong and old session tokens; disconnect revokes both relays including active streams; and the fixed upstream cannot be replaced by frontend input. Exercise real shared-host loopback behavior and CLI/extension/proxy clients, not only mocks. The existing transparent Codex streaming/tool tests must continue to pass with AI authentication enabled.
