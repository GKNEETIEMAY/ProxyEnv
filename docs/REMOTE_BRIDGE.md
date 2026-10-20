# RouteSpan Remote Bridge / 远程桥接

English · [简体中文](REMOTE_BRIDGE.zh-CN.md)

This is the single current user-level guide for Remote Bridge in RouteSpan v0.2.1.
It extends selected local network/AI capabilities to a non-root Linux SSH account,
not a VPN or proxy core. RouteSpan v0.2.1 is publicly released.
See [Getting started](GETTING_STARTED.md) for requirements and first use.

## Target discovery / 连接来源

Targets come from current-user OpenSSH configuration, an explicit VS Code SSH
configuration, supported MobaXterm bookmarks, and saved manual connections.
Source labels distinguish them. Unsupported imported behavior stays unavailable;
finding a name does not prove the server permits reverse forwarding.

Add/Edit share a validated form: display name (1–32 characters), host or
`user@host`, port (default 22), username and authentication mode. The identity
picker stores only a path and validates a bounded key-format header; it does not
upload or persist the private key. OpenSSH decodes keys and authenticates.

Editing an imported connection creates a RouteSpan-owned saved copy and hides
the original entry. Removing an imported connection hides it; removing a manual
connection deletes its owned record. Neither operation rewrites SSH config or
Moba bookmarks. Removing selection picks an appropriate remaining connection or
leaves an empty, non-connectable state. Refresh preserves a valid selection.

## Connect / 建立连接

Select the target on the left and capabilities on the right, then Connect.
There is no separate confirmation page; backend preview still revalidates target,
local endpoints and remote port availability before forwarding.

Key/Agent login uses native OpenSSH. Interactive login shows real password/KBI
prompts. Answers are scoped to the current prompt. Explicitly confirm a trusted
first-seen fingerprint; an existing known-key mismatch is never bypassed.
Cancel terminates the owned authentication attempt.

Connected means the SSH process, requested relays and remote loopback listeners
were verified. Optional preparation follows independently. SSH success does not
guarantee server internet, proxy egress or a model response.

## Bridge local network / 桥接本机网络

This lane reuses Local Environment's active proxy selection, not a second detector
or a silently changing upstream. The selected protocol/endpoint remain bound to
the session. Remote and local ports need not match: allocation considers the
supported VS Code user-proxy hint, the local port and conflicts. Use the connected
summary instead of hard-coding a port.

Once ready, one automatic egress probe runs independently of tool setup. Retry
through diagnostics if needed. Listener verified ≠ external internet guaranteed.
General Proxy intentionally has no username/password authentication; loopback does
not isolate UIDs on a shared host. Disable it if that risk is unacceptable.

## CC Switch AI Route / AI 路由

CC Switch owns provider/model routing and upstream credentials; RouteSpan extends
the route. Enable **CC Switch → 设置 → 路由 → 本地路由 → 路由总开关 →
开启 Claude/Codex 路由** before use. The local service/address is detected from app
startup and refreshed. Confirmed service identity is not per-tool switch readback
or a successful upstream authentication test.

AI and network are independent switches; both require SSH. AI-only needs no
general proxy export. Enable both for AI plus general downloads/web access.

Requests use backend-managed `X-ProxyEnv-Session` authentication without copying
provider secrets. The relay preserves request/response content, tools and SSE.
It does not reinterpret textual DSML/tool markup or convert incompatible upstream
protocols. Route configured ≠ real model request verified.

## Codex / Claude states / 工具状态

| State | Meaning | Does not imply |
| --- | --- | --- |
| Installed | Compatible CLI or remote plugin evidence was found | Configuration/provider access |
| Configured | Managed profile fields were written/read back | Successful model response |
| Verified | Explicit verification succeeded in its scope | All entry points/providers work |

CLI/plugin versions are independent. Supported CLIs: Codex `0.x` from `0.134`,
Claude `2.x`; session-header Claude access needs `2.1.227` or newer.
Compatible plugins can be configured without a CLI. The built-in real-model probe
currently supports installed Claude CLI only; it is unavailable for plugin-only
Claude and for Codex. Verify Codex through its actual CLI or plugin conversation.

