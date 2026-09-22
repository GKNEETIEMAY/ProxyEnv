# v0.2.0 Remote Environment Bridge — historical implementation notes

> Archived 2026-09-22. This document mixes implementation snapshots and prior acceptance assumptions. Use [the current Remote Bridge entry](../remote-bridge/README.md), [scope](../remote-bridge/V0.2_SCOPE.md) and [acceptance](../remote-bridge/ACCEPTANCE_V0.2.md) for present status; do not treat all statements below as current product guarantees.

Development branch: `v0.2.0dev`, based on v0.1.4 commit `fbf60b1`.
This is development scope, not a published stable-release announcement.

## 使用方式

1. 在“本机环境”选择可用的活动代理，再切换到一级页面“远程桥接”。
2. 从本机 OpenSSH、VS Code Remote 或 MobaXterm 发现结果中选择一个结构化远程目标。新主机仍需先在自己的终端确认主机指纹。
3. 检查 SSH 连接。ProxyEnv 先复用密钥、IdentityFile 或 ssh-agent 做非交互认证；仅当 OpenSSH 明确要求认证时，才显示应用内交互窗口承载密码或 Keyboard Interactive / PAM 提示。普通代理优先匹配安全识别到的本机 VS Code 用户 `http.proxy` 端口；没有明确线索时优先镜像当前活动代理端口。AI 路由独立优先使用 `15721`。远端占用时按 SSH 目标选择可重复的备用端口，而不是覆盖已有监听。
4. 在能力页分别查看服务器互联网、本机活动代理与 CC Switch AI 路由。三项检测彼此独立：SSH 成功不代表服务器能够联网，普通代理可用也不代表 CC Switch 可用。按需选择桥接本机代理、CC Switch，或同时选择两者。CC Switch 默认检查 `127.0.0.1:15721`，也可输入实际本地路由端口；结果会区分已确认的 CC Switch、身份未知的监听程序和未检测到监听。
5. 预览本机和远端端点。建立连接前会再次检查远程端口；如发生端口竞争，页面会选择备用端口并要求再次确认。
6. 代理桥接成功后，点击“启动代理终端”即可打开新的 PowerShell 窗口，由系统 OpenSSH 连接远端并自动注入代理环境。符合条件的普通服务器密码只在本次桥接中以 Windows 当前用户 DPAPI 密文缓存，切换目标、断开或退出即清除；私钥口令、OTP 和未知挑战不缓存。已经打开的 SSH、VS Code Remote 或 MobaXterm 终端可展开高级入口，复制环境变量后在当前 Shell 执行。仅“测试桥接”会经代理请求 `https://www.gstatic.com/generate_204`。
7. CC Switch 桥接成功本身不会修改 Codex / Claude Code 配置。两者各有一个启用/停用开关，启用时内部校验并备份后写入共享用户配置，关闭时恢复接入前的托管字段，同时保留无关后续修改。后续使用同一稳定端口重连时会自动识别开关状态。启用后可直接运行 `codex` 或 `claude`。
8. 断开桥接需确认。退出 ProxyEnv 会结束隧道；关闭窗口到托盘仍属同一运行会话。

## Current implementation / 当前实现

