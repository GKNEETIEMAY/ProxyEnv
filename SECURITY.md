# RouteSpan security policy

English · [简体中文](SECURITY.zh-CN.md)

RouteSpan v0.2.0 is the current public stable release, published on 2026-10-13.
The legacy brand was ProxyEnv; v0.1.4 remains a Draft predecessor baseline.
This policy describes security boundaries, not a guarantee for every environment.
See [release QA](docs/RELEASE_QA_V0.2.md).

Report vulnerabilities privately to repository maintainers, not a public issue.
Include affected build/OS, sanitized reproduction and impact. Do not attach keys,
passwords, provider/session tokens or complete private settings.

## Trust model

Discovery is evidence, not authorization. Rust revalidates targets, paths, process
identity and endpoints before mutation. User intent authorizes bounded effects:
read fresh state → preserve backup → apply → verify → restore safely.
A stale plan, unknown structure, foreign ownership or conflicting change stops
the operation. Rules are data, never arbitrary scripts or executable adapters.

## Local effects

RouteSpan may observe local listeners, visible applications, System Proxy and TUN;
manage selected current-user `HKCU\\Environment` values with snapshots/rollback;
and launch a new process with explicit proxy/direct environment. The guarded manual
restart flow additionally requires confirmation and fresh executable/PID checks.

It must not write HKLM, change System Proxy/TUN, install drivers, control client
nodes/subscriptions/rules, scan the entire disk, inject into running processes,
or rewrite their environments. Bundled rule transactions are limited to reviewed
paths/formats/fields, not arbitrary user-supplied configuration repair.

Local storage uses bounded schema/size checks, regular-file and symlink/reparse
rejection, and atomic replacement where mutation is allowed. Prior field/address
values retained for restore are account-local data, not safe diagnostic content.

## Remote transport and authentication

OpenSSH retains native key/Agent handling. First trust requires explicit fingerprint
confirmation. Changed known keys fail; checks are never globally disabled. Agent/X11
forwarding and local-command execution are disabled for the bridge.

PTY responses are prompt-ID bound and do not enter command arguments, configuration,
logs or snapshots. Plaintext handling is bounded/cleared; eligible plain server
passwords alone may remain as target-bound Windows-user DPAPI ciphertext in memory
for the current bridge. Key passphrases, OTP and unknown challenges are not cached.
Target switch, rejection, disconnect and exit clear session credentials.
This exception does not permit storing proxy/provider passwords.

Core readiness requires owned process/relay liveness and remote loopback listener
evidence. Local upstreams are fixed backend-selected capabilities, not arbitrary
forward destinations. Recovery never bypasses authentication, changes target or
replays interrupted application traffic.

## Shared-host limits

General Proxy deliberately has **no proxy username/password authentication**.
Linux loopback **does not isolate UIDs**; another account can use an active general
network bridge. Do not claim shared-group membership alone prevents access.
The UI must not reintroduce proxy credential forms or fallback instructions.

AI routing separately uses `X-ProxyEnv-Session`, a legacy compatibility identifier
retained from ProxyEnv. Backend constant-time validation rejects missing, wrong,
stale and cross-session tokens; the header is stripped before the fixed upstream.
Tokens stay out of WebView data, copied exports, diagnostics, process arguments,
real provider API-key fields and `auth.json`. Remote managed config is validated
and protected by account-local file permissions.

Root and hostile same-account processes are outside user-space isolation guarantees.
No session mechanism can prevent a privileged administrator reading/modifying the
account's files or processes. Use trusted accounts/hosts for sensitive work.

## Remote writes and cleanup

Supported tool configuration uses backup, atomic managed-field merge and readback.
Provider secrets, upstream URLs, onboarding, trust and unrelated configuration
are not copied. Tool OFF restores only its owned fields; conflicts fail closed.
Disconnect revokes forwarding first, attempts owned VS Code/session cleanup, and
retains enabled tool profiles separately. Abrupt exit may leave recovery evidence.

VS Code setup is remote Machine JSONC only; local Windows User Settings and extension
code are not patched. Runtime/parent owner, group, mode and link checks protect
credentials. Verified user-private primary groups have a narrow acceptance exception,
not a general permission to write through shared-group-writable paths.

Default shell setup changes only owned session exports, not `.bashrc`, `.profile`
or `/etc/environment`. Any persistent PATH example is a user's explicit manual action.

Skills project only eligible CC Switch-managed links. Native/system Skills,
unrelated links and nested links are excluded. Bounded staging, hashes and ownership
markers protect uploads; foreign same-name content is not overwritten and removal
deletes only owned copies. Nothing deletes a local Skill as a side effect.

## Network checks and diagnostic data

Enabling network bridging schedules one automatic external egress check after
the network lane is ready. Manual diagnostics may perform additional explicit tests.
Model verification is explicit and can consume tokens; tool installation is never
automatic. This documented egress behavior is not a claim of zero background traffic.

Diagnostic report generation itself starts no new discovery/network test and
never auto-saves/uploads. The allowlisted DTO contains only enums, counts, variable
identifiers and safe version/client metadata, not raw logs or settings.
Reports exclude addresses, usernames, paths, PIDs, raw errors and credentials.
Review the preview before sharing.

Typed remote bridge events use bounded rotating JSONL files; arbitrary debug/request
state is not serialized. Shared redaction removes sensitive textual signatures;
unstructured config must be treated wholly sensitive. Redaction is defense in depth,
not permission to record secrets. Clear Logs removes only validated managed files,
not settings/backups or the whole application directory.

## WebView and release trust

Production CSP restricts scripts/assets to the app and outbound frontend access
to IPC plus the user-triggered fixed GitHub update check. Privileged IPC does not
accept arbitrary shell commands, executable paths or update URLs as authority.

The display brand is RouteSpan; legacy `%LOCALAPPDATA%\\ProxyEnv`, `~/.proxyenv`,
repository and identifier remain for compatibility, not accidental stale branding.
See [Internal contracts](docs/INTERNAL_CONTRACTS.md).

Formal releases require locked audit/build/tests, checksums, provenance, signed
Tauri Updater assets and explicit update verification. Authenticode is not adopted;
Unknown Publisher/SmartScreen warnings must be disclosed. Private updater keys
belong only in authorized secrets/protected recovery storage, never source,
packages or printed CI output. [Release security](docs/release-security.md) defines
the complete release boundary. Audits and full CI remain gates for future releases.
