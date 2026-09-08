# v0.2.0 Remote Environment Bridge

Development branch: `v0.2.0dev`, based on v0.1.4 commit `fbf60b1`.
This is development scope, not a published stable-release announcement.

## 使用方式

1. 在“本机环境”选择可用的活动代理，再切换到一级页面“远程桥接”。
2. 从本机 OpenSSH、VS Code Remote 或 MobaXterm 发现结果中选择一个结构化远程目标。请先在自己的终端完成主机指纹验证与密钥/ssh-agent 登录。
3. 检查 SSH 连接。检查通过后，ProxyEnv 会在 `20000–60000` 中自动生成两个互不相同且当时未占用的远程 Loopback 端口。
4. 在能力页分别查看服务器互联网、本机活动代理与 CC Switch AI 路由。三项检测彼此独立：SSH 成功不代表服务器能够联网，普通代理可用也不代表 CC Switch 可用。按需选择桥接本机代理、CC Switch，或同时选择两者。CC Switch 默认检查 `127.0.0.1:15721`，也可输入实际本地路由端口；结果会区分已确认的 CC Switch、身份未知的监听程序和未检测到监听。
5. 预览本机和远端端点。建立连接前会再次检查远程端口；如发生端口竞争，页面会重新生成并要求再次确认。
6. 代理桥接成功后复制环境变量，在远端当前 Shell 主动执行。仅“测试桥接”会经代理请求 `https://www.gstatic.com/generate_204`。
7. CC Switch 桥接成功后，状态页会直接显示 Codex / Claude Code 配置入口和启动命令；预览并应用专用 CLI 接入文件后，再在远端显式使用对应命令。
8. 断开桥接需确认。退出 ProxyEnv 会结束隧道；关闭窗口到托盘仍属同一运行会话。

## Current implementation / 当前实现

| Area | Implementation |
| --- | --- |
| Stack | Existing Vue 3 + TypeScript + Tauri 2 + Rust and OpenSSH. Fixed POSIX Shell operations plus a bundled JavaScript configuration helper using an already-installed remote Node 20+. No Python, new frontend framework, SSH library, runtime installation or daemon. |
| UI | Local Environment and Remote Bridge are peer first-level pages; Assistant remains a Local Environment drill-down and Settings remains global. Remote Bridge uses an independent four-step page and keeps dialogs for protected mutations; Chinese, English, Japanese and Korean. |
| Status UI | Shared `StatusIndicator`, `CheckRow`, `HelpHint`, and `LastChecked` components provide one six-state vocabulary, accessible icon/text feedback, structured help, independent timestamps, and a unified recheck action. |
| Network observations | Server direct internet, the shared local `ActiveProxyContext`, and CC Switch routing are evaluated independently. Direct server testing bypasses proxy variables; a missing remote `curl` becomes Unknown rather than a false success or failure. |
| Active proxy | Reads `active::snapshot()` only. No bridge discovery or secondary selection. Captures revision, local endpoint and protocol; changes become Stale, loss becomes Unavailable. |
| Protocol | HTTP → HTTP_PROXY/HTTPS_PROXY; SOCKS5 → ALL_PROXY with socks5h; Mixed → all three. Unknown is refused. Existing variable mapping is reused. |
| SSH target | Structured targets from `~/.ssh/config`, the default VS Code user `remote.SSH.configFile`, and bounded MobaXterm bookmark sources. IDs bind source, configuration identity, and alias/session name. OpenSSH resolves its own aliases; no private-key or credential contents are read. |
| Port allocation | After SSH verification, distinct remote Loopback ports are selected from `20000–60000` and checked again immediately before connection. A race causes one regeneration and a return to review. |
| CC Switch | Loopback listener ownership is classified as confirmed CC Switch, listening with unknown identity, or not detected. A listening port alone is not treated as service identity. |
| Forward | Explicit `127.0.0.1:remote:loopback:local`, ExitOnForwardFailure, strict host-key checks, BatchMode and bounded connection/keepalive timeouts. |
| Remote listener | Checks remote TCP listeners before creation and validates actual loopback-only listeners after creation. A wildcard/unknown binding closes the new tunnel. |
| Session | One combined target/session at a time. No automatic reconnect. Windows Job Objects close created SSH processes and descendants on process exit, including abnormal exit. |
| Config | CLI uses dedicated files. Opt-in VS Code extension adapters patch shared remote configuration with parser-based edits. Read/validate → preview → confirmation → remote backup → atomic replace → hash readback. Conflicts stop writes and restore. |
| Diagnostics | Structured command errors expose only code, phase, safe target category and retryability. Cached summaries remain allowlisted: no usernames, home paths, keys, secrets, raw SSH stderr or upstream URLs. |

