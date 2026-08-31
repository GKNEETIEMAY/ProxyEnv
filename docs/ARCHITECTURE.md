# ProxyEnv architecture / 工程架构

ProxyEnv uses a feature-oriented Vue frontend and a layered Rust/Tauri backend. v0.2 remains Windows-first while adding a read-only network-observation layer and a constrained application assistant. Platform effects are implemented for Windows; domain boundaries leave room for future Linux and macOS adapters.

ProxyEnv 前端按功能域组织，Rust/Tauri 后端按职责分层。v0.2 仍定位为 Windows-first，并加入只读网络观测层和受限的应用助手；平台副作用当前由 Windows 实现，领域边界为未来 Linux/macOS 适配保留空间。

## Directory map / 目录地图

```text
ProxyEnv/
├─ src/
│  ├─ app/                           # Shell, lifecycle, cross-feature orchestration
│  ├─ features/
│  │  ├─ application-assistant/      # Guided app selection, diagnosis, actions, results
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
│  └─ resources/app-rules/           # Reviewed bundled JSON rules; no scripts or downloads
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

Refresh and discovery are read-only. A detected endpoint change produces `Mismatch`; only an explicit Apply/Sync/Disable/Restore action can mutate the Registry.

刷新与自动识别保持只读。检测到端点变化时只产生 `Mismatch`，只有用户明确触发 Apply/Sync/Disable/Restore 才能修改注册表。

## Network Observation / 网络观测

`features/network_observation` is read-only. On Windows it combines `GetAdaptersAddresses` with the IPv4 routing table, recording interface type, name/description hints, operational state, and whether a default or split-default route uses the interface. It then classifies the evidence:

- `NotDetected`: no relevant virtual-interface evidence;
- `Possible`: one signal or ambiguous signals exist;
- `Detected`: an operational candidate has at least two independent signals among tunnel type, recognized virtual-tunnel identity, and default/split-default route ownership;
- `Unknown`: enumeration is unavailable or failed.

A single adapter name never produces `Detected`, and an ordinary physical adapter is not treated as TUN merely because it owns the default route. The observer does not inspect traffic, change routes, enable adapters, install drivers, or call a proxy client API. Linux and macOS currently return `Unknown` until platform observers are implemented.

`features/network_observation` 始终只读。Windows 实现组合接口类型、名称/描述特征、运行状态，以及接口是否承载默认或分流默认路由；至少两项独立信号一致时才得到 `Detected`。单个名称最多只能得到 `Possible`，普通物理网卡也不会仅因承载默认路由而被误判。模块不会检查流量、修改路由、启停网卡、安装驱动或调用代理客户端 API。

## Application Assistant / 应用网络助手

The assistant is an orchestration feature, not a packet router. Its state machine is:

```text
select application → diagnose read-only state → recommend one action
                   → preview/confirm protected write → verify → result/restore
