# RouteSpan getting started / 快速开始

English · [简体中文](GETTING_STARTED.zh-CN.md)

This guide describes the RouteSpan v0.2.1 stable UI. Install an
[official v0.2.1 package](https://github.com/GKNEETIEMAY/ProxyEnv/releases/tag/v0.2.1).
Existing RouteSpan and legacy ProxyEnv NSIS users can use About → Check for updates.
v0.1.4 is a public historical release. A preview is not a signed update release.

## Before installing / 安装前

- Local machine: Windows 10/11 x64. End users do not need Node.js or Rust.
- WebView2 is required; the installer may download it if absent.
- Use your own local proxy client for network bridging, and CC Switch for AI routing.
- Remote access requires Windows OpenSSH and an accessible non-root Linux account.
  The server must provide `ss`, `flock` and coreutils. Managed tool JSON operations
  support Python 2.7 or Python 3; supported runtime/file checks still apply.
- VS Code integration requires VS Code and Remote - SSH. MobaXterm is optional.

Exit the old app and tray instance before manual installation. The retained app
identity means an installer may replace the previous installation, not create a
parallel copy. Back up important settings before upgrades. Builds without Authenticode
may show Unknown Publisher/SmartScreen warnings: verify source and checksums, and
do not globally disable protection. See [release security](release-security.md).

## First launch / 首次启动

Choose language/theme in Settings if needed. Local Environment and Remote Bridge
are the two primary workspaces. Settings returns to the workspace that opened it;
the title-bar diagnostic report is global. Closing may hide to tray according to
your saved preference.

## Local Environment / 本机环境

1. Start the local proxy client you already use.
2. Confirm a listening candidate and select one active endpoint.
3. Review System Proxy, TUN and environment-variable observations independently.
4. Apply proxy variables only if wanted; clear/restore using explicit actions.
   Existing applications do not inherit later changes.
5. Open Application Assistant for a selected application's diagnosis or a new
   proxy/direct launch. It does not automatically reconfigure arbitrary applications.

Automatic refresh does not silently replace an unavailable selection. Re-select
a valid endpoint rather than relying on stale variables.

## First remote connection / 首次远程连接

1. Choose an imported target or add an SSH connection. Default SSH port is 22.
2. Choose **Bridge local network**, **CC Switch AI Route**, or both.
3. For AI, enable **CC Switch → 设置 → 路由 → 本地路由 → 路由总开关 →
   开启 Claude/Codex 路由** first. A listener alone does not prove both routes work.
4. Connect. Reuse a key/Agent, or answer the real OpenSSH password/KBI prompt.
   Confirm a first-seen fingerprint only after checking trust. Changed keys fail.
5. Core forwarding verification yields Connected Overview. Optional tool and
   environment preparation continue independently.

A terminal SSH check uses `ssh -p 22 user@host`, not `ssh user@host:22`.
Host and port are separate UI fields. Editing an imported target creates a saved
copy; removing it hides it without rewriting the source configuration.

## Tools and Skills / 工具与 Skills

Installed, Configured and Verified are different observations. CLI and VS Code
plugins are detected independently. With AI routing enabled, compatible discovered
tools are automatically prepared unless explicitly switched off for this session.
A compatible remote plugin does not require installing a standalone CLI.

The installation dialog offers user-executed commands, terminal and Recheck;
it never automatically installs software. Native Claude installation is preferred.
If `~/.local/bin` is missing from PATH, use its session/PATH guidance. A persistent
shell edit is your optional manual action, not RouteSpan's default. Codex npm
installations can also be outside PATH; check the prefix and active shell.

Claude npm guidance retains Node.js 18+ with npm as the project threshold and
separately labels the reviewed official recommendation as 22+. That lower
threshold does not guarantee the newest npm package works on Node.js 18.

Expand Overview's collapsed Skills manager to search and choose each agent's
synchronization. Only CC Switch-managed links are eligible; native Skills and
Codex `.system` are not copied. Cancel removes only owned remote copies.

## Launch and verify / 启动与验证

- **Open terminal** loads the session environment.
- **Open in VS Code** opens the confirmed SSH target and attempts reversible remote
  network setup. A new server may need Remote - SSH Server initialization first.
  Reload existing Remote windows and create a new terminal as instructed.
- **MobaXterm / external SSH** may need the copied terminal setup command executed
  in the authenticated remote shell before launching tools.

Network bridging performs one automatic egress check when ready; this cannot
prove every website works. Built-in model verification supports installed Claude
CLI, is opt-in and may consume tokens; check Codex with its actual client instead.
Advanced contains independent results and recent events. The VS Code Ports panel
is not the authoritative status of this reverse bridge.

## Disconnect and recovery / 断开与恢复

Transport loss triggers bounded same-target recovery; ordinary retry and success
stay silent. Authentication, host trust or configuration changes may require action.
Interrupted application requests are not replayed.

Disconnect revokes relays first, clears session SSH credentials and attempts owned
VS Code network restoration and session-export cleanup. Enabled tool profiles are
retained; turn off an individual tool to restore its owned fields. Changed recovery
evidence stops restoration instead of overwriting user changes. Old session tokens
cannot authorize a disconnected route.

## Next / 下一步

The [Remote Bridge guide](REMOTE_BRIDGE.md) is the current remote workflow source.
Use Advanced and the safe diagnostic report for feedback. Report generation does
not initiate new network tests. Never send keys, passwords, session headers or
complete private configuration in an issue.
