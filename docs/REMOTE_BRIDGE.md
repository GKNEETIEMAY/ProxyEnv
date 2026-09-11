# v0.2.0 Remote Environment Bridge

Development branch: `v0.2.0dev`, based on v0.1.4 commit `fbf60b1`.
This is development scope, not a published stable-release announcement.

## 使用方式

1. 在“本机环境”选择可用的活动代理，再切换到一级页面“远程桥接”。
2. 从本机 OpenSSH、VS Code Remote 或 MobaXterm 发现结果中选择一个结构化远程目标。新主机仍需先在自己的终端确认主机指纹。
3. 检查 SSH 连接。ProxyEnv 先复用密钥、IdentityFile 或 ssh-agent 做非交互认证；仅当 OpenSSH 明确要求认证时，才显示应用内交互窗口承载密码或 Keyboard Interactive / PAM 提示。检查通过后优先复用普通代理端口 `17897` 与 AI 路由端口 `15721`。在高校实验室等共享服务器上发生占用时，会按 SSH 目标选择可重复的备用端口，而不是每次随机变化。
4. 在能力页分别查看服务器互联网、本机活动代理与 CC Switch AI 路由。三项检测彼此独立：SSH 成功不代表服务器能够联网，普通代理可用也不代表 CC Switch 可用。按需选择桥接本机代理、CC Switch，或同时选择两者。CC Switch 默认检查 `127.0.0.1:15721`，也可输入实际本地路由端口；结果会区分已确认的 CC Switch、身份未知的监听程序和未检测到监听。
5. 预览本机和远端端点。建立连接前会再次检查远程端口；如发生端口竞争，页面会选择备用端口并要求再次确认。
6. 代理桥接成功后，点击“启动代理终端”即可打开新的 PowerShell 窗口，由系统 OpenSSH 连接远端并自动注入代理环境。符合条件的普通服务器密码只在本次桥接中以 Windows 当前用户 DPAPI 密文缓存，切换目标、断开或退出即清除；私钥口令、OTP 和未知挑战不缓存。已经打开的 SSH、VS Code Remote 或 MobaXterm 终端可展开高级入口，复制环境变量后在当前 Shell 执行。仅“测试桥接”会经代理请求 `https://www.gstatic.com/generate_204`。
7. CC Switch 桥接成功本身不会修改 Codex / Claude Code 配置。每个 CLI 只显示一个启用/停用开关；打开时预览并确认受备份保护的写入，关闭时预览并恢复托管字段。后续使用同一稳定端口重连时会自动识别开关状态，不需要重复配置。启用后可直接运行 `codex` 或 `claude`。
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
| Port allocation | The proxy prefers stable remote Loopback port `17897`; AI routing prefers `15721`. If either is occupied, a deterministic fallback derived from the SSH target is reused. Every selected port is checked again immediately before connection. |
| CC Switch | Loopback listener ownership is classified as confirmed CC Switch, listening with unknown identity, or not detected. A listening port alone is not treated as service identity. |
| SSH auth | `BatchMode=yes` remains the first path for IdentityFile / ssh-agent. Authentication failures may opt into a short-lived OpenSSH session hosted by Windows ConPTY with `BatchMode=no`, password and keyboard-interactive enabled. |
| Forward | Explicit `127.0.0.1:remote:loopback:local`, ExitOnForwardFailure, strict host-key checks and bounded connection/keepalive timeouts. Interactive forwarding is not accepted until the remote listeners are verified as loopback-only. |
| Remote listener | Checks remote TCP listeners before creation and validates actual loopback-only listeners after creation. A wildcard/unknown binding closes the new tunnel. |
| Session | One combined target/session at a time. No automatic reconnect. The bridge SSH process uses the existing Windows Job Object lifecycle; authentication OpenSSH is owned by a short-lived ConPTY session. A user-launched proxy terminal is a separate visible OpenSSH process whose network route still depends on the active bridge. |
| Config | Codex CLI and Claude CLI use their standard user configuration so plain `codex` and `claude` inherit the bridge route. Parser-based managed-field edits preserve unrelated settings. Read/validate → preview → confirmation → exact remote backup → atomic replace → hash readback. Reconnect inspects existing ProxyEnv markers and restores each switch state when the configured port matches. Ambiguous, duplicate or conflicting managed semantics fail closed. |
| Tool adapters | Codex CLI and Claude Code CLI use a shared Rust/TypeScript `RemoteToolAdapter` contract for detection, inspection, compatibility, preview, apply, restore, launch and verification state. Page code iterates the registry; unknown IDs fail before SSH. VS Code extension internals remain a separate later migration. |
| Diagnostics | Structured command errors expose only code, phase, safe target category and retryability. Cached summaries remain allowlisted: no usernames, home paths, keys, secrets, raw SSH stderr or upstream URLs. |