| Area | Implementation |
| --- | --- |
| Stack | Existing Vue 3 + TypeScript + Tauri 2 + Rust and OpenSSH. Fixed POSIX Shell operations; Codex TOML and Claude JSON shared-settings merges use an existing Python 2.7 or Python 3 interpreter, while VS Code extension configuration uses an existing remote Node 20+. ProxyEnv installs no runtime, SSH library or daemon. |
| UI | Local Environment and Remote Bridge are peer first-level pages that remain mounted after first use and switch without cross-fades. Assistant remains a Local Environment drill-down and Settings remains global. Remote Bridge uses one continuous setup/status workspace without a step rail or separate tunnel-review page; dialogs remain for OpenSSH interaction and protected mutations. Chinese, English, Japanese and Korean are supported. |
| Status UI | Shared `StatusIndicator`, `CheckRow`, `HelpHint`, and `LastChecked` components provide one six-state vocabulary, accessible icon/text feedback, structured help, independent timestamps, and a unified recheck action. |
| Network observations | Server direct internet, the shared local `ActiveProxyContext`, and CC Switch routing are evaluated independently. Direct server testing bypasses proxy variables; a missing remote `curl` becomes Unknown rather than a false success or failure. |
| Active proxy | Reads `active::snapshot()` only. No bridge discovery or secondary selection. Captures revision, local endpoint and protocol; changes become Stale, loss becomes Unavailable. |
| Protocol | HTTP → HTTP_PROXY/HTTPS_PROXY; SOCKS5 → ALL_PROXY with socks5h; Mixed → all three. NO_PROXY always covers localhost loopback. The managed snippet clears stale values before applying the selected mapping; Unknown is refused. |
| SSH target | Structured targets from `~/.ssh/config`, the default VS Code user `remote.SSH.configFile`, and bounded MobaXterm bookmark sources. IDs bind source, configuration identity, and alias/session name. OpenSSH resolves its own aliases; no private-key or credential contents are read. |
| Port allocation | The local upstream always comes from the selected Active Proxy. The remote general listener prefers an explicit, safe default VS Code user `http.proxy` port, otherwise the active local port; it may differ from the local upstream. AI routing independently prefers `15721`. An occupied port uses a deterministic target-specific fallback. If the VS Code consumer port is occupied, that fallback serves managed terminals but cannot be presented as extension-ready. Ports are checked again immediately before connection. |
| CC Switch | Loopback listener ownership is classified as confirmed CC Switch, listening with unknown identity, or not detected. A listening port alone is not treated as service identity. |
| SSH auth | `BatchMode=yes` remains the first path for IdentityFile / ssh-agent. Authentication failures may opt into a short-lived OpenSSH session hosted by Windows ConPTY with `BatchMode=no`, password and keyboard-interactive enabled. |
| Forward | Explicit `127.0.0.1:remote:loopback:local`, ExitOnForwardFailure, strict host-key checks and bounded connection/keepalive timeouts. Interactive forwarding is not accepted until the remote listeners are verified as loopback-only. |
| Remote listener | Checks remote TCP listeners before creation and validates actual loopback-only listeners after creation. A wildcard/unknown binding closes the new tunnel. |
| Session | One combined target/session at a time. No automatic reconnect. The bridge SSH process uses the existing Windows Job Object lifecycle; authentication OpenSSH is owned by a short-lived ConPTY session. A user-launched proxy terminal is a separate visible OpenSSH process whose network route still depends on the active bridge. |
| Config | Codex CLI and Claude CLI use their standard shared user configuration so plain `codex` and `claude` inherit the bridge route. Each switch internally validates, backs up, atomically writes and verifies the shared configuration. Disabling restores managed fields while preserving unrelated later edits. A verified legacy Codex Extension transaction is adopted automatically. Reconnect reads the ownership markers to restore switch state. |
| Client profile synchronization | No credential, provider URL, API key, `auth.json`, MCP setting or unrelated configuration is copied. While the bridge is stopped there is no polling. Each enabled client checks only its local config (and Codex catalog) modification metadata every two seconds. A change triggers a 500 ms debounce, safe parse and hash; managed fields are reconciled without overwriting unrelated remote edits. Concurrent writes during replacement still fail closed. Claude displays its separate synchronization state below the access switch; this never substitutes for model-request verification. |
| Runtime relay | The local loopback relay is transport-only. It forwards the request body and response stream without changing `model`, tool declarations/calls, reasoning fields or protocol semantics. Model identity and capability metadata are made consistent before launch by profile synchronization, not repaired per request. |
| Tool adapters | Codex CLI and Claude Code CLI use a shared Rust/TypeScript `RemoteToolAdapter` contract for detection, inspection, compatibility, preview, apply, restore, launch and verification state. Each CLI and its VS Code Remote Extension share one remote user Profile transaction; the switches do not write separate extension settings. Page code iterates the registry and unknown IDs fail before SSH. |
| Diagnostics | Structured command errors expose only code, phase, safe target category and retryability. Cached summaries remain allowlisted: no usernames, home paths, keys, secrets, raw SSH stderr or upstream URLs. |

## CLI configuration compatibility

