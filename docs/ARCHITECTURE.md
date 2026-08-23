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
│  │  ├─ proxy/components/           # Three-layer proxy status and actions
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
│  │  │  ├─ diagnosis.rs             # Read-only diagnosis and recommendation policy
│  │  │  ├─ processes.rs             # Visible user-application enumeration
│  │  │  ├─ launcher.rs              # Explicit child-process environment construction
│  │  │  └─ rules/                   # Schema, match, preview, backup, apply, restore
│  │  ├─ services/settings.rs        # Durable application preferences
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
validate unique names → read before → mutate → broadcast → read after → verify
```

Snapshots preserve both present values and missing values. Writes use an atomic temporary-file replacement. The current schema is stored at `%LOCALAPPDATA%\ProxyEnv\snapshots\latest.json`; the legacy snapshot remains readable.

`EnvironmentManager` 只处理通用的 Set/Delete、Scope、Entry 与 Snapshot。快照同时保留“存在的值”和“不存在”状态，并通过临时文件原子替换写入。

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

`features/network_observation` is read-only. On Windows it calls `GetAdaptersAddresses`, records interface type, name/description hints, and operational state, then classifies the evidence:

- `NotDetected`: no relevant virtual-interface evidence;
- `Possible`: one signal or ambiguous signals exist;
- `Detected`: an operational adapter has both an OS tunnel type and a recognized virtual-tunnel description;
- `Unknown`: enumeration is unavailable or failed.

A single adapter name never produces `Detected`. The observer does not inspect traffic, change routes, enable adapters, install drivers, or call a proxy client API. Linux and macOS currently return `Unknown` until platform observers are implemented.

`features/network_observation` 始终只读。Windows 实现组合接口类型、名称/描述特征与运行状态；单个名称最多只能得到 `Possible`，不会直接宣称 TUN 已开启。模块不会检查流量、修改路由、启停网卡、安装驱动或调用代理客户端 API。

## Application Assistant / 应用网络助手

The assistant is an orchestration feature, not a packet router. Its state machine is:

```text
select application → diagnose read-only state → recommend one action
                   → preview/confirm protected write → verify → result/restore
```

Running applications are limited to visible, non-system processes. Browsing uses Tauri's native dialog. The selected executable path becomes a `ManagedApplication`, but ProxyEnv never attaches to that process. Launch actions create a new child with either the active proxy values or all managed proxy variables removed.

The rule engine accepts only bundled, schema-versioned JSON. It rejects unknown fields, scripts, wildcard/traversal paths, unsupported formats, ambiguous matches, missing fields, stale plans, symlinks/reparse points, and changed files. A protected apply must read and preview the exact field, require confirmation, create an atomic local backup, write one existing field, read back and verify, and stop on conflict. Restore is also confirmed and only succeeds if the applied value is still current.

应用助手只做编排，不做流量路由。运行中应用仅用于确定可执行文件，ProxyEnv 不附加、不注入、不结束该进程。规则引擎只接受随软件打包、带 Schema 版本的 JSON 数据；写入必须经历读取、预览、确认、备份、单字段写入、读回验证，冲突时停止且不覆盖。

## Command semantics / 命令语义

| Command | Purpose | Snapshot / backup | Write target |
| --- | --- | --- | --- |
| `get_environment_status` | Read entries, detect active candidate, diagnose state | No | No |
| `detect_proxies` | Discover and score local candidates | No | No |
| `get_tun_observation` | Classify local virtual-adapter evidence | No | No |
| `list_running_applications` | List visible selectable applications | No | No |
| `diagnose_application` | Combine proxy, environment, system proxy, TUN, and rule state | No | No |
| `launch_application_with_current_proxy` | Start a new child with explicit proxy values | No | Child environment only |
| `launch_application_without_proxy` | Start a new child with proxy variables cleared | No | Child environment only |
| `preview_application_rule_fix` | Read and plan one known configuration-field change | No | No |
| `apply_application_rule_fix` | Confirm, back up, write, and verify a known field | Rule backup | Application config only |
| `restore_application_rule_change` | Conflict-check and restore the backed-up field | Uses existing | Application config only |
| `sync_proxy_environment` | Apply a detected endpoint | Before apply | `HKCU\\Environment` |
| `sync_manual_proxy_environment` | Validate and apply a manual endpoint | Before apply | `HKCU\\Environment` |
| `disable_proxy_environment` | Remove managed values | Before delete | `HKCU\\Environment` |
| `restore_proxy_environment` | Restore the latest snapshot exactly | Uses existing | `HKCU\\Environment` |

The Windows System Proxy and TUN observation are read-only sources. ProxyEnv never toggles either one.

## Frontend boundary / 前端边界

Frontend dependencies flow `App.vue → app → features → shared`. `AppShell.vue` owns lifecycle, view routing, periodic read-only refresh, and cross-feature state. Feature components own presentation, their own IPC orchestration, and local interaction.

The home surface exposes proxy-client, Windows System Proxy, and proxy-environment layers plus one clear entry to the application assistant. The assistant keeps selection, diagnosis, protected confirmation, and result in one guided surface. Advanced evidence is collapsed by default. Errors always state what happened, whether anything changed, and what to do next.

The proxy console exposes three distinct layers:

1. Proxy Client — detected process, listener, protocol, and confidence.
2. Windows System Proxy — read-only on/off state and endpoint.
3. Proxy Environment — state, source choice, explicit actions, and managed values.

All user-facing changes are explicit. Variable checkboxes save preference immediately but are not applied until the next Apply or Sync action.

## Desktop lifecycle / 桌面生命周期

The single-instance plugin establishes process ownership before tray and window setup. A second executable launch restores and focuses the existing window and emits a localized notice. Closing may hide to tray according to saved settings; left-click opens the window and the tray menu exposes proxy environment control.

单实例插件先于托盘和窗口初始化。第二次启动不会多开，而是恢复并聚焦现有窗口。关闭按钮是否隐藏到托盘由已保存设置决定。

## Verification / 验证

```powershell
pnpm build
cargo test --manifest-path src-tauri/Cargo.toml
cargo clippy --manifest-path src-tauri/Cargo.toml --all-targets -- -D warnings
```

Changes to Registry, broadcast, snapshots, tray, or single-instance behavior also require Windows integration testing. At minimum verify exact deletion/restoration, `WM_SETTINGCHANGE`, new-process inheritance, unchanged running-process environments, mismatch after a client port change, and explicit Sync to the new port.
