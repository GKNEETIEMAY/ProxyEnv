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
        │                    │
        ▼                    ▼
Application Assistant   Proxy Feature Service ──→ Proxy Discovery
   │       │    │                │
   │       │    └─→ Rule Engine  ▼
   │       └──────→ Launcher   Environment Manager
   └──────────────→ Network Observation     │
                                      Registry / Snapshot / Broadcast
```

The dependency is one-way. `features/proxy` may depend on `environment`; the application assistant may depend on proxy discovery and network observation. The Environment Core must not reference `ProxyProtocol`, `ProxyVariable`, application rules, proxy variable names, or client brands.

依赖只能单向流动：`features/proxy` 可以依赖 `environment`，应用助手可以依赖代理发现与网络观测；Environment Core 不得引用 `ProxyProtocol`、`ProxyVariable`、应用规则、代理变量名或客户端品牌。

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
| `generate_diagnostic_report` (Next) | Project a non-probing snapshot into a safe report DTO | No | No |
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

The Windows System Proxy and TUN observation are read-only sources. ProxyEnv never toggles either one.

## Frontend boundary / 前端边界

Frontend dependencies flow `App.vue → app → features → shared`. `AppShell.vue` owns lifecycle, view routing, periodic read-only refresh, and cross-feature state. Its single five-second refresh reads environment/proxy discovery and TUN observation together, then passes the same system-proxy and TUN snapshot to every active surface. A TUN observation failure becomes `Unknown` without preventing the other network layers from refreshing.

`features/network-observation/components/NetworkObservationPanel.vue` is the shared presentation for live system-proxy and TUN virtual-adapter state. Home adds it below proxy-client discovery; the application assistant reuses it with the local-listener fact enabled. Neither feature starts another timer or duplicates state-label, help, or status-icon logic. Feature components otherwise own their local IPC orchestration and interaction.

Proxy discovery keeps every endpoint candidate returned by the detector and groups candidates by PID or process identity for compact client navigation. Home renders the backend's active selection, including its unavailable last-known details, and exposes an explicit selector for all usable addresses. Client navigation also selects globally rather than maintaining a private viewed candidate. The automatic-detection label counts listening/total client processes rather than raw endpoints. Copy and manual fast-path validation use the active candidate; mismatch state comes from Rust. When no selection exists and TUN evidence is `Possible` or `Detected`, the UI can show a presentation-only suspected client. It never synthesizes an endpoint or replaces an unavailable selection.

The home surface exposes proxy-client, Windows System Proxy, and proxy-environment layers plus one clear entry to the application assistant. The assistant keeps selection, diagnosis, protected confirmation, and result in one guided surface. Advanced evidence is collapsed by default. Errors always state what happened, whether anything changed, and what to do next.

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
cargo test --manifest-path src-tauri/Cargo.toml --locked
cargo clippy --manifest-path src-tauri/Cargo.toml --all-targets --locked -- -D warnings
```

GitHub Actions repeats frozen pnpm installation, frontend audit/build, Rust formatting, Clippy, tests, locked release compilation, and RustSec audit for PRs and protected development/release branches. Current v0.1.4 includes `pnpm test:report` for focused locale/formatter and non-probing-boundary checks; broader frontend interaction tests remain planned. Dependabot tracks npm, Cargo, and Actions updates against `develop`.

Changes to Registry, broadcast, snapshots, tray, or single-instance behavior also require Windows integration testing. At minimum verify exact deletion/restoration, rollback after injected write/broadcast/verification failure, restore conflict behavior, `WM_SETTINGCHANGE`, new-process inheritance, unchanged running-process environments, mismatch after a client port change, and explicit Sync to the new port.
