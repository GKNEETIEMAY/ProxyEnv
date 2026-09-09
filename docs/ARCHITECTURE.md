# ProxyEnv architecture / 工程架构

```yaml
Current Stable: v0.1.4
Next: v0.2.0
```

ProxyEnv uses a feature-oriented Vue frontend and a layered Rust/Tauri backend. Current Stable v0.1.4 is Windows-first and includes read-only network observation, conservative application diagnosis, unified active proxy selection, safe diagnostic reports, and signed NSIS updates. See [ROADMAP.md](ROADMAP.md) for current and upcoming scope. Platform effects are implemented for Windows; Linux/macOS adapters have no committed delivery date.

ProxyEnv 前端按功能域组织，Rust/Tauri 后端按职责分层。当前稳定版 v0.1.4 已包含只读网络观测、保守诊断状态、全局活动代理、安全诊断报告和 NSIS 签名更新；当前与后续范围以路线图为准。Linux/macOS 没有承诺实现时间。

## Directory map / 目录地图

```text
ProxyEnv/
├─ src/
│  ├─ app/                           # Shell, lifecycle, cross-feature orchestration
│  ├─ features/
│  │  ├─ application-assistant/      # Guided app selection, diagnosis, actions, results
│  │  ├─ diagnostic-report/          # Safe snapshot preview and locale formatter
│  │  ├─ network-observation/         # Shared live system-proxy and TUN presentation
│  │  ├─ proxy/components/           # Proxy discovery, environment status, and actions
│  │  ├─ remote-bridge/              # First-level remote workflow, state, and tool dialogs
│  │  └─ settings/components/        # General and About surfaces
│  └─ shared/                        # Typed IPC, i18n, types, design tokens
├─ src-tauri/
│  ├─ src/
│  │  ├─ commands/                   # Thin Tauri command adapters
│  │  ├─ desktop/                    # Tray, native window, single instance
│  │  ├─ environment/                # Generic environment core
│  │  │  ├─ manager.rs               # Read/apply/snapshot/restore orchestration
│  │  │  ├─ models.rs                # Scope, entry, mutation, result, snapshot
│  │  │  ├─ registry.rs              # Windows user Registry adapter
│  │  │  ├─ broadcast.rs             # WM_SETTINGCHANGE
│  │  │  └─ snapshot.rs              # Atomic persistence and legacy migration
│  │  ├─ features/proxy/             # Discovery, probes, state, sync/restore/disable
│  │  ├─ features/diagnostic_report/ # Allowlisted report DTO, no network probes
│  │  ├─ features/network_observation/
│  │  │  ├─ observer.rs              # Read-only adapter enumeration and evidence classifier
│  │  │  └─ models.rs                # NotDetected/Possible/Detected/Unknown contract
│  │  ├─ features/application_assistant/
│  │  │  ├─ authorization.rs         # Short-lived backend application IDs and file identity
│  │  │  ├─ diagnosis.rs             # Read-only diagnosis and recommendation policy
│  │  │  ├─ processes.rs             # Visible user-application enumeration
│  │  │  ├─ launcher.rs              # Explicit child-process environment construction
│  │  │  └─ rules/                   # Schema, match, preview, backup, apply, restore
│  │  ├─ features/remote_bridge/
│  │  │  ├─ ssh.rs                   # Structured target discovery and hardened OpenSSH invocation
│  │  │  ├─ ssh_auth.rs              # Short-lived PTY / ConPTY interactive authentication sessions
│  │  │  ├─ mobaxterm.rs             # Bounded bookmark parsing and safe compatibility adapter
│  │  │  ├─ process.rs               # Owned SSH process and Windows Job Object lifecycle
│  │  │  ├─ remote.sh                # Fixed remote checks, overlays, and recovery operations
│  │  │  ├─ extension.rs             # Reviewed VS Code remote-extension adapters
│  │  │  └─ mod.rs                   # Session, ports, health, and bridge orchestration
│  │  ├─ services/
│  │  │  ├─ local_file.rs            # Bounded safe reads and atomic local writes
│  │  │  ├─ redaction.rs             # Shared diagnostic redaction boundary
│  │  │  └─ settings.rs              # Durable validated application preferences
│  │  ├─ error.rs                    # Serializable error contract
│  │  └─ lib.rs                      # Tauri composition root
│  └─ resources/app-rules/           # Schema only today; reviewed rule catalog is pending
└─ public/proxy-clients/             # Client icons and attribution
```

## Dependency direction / 依赖方向

```text
Tauri Commands / Desktop Tray
        │                 │                         │
        ▼                 ▼                         ▼
Application Assistant  Proxy Feature Service   Remote Bridge
   │       │    │          │       │             │      │
   │       │    └─→ Rule Engine    └─→ Discovery │      └─→ OpenSSH / remote helper
   │       └──────→ Launcher       ▼             └─→ ActiveProxyContext
   └──────────────→ Network Observation      Environment Manager
                                                   │
                                      Registry / Snapshot / Broadcast
```

