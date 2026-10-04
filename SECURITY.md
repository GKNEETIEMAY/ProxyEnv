# Security Policy

```yaml
Current Stable: v0.1.4
Next: v0.2.0
```

This policy covers Current Stable v0.1.4 and development changes planned for v0.2.0. The safe Diagnostic Report and unified ActiveProxyContext are shipped in v0.1.4. See the [Roadmap](docs/ROADMAP.md).

Remote Bridge is unreleased development functionality. Its reverse forwards terminate at ProxyEnv-owned relays. The General Proxy is credential-free and offers no per-UID isolation on a shared host; the AI Route remains session-authenticated. Real shared-host acceptance remains a release gate for v0.2.0.

Please report suspected vulnerabilities privately to the repository maintainers rather than opening a public issue. Include the affected version, operating system, reproduction steps, observed impact, and any relevant local logs with secrets removed.

## Trust model

ProxyEnv treats discovery as untrusted evidence and every mutation as a protected operation. Its core principles are:

1. **Automatic discovery, manual changes.** Refresh, process detection, listener inspection, system-proxy reading, and TUN observation never write settings.
2. **Read before write.** Every managed environment or application-rule change is based on freshly read state.
3. **Preview, confirm, back up, verify, restore.** Protected writes expose their exact target and consequence, require user intent, preserve the prior value, and read back the result.
4. **Uncertain means no write.** Missing, malformed, unsupported, or ambiguous state stops the operation.
5. **Conflict means no overwrite.** A stale plan or externally changed file is never replaced.
6. **Rules are data only.** Bundled rules cannot execute commands, scripts, adapters, templates, or regular expressions.
7. **Frontend is untrusted.** Executable paths and proxy endpoints are authorized or validated again by Rust; privileged IPC never relies on frontend-only checks.

## Allowed effects

ProxyEnv may:

- read localhost listeners, visible user applications, Windows System Proxy, current-user proxy environment values, and network-adapter metadata;
- probe only an explicitly selected or locally discovered proxy endpoint;
- modify only selected current-user proxy values under `HKCU\\Environment`, with snapshot, broadcast, and verification;
- launch a new child process with an explicit proxy environment or with managed proxy variables removed;
- only in the manual-proxy guide, after a warning and a second explicit confirmation, revalidate the selected PID and executable, request a normal close, use the guarded termination fallback if needed, and launch one replacement with proxy variables cleared;
- update one existing application configuration field only when a reviewed bundled rule identifies an exact process name, fixed user-profile-relative path, supported format, and typed value;
- create a local rule backup and restore it only when the current field still equals the value ProxyEnv applied;
- contact the fixed official GitHub Releases API and pinned HTTPS updater manifest only when the user explicitly selects **Check for updates**;
- after an explicit **Download and install** action, download only the manifest-selected installer, require Tauri signature verification, replace only the registered NSIS installation, and restart after successful installation.
- in v0.2 development, after explicit selection and confirmation, establish owned SSH reverse forwards through loopback-only relays to fixed, reviewed local proxy/AI-route upstreams, authenticate AI requests with a session header, and revoke both relays on disconnect;
- after the user enables a supported remote tool, validate, back up, atomically update and verify only allowlisted Codex/Claude user-profile fields, preserving unrelated content and refusing conflicting restore state;
- project only CC Switch-managed Skill links explicitly selected for Codex or Claude through a staged, hash-verified and ownership-marked transaction, and remove only the remote files owned by that transaction.

## Prohibited capabilities

ProxyEnv must not:

- act as a general VPN or proxy server, open arbitrary traffic-forwarding destinations, control TUN, install drivers, manage nodes or subscriptions; explicit owned Remote Bridge forwarding is the limited exception above;
- enable or disable TUN, adapters, routes, services, proxy clients, Windows System Proxy, proxy-client global settings, nodes, subscriptions, or client rules;
- call Clash, v2rayN, or other proxy-client control APIs;
- auto-download rules, run a rule marketplace, execute rule-provided code, shell commands, scripts, adapters, templates, or regular expressions;
- inject into, hook, debug, suspend, or call `WriteProcessMemory` on a running process; termination is prohibited except for the explicitly confirmed, identity-checked manual-guide restart described above;
- modify the environment of a running process or claim that registry broadcasts retroactively change it;
- scan the full disk, search arbitrary configuration directories, accept user-defined rule paths/fields, or follow symlinks/reparse points;
- silently modify arbitrary third-party settings, repair applications, follow changing ports, or run external connectivity tests in the background; a user-enabled remote tool may reconcile only its owned allowlisted profile fields while the bridge is active;
- collect or upload traffic, subscriptions, nodes, provider credentials, application configuration contents, or process lists. Transient SSH authentication responses and relay session tokens are restricted to their explicit session boundaries, never diagnostic data.

## Local data

Environment snapshots, settings, and application-rule backups are stored locally beneath the current user's application-data directories. They may contain previous proxy addresses or the specific application field value needed for restoration. Reads are bounded and reject non-regular files, symbolic links, and Windows reparse points. Mutable files use create-new temporary files and atomic replacement; rule backups are create-once records. They should still be protected with the same account-level access controls as the user's registry and application configuration.

The production WebView uses a restrictive Content Security Policy. Remote scripts are not permitted, and outbound frontend connections are limited to Tauri IPC plus the user-triggered GitHub Releases check. Application discovery and the native file picker return short-lived random IDs; backend commands resolve and revalidate those IDs instead of accepting executable paths from the frontend.

The application assistant displays executable paths and configuration targets locally. These values are not transmitted.

