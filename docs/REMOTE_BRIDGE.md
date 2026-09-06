# v0.2.0 Remote Environment Bridge

Development branch: `v0.2.0dev`, based on v0.1.4 commit `fbf60b1`.
This is development scope, not a published stable-release announcement.

## 使用方式

1. 在 Windows OpenSSH 的 `~/.ssh/config` 中配置明确的 Host 别名，先在自己的终端完成主机指纹验证与密钥/ssh-agent 登录。
2. 在首页选择可用的活动代理，打开应用助手入口下方的“远程环境桥接”。
3. 选择别名并检查连接。选择本机代理、CC Switch，或同时选择两者。
4. CC Switch 默认检查 `127.0.0.1:15721`，也可输入实际本地路由端口。检查结果仅表示端口监听，用户需确认服务身份和相应 CLI 的路由已开启。
5. 预览本机和远端端点。默认远程端口为 `17897` / `25721`，确认后创建 SSH reverse forward。
6. 代理桥接成功后复制环境变量，在远端当前 Shell 主动执行。仅“测试桥接”会经代理请求 `https://www.gstatic.com/generate_204`。
7. CC Switch 桥接成功后可预览并应用专用 CLI 接入文件；复制显示的启动命令在远端使用。
8. 断开桥接需确认。退出 ProxyEnv 会结束隧道；关闭窗口到托盘仍属同一运行会话。

## Current implementation / 当前实现

| Area | Implementation |
| --- | --- |
| Stack | Existing Vue 3 + TypeScript + Tauri 2 + Rust; existing OpenSSH; bundled fixed POSIX Shell operations on the remote host. No Python runtime, new frontend framework, SSH library or daemon. |
| UI | Existing Home / Assistant / Settings and header remain. One matching Home entry and independent four-step native dialog; Chinese, English, Japanese and Korean. |
| Active proxy | Reads `active::snapshot()` only. No bridge discovery or secondary selection. Captures revision, local endpoint and protocol; changes become Stale, loss becomes Unavailable. |
| Protocol | HTTP → HTTP_PROXY/HTTPS_PROXY; SOCKS5 → ALL_PROXY with socks5h; Mixed → all three. Unknown is refused. Existing variable mapping is reused. |
| SSH target | Explicit aliases from `~/.ssh/config` and the default VS Code user `remote.SSH.configFile`; OpenSSH resolves HostName, User, Port, IdentityFile and ProxyJump. No private-key contents are read. |
| Forward | Explicit `127.0.0.1:remote:loopback:local`, ExitOnForwardFailure, strict host-key checks, BatchMode and bounded connection/keepalive timeouts. |
| Remote listener | Checks remote TCP listeners before creation and validates actual loopback-only listeners after creation. A wildcard/unknown binding closes the new tunnel. |
| Session | One combined target/session at a time. No automatic reconnect. Windows Job Objects close created SSH processes and descendants on process exit, including abnormal exit. |
| Config | Dedicated files only. Read/validate → preview → confirmation → remote backup → replace → hash readback. Restore has its own preview token and current/backup hash checks. |
| Diagnostics | Cached, allowlisted summaries only; no aliases, usernames, home paths, keys, secrets, raw SSH stderr or upstream URLs. |

## CLI configuration compatibility