The dependency is one-way. `features/proxy` may depend on `environment`; the application assistant may depend on proxy discovery and network observation. The Environment Core must not reference `ProxyProtocol`, `ProxyVariable`, application rules, proxy variable names, or client brands.

依赖只能单向流动：`features/proxy` 可以依赖 `environment`，应用助手可以依赖代理发现与网络观测；远程桥接只读取统一的 `ActiveProxyContext`，不能自行重新选择代理。Environment Core 不得引用 `ProxyProtocol`、`ProxyVariable`、应用规则、代理变量名或客户端品牌。

## Environment Core / 通用环境核心

`EnvironmentManager` operates on generic `EnvironmentMutation::Set/Delete`, `EnvironmentScope`, `EnvironmentEntry`, and `EnvironmentSnapshot`. Apply operations always:

```text
validate unique names → lock → read before → save before/applied transaction
                      → mutate → broadcast → read after → verify
                      ├─ success → commit
                      └─ failure → rollback → broadcast → verify rollback
```

Snapshots preserve both present values and missing values in `before` and `applied` states. Restore proceeds only while `current == applied`, so an external change is a conflict and is never overwritten. Snapshot reads are schema/allowlist/size checked and reject links or Windows reparse points; writes use create-new temporary files and atomic replacement. The current schema is stored at `%LOCALAPPDATA%\ProxyEnv\snapshots\latest.json`; legacy v1 data is retired without being restored.

`EnvironmentManager` 只处理通用的 Set/Delete、Scope、Entry 与 Snapshot。Apply 任一步失败都会恢复并再次验证；Restore 仅在当前值仍等于 ProxyEnv 上次应用值时执行。快照通过 Schema、变量白名单、大小、链接与 reparse point 检查，并以临时文件原子替换写入。

## Proxy Feature / 代理功能域

`ProxyEnvironmentService` owns all proxy-specific policy:

- variable names and selected variables;
- HTTP, SOCKS5, and Mixed value plans;
- `Disabled`, `Partial`, `Enabled`, and `Mismatch` states;
- active candidate comparison;
- manual endpoint validation;
- Disable, Restore, and Sync semantics.

Refresh and discovery are read-only. In Current v0.1.4, a missing selected candidate remains unavailable and requires reselection; a newly selected address differing from saved variables produces `Mismatch`. Only an explicit Apply/Sync/Disable/Restore action can mutate the Registry.

刷新与自动识别保持只读。当前 v0.1.4 中，原候选消失后保留选择并标记不可用；用户重新选择的地址与环境变量不一致时显示 `Mismatch`。只有明确触发 Apply/Sync/Disable/Restore 才能修改注册表。

## Network Observation / 网络观测

`features/network_observation` is read-only. On Windows it combines `GetAdaptersAddresses` with the IPv4 routing table, recording interface type, name/description hints, operational state, and whether a default or split-default route uses the interface. It then classifies the evidence:

- `NotDetected`: no relevant virtual-interface evidence;
- `Possible`: one signal or ambiguous signals exist;
- `Detected`: an operational candidate has at least two independent signals among tunnel type, recognized virtual-tunnel identity, and default/split-default route ownership;
- `Unknown`: enumeration is unavailable or failed.

A single adapter name never produces `Detected`, and an ordinary physical adapter is not treated as TUN merely because it owns the default route. The observer does not inspect traffic, change routes, enable adapters, install drivers, or call a proxy client API. Linux and macOS currently return `Unknown` until platform observers are implemented.

`features/network_observation` 始终只读。Windows 实现组合接口类型、名称/描述特征、运行状态，以及接口是否承载默认或分流默认路由；至少两项独立信号一致时才得到 `Detected`。单个名称最多只能得到 `Possible`，普通物理网卡也不会仅因承载默认路由而被误判。模块不会检查流量、修改路由、启停网卡、安装驱动或调用代理客户端 API。

## Application Assistant / 应用网络助手

### Active proxy context / 当前活动代理

Current — v0.1.4: shipped / 已发布。

`features/proxy/active.rs` owns a session-wide `ActiveProxyContext`: selected candidate ID, candidate data (host/port/protocol/client/PID), selection source (`auto`, `user`, `systemProxy`, `manual`), availability, and a monotonic selection revision. Startup selects the first usable detector-ranked recommendation. Later refreshes update observations only; even a missing automatically recommended candidate is retained as unavailable, never replaced by the next listener. Manual endpoint application also updates this context. Restarting ProxyEnv starts a new recommendation session.