Codex CLI `0.134.0` and later in the `0.x` series uses its standard user configuration. The backend checks the remote `codex --version` and refuses earlier/unknown formats instead of guessing. A custom `CODEX_HOME` is refused in this MVP. Configuration layering and the version transition were checked against the [official configuration documentation](https://learn.chatgpt.com/docs/config-file/config-advanced).

```text
~/.codex/config.toml
codex
```

The switch synchronizes the local selected Codex model and referenced model catalog into the remote shared configuration. The opaque catalog is written as `~/.codex/proxyenv-codex-model-catalog.json`; its unknown and future fields are preserved byte for byte. The shared config selects that catalog, the same model name, and a dedicated `proxyenv_bridge` provider with a loopback `/v1` base URL, Responses wire protocol, `requires_openai_auth = false` and `supports_websockets = false`. Authentication remains the responsibility of the local CC Switch route, so no provider credential is copied to the server.

Synchronization is lifecycle-bound. No filesystem observation runs before the Codex bridge is active. Once active and configured, ProxyEnv reads only mtime/size metadata every two seconds. After a change, it waits 500 ms for atomic editor saves to settle, then opens both files once, validates the catalog and selected model, computes a profile hash, and reconciles the managed remote Profile fields while retaining unrelated remote edits. Only a write racing the current atomic transaction is rejected. After a successful update the UI requests a remote Codex restart.

For `/responses` and `/v1/responses`, the local relay is a pure byte-preserving transport to CC Switch. It does not parse DSML, reinterpret tool calls, resolve models or rewrite `request.model`. Request structure, tools, reasoning fields, response status/headers/body, SSE framing and upstream errors pass through unchanged. Earlier development profiles are migrated only when their recovery marker and backup prove ownership; otherwise ProxyEnv stops without guessing. Provider fields follow the [official configuration reference](https://learn.chatgpt.com/docs/config-file/config-reference).

Claude Code `2.x` uses its standard user file `~/.claude/settings.json`, shared with its VS Code Remote extension. ProxyEnv projects only local model selection, available models, model overrides, effort choice and allowlisted model-name environment fields into the remote file. Local credentials and upstream URLs are excluded. It preserves unrelated remote fields, then sets `ANTHROPIC_BASE_URL` and the public `ANTHROPIC_AUTH_TOKEN=PROXY_MANAGED` placeholder. Conflicting remote authentication environment keys are removed from the managed replacement after the original file is backed up, and restored on disable. ProxyEnv does not modify `~/.claude.json`, pre-complete onboarding or grant project trust. A custom `CLAUDE_CONFIG_DIR` is refused. JSON merge accepts Python 2.7 or Python 3. See [Claude Code settings](https://code.claude.com/docs/en/settings). Extension authentication and live model behavior still need a real VS Code Remote acceptance test.

```text
~/.claude/settings.json
claude
```

Claude 用户配置完成后，普通 `claude` 启动会读取该用户级路由。项目、本地、命令行、进程环境变量或受管理策略仍可能按 Claude 的优先级覆盖它；文件写入成功不等于已验证模型调用。ProxyEnv 仅在用户点击“验证 Claude 请求”时使用临时最小配置发送固定的非敏感 Prompt；该操作可能消耗少量额度。验证会禁用工具、MCP 与会话持久化，模型与错误原文均不回传、不保存。

两种 CLI 的检测、检查、兼容性判断、预览、启用、停用、启动命令与验证状态均通过统一 `RemoteToolAdapter` 注册表提供。重新连接时会读取远端 ProxyEnv 状态标记并核对当前稳定端口；匹配时恢复“已启用”界面状态，不要求用户再次修改配置。状态快照中的“已配置”只会进入“待验证”，不会直接显示为“接入完成”。M6 已为 Claude CLI 实现显式的真实请求验证：只有收到预期模型响应才显示“模型请求已验证”；仍需登录、路由不可用、超时和其它失败分别保留为非绿色状态。Codex CLI 仍保持“待验证”，不会伪装成已验收。

CC Switch AI 路由与普通网络代理是两项独立能力：用户可以只桥接 AI 路由，不必同时桥接 `HTTP_PROXY` / `ALL_PROXY`。但远端的 `127.0.0.1:<AI 端口>` 来自 SSH Reverse Forward，因此真正从服务器使用 AI 路由时，ProxyEnv 的该 SSH 桥接仍必须保持连接。断开不会删除已启用的 CLI 配置，只会使对应远端 Loopback 入口暂时不可达。

## Supported remote environment

### Structured targets and MobaXterm

Target discovery returns structured records rather than encoded strings such as `vscode:<alias>`. A target ID binds the source, configuration-file identity, and alias/session name, so identically named hosts from different files cannot cross-connect. User-facing cards show the display name, source, and a sanitized configuration-path hint; resolved usernames, identity files, and other connection details are not promoted in the UI.

MobaXterm discovery is deliberately bounded to an active process `-i` argument, an executable-adjacent `MobaXterm.ini`, and the current user's conventional Documents/config locations. Only `[Bookmarks]` and `[Bookmarks_N]` SSH sessions are parsed. Password, credential, and master-password sections are never read or decrypted, and ProxyEnv never performs a full-disk search. A simple session with deterministic host/user/port data can be adapted to Windows OpenSSH arguments. Sessions requiring MobaXterm-only authentication, unsupported key formats, jump behavior, or ambiguous fields remain listed as “recognized, currently unavailable” rather than being guessed.

### VS Code Remote - SSH

提供 Remote - SSH 的同机远程终端、CLI 接入，以及 Codex / Claude Code 图形化扩展的共享用户配置入口。各自的 CLI 和扩展使用同一个远端用户配置事务；当前开关不写第二套扩展 Remote Settings。扩展的登录检查与真实模型链路仍待实机验收，不能仅凭共享文件写入宣称可用。主机列表同时读取 Windows VS Code 默认用户 `Code/User/settings.json` 的 `remote.SSH.configFile`（支持 JSONC 注释和尾逗号）；来源不同的同名别名使用不同结构化 ID，默认配置与本机 OpenSSH 重合时不重复展示。桥接的 OpenSSH 调用使用对应的 `-F` 配置文件。

目标选择页只提供配置查看与检查连接。桥接建立后继续复用既有“启动代理终端”流程，它统一处理 PowerShell、当前桥接、会话凭据与代理环境注入，不再次要求密码。高级区域集中提供不注入变量的手动终端、MobaXterm 与 VS Code 入口；用户在对应客户端连接同一远程环境后，再复制并执行所示 `export`。ProxyEnv 不向第三方客户端传递缓存的 SSH 密码。VS Code 来源可打开本机 VS Code Settings；MobaXterm 来源可打开配置位置。无法桥接的目标仍可选择并查看配置，但“检查连接”保持禁用。配置操作使用后台重新解析的真实路径，展示用的 `~` 路径不会作为文件参数。兼容目标可在后续阶段使用本机 VS Code 以 `--new-window --remote ssh-remote+<alias>` 打开。VS Code 可执行文件优先从当前运行进程识别，以兼容非默认安装目录。打开前比较 VS Code 与桥接使用的 SSH 配置文件，来源不同则停止，避免同名别名连到不同机器。OpenSSH 有效配置也记录哈希，在建立隧道前后及后续远端写入/联网测试前校验，变化后要求重新建立。

Remote - SSH 与 ProxyEnv 各自管理 SSH 连接；关闭 VS Code 不会关闭 ProxyEnv 桥接，退出 ProxyEnv 会令依赖桥接的远程 CLI 失去连接。在 Remote - SSH 的远程终端粘贴环境变量后运行工具；CLI 的专用配置同样在该远程账户中生效。

The integration follows the [Remote - SSH configuration guide](https://code.visualstudio.com/docs/remote/ssh) and [VS Code command line reference](https://code.visualstudio.com/docs/configure/command-line). VS Code Server and already-running remote extension hosts do not inherit later terminal exports. ProxyEnv reads only the bounded default local VS Code user settings file for an explicit `http://127.0.0.1:<port>` user proxy; malformed, unsupported, ambiguous-profile, `localhost`/IPv6, or non-loopback settings yield an unknown expectation because the remote tunnel binds IPv4 loopback only. This is a user-setting clue, not proof of the effective remote/workspace configuration or extension runtime. A matching listener does not prove extension authentication, a real model request, or correct `NO_PROXY`; those remain separate live checks. ProxyEnv does not edit VS Code settings, restart Server/extension hosts, or edit shell startup files. Custom profiles, Insiders and portable settings are not imported automatically.

### Codex / Claude Code 图形化扩展（开发中，待实机验收）

先阅读[实现可行性审计](REMOTE_BRIDGE_EXTENSION_AUDIT_2026-09-06.md)。Codex 与 Claude Code 的 CLI / VS Code Remote Extension 分别共享远端 `~/.codex/config.toml` / `~/.claude/settings.json`；每种工具只执行一次用户配置事务。已有的旧扩展独立事务仍允许恢复。

1. 在已连接且 CC Switch 可用的桥接状态页，分别打开 Codex 或 Claude Code 开关；内部校验、备份并写入该工具的共享用户配置。
2. 在同一远程账户中启动 CLI 或重载 VS Code Remote 窗口并新建扩展会话。用户配置被读回验证不代表扩展的登录检查与真实模型请求已经成功。
3. 停用开关恢复接入前的托管字段，保留无关后续修改。配置结构损坏、恢复证据异常或写入瞬间发生并发冲突时停止覆盖。

| 扩展 | 受控修改 | 必须了解的影响 |
| --- | --- | --- |
| Codex CLI / Remote Extension | 两者共享 `~/.codex/config.toml` 的 `model`、`model_provider`、`model_catalog_json` 与 `model_providers.proxyenv_bridge`，以及 ProxyEnv 自有模型目录 | 模型目录按原始字节同步，未知字段不丢失；凭据、Provider 地址、`auth.json`、MCP 与其它字段不复制。选择任一入口都复用同一个事务。停用时按备份恢复，远端发生冲突时停止覆盖。桥接未启动时不检查；启动后每 2 秒只查 mtime/size，变化后防抖并完整校验哈希。 |
| Claude Code CLI / Remote Extension | 共享 `~/.claude/settings.json` 中的安全模型字段、模型别名、桥接路由与公开占位认证值 | 本地凭据、真实 Provider URL 与无关设置不复制；启动桥接且启用后每 2 秒只检查本地配置 mtime/size，变化后防抖并校验哈希。扩展登录检查可能有独立优先级，必须实机验收，不能凭配置写入宣称可用。 |

各工具的扩展与 CLI 使用同一远端用户配置；启用开关只写一次，不复制 `.env`、`auth.json` 或真实 Provider Secret。`PROXY_MANAGED` 的可用性取决于实际 CC Switch 接入方式，不能作为通用网关认证。旧版独立扩展事务的历史行为见[审计](REMOTE_BRIDGE_EXTENSION_AUDIT_2026-09-06.md)，当前仍需实机确认扩展登录门槛。

扩展检测与适配要求：远端现有 Node 20+，安装在 `/usr/bin/node`、`/usr/local/bin/node`，或受支持 VS Code Server 自带位置。使用固定脚本通过 SSH stdin 一次性运行，不新增远端运行时、不修改 VS Code Server、不创建后台服务。多个 Server 根目录会显示为 `ambiguous` 并停止写入；同一 Context 中保留多个扩展版本时会全部列出，ProxyEnv 不自动选择所谓“最高版本”。用户必须在 `Developer: Show Running Extensions` 中确认实际远端运行位置，重新检测或 Context 指纹变化会令确认失效。

Codex Extension 检测读取远端扩展内置 `bin/linux-<architecture>/codex --version`，确认扩展确实安装在所选 Remote Context；配置写入则复用共享客户端 Profile。Claude 扩展使用已安装的 `2.x` manifest 识别配置能力。该范围是配置适配门槛，不是对所有版本已完成兼容测试的声明。

解析在远端进行，原始配置、备份及未知字段不传回 Windows。JS 解析器按语法节点插入/替换目标内容，保留其他字节；不兼容的 TOML 内联表形态、JSONC 重复键、非法 UTF-8 和未知目标类型都失败关闭。凭据字段可能作为远端文件的不透明字节参与保留和备份，但不提取、不展示、不回传、不用于请求。

扩展事务使用同目录 `.proxyenv-extension-original`、`.proxyenv-extension-state` 和临时 `.proxyenv-extension-lock`，写入权限为 0600。预览令牌有效期五分钟，并绑定别名的有效 SSH 配置、账户与 runtime 指纹、文件和恢复记录哈希。正常错误回滚并验证；进程被强制终止可留下 prepared 记录或锁，此时停止自动写入，需在远端检查恢复证据。不会自动删除来历不明或中断留下的锁。

断开桥接保留 loopback 配置，不自动改回原 provider。用户显式恢复后可能重新使用原路由。此功能不提供整个远端系统的网络隔离，也不保证其他插件、遥测、MCP 或用户更改的配置不联网。

### Remote prerequisites

- Linux with a non-root SSH account, POSIX `sh`, `ss` (iproute2), `flock` (util-linux), and standard GNU/coreutils tools including `sha256sum`, `stat`, `sync -f`, `mktemp`, `grep`, `cut`, `cp`, `mv`, `cat`, `unlink`.
- `timeout`, Python 2.7 or Python 3, and a supported CLI are required for CLI configuration. ProxyEnv checks the noninteractive SSH `PATH` plus bounded user-level locations for native, npm, NVM, pnpm, Volta, Bun, asdf, mise and legacy Claude installs; it never sources interactive shell profiles. A CLI directory may be group-writable only when it belongs to the account's same-named primary group (for example `lxl:lxl`); world-writable and shared-group directories are skipped. `curl` is needed only for the explicit external network test.
- Existing OpenSSH target configuration. The bridge check can complete password and keyboard-interactive authentication in ProxyEnv. A successfully verified plain server password may be reused only for the current bridge; it is DPAPI-protected for the current Windows user, kept as ciphertext in memory, and cleared on target switch, disconnect, rejection, or exit. Key passphrases, OTP, and unknown challenges are never cached. First-use host keys still require explicit fingerprint confirmation.
- No existing LocalForward/RemoteForward/DynamicForward in the selected effective SSH configuration. These are rejected so the new connection opens only reviewed ports. Use a separate alias with no inherited forwards.
- The remote home must not be writable by another account. CLI configuration directories and files must belong to the SSH user and must not be symlinks, hard-linked files, oversized files, or world-writable. User-private-group permissions such as `lxl:lxl` with directory `775` / file `664` are accepted. For a university or laboratory shared primary group, preview explicitly discloses permission hardening; confirmation changes only the selected CLI configuration directory to `700` and settings file to `600` before writing. Other home-directory permissions are not changed. Codex accepts supported shared TOML while preserving unrelated fields and comments; ambiguous managed tables, duplicate managed keys and malformed managed values fail closed. Claude accepts only a valid JSON object with an object-valued `env`; duplicate keys, malformed JSON and unknown file types fail closed.

Explicit aliases inside complex Include/Match configurations are not enumerated in this MVP. Remote Windows/macOS, custom CLI home directories, older Codex profiles, service identity attestation, AI request verification, permanent tunnels and automatic reconnect are outside this implementation.

### SSH 交互认证边界

交互认证继续由系统 OpenSSH 实现协议，ProxyEnv 不解析或替代 SSH 认证。Windows 10 1809 及以上使用 ConPTY 承载终端提示；非交互路径保持 `StrictHostKeyChecking=yes`，交互路径使用 `StrictHostKeyChecking=ask`，只允许用户明确确认首次连接的真实指纹，绝不使用 `no`。已有主机指纹不匹配会失败关闭。`ForwardAgent=no`、`ForwardX11=no`、`PermitLocalCommand=no` 与 `ExitOnForwardFailure=yes` 在两种模式中保持一致。

后端基于有界累积缓冲识别服务器密码、私钥密码、验证码、首次主机指纹确认和常见 Keyboard Interactive 提示，只把当前经过清理且带有会话内唯一 ID 的 Prompt 交给界面。提交回答时后端只接受当前会话保存的 Prompt ID，不会重新解析终端历史来猜测问题。密码、私钥路径、历史终端输出和用户回答不会进入状态快照。Password → OTP 等多轮认证在同一会话内继续，不会重复创建弹窗。

ConPTY 输出先经过可跨分片工作的终端控制序列解析器；OpenSSH 发出光标位置查询（CSI 6n）时，ProxyEnv 会回复受控的 CSI 1;1R，控制字节不会进入 Prompt。连接检查使用由后端生成的短小固定远端命令，不再在认证后通过交互 PTY 注入完整 `remote.sh`，因此不会把大段脚本滞留在终端输入或回显缓冲。界面在登录后显示“认证通过，正在检查远端环境”，只有收到并解析远端结果后才显示最终成功并进入下一步；检查结果超时会明确失败，不会停留在假成功状态。提示等待按“最后一次 PTY 活动”计时：服务器横幅或认证协商输出会刷新等待窗口；连续 20 秒没有新输出且仍未收到可识别提示，才进入“未能读取认证提示”。此时停止认证进程并允许用户取消、重试或查看安全诊断计数；系统不会生成一个假的通用输入框。

用户回答只作为一次 Tauri 调用中的临时值写入 PTY stdin：不进入 SSH 参数、配置文件或日志，提交后立即清空前后端明文缓冲。仅普通服务器密码在完整认证成功后可进入本次桥接缓存，内存中只长期保存 Windows 当前用户 DPAPI 密文，并绑定目标 SSH 配置指纹；切换目标、断开、认证拒绝或退出都会清除。私钥口令、OTP 和未知挑战不缓存。仅当 PTY 返回与本次回答逐字节一致的回显时才抑制该段内容，不再丢弃回答后的任意首行，因此后续 OTP Prompt 不会被误吞。交互会话使用随机 ID 与随机成功标记，三分钟未完成会被销毁。取消、提示超时、窗口退出或应用退出都会终止对应 OpenSSH 进程。

当前交互路径覆盖连接检查和桥接建立。连接预检查、持续运行的反向转发进程和后续远端操作使用同一认证来源；密码目标不会在预检查成功后让真正的隧道退回 `BatchMode=yes`。建桥完成仍不等于后续 CLI 配置或模型请求已验证。普通服务器密码可在本次桥接内以 Windows DPAPI 保护的内存密文供后续配置、恢复和 Claude 验证复用；目标切换、断开或退出会清除。私钥口令、OTP 和未知交互回答仍不缓存，相关远端操作可能要求用户重新认证。

M3 的实现验证包含控制序列、分片 Prompt、多轮提示和回显安全的自动化测试；在实际 Windows OpenSSH、服务器认证策略与目标账户上的端到端验收仍必须由真实环境完成，未完成实机验收前不得把 M3 标记为最终完成。

## Recovery and conflict behavior

Backups stay beside each managed remote file (`.proxyenv-original`, if an original existed), with an applied-state marker (`.proxyenv-applied`) and an advisory lock (`.proxyenv-lock`). The first exact original backup is retained across subsequent ProxyEnv port updates. A write or readback failure restores the pre-write generation. For both shared configurations, later changes outside ProxyEnv-owned route fields are preserved during a port refresh and field-level disable. Unknown managed fields, duplicate fields or route changes still conflict. No full user configuration or secrets are copied locally.

启用/停用开关位于桥接状态页。重启并重新连接相同别名后，ProxyEnv 会恢复已识别的开关状态。两种工具均在开关操作内部校验、备份、原子写入并读回验证；停用时恢复接入前托管字段。旧版 Codex 扩展留下且哈希可验证的事务会自动迁移。远端后续增加的无关设置会保留，只有操作瞬间发生并发写入、文件结构不受支持或恢复证据损坏时才停止。

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

Browser checks use the existing development preview and mocked IPC: host selection, in-place capability setup, backend validation before connection, protected-write previews, apply/restore confirmation, disconnect cancellation, stale state, ESC/focus restoration, dark theme and 560px window layout. No browser mock is included in production source.

Real Windows → Linux password/PAM authentication, ProxyJump authentication, server forwarding rejection, Windows shutdown cleanup, and actual Codex/Claude routing still require acceptance on a user-provided test host. Unit and source-boundary tests cover PTY ownership, response zeroization, terminal-control filtering, hardening flags and Claude verification result classification, but do not claim a live-server or live-model result. No real SSH target or model API was used during implementation.

`pnpm test:extensions` 同时检查远端 bundle 与源代码一致，并覆盖无损增量修改、重复键/凭据冲突、预览后第三方修改、恢复、原子替换故障回滚和硬链接拒绝。Windows 测试不证明 Linux 的权限/锁语义；生产入口仅允许非 root Linux。模拟 IPC 的浏览器回归覆盖范围选择、远端确认门槛、写入前预览、部分成功、恢复及键盘焦点。

发布前必须分别完成两种 CLI 和两种图形化扩展的真实模型请求、Provider A → B 切换、断连请求失败，以及冲突恢复测试。Codex Extension 必须验证它在重载后读取共享 Profile 与模型目录；当前这些实机条目仍未完成，不应发布“VS Code AI extensions supported”的稳定版声明。

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
