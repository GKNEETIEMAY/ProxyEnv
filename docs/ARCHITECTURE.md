# RouteSpan architecture / 工程架构

English · [简体中文](ARCHITECTURE.zh-CN.md)

This describes implemented `v0.2.0dev` behavior. It is not release acceptance.
User workflow is defined in [Remote Bridge](REMOTE_BRIDGE.md), trust limits in
[Security](../SECURITY.md), durable identities in [Internal contracts](INTERNAL_CONTRACTS.md),
and verification evidence in [release QA](RELEASE_QA_V0.2.md).

## Product boundary

RouteSpan observes local proxy layers, manages selected current-user environment
variables and extends explicitly enabled local capabilities over SSH.
CC Switch owns AI providers/routes; RouteSpan supplies transport and selected
client-profile/Skills projection, not provider management or protocol conversion.
General network and AI are independent lanes sharing one owned SSH transport.

## Directory map

| Area | Responsibility |
| --- | --- |
| `src/app` | Shell, lifecycle, primary navigation and shared refresh |
| `src/features` | Local/remote workflows and feature presentation |
| `src/shared` | i18n, status/help/notification components and shared contracts |
| `src-tauri/src/commands` | Narrow IPC entry points |
| `src-tauri/src/desktop` | Single instance, tray, windows and notifications |
| `src-tauri/src/environment` | Environment transactions and snapshots |
| `src-tauri/src/features/remote_bridge` | Discovery, auth, relays, tools, Skills, recovery and typed events |
| `src-tauri/src/services` | Local-file validation, settings and redaction |
| `scripts` | Generated-helper checks and offline/UI regression tests |

## Frontend architecture

Dependencies flow `App.vue → app → features → shared`. The shell owns Local
Environment/Remote Bridge navigation, contextual assistant/settings return and
global diagnostic reporting. Primary surfaces stay mounted; visibility changes
do not recreate connection state. Overview and Advanced share a connected header
and backend summary, not separate state stores.

The session-wide ActiveProxyContext is reused by detection, environment operations,
assistant and remote setup. A five-second shell refresh gathers local observations;
it cannot silently replace a user-selected endpoint. CC Switch service detection
starts at app initialization and remains a separate observation.

Shared status/check/help components and the upper-right NotificationCenter keep
operational feedback consistent. Notifications are session-only and deduplicated;
field validation and durable workflow results remain beside their controls.
Diagnostic snapshots live in the backend with generation/ticket checks, so switching
tabs does not reset completed observations or accept late results for a new target.

## Local Environment

System Proxy and TUN are read-only observations. Registry writes are limited to
selected values in `HKCU\\Environment`: read/snapshot → apply → broadcast/readback,
with rollback on failure. Restore refuses a conflicting value instead of replacing
a later user edit. Running-process environments are never rewritten.

The assistant authorizes a canonical executable via short-lived backend identity,
then starts a new child with proxy/direct variables. The exceptional manual-guide
restart requires explicit confirmation and fresh process identity checks.
Bundled configuration rules are bounded data, not executable plugins or a guarantee
of arbitrary application configuration support.

## Remote Bridge

### Target discovery and SSH authentication

Opaque target IDs bind source, file identity and alias/bookmark. Bounded sources
are current-user OpenSSH config, VS Code's explicit SSH config, supported Moba
bookmarks and saved manual connections. Password/bookmark credential sections are
not imported. Editing imported data creates an owned copy; removal hides the source
entry, never crudely rewriting external config.

Native OpenSSH resolves aliases, keys, Agent and supported jump behavior. The
noninteractive path rejects unknown hosts; interactive first trust requires explicit
confirmation. Changed keys always fail. Agent/X11 forwarding and local commands
are disabled. The PTY parser handles fragmented control sequences and prompt IDs,
not guessed password questions. Answers never become shell arguments.

Eligible plain server passwords may be retained as target-bound DPAPI ciphertext
in memory for this bridge. Key passphrases, OTP and unknown responses are never
cached. Target switch, rejection, disconnect and exit clear session credentials.

### Core readiness and reverse forwarding

Authentication → Core Bridge → post-connect preparation are distinct phases.
Core Connected requires a live owned SSH process, live relays, successful reverse
forwards and remote `ss` evidence of IPv4 loopback listeners. Optional environment,
VS Code, tool and Skills preparation follows without blocking that snapshot.

SSH points at the local relay's actual port, not directly at the detected proxy.
The relay then connects to the fixed backend-reviewed upstream. General remote
port selection can use a supported local VS Code user-proxy hint or the selected
proxy's port; AI prefers 15721. Occupied ports use bounded deterministic fallbacks.
The two lanes allocate independently and are rechecked for races. A hint is not
proof of a running extension's effective proxy setting.

Capability OFF gates its relay while retaining reserved port context. AI OFF also
uses managed tool restore/pause behavior; re-enabling uses existing safe transactions.
Recovery uses retained target, ports and live session with bounded backoff, never
credential rotation, automatic target substitution or application-request replay.
Disconnect invalidates recovery generations before revocation and cleanup.

### General Proxy

Credential-free HTTP/Mixed/SOCKS forwarding retains correct bidirectional and
half-close behavior. There is no proxy username/password workflow. Fixed-upstream
forwarding is not an arbitrary local destination gateway. AI-only headers and
proxy authorization are not forwarded as general network credentials.

A live unchecked network lane schedules one automatic egress test. The result is
generation/ticket-bound and does not gate core readiness or prove all websites work.
Manual reruns are explicit. This is a current behavior exception to older documents
that claimed all external checks were opt-in; do not repeat that obsolete claim.

### AI Route