Environment sync, mismatch classification, connectivity tests, assistant diagnosis, proxy launches, application-rule preview/apply, and tray actions all resolve this context. Detected IDs include the listener owner and protocol; reuse of a port by a different process or a protocol change requires explicit reselection. Proxy writes and launches hold the selection lock, while asynchronous connectivity tests capture a target and reject completion if its revision changed. Frontend actions carry the diagnosis/selection revision, so an old preview cannot be applied to a newly selected proxy. Selection alone never writes environment variables; unavailable selections disable proxy actions and show a global reselect notice.

后端统一维护会话级活动代理，不再由各模块调用检测后取第一个监听结果。启动时推荐一次；用户选择、手动代理应用才会主动改变目标。刷新发现原代理消失时保留原地址并标记不可用，不转向其它客户端。所有环境同步、Mismatch、测试、诊断、启动、规则预览/应用与托盘共用同一上下文；操作携带选择版本，切换后旧预览与旧测试结果会被拒绝。选择本身不写环境变量，重启 ProxyEnv 后重新进行初始推荐。

### Diagnosis and protected actions / 诊断与受保护操作

The assistant, protected internal rule engine, and `ApplicationNetworkState` model exist in Current v0.1.4. The catalog currently contains only `schema.json`, not production application rules. Engine support must not be presented as verified coverage for real applications or promoted as a current product direction.

应用助手、内部受保护规则引擎与 `ApplicationNetworkState` 模型已在 v0.1.4 提供。当前目录只有 `schema.json`，没有生产应用规则，不能把引擎支持表述为具体应用已适配或当前产品方向。

The assistant is an orchestration feature, not a packet router. Its state machine is:

```text
select application → diagnose read-only state → recommend one action
                   → preview/confirm protected write → verify → result/restore
```

`ApplicationNetworkState` is the single diagnosis source of truth. `ConfirmedReady` requires a reviewed application rule that is already current; an enabled proxy environment alone becomes `EnvironmentConfigured`, because the application may not read those variables. A usable local proxy with a disabled environment becomes `ProxyLaunchRecommended`, while a reviewed rule that needs a change becomes `RuleSyncRecommended`. Incomplete/mismatched environment values or ambiguous rules become `Conflict`; failed rule inspection becomes `Unknown`; and an unavailable proxy becomes `Unsupported`. Only the two recommendation states produce a write or proxy-launch action.

System Proxy, proxy environment, TUN, and application-rule evidence are observed independently. System Proxy and TUN are always displayed as read-only evidence and never promote an application to `ConfirmedReady` or change the recommended action.

Running applications are limited to visible, non-system processes. Browsing uses Tauri's native Rust dialog. Both discovery paths issue a random, short-lived `application_id`; the frontend never submits an executable path to diagnosis, rule, or launch commands. The backend maps the ID to a canonical path and rechecks path, file type, extension/execute permission, and file identity before each use. Normal launch actions never attach to or alter the selected process; they create a new child with either the active proxy values or all managed proxy variables removed. The manual-proxy guide has one explicit restart exception: after a destructive-action warning and second confirmation, Rust verifies that the live PID still resolves to the authorized executable, attempts a normal window close, revalidates identity before any forced termination fallback, and starts one replacement process with proxy variables cleared.

The rule engine accepts only bundled, schema-versioned JSON. It rejects unknown fields, scripts, wildcard/traversal paths, unsupported formats, ambiguous matches, missing fields, stale plans, symlinks/reparse points, and changed files. A protected apply must read and preview the exact field, require confirmation, create an atomic local backup, write one existing field, read back and verify, and stop on conflict. Restore is also confirmed and only succeeds if the applied value is still current.

应用助手只做编排，不做流量路由。`ApplicationNetworkState` 是唯一诊断真源：只有已评审且当前值正确的应用规则能得到 `ConfirmedReady`；仅启用环境变量只能得到 `EnvironmentConfigured`，因为目标应用未必读取这些变量。只有 `ProxyLaunchRecommended` 和 `RuleSyncRecommended` 会产生代理启动或写入建议。系统代理、代理环境、TUN 与应用规则始终独立观察，系统代理和 TUN 只作为只读证据展示，不会把应用提升为“已确认可用”。Rust 枚举与原生文件选择器会签发短期随机 `application_id`，前端调用诊断、规则或启动 IPC 时不再传入可执行路径；后端每次使用前重新验证规范路径与文件身份。普通启动操作不附加、不注入、不结束已选择进程。手动代理引导只有一个明确的重启例外：先提示破坏性风险并进行第二次确认，Rust 再校验实时 PID 仍对应已授权可执行文件，优先请求正常关闭，在强制终止回退前再次校验身份，最后启动一个已清除代理变量的替代进程。规则引擎只接受随软件打包、带 Schema 版本的 JSON 数据；写入必须经历读取、预览、确认、备份、单字段写入、读回验证，冲突时停止且不覆盖。