With AI routing on, completed discovery prepares compatible tools automatically
unless individually switched off for this session. CLI and plugin share the tool's
standard remote profile, not duplicate configuration transactions. Enable validates,
backs up, writes atomically and reads back; Disable restores owned fields while
preserving unrelated content. Conflicting ownership evidence stops the transaction.

Enabled profiles follow local changes: two-second metadata checks, then debounce,
full parse/hash and reconciliation only on change. There is no stopped-state
profile polling. Running clients may need restart/reload for a new model/provider.
No real provider key, `auth.json`, onboarding or project trust is copied.

Installation guidance supplies commands to run yourself; it never installs software
automatically. A missing command can be a PATH problem in `~/.local/bin` or an npm
prefix. Use the [Getting started guidance](GETTING_STARTED.md) rather than reinstalling blindly.

## Skills / 技能同步

Eligible sources are same-named CC Switch-managed links in the local Codex/Claude
Skills roots. Native Skills, Codex `.system`, unrelated and nested links are excluded.
Names merge into one row with independent per-agent controls. Detected managed
links default on unless opted out.

Expand Overview's manager to search, sync or cancel. While connected, changes are
checked dynamically; removal of a CC Switch link removes the corresponding owned
remote projection. Cancel never deletes local Skills or unrelated remote files.
A foreign same-name directory is a conflict, not permission to overwrite.

## Launch actions / 启动入口

- **Open terminal:** opens the confirmed target and loads the session environment.
- **Open in VS Code:** uses the detected installation, including supported portable/
  shortcut paths, and matching SSH config. Temporary network setup targets remote
  Server settings, not local Windows User Settings.
- **MobaXterm:** uses supported target/bookmark launch behavior. An external shell
  may require the copied session setup command.
- **Copy terminal configuration:** gives a token-free command to execute in the
  authenticated shell. Copy success does not mean the shell ran it.

On a new server, allow Remote - SSH to initialize Server and then retry from
RouteSpan. Reload existing Remote windows and recreate terminals after environment
changes. The Ports panel does not verify RouteSpan's reverse forwarding.

## Overview and Advanced / 概述与高级

Overview has capability switches, readiness, collapsed Skills and launch actions.
Advanced has environment facts, link verification, SSH transport help, security,
events and manual tools. Both read shared backend observations; tab switches do
not reset valid results.

Server direct network is independent and bypasses proxy variables when checked.
First connection does not send a paid model request. Explicit model verification
may consume tokens; failure reasons appear in recent events. A VS Code user-proxy
hint is not proof of effective runtime/profile/workspace settings.

## Reconnect and disconnect / 重连与断开

Transport loss triggers bounded same-target/port/session recovery. Ordinary retries
and success stay silent. Authentication, host-key or configuration changes may
require action. Recovery never replays application requests or substitutes a proxy.

Manual disconnect invalidates recovery and revokes relays before attempting owned
VS Code restoration and session cleanup. Session SSH credentials are cleared.
Enabled tool profiles remain saved; their old token cannot authorize a disconnected
route. Turn a tool off to restore owned fields. Crash/forced exit may leave journals;
conflicting recovery evidence stops cleanup rather than guessing.

## Troubleshooting / 常见问题

| Symptom | Safe next step |
| --- | --- |
| SSH fails / host contains `:22` | Separate host/port; terminal syntax is `ssh -p 22 user@host` |
| New or changed host key | Verify trust; never disable host-key checking |
| Connected but no external internet | Check local proxy, network lane, egress and current session exports |
| AI 401 | Distinguish session rejection from upstream authentication; check CC Switch without sharing keys |
| VS Code 407 / stale proxy | Reload Remote window; check setup and effective profile/workspace overrides |
| VS Code first launch unavailable | Confirm local VS Code/Remote - SSH, allow Server initialization, retry safe setup |
| Conflict / unsafe path | Stop and inspect ownership, recovery evidence and permissions; do not delete unknown backups or broadly chmod shared paths |
| Model list unchanged | Wait for sync then restart/reload; a live client may retain old settings |
| Installed but not configured | Check compatibility, AI switch, per-tool OFF and preparation errors |

Use sanitized reports/events and [Security](../SECURITY.md) for feedback.
Never send private keys, passwords, complete settings or session headers.
Root and same-account hostile processes are outside the isolation guarantee.