Codex CLI `0.134.0` and later in the `0.x` series uses a separate profile file. The backend checks the remote `codex --version` and refuses earlier/unknown formats instead of guessing. A custom `CODEX_HOME` is refused in this MVP. Profile layering and the version transition were checked against the [official configuration documentation](https://learn.chatgpt.com/docs/config-file/config-advanced).

```text
~/.codex/proxyenv_bridge.config.toml
codex --profile proxyenv_bridge
```

The profile selects a dedicated `proxyenv_bridge` provider, a loopback `/v1` base URL and Responses wire protocol. It does not select a model or read `auth.json`. Provider fields follow the [official configuration reference](https://learn.chatgpt.com/docs/config-file/config-reference).

Claude Code `2.x` uses a dedicated JSON file with `env.ANTHROPIC_BASE_URL` and the public `PROXY_MANAGED` placeholder. It does not change `settings.json` or read authentication files. A custom `CLAUDE_CONFIG_DIR` is refused. The settings mechanism follows [Claude Code settings](https://code.claude.com/docs/en/settings); routing behavior is described in [CC Switch routing](https://github.com/farion1231/cc-switch/blob/main/docs/user-manual/en/4-proxy/4.2-routing.md).

```text
~/.claude/proxyenv-bridge.json
claude --settings "$HOME/.claude/proxyenv-bridge.json"
```

用户必须用显示的命令主动选择接入配置。已有 CLI 配置、环境变量或受管理策略可能具有更高优先级；文件写入成功不等于已验证模型调用。ProxyEnv 不发送 AI Prompt、不调用模型，也不复制真实 Provider 凭据。

## Supported remote environment

### VS Code Remote - SSH

兼容 Remote - SSH 的同机远程终端和 Codex / Claude CLI。主机列表同时读取 Windows VS Code 默认用户 `Code/User/settings.json` 的 `remote.SSH.configFile`（支持 JSONC 注释和尾逗号）；来源不同的同名别名分开展示，VS Code 来源带有标识。桥接的 OpenSSH 调用使用对应的 `-F` 配置文件。

在桥接状态页点击“在 VS Code 中打开”，使用本机已安装的 VS Code 以 `--new-window --remote ssh-remote+<alias>` 打开目标。打开前比较 VS Code 与桥接使用的 SSH 配置文件，来源不同则停止，避免同名别名连到不同机器。OpenSSH 有效配置也记录哈希，在建立隧道前后及后续远端写入/联网测试前校验，变化后要求重新建立。

Remote - SSH 与 ProxyEnv 各自管理 SSH 连接；关闭 VS Code 不会关闭 ProxyEnv 桥接，退出 ProxyEnv 会令依赖桥接的远程 CLI 失去连接。在 Remote - SSH 的远程终端粘贴环境变量后运行工具；CLI 的专用配置同样在该远程账户中生效。

The integration follows the [Remote - SSH configuration guide](https://code.visualstudio.com/docs/remote/ssh) and [VS Code command line reference](https://code.visualstudio.com/docs/configure/command-line). VS Code Server and already-running remote extension hosts do not inherit later terminal exports. ProxyEnv does not claim to configure every extension's network stack, install extensions, restart VS Code Server, or edit shell startup files. The local Windows OpenSSH client is used; custom VS Code SSH executables, profiles, Insiders and portable settings are not imported automatically.

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
cargo fmt --manifest-path src-tauri/Cargo.toml --all -- --check
cargo clippy --manifest-path src-tauri/Cargo.toml --all-targets --locked -- -D warnings
```

`pnpm test:remote` uses Node's existing test runner and a POSIX shell. On Windows it finds Git Bash beside the installed Git executable; `PROXYENV_TEST_SHELL` can override the test shell. It exercises real temporary-file/hash/replace/restore behavior. SSH transport, `ss` output, CLI version output and `flock` are fixture adapters; Windows permissions are also adapted. These tests do not claim to test Linux locking or live SSH server policy.

Browser checks use the existing development preview and mocked IPC: host selection, capabilities, review before mutation, apply/restore confirmation, disconnect cancellation, stale state, ESC/focus restoration, dark theme and 560px window layout. No browser mock is included in production source.

Real Windows → Linux SSH forwarding, ProxyJump authentication, server forwarding rejection, Windows shutdown cleanup, and actual Codex/Claude routing still require acceptance on a user-provided test host. No real SSH target or model API was used during implementation.

## Source map

- `src-tauri/src/features/remote_bridge/`: Rust session/state/preview management, OpenSSH process ownership and audited remote operations.
- `src-tauri/src/commands/remote_bridge.rs`: typed Tauri commands; blocking work runs outside the UI thread.
- `src/features/remote-bridge/`: frontend state and two components.
- `src/shared/i18n/remote-bridge.ts`: four-language messages and safe error categories.
- `scripts/test-remote-bridge.mjs`: isolated regression tests.