## Remote Bridge / 远程环境桥接

Next — v0.2.0 development / 开发中。

Remote Bridge is a first-level product surface beside Local Environment. `AppShell.vue` owns the primary `local | remote` navigation context and keeps both primary surfaces mounted after first use, switching visibility without a cross-fade or keyed remount. The application assistant remains a Local Environment drill-down, while Settings returns to whichever primary surface opened it. `RemoteBridgePage.vue` owns one continuous setup/status workspace and uses dialogs only for OpenSSH interaction, reviewed Codex/Claude configuration, restore, and disconnect confirmation.

远程桥接与本机环境并列为一级页面，两者首次打开后保持挂载，切换时不再交叉淡入淡出或按 `key` 重建。远程页在同一工作区内完成目标选择、SSH 检查、能力配置和连接；连接前仍调用后端预览与重校验，但不再展示独立且重复的“确认桥接配置”页面。连接后原位显示精简状态与后续操作。弹窗仅用于 OpenSSH 交互、配置写入、恢复和断开等独立任务。

### Independent remote checks / 独立远端检查

The first v0.2 remediation stage introduces one shared `CheckState` vocabulary (`Idle`, `Checking`, `Healthy`, `Warning`, `Failed`, `Disabled`) and reusable `StatusIndicator`, `CheckRow`, `HelpHint`, and `LastChecked` components. Every row includes text and an authored icon in addition to semantic color. Help content follows the same four-part contract: what is checked, what success means, what failure means, and what to do next. A single refresh starts independent checks and applies each result separately; one failed check cannot erase successful sibling observations, and stale asynchronous results are discarded when the target changes.

第一阶段统一使用一套状态枚举与状态行组件。状态不仅依赖颜色，同时显示图标和文字；帮助提示固定说明“检查什么、成功代表什么、失败代表什么、下一步怎么做”。统一刷新会分别更新各项结果，单项失败不会阻断或覆盖其它检查；目标变化后，旧的异步结果不会写回新目标。

Server internet, the session-wide local active proxy, and CC Switch AI routing are independent observations. `remote_bridge_check_network` performs a direct remote HTTPS reachability check with proxy variables explicitly bypassed and returns only `Reachable`, `Unreachable`, or `Unknown`. The local proxy row reads the shared `ActiveProxyContext`; it never reselects a candidate. CC Switch keeps its separate listener-identity classification. General proxy forwarding and CC Switch forwarding now expose their own runtime status in `BridgeSummary`, so an unavailable endpoint no longer visually rewrites the other route's state. SSH success remains transport evidence only and never implies server internet, proxy, or AI-route availability.

服务器互联网、本机会话级活动代理和 CC Switch AI 路由是三项独立观测。服务器直连检测明确绕过代理环境，只返回可达、不可达或无法判断；本机代理继续读取统一 `ActiveProxyContext`，不会自行选择候选项；CC Switch 保留独立的监听进程身份判断。普通代理转发与 CC Switch 转发分别在 `BridgeSummary` 暴露运行状态，任何一个入口失效都不会在界面上改写另一项结论。SSH 成功只代表传输层可用，不代表服务器联网、代理桥接或 AI 路由可用。

### SSH authentication / SSH 认证

Every SSH operation first keeps the existing non-interactive OpenSSH path (`BatchMode=yes`) so IdentityFile and ssh-agent behavior stays native. Only the stable `sshAuth` failure category offers an interactive session. `ssh_auth_*` commands own a short-lived `portable-pty` session (Windows ConPTY). A streaming control parser handles fragmented ANSI/CSI sequences and answers OpenSSH cursor-position queries without exposing control bytes as prompt text. The session owns one typed, uniquely identified current prompt; submit and host-confirm commands must match that ID and never reparse the transcript to infer the question. Password, key-passphrase, verification-code, host-key confirmation, keyboard-interactive and real unknown prompts share one multi-round state machine. No synthetic fallback prompt exists: a real prompt timeout becomes `PromptUnavailable`, terminates the PTY and exposes only safe byte/count flags for diagnosis and retry. Interactive connection checks use a bounded backend-generated remote command instead of streaming the full helper through PTY stdin after authentication; the command performs the same platform, account, dependency and port preflight before emitting allowlisted JSON. `Authenticated` is displayed as an in-progress remote check, while only parsed completion becomes `Succeeded`, with a bounded completion timeout. Backend input uses a zeroizing buffer; frontend input is cleared before awaiting the IPC result. Only a byte-exact submitted-response echo is suppressed, so the next OTP or challenge line remains visible. Sessions use random opaque IDs and per-session authenticated markers, expire after three minutes, and die on cancel, prompt timeout or shutdown.