## CLI configuration compatibility

Codex CLI `0.134.0` and later in the `0.x` series uses its standard user configuration. The backend checks the remote `codex --version` and refuses earlier/unknown formats instead of guessing. A custom `CODEX_HOME` is refused in this MVP. Configuration layering and the version transition were checked against the [official configuration documentation](https://learn.chatgpt.com/docs/config-file/config-advanced).

```text
~/.codex/config.toml
codex
```

The switch selects a dedicated `proxyenv_bridge` provider in the shared file, with a loopback `/v1` base URL, Responses wire protocol and `requires_openai_auth = false`. Authentication for the upstream model remains the responsibility of the local CC Switch route, so remote Codex does not ask the user to sign in with ChatGPT or enter another API key. ProxyEnv preserves unrelated model, permission, MCP and comment content and does not modify `auth.json`. Other default Codex sessions under the same remote account therefore use the selected bridge while the switch is enabled. Provider fields follow the [official configuration reference](https://learn.chatgpt.com/docs/config-file/config-reference). A configuration produced by the earlier development build with `requires_openai_auth = true` is recognized as a ProxyEnv-owned legacy route, shown as disabled, and safely rewritten after the user previews and enables it again.

Claude Code `2.x` uses its standard user file `~/.claude/settings.json`. ProxyEnv preserves the root object, unknown fields and unrelated `env` entries, then sets `ANTHROPIC_BASE_URL` and a single public `ANTHROPIC_AUTH_TOKEN=PROXY_MANAGED` placeholder. Conflicting Anthropic/OpenAI/OpenRouter authentication environment keys are removed only from the managed replacement after the exact original bytes have been backed up. This avoids Claude Code's dual-authentication warning without copying a real Provider credential. ProxyEnv does not modify `~/.claude.json`, pre-complete onboarding or grant project trust. A custom `CLAUDE_CONFIG_DIR` is refused. Safe TOML/JSON merge accepts an existing Python 2.7 or Python 3 interpreter; if neither exists, CLI configuration is refused with a specific dependency message. The settings mechanism follows [Claude Code settings](https://code.claude.com/docs/en/settings); the takeover-field policy is aligned with [CC Switch's implementation](https://github.com/farion1231/cc-switch/blob/main/src-tauri/src/services/proxy.rs).

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

提供 Remote - SSH 的同机远程终端与 CLI 接入，以及可选的 Codex / Claude Code 图形化扩展配置适配。扩展适配的实现和本地测试已具备，真实模型链路尚待验收。主机列表同时读取 Windows VS Code 默认用户 `Code/User/settings.json` 的 `remote.SSH.configFile`（支持 JSONC 注释和尾逗号）；来源不同的同名别名使用不同结构化 ID，默认配置与本机 OpenSSH 重合时不重复展示。桥接的 OpenSSH 调用使用对应的 `-F` 配置文件。

目标选择页只提供配置查看与检查连接。桥接建立后继续复用既有“启动代理终端”流程，它统一处理 PowerShell、当前桥接、会话凭据与代理环境注入，不再次要求密码。高级区域集中提供不注入变量的手动终端、MobaXterm 与 VS Code 入口；用户在对应客户端连接同一远程环境后，再复制并执行所示 `export`。ProxyEnv 不向第三方客户端传递缓存的 SSH 密码。VS Code 来源可打开本机 VS Code Settings；MobaXterm 来源可打开配置位置。无法桥接的目标仍可选择并查看配置，但“检查连接”保持禁用。配置操作使用后台重新解析的真实路径，展示用的 `~` 路径不会作为文件参数。兼容目标可在后续阶段使用本机 VS Code 以 `--new-window --remote ssh-remote+<alias>` 打开。VS Code 可执行文件优先从当前运行进程识别，以兼容非默认安装目录。打开前比较 VS Code 与桥接使用的 SSH 配置文件，来源不同则停止，避免同名别名连到不同机器。OpenSSH 有效配置也记录哈希，在建立隧道前后及后续远端写入/联网测试前校验，变化后要求重新建立。

Remote - SSH 与 ProxyEnv 各自管理 SSH 连接；关闭 VS Code 不会关闭 ProxyEnv 桥接，退出 ProxyEnv 会令依赖桥接的远程 CLI 失去连接。在 Remote - SSH 的远程终端粘贴环境变量后运行工具；CLI 的专用配置同样在该远程账户中生效。

The integration follows the [Remote - SSH configuration guide](https://code.visualstudio.com/docs/remote/ssh) and [VS Code command line reference](https://code.visualstudio.com/docs/configure/command-line). VS Code Server and already-running remote extension hosts do not inherit later terminal exports. ProxyEnv does not claim to configure every extension's network stack, install extensions, restart VS Code Server, or edit shell startup files. The local Windows OpenSSH client is used; custom VS Code SSH executables, profiles, Insiders and portable settings are not imported automatically.

### Codex / Claude Code 图形化扩展配置（开发中，待实机验收）

先阅读[实现可行性审计](REMOTE_BRIDGE_EXTENSION_AUDIT.md)。2026-09-07 起，配置入口可分别选择 CLI、VS Code Extension 或两者；每次默认只选择 CLI。

1. 在已连接且 CC Switch 可用的桥接状态页选择“配置 Codex”或“配置 Claude Code”。
2. 勾选图形化扩展后，点击“检测 VS Code Remote”。ProxyEnv 会区分 Stable、Insiders、旧版目录以及受约束的 `VSCODE_AGENT_FOLDER` 证据；不会把默认 `~/.vscode-server` 当作所有安装都适用，也不会安装、卸载或更新扩展。
3. 在目标 Remote - SSH 窗口运行 `Developer: Show Running Extensions`，确认扩展运行在界面所示远端账户，再核对实际配置路径。用户勾选确认后才可预览；这项状态是用户确认，不是自动进程证明。
4. 逐文件预览修改与影响范围，确认后执行。CLI 和扩展是独立文件事务；若后续文件失败，界面保留先前成功结果，不宣称跨文件原子成功。
5. 重载 VS Code 窗口并新建扩展会话。状态“已写入 · 待重载及模型验收”只表示配置文件经读回验证。
6. 恢复入口也可选择 CLI 或扩展，在未建立隧道时仍可针对同一 SSH 别名恢复。共享文件或备份被第三方修改时拒绝覆盖。

| 扩展 | 受控修改 | 必须了解的影响 |
| --- | --- | --- |
| Codex | `~/.codex/config.toml` 的 `model_provider`，以及新建的 `model_providers.proxyenv_bridge`；端点 `http://127.0.0.1:<port>/v1`、Responses 协议 | 同账户其他使用默认配置的 Codex 会话也受影响；保留 model、权限、MCP 等未知字段及注释。已有同名 provider 或旧 profile 选择器会冲突。 |
| Claude Code | 已确认的 VS Code Server Context 对应 Remote Settings 中，增加 `claudeCode.environmentVariables` 的 `ANTHROPIC_BASE_URL`、公开占位值 `ANTHROPIC_AUTH_TOKEN=PROXY_MANAGED`，以及 `claudeCode.disableLoginPrompt: true` | 只有唯一 Server Context 时才生成精确路径。已有路由/凭据项不覆盖；已有 `disableLoginPrompt: false` 会在预览中明确显示，确认后才修改。共享 Claude 用户设置或非交互 SSH 环境存在冲突路由时也停止。实际工作区与受管理策略仍需实机核验。 |

“只选择扩展”表示只执行扩展的配置适配，并不保证共享 Codex 默认配置对其他 CLI 无影响。无需修改 `.env`、复制 `auth.json` 或任何真实 Provider Secret。PROXY_MANAGED 的可用性取决于实际 CC Switch 接入方式，不能作为通用网关认证。

扩展检测与适配要求：远端现有 Node 20+，安装在 `/usr/bin/node`、`/usr/local/bin/node`，或受支持 VS Code Server 自带位置。使用固定脚本通过 SSH stdin 一次性运行，不新增远端运行时、不修改 VS Code Server、不创建后台服务。多个 Server 根目录会显示为 `ambiguous` 并停止写入；同一 Context 中保留多个扩展版本时会全部列出，ProxyEnv 不自动选择所谓“最高版本”。用户必须在 `Developer: Show Running Extensions` 中确认实际远端运行位置，重新检测或 Context 指纹变化会令确认失效。

Codex 扩展单独检查内置 `bin/linux-<architecture>/codex --version`，不依赖 PATH 中是否另装 CLI；目前格式适配范围为 `0.x` 且 minor ≥134，允许记录预发布版本，具体版本组合仍需验收。Claude 扩展使用已安装的 `2.x` manifest 识别配置能力。该范围是配置适配门槛，不是对所有版本已完成兼容测试的声明。

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

ConPTY 输出先经过可跨分片工作的终端控制序列解析器；OpenSSH 发出光标位置查询（CSI 6n）时，ProxyEnv 会回复受控的 CSI 1;1R，控制字节不会进入 Prompt。连接检查使用由后端生成的短小固定远端命令，不再在认证后通过交互 PTY 注入完整 `remote.sh`，因此不会把大段脚本滞留在终端输入或回显缓冲。界面在登录后显示“认证通过，正在检查远端环境”，只有收到并解析远端结果后才显示最终成功并进入下一步；检查结果超时会明确失败，不会停留在假成功状态。若在真实提示等待时间内仍未收到可识别提示，会进入“未能读取认证提示”，停止认证进程并允许用户取消、重试或查看安全诊断计数；系统不会生成一个假的通用输入框。

用户回答只作为一次 Tauri 调用中的临时值写入 PTY stdin：不进入 SSH 参数、配置文件或日志，提交后立即清空前后端明文缓冲。仅普通服务器密码在完整认证成功后可进入本次桥接缓存，内存中只长期保存 Windows 当前用户 DPAPI 密文，并绑定目标 SSH 配置指纹；切换目标、断开、认证拒绝或退出都会清除。私钥口令、OTP 和未知挑战不缓存。仅当 PTY 返回与本次回答逐字节一致的回显时才抑制该段内容，不再丢弃回答后的任意首行，因此后续 OTP Prompt 不会被误吞。交互会话使用随机 ID 与随机成功标记，三分钟未完成会被销毁。取消、提示超时、窗口退出或应用退出都会终止对应 OpenSSH 进程。

当前交互路径覆盖连接检查和桥接建立。连接预检查、持续运行的反向转发进程和后续远端操作使用同一认证来源；密码目标不会在预检查成功后让真正的隧道退回 `BatchMode=yes`。建桥完成仍不等于后续 CLI 配置或模型请求已验证。普通服务器密码可在本次桥接内以 Windows DPAPI 保护的内存密文供后续配置、恢复和 Claude 验证复用；目标切换、断开或退出会清除。私钥口令、OTP 和未知交互回答仍不缓存，相关远端操作可能要求用户重新认证。

M3 的实现验证包含控制序列、分片 Prompt、多轮提示和回显安全的自动化测试；在实际 Windows OpenSSH、服务器认证策略与目标账户上的端到端验收仍必须由真实环境完成，未完成实机验收前不得把 M3 标记为最终完成。

## Recovery and conflict behavior

Backups stay beside each managed remote file (`.proxyenv-original`, if an original existed), with an applied-state marker (`.proxyenv-applied`) and an advisory lock (`.proxyenv-lock`). The first exact original backup is retained across subsequent ProxyEnv port updates. A write or readback failure restores the pre-write generation. For both shared configurations, later changes outside ProxyEnv-owned route fields are preserved during a port refresh and field-level disable. Unknown managed fields, duplicate fields or route changes still conflict. No full user configuration or secrets are copied locally.

启用/停用开关位于桥接状态页。重启并重新连接相同别名后，ProxyEnv 会恢复已识别的开关状态。预览与应用之间发生变化时仍会停止当前事务并要求重新预览。Codex 与 Claude 都只恢复 ProxyEnv 托管字段，保留其余受支持字段的后续修改；托管路由/认证字段、文件结构或备份异常时停止操作。

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