## CLI configuration compatibility

Codex CLI `0.134.0` and later in the `0.x` series uses a separate profile file. The backend checks the remote `codex --version` and refuses earlier/unknown formats instead of guessing. A custom `CODEX_HOME` is refused in this MVP. Profile layering and the version transition were checked against the [official configuration documentation](https://learn.chatgpt.com/docs/config-file/config-advanced).

```text
~/.codex/proxyenv_bridge.config.toml
codex --profile proxyenv_bridge
```

The profile selects a dedicated `proxyenv_bridge` provider, a loopback `/v1` base URL and Responses wire protocol. It does not select a model or read `auth.json`. Provider fields follow the [official configuration reference](https://learn.chatgpt.com/docs/config-file/config-reference).

Claude Code `2.x` uses a dedicated JSON file with `env.ANTHROPIC_BASE_URL` and the public `PROXY_MANAGED` placeholder. It does not change `settings.json` or read authentication files. To prevent the interactive CLI from ignoring gateway settings and returning to the three-way first-run login chooser, the same reviewed apply transaction minimally sets top-level `hasCompletedOnboarding: true` in `~/.claude.json`. Existing fields are retained, malformed or concurrently changed state fails closed, and no project trust entry is created: Claude Code still asks the user to trust the current folder before starting a conversation. A custom `CLAUDE_CONFIG_DIR` is refused. The settings mechanism follows [Claude Code settings](https://code.claude.com/docs/en/settings); routing behavior is described in [CC Switch routing](https://github.com/farion1231/cc-switch/blob/main/docs/user-manual/en/4-proxy/4.2-routing.md).

```text
~/.claude/proxyenv-bridge.json
claude --settings "$HOME/.claude/proxyenv-bridge.json"
```

用户必须用显示的命令主动选择接入配置。已有 CLI 配置、环境变量或受管理策略可能具有更高优先级；文件写入成功不等于已验证模型调用。ProxyEnv 不发送 AI Prompt、不调用模型，也不复制真实 Provider 凭据。

## Supported remote environment

### Structured targets and MobaXterm

Target discovery returns structured records rather than encoded strings such as `vscode:<alias>`. A target ID binds the source, configuration-file identity, and alias/session name, so identically named hosts from different files cannot cross-connect. User-facing cards show the display name, source, and a sanitized configuration-path hint; resolved usernames, identity files, and other connection details are not promoted in the UI.

MobaXterm discovery is deliberately bounded to an active process `-i` argument, an executable-adjacent `MobaXterm.ini`, and the current user's conventional Documents/config locations. Only `[Bookmarks]` and `[Bookmarks_N]` SSH sessions are parsed. Password, credential, and master-password sections are never read or decrypted, and ProxyEnv never performs a full-disk search. A simple session with deterministic host/user/port data can be adapted to Windows OpenSSH arguments. Sessions requiring MobaXterm-only authentication, unsupported key formats, jump behavior, or ambiguous fields remain listed as “recognized, currently unavailable” rather than being guessed.

### VS Code Remote - SSH

提供 Remote - SSH 的同机远程终端与 CLI 接入，以及可选的 Codex / Claude Code 图形化扩展配置适配。扩展适配的实现和本地测试已具备，真实模型链路尚待验收。主机列表同时读取 Windows VS Code 默认用户 `Code/User/settings.json` 的 `remote.SSH.configFile`（支持 JSONC 注释和尾逗号）；来源不同的同名别名使用不同结构化 ID，默认配置与本机 OpenSSH 重合时不重复展示。桥接的 OpenSSH 调用使用对应的 `-F` 配置文件。

在桥接状态页点击“在 VS Code 中打开”，使用本机已安装的 VS Code 以 `--new-window --remote ssh-remote+<alias>` 打开目标。打开前比较 VS Code 与桥接使用的 SSH 配置文件，来源不同则停止，避免同名别名连到不同机器。OpenSSH 有效配置也记录哈希，在建立隧道前后及后续远端写入/联网测试前校验，变化后要求重新建立。

Remote - SSH 与 ProxyEnv 各自管理 SSH 连接；关闭 VS Code 不会关闭 ProxyEnv 桥接，退出 ProxyEnv 会令依赖桥接的远程 CLI 失去连接。在 Remote - SSH 的远程终端粘贴环境变量后运行工具；CLI 的专用配置同样在该远程账户中生效。

The integration follows the [Remote - SSH configuration guide](https://code.visualstudio.com/docs/remote/ssh) and [VS Code command line reference](https://code.visualstudio.com/docs/configure/command-line). VS Code Server and already-running remote extension hosts do not inherit later terminal exports. ProxyEnv does not claim to configure every extension's network stack, install extensions, restart VS Code Server, or edit shell startup files. The local Windows OpenSSH client is used; custom VS Code SSH executables, profiles, Insiders and portable settings are not imported automatically.

### Codex / Claude Code 图形化扩展配置（开发中，待实机验收）

先阅读[实现可行性审计](REMOTE_BRIDGE_EXTENSION_AUDIT.md)。2026-09-07 起，配置入口可分别选择 CLI、VS Code Extension 或两者；每次默认只选择 CLI。

1. 在已连接且 CC Switch 可用的桥接状态页选择“配置 Codex”或“配置 Claude Code”。
2. 勾选图形化扩展后，点击“检查远端扩展”。只读取默认 `~/.vscode-server/extensions` 的官方扩展元数据；不会安装、卸载或更新扩展。
3. 在目标 Remote - SSH 窗口运行 `Developer: Show Running Extensions`，确认扩展运行在界面所示远端账户，再核对实际配置路径。用户勾选确认后才可预览；这项状态是用户确认，不是自动进程证明。
4. 逐文件预览修改与影响范围，确认后执行。CLI 和扩展是独立文件事务；若后续文件失败，界面保留先前成功结果，不宣称跨文件原子成功。
5. 重载 VS Code 窗口并新建扩展会话。状态“已写入 · 待重载及模型验收”只表示配置文件经读回验证。
6. 恢复入口也可选择 CLI 或扩展，在未建立隧道时仍可针对同一 SSH 别名恢复。共享文件或备份被第三方修改时拒绝覆盖。

| 扩展 | 受控修改 | 必须了解的影响 |
| --- | --- | --- |
| Codex | `~/.codex/config.toml` 的 `model_provider`，以及新建的 `model_providers.proxyenv_bridge`；端点 `http://127.0.0.1:<port>/v1`、Responses 协议 | 同账户其他使用默认配置的 Codex 会话也受影响；保留 model、权限、MCP 等未知字段及注释。已有同名 provider 或旧 profile 选择器会冲突。 |
| Claude Code | `~/.vscode-server/data/Machine/settings.json` 中 `claudeCode.environmentVariables` 的两个新增项：`ANTHROPIC_BASE_URL`、公开占位值 `ANTHROPIC_AUTH_TOKEN=PROXY_MANAGED` | 只针对默认稳定版 VS Code Server 的 Remote Settings。已有路由/凭据项不覆盖；共享 Claude 用户设置或非交互 SSH 环境存在冲突路由时也停止。实际工作区与受管理策略仍需实机核验。 |

“只选择扩展”表示只执行扩展的配置适配，并不保证共享 Codex 默认配置对其他 CLI 无影响。无需修改 `.env`、复制 `auth.json` 或任何真实 Provider Secret。PROXY_MANAGED 的可用性取决于实际 CC Switch 接入方式，不能作为通用网关认证。

扩展检测与适配要求：远端现有 Node 20+，安装在 `/usr/bin/node`、`/usr/local/bin/node`，或默认稳定版 VS Code Server 自带位置。使用固定脚本通过 SSH stdin 一次性运行，不新增远端运行时、不修改 VS Code Server、不创建后台服务。自定义 Server 目录、Insiders、多个并存安装版本、无法识别的扩展 runtime 均停止自动配置。可检查到安装不代表已确认激活位置。

Codex 扩展单独检查内置 `bin/linux-<architecture>/codex --version`，不依赖 PATH 中是否另装 CLI；目前格式适配范围为 `0.x` 且 minor ≥134，允许记录预发布版本，具体版本组合仍需验收。Claude 扩展使用已安装的 `2.x` manifest 识别配置能力。该范围是配置适配门槛，不是对所有版本已完成兼容测试的声明。

解析在远端进行，原始配置、备份及未知字段不传回 Windows。JS 解析器按语法节点插入/替换目标内容，保留其他字节；不兼容的 TOML 内联表形态、JSONC 重复键、非法 UTF-8 和未知目标类型都失败关闭。凭据字段可能作为远端文件的不透明字节参与保留和备份，但不提取、不展示、不回传、不用于请求。

扩展事务使用同目录 `.proxyenv-extension-original`、`.proxyenv-extension-state` 和临时 `.proxyenv-extension-lock`，写入权限为 0600。预览令牌有效期五分钟，并绑定别名的有效 SSH 配置、账户与 runtime 指纹、文件和恢复记录哈希。正常错误回滚并验证；进程被强制终止可留下 prepared 记录或锁，此时停止自动写入，需在远端检查恢复证据。不会自动删除来历不明或中断留下的锁。

断开桥接保留 loopback 配置，不自动改回原 provider。用户显式恢复后可能重新使用原路由。此功能不提供整个远端系统的网络隔离，也不保证其他插件、遥测、MCP 或用户更改的配置不联网。

### Remote prerequisites

- Linux with a non-root SSH account, POSIX `sh`, `ss` (iproute2), `flock` (util-linux), and standard GNU/coreutils tools including `sha256sum`, `stat`, `sync -f`, `mktemp`, `cmp`, `sed`, `grep`, `cut`, `cp`, `mv`, `cat`, `unlink`.
- `timeout` and a supported CLI already in the noninteractive SSH PATH are required for CLI configuration. `curl` is needed only for the explicit external network test.
- Existing working key/agent authentication and verified host keys. Password entry and first-use host-key acceptance are done in the user's terminal.
- No existing LocalForward/RemoteForward/DynamicForward in the selected effective SSH configuration. These are rejected so the new connection opens only reviewed ports. Use a separate alias with no inherited forwards.
- The remote home/config path and recovery files must not be symlinks, have another owner, or be group/world-writable. Only exact ProxyEnv-generated overlays may be read back or replaced; unknown contents fail closed.

Explicit aliases inside complex Include/Match configurations are not enumerated in this MVP. Interactive passwords, remote Windows/macOS, custom CLI home directories, older Codex profiles, service identity attestation, AI request verification, permanent tunnels and automatic reconnect are outside this implementation.

## Recovery and conflict behavior

Backups stay beside the dedicated remote file (`.proxyenv-original`, if an original existed), with an applied-hash marker (`.proxyenv-applied`) and an advisory lock (`.proxyenv-lock`). The original backup is retained across subsequent changes. No full user configuration or secrets are copied locally.

恢复入口在连接前的主机选择页及桥接状态页都可使用，因此重启 ProxyEnv 后也能从相同别名恢复。预览与应用之间、应用与恢复之间出现文件变动时均停止覆盖。备份变化也会使恢复预览失效。

Writes use same-directory temporary files, flush, atomic rename and readback. Normal write failures attempt to restore the pre-operation file and marker, then verify the hash. If a third party changes the destination during the transaction, the operation retains recovery evidence and refuses to overwrite that change. A hard interruption can leave recovery markers/temp files; inconsistent evidence is refused and requires inspection on the remote host. Advisory locks cannot stop unrelated editors that ignore them.

## Validation

```sh
pnpm check
pnpm build:remote
pnpm test:extensions
cargo fmt --manifest-path src-tauri/Cargo.toml --all -- --check
cargo clippy --manifest-path src-tauri/Cargo.toml --all-targets --locked -- -D warnings
```

`pnpm test:remote` uses Node's existing test runner and a POSIX shell. On Windows it finds Git Bash beside the installed Git executable; `PROXYENV_TEST_SHELL` can override the test shell. It exercises real temporary-file/hash/replace/restore behavior. SSH transport, `ss` output, CLI version output and `flock` are fixture adapters; Windows permissions are also adapted. These tests do not claim to test Linux locking or live SSH server policy.

Browser checks use the existing development preview and mocked IPC: host selection, capabilities, review before mutation, apply/restore confirmation, disconnect cancellation, stale state, ESC/focus restoration, dark theme and 560px window layout. No browser mock is included in production source.

Real Windows → Linux SSH forwarding, ProxyJump authentication, server forwarding rejection, Windows shutdown cleanup, and actual Codex/Claude routing still require acceptance on a user-provided test host. No real SSH target or model API was used during implementation.

`pnpm test:extensions` 同时检查远端 bundle 与源代码一致，并覆盖无损增量修改、重复键/凭据冲突、预览后第三方修改、恢复、原子替换故障回滚和硬链接拒绝。Windows 测试不证明 Linux 的权限/锁语义；生产入口仅允许非 root Linux。模拟 IPC 的浏览器回归覆盖范围选择、远端确认门槛、写入前预览、部分成功、恢复及键盘焦点。

发布前必须分别完成两种 CLI 和两种图形化扩展的真实模型请求、Provider A → B 切换、断连请求失败，以及冲突恢复测试。当前这些实机条目仍未执行，不应发布“VS Code AI extensions supported”的稳定版声明。

2026-09-07 本机验证记录：Vue/TypeScript 构建、远程桥接测试、扩展测试、Rust 测试、Rust 格式检查与 Clippy 均通过；Linux 专用用例在 Windows 按预期跳过。已添加独立 Ubuntu CI 作业运行 Linux 权限、软链接和完整 helper 流程测试，该 CI 作业尚未在本轮远端执行。Remote Bridge 的桌面、窄窗口和连接后状态已完成静态视觉复核，评审范围不包含真实 SSH 或模型请求；准确测试数量以本次验证输出与 CI 为准。

## Source map

- `src-tauri/src/features/remote_bridge/`: Rust session/state/preview management, OpenSSH process ownership and audited remote operations.
- `src-tauri/src/commands/remote_bridge.rs`: typed Tauri commands; blocking work runs outside the UI thread.
- `src/features/remote-bridge/`: first-level page state, workflow, and protected tool dialogs.
- `src/shared/i18n/remote-bridge.ts`: four-language messages and safe error categories.
- `scripts/test-remote-bridge.mjs`: isolated regression tests.
- `scripts/remote-extension/`: fixed remote JSONC/TOML patcher, file transactions and read-only inspection.
- `scripts/build-remote-extension.mjs`: bundles parsers and their MIT notices into the Rust-embedded helper; run after changing helper source.
- `scripts/test-remote-extension.mjs`: parser and transaction regression tests.