非交互模式严格拒绝未知主机，交互模式只把首次主机指纹交给用户确认；任何 `known_hosts` 不匹配仍直接失败。两种模式都禁用 Agent/X11 转发、禁止本地命令并在转发失败时退出。交互建桥只有在 OpenSSH 认证完成、反向转发已建立，并且远端 `ss` 证据确认所有端口仅监听 `127.0.0.1` 后才进入 Connected。认证状态独立记录为 `SshAuthState`。仅普通服务器密码在认证成功后可进入当前桥接的会话缓存：内存长期只保存以 Windows 当前用户 DPAPI 和目标 SSH 配置指纹保护的密文，按需解密到可清零缓冲；端口预检查、持续运行的 `ssh -R` 进程与后续远端操作统一使用这一目标绑定的认证来源，避免预检查通过后隧道退回 `BatchMode=yes`。切换目标、断开桥接、认证拒绝或应用退出会清除密文。私钥口令、验证码和未知交互回答从不缓存，也不会将明文写入参数、配置、磁盘或日志。

### Structured target discovery / 结构化目标发现

Every target is represented by an opaque `RemoteTarget.id` plus a display name, source, sanitized configuration path, resolved SSH fields, availability, compatibility state, and source-specific capabilities. IDs bind the source, configuration-file identity, and alias/session name; the frontend never parses a `vscode:`-style prefix. Target discovery is bounded to:

- the current user's default OpenSSH config;
- the explicit `remote.SSH.configFile` from VS Code's default user settings, without duplicating the default OpenSSH config;
- MobaXterm's active `-i` file, executable-adjacent file, user Documents file, and user configuration directory.

MobaXterm discovery reads bookmark sections only. It never reads or decrypts password, credential, or master-password sections and never scans the whole disk. Only simple SSH sessions that can be converted deterministically into Windows OpenSSH arguments are connectable; unsupported authentication, key, jump, or session behavior stays visible as `Unsupported` with a reason. OpenSSH continues to resolve aliases, keys, agents, ports, and `ProxyJump`; private-key contents never enter ProxyEnv.

### Session, ports, and errors / 会话、端口与错误

After an explicit SSH check, the backend queries remote listeners and allocates distinct loopback ports in `20000–60000`. Connection performs the listener check again immediately before creating reverse forwards. If a port becomes occupied in between, the frontend performs one new allocation, returns to review, and explains the race instead of silently connecting with an unreviewed endpoint.

The bridge captures the current `ActiveProxyContext` revision. Refresh can make the session `Stale` or `Unavailable`, but it cannot silently switch the tunnel to another local proxy. One owned SSH session is active at a time; Windows Job Objects terminate the process tree when ProxyEnv exits. Remote listeners must verify as loopback-only, otherwise the new tunnel is closed.

Tauri commands serialize `BridgeCommandError { code, phase, target, retryable }`. Stable codes identify SSH, port allocation, local endpoint, process, network, remote helper, and configuration phases without exposing raw command output, aliases, paths, or credentials. CC Switch detection combines loopback listener ownership with recognized process identity and reports `Confirmed`, `ListeningUnknown`, or `NotDetected`; an open TCP port alone is never presented as confirmed CC Switch routing.

### Post-connect guidance / 建桥后指引

The status surface separates transport health from product completion. It reports the selected target, SSH process, local endpoints, and remote reverse forwards, then provides capability-specific next steps. Proxy bridging primarily launches a visible Windows PowerShell host, which runs the system OpenSSH client with backend-generated `HTTP_PROXY`, `HTTPS_PROXY`, `ALL_PROXY`, and `NO_PROXY` state scoped to the remote shell and its descendants. The GUI-subsystem backend creates that host with `CreateProcessW`, a new console, and an untouched `STARTUPINFO` so invalid parent standard handles cannot make the terminal non-interactive. OpenSSH arguments cross the PowerShell boundary as a JSON environment payload and are splatted as data rather than interpolated into a command string. It does not edit `.bashrc`, `.profile`, `/etc/environment`, or require sudo. For eligible plain-password sessions, post-connect bridge operations receive only a DPAPI-protected payload and obtain the temporary plaintext through ProxyEnv's restricted `SSH_ASKPASS` mode; key passphrases, OTP and unknown prompts are never reused. Explicit exports remain behind an advanced entry for an already authenticated terminal. Connectivity testing remains opt-in. The target and endpoint come from the connected backend summary, SSH configuration is fingerprint-checked again, and no frontend-provided shell command is accepted. CC Switch bridging yields visible Codex/Claude configuration actions and launch commands. Claude CLI apply also performs a hash-bound, minimal top-level update of `~/.claude.json` to mark first-run onboarding complete; it never seeds project trust, so Claude retains its folder trust prompt. A successful tunnel or configuration write does not imply that a CLI, extension, provider route, or model request has been verified. Disconnect closes reverse forwards but deliberately does not remove reviewed remote overlays; restore remains a separate confirmed transaction.