A CSPRNG session token authenticates AI requests via `X-ProxyEnv-Session` (legacy
compatibility identifier). Validation is backend-owned and constant-time; missing,
invalid, stale or cross-session credentials are rejected. The header is removed
before forwarding to the fixed CC Switch upstream. Tokens are not exposed to the
WebView, reports, copied exports or command lines.

The relay preserves supported endpoint traffic, status, headers, body and SSE,
without resolving models or translating textual tool syntax. Root and same-UID
hostile processes remain trusted/privileged boundaries. General loopback has no
per-UID access guarantee.

### Tool adapters and profile synchronization

Matching Rust/TypeScript adapter identities define Codex/Claude discovery,
compatibility, configuration and verification. CLI/plugin observations are separate.
Compatible remote-plugin evidence permits shared-profile configuration without a
CLI; CLI-dependent probes remain unavailable there. Completed discovery plus enabled
AI routing automatically prepares compatible tools unless opted out.

Codex uses the standard remote user configuration and an owned provider/catalog;
Claude uses standard user settings shared by CLI/plugin. Transactions read fresh
state, back up, atomically merge allowlisted fields and read back. Unrelated fields
remain; malformed, foreign or conflicting recovery evidence fails closed.
Python JSON tooling supports 2.7/3. Custom configuration roots are not guessed.

Local model/profile projection excludes provider secrets, real upstream URLs,
`auth.json`, MCP, onboarding and project trust. Codex catalog bytes remain opaque.
While connected and enabled, two-second mtime/size polling triggers debounce +
full parse/hash, then owned-field reconciliation on actual changes. Stopped bridges
do not poll profiles. A running client may require restart/reload.

Configuration success is not request verification. The current paid model probe
supports an installed Claude CLI only, with its route enabled. Codex remains pure
passthrough and has no built-in real-model probe; validate it through the actual
client. Paid verification is explicit; reasons appear in typed recent events.

### Skills

Discovery accepts only top-level Codex/Claude links whose canonical, same-named
target is a direct child of the CC Switch Skills root. Native Skills, Codex
`.system`, unrelated links and nested reparse points are excluded. Same names
merge visually; per-agent projections remain independent.

Connected metadata polling detects link/content changes before debounced hash
work. Staging uses bounded regular-file transfers, manifests and SHA-256; remote
owner/mode/link checks and atomic activation protect the destination. Foreign
same-name content is never overwritten. Removal affects only owned, unchanged
projections, not local or unrelated remote files.

### Session environment and VS Code

A token-free owned `~/.proxyenv/sessions/<id>/env.sh` exports uppercase/lowercase
proxy values and merged loopback bypasses. Default setup never edits shell startup
files. RouteSpan's terminal loads it; external shells execute the copied command.

VS Code setup changes only the detected remote Server Machine JSONC settings:
disable inherited local proxy configuration, temporarily remove conflicting
`http.proxy`/`http.proxyAuthorization`, merge loopback `http.noProxy`, and prepare
`terminal.integrated.env.linux`. Local Windows User Settings are unchanged.
Existing windows/terminals require reload/recreation. No extension bundle patching.

Runtime and parent paths are ownership/mode/link checked. Verified private primary
groups have a narrowly scoped acceptance path; shared writable ancestry is rejected.
Missing Server/runtime can require first-launch initialization; unsafe/conflicting
state blocks automatic changes. A settings hint never proves a real plugin request.

Disconnect revokes relays before attempting VS Code restore and session cleanup;
enabled tool profiles are retained separately. Tool OFF restores owned fields.
Abrupt termination may leave recovery journals; later cleanup stays conflict checked.

## Diagnostics and redaction

Reports project a non-probing snapshot into allowlisted enums, counts, safe versions
and canonical client labels before IPC. They exclude addresses, usernames, paths,
PIDs, raw errors, config values and secrets. Language changes reformat the same
snapshot; generation never starts a new request, auto-save or upload.

Remote typed events can be written to bounded rotating JSONL logs (three files,
2 MiB each). The shared Rust redaction boundary is defense in depth, not permission
to serialize raw summaries or requests. Clear Logs targets managed log files only.
Diagnostic report TXT and machine-readable runtime JSONL serve different purposes.

## Persistent compatibility identities

The display brand is 境枢 RouteSpan. Legacy package/crate, `proxyenv_lib`,
`proxyenv.exe`, app identifier, `%LOCALAPPDATA%\\ProxyEnv`, `~/.proxyenv`,
session header, storage keys, repository and updater source remain unchanged.
See [Internal contracts](INTERNAL_CONTRACTS.md) before any migration.

## Updater and desktop lifecycle

Single-instance registration precedes tray setup; second launch focuses the existing
window, not a second bridge. Tray/close behavior follows saved preferences.

Official Tauri Updater uses a pinned HTTPS source, embedded public key, default
anti-downgrade and explicit check/install actions. Registered NSIS installs support
replacement; MSI/portable use manual updates. Authenticode is not adopted.
Draft generation is not publication or acceptance; [release security](release-security.md)
defines the formal artifact/signature requirements.

## Trust boundaries and verification

Frontend strings, discovered paths and existing remote files are untrusted inputs.
Rust revalidates mutations; local/remote files use bounded reads, owner/type/link
checks and atomic writes where supported. Production CSP permits bundled assets,
IPC and the explicit GitHub update check, not arbitrary remote scripts.

Use [Contributing](../CONTRIBUTING.md) for tests and the [QA ledger](RELEASE_QA_V0.2.md)
for actual outcomes. Linux tests, mock browser checks and local compilation do not
prove real Windows upgrade, native DPI, live SSH or provider behavior.