## Diagnostic data and redaction

ProxyEnv does not persist an application log by default. Text that crosses a runtime log, diagnostic, serialized-error, or error-report boundary must pass through the shared Rust redaction component. It removes or replaces user names and user-directory paths, full application and configuration paths, local proxy addresses and ports, executable/process names, and process identifiers. Configuration field contents and other values whose format cannot be identified reliably must be treated as fully sensitive and replaced as a whole rather than passed through heuristic text redaction.

The redaction layer is defense in depth, not permission to log local state. New diagnostics should prefer fixed error categories, counts, and booleans over paths, endpoints, process details, configuration values, or raw debug representations. Logs and screenshots shared in vulnerability reports should still be reviewed before submission.

The Diagnostic Report preview uses a backend-issued allowlisted DTO, not serialized raw diagnostics. Client names must match the bundled detector catalog; unknown names are generalized. OS-version metadata is restricted to numeric version/build notation. Reports omit all proxy addresses, application/interface names, paths, process IDs, raw errors, rule identifiers and configuration values. They include only status summaries, counts, variable names and safe version/client metadata. Changing output language reformats the same snapshot; generation does not trigger external connectivity tests. Copying is explicit, and reports are never saved or uploaded automatically. Review the preview before sharing it in an Issue.

ProxyEnv does not read, save, or manage proxy user names or passwords, subscription tokens, node credentials, or any other proxy authentication material. Such values must never be added to logs, reports, snapshots, backups, settings, or application rules.

Remote Bridge may accept an SSH password, key passphrase, verification code, or keyboard-interactive response only after the user explicitly opens an interactive authentication flow. Every response is bound to the session-owned current prompt ID, written directly to the current OpenSSH PTY stdin, never added to command arguments, configuration, settings, diagnostics, logs, terminal snapshots, or authentication snapshots, and cleared from frontend and backend plaintext input buffers after submission. Except for the eligible, session-encrypted plain server password described below, it is not retained for later operations. Echo suppression removes only a byte-exact copy of the submitted response and never discards an arbitrary following line. If ConPTY does not expose a real prompt before the bounded wait expires, ProxyEnv stops the PTY and reports only safe diagnostic counts and flags; it never invents a fallback authentication question. Non-interactive checks reject unknown hosts; the interactive flow may ask the user to confirm a first-seen fingerprint, but never disables host-key checking and never bypasses an existing `known_hosts` mismatch. Cancelling, prompt timeout, expiry, disconnect, or application shutdown terminates the owned authentication process.

An eligible plain SSH server password may be retained **only during the current bridge** as Windows-current-user DPAPI ciphertext in memory, bound to the selected SSH configuration; target switch, disconnect, rejection and exit clear it. Key passphrases, OTPs and unknown challenges are never cached. This transport exception does not authorize storing proxy or provider credentials.

## Remote Bridge shared-host boundary (v0.2 development)

Loopback binding of an SSH reverse forward does not isolate Unix UIDs: another user on the same Linux host may connect to the General Proxy while the bridge is active. This capability deliberately uses no proxy username or password and makes no different-UID isolation claim. It remains loopback-only, forwards to a fixed backend-selected upstream and is revoked before disconnect cleanup. Codex and Claude use a distinct transient `X-ProxyEnv-Session` header through their owned configuration transaction; missing, invalid, stale and cross-session AI tokens are rejected. AI tokens stay out of the WebView, copied proxy exports, logs, diagnostic reports, provider API-key fields, `auth.json` and process command lines. Root remains outside any user-space isolation guarantee. See the [relay boundary design](docs/remote-bridge/AUTHENTICATED_RELAY.md).

The Remote Bridge UI must not reintroduce General Proxy username/password fields, copy actions or fallback instructions. SSH authentication remains a separate OpenSSH transport boundary, and AI Route session authentication remains backend-managed. Source-specific launch actions are authorized by backend target capabilities rather than inferred from a frontend label.

Remote Skills projection accepts only user-managed CC Switch links whose resolved source stays inside the CC Switch Skills directory. Native/system Skills are excluded. Upload staging, manifest/hash verification, an ownership marker, foreign same-name conflict handling and owned-only removal protect the remote account; no remote user file is overwritten merely because a name matches. Automated regression coverage exists, while real Codex/Claude Agent discovery on the release matrix remains pending. Legacy extension recovery code remains while its callers and restore obligations exist.

## Release security priorities

Release CI, locked dependencies, SHA-256 checksums, GitHub Artifact Attestation, the environment transaction, restore conflict detection, CSP, backend IPC authorization, snapshot validation, signed Tauri Updater artifacts, an embedded public verification key, a fixed HTTPS source, and default anti-downgrade behavior are the trust requirements for a formal public Windows release.

Windows Authenticode is not a ProxyEnv release dependency. The project does not plan to purchase or use paid Windows code-signing certificates or services, and an unsigned Windows build may be published as stable. Release notes and documentation must state the resulting Unknown Publisher and SmartScreen warnings transparently and direct users to the official GitHub Releases source, checksums, and artifact attestation.

Tauri Updater signing private keys used by CI must exist only in the authorized release secret store; a maintainer recovery copy may exist only outside tracked source in protected offline storage. Private material must never be committed, packaged, decoded, or printed by CI. Updater builds use only Tauri's official `TAURI_SIGNING_PRIVATE_KEY` and `TAURI_SIGNING_PRIVATE_KEY_PASSWORD` environment-variable mechanism. See [`docs/release-security.md`](docs/release-security.md) for the complete release trust model.