### Remote tool adapters / 远程工具适配器

M5 moves the first CLI integrations behind matching Rust and TypeScript `RemoteToolAdapter` registries. Codex CLI and Claude Code CLI each own their identity, compatibility gate, dedicated overlay path and renderer, preview/apply/restore transaction entry points, launch command, supported route modes, and verification state. `RemoteBridgePage.vue` and `RemoteToolDialog.vue` iterate the registry instead of branching on tool names; unknown tool IDs are rejected by the backend before any SSH operation. Adding another CLI therefore requires a registered adapter rather than another page-level conditional.

M5 首批迁移 Codex CLI 与 Claude Code CLI。两端适配器统一负责工具标识、版本兼容性、专用配置路径与内容、预览/应用/恢复、启动命令、支持的路由模式和验证状态；页面与弹窗只消费注册表，不再用工具名称堆叠条件分支。后端会在发起 SSH 操作前拒绝未知工具。配置写入后状态仅为 `VerifyPending`，不会把“文件已写入”冒充“真实模型请求已验证”；真实验证属于 M6。

M6 closes the Claude CLI verification loop with an explicit user-triggered request. The remote helper uses Claude's dedicated overlay while disabling inherited setting sources, MCP servers, tools and session persistence; it sends one fixed non-sensitive prompt with a single turn and discards stdout/stderr after classifying the result. Only the allowlisted states `Verified`, `AuthenticationRequired`, `RouteUnavailable`, `TimedOut`, or `Failed` cross the SSH boundary. Raw model responses, login output, remote paths and credentials never enter application state or logs. A route change, tunnel loss or restore invalidates prior verification. Codex remains honestly `VerifyPending` until its own adapter receives an equivalent verified implementation.

Claude Code's Remote - SSH adapter transaction also manages `claudeCode.disableLoginPrompt: true` beside `claudeCode.environmentVariables`. An existing `false` value is visible in the preview and changes only after the user confirms the exact transaction; restore returns the complete original file bytes. Reloading VS Code is still required, and a successful file readback is not presented as a successful extension model request.

## Local data and WebView boundary / 本地数据与 WebView 边界

### Safe diagnostic reports / 安全诊断报告

Current — v0.1.4: shipped / 已发布。

`generate_diagnostic_report` collects a read-only snapshot using non-refreshing `active::snapshot_status`/`snapshot` APIs, current environment status, independent system-proxy/TUN observations, and only a valid cached connectivity result. It never invokes discovery, port/protocol probes or external connectivity tests. An optional backend-issued application ID enables a fresh local rule diagnosis through the existing authorization boundary and the shared `diagnose_snapshot` decision path, using the same captured proxy context; absent, expired, or uninspectable applications remain explicitly unselected/unavailable, without guessed recommendations. A changed active-proxy revision rejects collection rather than combining different proxy selections.

`features/diagnostic_report` projects observations into `DiagnosticReportData` before IPC serialization. Only enums, booleans, counts, managed-variable identifiers, build version, constrained numeric OS-version metadata, and canonical client names from the detector's allowlist survive. The existing `services/redaction` boundary sanitizes allowlisted labels and validates version metadata. Raw environment entries, error strings, URLs, addresses (including loopback), interface names, application names/paths, rule IDs/values, and PIDs are not report fields. Unknown client names become a localized generic label. Multiple ports of one process count as one client; process identity is used only internally for deduplication.

The frontend's pure `formatDiagnosticReport` uses the shared i18n catalog, reusing existing state descriptions. Output language defaults to the current interface language and can be changed to Chinese, English, Japanese, or Korean without collecting again or changing the DTO. A native modal provides a read-only preview, explicit refresh, and shared clipboard copying; it never saves or uploads automatically. Closing discards the snapshot and late requests cannot repopulate it. `pnpm test:report` verifies four-locale formatting and immutable states; Rust tests inject sensitive values to verify exclusion from the serialized DTO.

报告由后端先投影为安全白名单数据，再通过现有脱敏边界进入共享 i18n 格式化器。语言仅改变表述，不改变状态或触发新诊断。报告不输出地址、用户名、路径、原始错误、配置原值或进程标识；未做连通性测试时明确显示“未测试”。入口位于窗口标题栏，应用部分只读取当前助手选中的应用，不自动保存文件或上传。

### Storage boundaries / 存储边界

Settings, environment snapshots, and application-rule backups use bounded reads, regular-file checks, symlink/reparse-point rejection, and atomic replacement where mutation is allowed. Settings reject unknown JSON fields and duplicate proxy-variable entries. Rule backups remain create-once records and are never followed through a link.

