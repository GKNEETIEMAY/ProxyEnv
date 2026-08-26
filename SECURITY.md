# Security Policy

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
- update one existing application configuration field only when a reviewed bundled rule identifies an exact process name, fixed user-profile-relative path, supported format, and typed value;
- create a local rule backup and restore it only when the current field still equals the value ProxyEnv applied;
- contact GitHub Releases only when the user explicitly selects **Check for updates**.

## Prohibited capabilities

ProxyEnv must not:

- act as a VPN, proxy server, traffic forwarder, TUN controller, TUN driver installer, node manager, or subscription manager;
- enable or disable TUN, adapters, routes, services, proxy clients, Windows System Proxy, proxy-client global settings, nodes, subscriptions, or client rules;
- call Clash, v2rayN, or other proxy-client control APIs;
- auto-download rules, run a rule marketplace, execute rule-provided code, shell commands, scripts, adapters, templates, or regular expressions;
- inject into, hook, debug, suspend, terminate, or call `WriteProcessMemory` on a running process;
- modify the environment of a running process or claim that registry broadcasts retroactively change it;
- scan the full disk, search arbitrary configuration directories, accept user-defined rule paths/fields, or follow symlinks/reparse points;
- modify settings, repair applications, follow changing ports, or run external connectivity tests in the background;
- collect or upload traffic, subscriptions, nodes, credentials, tokens, passwords, application configuration contents, or process lists.

## Local data

Environment snapshots, settings, and application-rule backups are stored locally beneath the current user's application-data directories. They may contain previous proxy addresses or the specific application field value needed for restoration. Reads are bounded and reject non-regular files, symbolic links, and Windows reparse points. Mutable files use create-new temporary files and atomic replacement; rule backups are create-once records. They should still be protected with the same account-level access controls as the user's registry and application configuration.

The production WebView uses a restrictive Content Security Policy. Remote scripts are not permitted, and outbound frontend connections are limited to Tauri IPC plus the user-triggered GitHub Releases check. Application discovery and the native file picker return short-lived random IDs; backend commands resolve and revalidate those IDs instead of accepting executable paths from the frontend.

The application assistant displays executable paths and configuration targets locally. These values are not transmitted. Logs and screenshots shared in vulnerability reports should be reviewed for user names, directory names, proxy addresses, tokens, and other sensitive data.

## Release security priorities

Release CI, locked dependencies, the environment transaction, restore conflict detection, CSP, backend IPC authorization, and snapshot validation are P0 requirements. If automatic updates are enabled later, signed Tauri Updater artifacts, an embedded public verification key, a fixed HTTPS source, and default anti-downgrade behavior also become P0.

Windows Authenticode is P1 future release hardening, not the updater integrity mechanism. ProxyEnv may publish an open-source Windows build without it, but users can then see Unknown Publisher, Microsoft Defender SmartScreen, or Windows protected your PC. Authenticode may later use Microsoft Artifact Signing or a trusted OV certificate to establish publisher identity and reputation.

Signing private keys and certificate private keys must exist only in the authorized release secret store. They must never be committed, packaged, decoded, or printed by CI. Future Tauri Updater builds will use only Tauri's official `TAURI_SIGNING_PRIVATE_KEY` and `TAURI_SIGNING_PRIVATE_KEY_PASSWORD` environment-variable mechanism. See [`docs/release-security.md`](docs/release-security.md) for the complete distinction between updater signing, SHA-256, Authenticode, and the release workflow.