```

Running applications are limited to visible, non-system processes. Browsing uses Tauri's native Rust dialog. Both discovery paths issue a random, short-lived `application_id`; the frontend never submits an executable path to diagnosis, rule, or launch commands. The backend maps the ID to a canonical path and rechecks path, file type, extension/execute permission, and file identity before each use. Normal launch actions never attach to or alter the selected process; they create a new child with either the active proxy values or all managed proxy variables removed. The manual-proxy guide has one explicit restart exception: after a destructive-action warning and second confirmation, Rust verifies that the live PID still resolves to the authorized executable, attempts a normal window close, revalidates identity before any forced termination fallback, and starts one replacement process with proxy variables cleared.

The rule engine accepts only bundled, schema-versioned JSON. It rejects unknown fields, scripts, wildcard/traversal paths, unsupported formats, ambiguous matches, missing fields, stale plans, symlinks/reparse points, and changed files. A protected apply must read and preview the exact field, require confirmation, create an atomic local backup, write one existing field, read back and verify, and stop on conflict. Restore is also confirmed and only succeeds if the applied value is still current.

应用助手只做编排，不做流量路由。Rust 枚举与原生文件选择器会签发短期随机 `application_id`，前端调用诊断、规则或启动 IPC 时不再传入可执行路径；后端每次使用前重新验证规范路径与文件身份。普通启动操作不附加、不注入、不结束已选择进程。手动代理引导只有一个明确的重启例外：先提示破坏性风险并进行第二次确认，Rust 再校验实时 PID 仍对应已授权可执行文件，优先请求正常关闭，在强制终止回退前再次校验身份，最后启动一个已清除代理变量的替代进程。规则引擎只接受随软件打包、带 Schema 版本的 JSON 数据；写入必须经历读取、预览、确认、备份、单字段写入、读回验证，冲突时停止且不覆盖。

## Local data and WebView boundary / 本地数据与 WebView 边界

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
| `detect_proxies` | Discover and score local candidates | No | No |
| `get_tun_observation` | Classify local virtual-adapter evidence | No | No |
| `list_running_applications` | List visible selectable applications | No | No |
| `pick_application` | Native selection and short-lived backend authorization | No | No |
| `diagnose_application` | Combine proxy, environment, system proxy, TUN, and rule state | No | No |
| `launch_application_with_current_proxy` | Start a new child with explicit proxy values | No | Child environment only |
| `launch_application_without_proxy` | Start a new child with proxy variables cleared | No | Child environment only |
| `restart_application_without_proxy` | Reverify and close one confirmed PID, then start a replacement with proxy variables cleared | No | Selected process lifecycle and child environment |
| `preview_application_rule_fix` | Read and plan one known configuration-field change | No | No |
| `apply_application_rule_fix` | Confirm, back up, write, and verify a known field | Rule backup | Application config only |
| `restore_application_rule_change` | Conflict-check and restore the backed-up field | Uses existing | Application config only |
| `sync_proxy_environment` | Apply a detected endpoint | Before apply | `HKCU\\Environment` |
| `sync_manual_proxy_environment` | Validate and apply a manual endpoint | Before apply | `HKCU\\Environment` |
| `disable_proxy_environment` | Remove managed values | Before delete | `HKCU\\Environment` |
| `restore_proxy_environment` | Restore the latest snapshot exactly | Uses existing | `HKCU\\Environment` |

The Windows System Proxy and TUN observation are read-only sources. ProxyEnv never toggles either one.

## Frontend boundary / 前端边界

Frontend dependencies flow `App.vue → app → features → shared`. `AppShell.vue` owns lifecycle, view routing, periodic read-only refresh, and cross-feature state. Its single five-second refresh reads environment/proxy discovery and TUN observation together, then passes the same system-proxy and TUN snapshot to every active surface. A TUN observation failure becomes `Unknown` without preventing the other network layers from refreshing.

`features/network-observation/components/NetworkObservationPanel.vue` is the shared presentation for live system-proxy and TUN virtual-adapter state. Home adds it below proxy-client discovery; the application assistant reuses it with the local-listener fact enabled. Neither feature starts another timer or duplicates state-label, help, or status-icon logic. Feature components otherwise own their local IPC orchestration and interaction.

Proxy discovery keeps every endpoint candidate returned by the detector, then groups candidates by PID or process identity for presentation. Home selects the first listening client initially, lists only the remaining client identities, and pages every client through the same primary detail view. The automatic-detection label counts listening/total client processes rather than raw endpoints. Copy, manual fast-path validation, mismatch diagnosis, and Apply all target the currently displayed client; changing pages never writes automatically. When TUN evidence is `Possible` or `Detected` but no candidate is listening, the UI creates a presentation-only “suspected proxy client” observation. It never synthesizes a host or port, never becomes a `ProxyCandidate`, and cannot enable the automatic Apply action.

The home surface exposes proxy-client, Windows System Proxy, and proxy-environment layers plus one clear entry to the application assistant. The assistant keeps selection, diagnosis, protected confirmation, and result in one guided surface. Advanced evidence is collapsed by default. Errors always state what happened, whether anything changed, and what to do next.

The proxy console exposes four distinct observable layers:

1. Proxy Client — detected process, listener, protocol, and confidence.
2. Windows System Proxy — read-only on/off state and endpoint.
3. TUN / virtual adapter — read-only evidence state and identified interface when available.
4. Proxy Environment — state, source choice, explicit actions, and managed values.

All user-facing changes are explicit. Variable checkboxes save preference immediately but are not applied until the next Apply or Sync action.

## Desktop lifecycle / 桌面生命周期

The single-instance plugin establishes process ownership before tray and window setup. A second executable launch restores and focuses the existing window and emits a localized notice. Closing may hide to tray according to saved settings; left-click opens the window and the tray menu exposes proxy environment control.

单实例插件先于托盘和窗口初始化。第二次启动不会多开，而是恢复并聚焦现有窗口。关闭按钮是否隐藏到托盘由已保存设置决定。

## Verification / 验证

```powershell
pnpm build
cargo test --manifest-path src-tauri/Cargo.toml --locked
cargo clippy --manifest-path src-tauri/Cargo.toml --all-targets --locked -- -D warnings
```

GitHub Actions repeats frozen pnpm installation, frontend audit/build, Rust formatting, Clippy, tests, locked release compilation, and RustSec audit for PRs and protected development/release branches. Dependabot tracks npm, Cargo, and Actions updates against `develop`.

Changes to Registry, broadcast, snapshots, tray, or single-instance behavior also require Windows integration testing. At minimum verify exact deletion/restoration, rollback after injected write/broadcast/verification failure, restore conflict behavior, `WM_SETTINGCHANGE`, new-process inheritance, unchanged running-process environments, mismatch after a client port change, and explicit Sync to the new port.