生产 WebView 启用严格 CSP：脚本、字体与样式只从应用自身加载，远程连接仅允许 Tauri IPC 和用户主动触发的 GitHub Releases 检查；开发 CSP 单独允许 Vite 的本机端口与 WebSocket。Capability 不开放 shell、通用文件写入或前端文件选择路径授权。

## Diagnostic boundary / 诊断信息边界

`services/redaction.rs` is the single boundary for text entering logs, debug output, serialized errors, or error reports. It replaces user/profile paths, full application and configuration paths, local proxy endpoints, executable/process names, and process identifiers. `ProxyEnvError` applies this sanitizer for `Display`, `Debug`, and serialization, so backend errors exposed through IPC use the same policy. Values with no dependable textual signature—especially raw configuration fields—must use the format-agnostic `sensitive` wrapper and are replaced in full. New diagnostics should emit stable categories, counts, and booleans wherever possible.

`services/redaction.rs` 是日志、调试输出、序列化错误与错误报告的统一文本边界，会替换用户名与用户目录、应用和配置完整路径、本机代理地址与端口、可执行文件/进程名和进程标识。`ProxyEnvError` 的 `Display`、`Debug` 与序列化共用该策略。无法可靠识别格式的数据（尤其配置字段原始值）必须通过 `sensitive` 包装器整体隐藏；新增诊断优先只输出稳定错误类别、数量与布尔状态。

ProxyEnv does not read, persist, or manage proxy credentials, subscription tokens, node credentials, or other proxy authentication material. These values are outside the application-assistant rule and backup model.

## Command semantics / 命令语义

| Command | Purpose | Snapshot / backup | Write target |
| --- | --- | --- | --- |
| `get_environment_status` | Read entries, detect active candidate, diagnose state | No | No |
| `get_active_proxy_context` | Read the shared selection and refresh its availability | No | No |
| `select_active_proxy` | Explicitly select one currently usable discovered candidate | No | Session selection only |
| `detect_proxies` | Discover and score local candidates | No | No |
| `get_tun_observation` | Classify local virtual-adapter evidence | No | No |
| `generate_diagnostic_report` | Project a non-probing snapshot into a safe report DTO | No | No |
| `list_running_applications` | List visible selectable applications | No | No |
| `pick_application` | Native selection and short-lived backend authorization | No | No |
| `diagnose_application` | Combine proxy, environment, system proxy, TUN, and rule state | No | No |
| `launch_application_with_current_proxy` | Start a new child with explicit proxy values | No | Child environment only |
| `launch_application_without_proxy` | Start a new child with proxy variables cleared | No | Child environment only |
| `restart_application_without_proxy` | Reverify and close one confirmed PID, then start a replacement with proxy variables cleared | No | Selected process lifecycle and child environment |
| `preview_application_rule_fix` | Read and plan one known configuration-field change | No | No |
| `apply_application_rule_fix` | Confirm, back up, write, and verify a known field | Rule backup | Application config only |
| `restore_application_rule_change` | Conflict-check and restore the backed-up field | Uses existing | Application config only |
| `sync_proxy_environment` | Apply the shared active selection (Next); reject stale revisions | Before apply | `HKCU\\Environment` |
| `sync_manual_proxy_environment` | Validate and apply a manual endpoint | Before apply | `HKCU\\Environment` |
| `disable_proxy_environment` | Remove managed values | Before delete | `HKCU\\Environment` |
| `restore_proxy_environment` | Restore the latest snapshot exactly | Uses existing | `HKCU\\Environment` |
| `remote_bridge_targets` | Discover structured OpenSSH, VS Code, and MobaXterm targets | No | No |
| `remote_bridge_check` | Verify one target and allocate reviewed remote loopback ports | No | No |
| `remote_bridge_check_network` | Independently classify direct server HTTPS reachability while bypassing proxy variables | No | No |
| `ssh_auth_begin/state/submit/finish/cancel` | Own one bounded OpenSSH PTY authentication flow; never persist the submitted response | No | Short-lived PTY process only; successful connect transfers the owned tunnel to bridge state |
| `remote_bridge_allocate_ports` | Regenerate distinct unused remote loopback ports | No | No |
| `remote_bridge_detect_cc` | Classify CC Switch listener ownership without probing AI APIs | No | No |
| `remote_bridge_preview` | Revalidate target, active proxy revision, capabilities, and ports | No | No |
| `remote_bridge_connect` | Create one confirmed, loopback-only reverse-forward session | No | Owned SSH child process |
| `remote_bridge_disconnect` | Stop the owned reverse-forward process after confirmation | No | Owned SSH child process |
| `remote_bridge_config_*` | Preview, apply, or restore dedicated remote CLI overlays; Claude apply may mark first-run onboarding complete without granting project trust | Remote backup plus preview-bound state hash | Dedicated remote overlay; Claude apply also minimally updates `~/.claude.json` |

The Windows System Proxy and TUN observation are read-only sources. ProxyEnv never toggles either one.

## Frontend boundary / 前端边界

Frontend dependencies flow `App.vue → app → features → shared`. `AppShell.vue` owns lifecycle, primary `local | remote` routing, drill-down return context, periodic read-only refresh, and cross-feature state. Its single five-second refresh reads environment/proxy discovery and TUN observation together, then passes the same system-proxy and TUN snapshot to every active surface. A TUN observation failure becomes `Unknown` without preventing the other network layers from refreshing.

`features/network-observation/components/NetworkObservationPanel.vue` is the shared presentation for live system-proxy and TUN virtual-adapter state. Home adds it below proxy-client discovery; the application assistant reuses it with the local-listener fact enabled. Neither feature starts another timer or duplicates state-label, help, or status-icon logic. Feature components otherwise own their local IPC orchestration and interaction.

Proxy discovery keeps every endpoint candidate returned by the detector and groups candidates by PID or process identity for compact client navigation. Home renders the backend's active selection, including its unavailable last-known details, and exposes an explicit selector for all usable addresses. Client navigation also selects globally rather than maintaining a private viewed candidate. The automatic-detection label counts listening/total client processes rather than raw endpoints. Copy and manual fast-path validation use the active candidate; mismatch state comes from Rust. When no selection exists and TUN evidence is `Possible` or `Detected`, the UI can show a presentation-only suspected client. It never synthesizes an endpoint or replaces an unavailable selection.

The Local Environment surface exposes proxy-client, Windows System Proxy, and proxy-environment layers plus one clear entry to the application assistant. The Remote Bridge surface is its first-level peer and owns remote target discovery, in-place capability setup, connection status, and next-step guidance. It has no step rail or separate tunnel-review screen; backend preview remains the connection-time validation boundary. The assistant keeps selection, diagnosis, protected confirmation, and result in one guided surface. Advanced evidence is collapsed by default. Errors always state what happened, whether anything changed, and what to do next.

The proxy console exposes four distinct observable layers:

1. Proxy Client — detected process, listener, protocol, and confidence.
2. Windows System Proxy — read-only on/off state and endpoint.
3. TUN / virtual adapter — read-only evidence state and identified interface when available.
4. Proxy Environment — state, source choice, explicit actions, and managed values.

All user-facing changes are explicit. Variable checkboxes save preference immediately but are not applied until the next Apply or Sync action.

## Signed updates / 签名更新

Current Stable v0.1.4 includes the official Tauri Updater. About → Check for updates reads localized GitHub Release notes and the pinned HTTPS manifest; only an explicit Download and install action fetches and verifies the signed NSIS installer, performs passive replacement of the registered installation, then restarts. Default version comparison rejects older/equal versions. MSI and portable builds link to the official Release for manual updates. This is not silent/background auto-installation. See [release-security.md](release-security.md).

当前稳定版已支持签名更新，而非“只能手动检查”。NSIS 安装版由用户主动检查并确认下载安装，验证签名后覆盖已登记安装并重启；MSI 与 Portable 仍手动更新，不在后台静默安装。

## Desktop lifecycle / 桌面生命周期

The single-instance plugin establishes process ownership before tray and window setup. A second executable launch restores and focuses the existing window and emits a localized notice. Closing may hide to tray according to saved settings; left-click opens the window and the tray menu exposes proxy environment control.

单实例插件先于托盘和窗口初始化。第二次启动不会多开，而是恢复并聚焦现有窗口。关闭按钮是否隐藏到托盘由已保存设置决定。

## Verification / 验证

```powershell
pnpm build
pnpm test:report
pnpm test:remote
pnpm test:extensions
cargo test --manifest-path src-tauri/Cargo.toml --locked
cargo clippy --manifest-path src-tauri/Cargo.toml --all-targets --locked -- -D warnings
```

GitHub Actions repeats frozen pnpm installation, frontend audit/build, Rust formatting, Clippy, tests, locked release compilation, and RustSec audit for PRs and protected development/release branches. Current v0.1.4 includes `pnpm test:report` for focused locale/formatter and non-probing-boundary checks. The v0.2.0 development branch adds fixed remote-helper, localized error, extension parser, and recovery-transaction tests; live SSH and model-routing acceptance remains manual. Dependabot tracks npm, Cargo, and Actions updates against `develop`.

Changes to Registry, broadcast, snapshots, tray, or single-instance behavior also require Windows integration testing. At minimum verify exact deletion/restoration, rollback after injected write/broadcast/verification failure, restore conflict behavior, `WM_SETTINGCHANGE`, new-process inheritance, unchanged running-process environments, mismatch after a client port change, and explicit Sync to the new port.
