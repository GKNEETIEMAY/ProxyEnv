# ProxyEnv architecture / 工程架构

ProxyEnv uses a feature-oriented Vue frontend and a layered Rust/Tauri backend. v0.1 is Windows-first: platform effects are implemented for Windows while the domain boundaries leave room for future Linux and macOS adapters.

ProxyEnv 前端按功能域组织，Rust/Tauri 后端按职责分层。v0.1 定位为 Windows-first：平台副作用当前由 Windows 实现，领域边界为未来 Linux/macOS 适配保留空间。

## Directory map / 目录地图

```text
ProxyEnv/
├─ src/
│  ├─ app/                           # Shell, lifecycle, cross-feature orchestration
│  ├─ features/
│  │  ├─ proxy/components/           # Three-layer proxy status and actions
│  │  └─ settings/components/        # General and About surfaces
│  └─ shared/                        # Typed IPC, i18n, types, design tokens
├─ src-tauri/src/
│  ├─ commands/                      # Thin Tauri command adapters
│  ├─ desktop/                       # Tray, native window, single instance
│  ├─ environment/                   # Generic environment core
│  │  ├─ manager.rs                  # Read/apply/snapshot/restore orchestration
│  │  ├─ models.rs                   # Scope, entry, mutation, result, snapshot
│  │  ├─ registry.rs                 # Windows user Registry adapter
│  │  ├─ broadcast.rs                # WM_SETTINGCHANGE
│  │  └─ snapshot.rs                 # Atomic persistence and legacy migration
│  ├─ features/proxy/
│  │  ├─ service.rs                  # State, plans, Disable/Restore/Sync/manual endpoint
│  │  ├─ models.rs                   # Proxy domain types
│  │  ├─ detector.rs                 # Candidate merge and confidence
│  │  ├─ system_proxy.rs             # Read-only Windows System Proxy
│  │  ├─ processes.rs                # Known process discovery
│  │  ├─ listeners.rs                # Listener/PID correlation
│  │  └─ probe.rs                    # Local HTTP/SOCKS protocol probes
│  ├─ services/settings.rs           # Durable application preferences
│  ├─ error.rs                       # Serializable error contract
│  └─ lib.rs                         # Tauri composition root
└─ public/proxy-clients/             # Client icons and attribution
```

## Dependency direction / 依赖方向

```text
Tauri Commands / Desktop Tray
             │
             ▼
      Proxy Feature Service ─────→ Proxy Discovery
             │
             ▼
      Environment Manager
             │
       ┌─────┼────────┐
       ▼     ▼        ▼
   Registry Snapshot Broadcast
```

The dependency is one-way. `features/proxy` may depend on `environment`; the Environment Core must not reference `ProxyProtocol`, `ProxyVariable`, proxy variable names, or client brands.

依赖只能单向流动：`features/proxy` 可以依赖 `environment`；Environment Core 不得引用 `ProxyProtocol`、`ProxyVariable`、代理变量名或客户端品牌。

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

## Command semantics / 命令语义

| Command | Purpose | Snapshot | Registry write |
| --- | --- | --- | --- |
| `get_environment_status` | Read entries, detect active candidate, diagnose state | No | No |
| `detect_proxies` | Discover and score local candidates | No | No |
| `sync_proxy_environment` | Apply a detected endpoint | Before apply | Yes |
| `sync_manual_proxy_environment` | Validate and apply a manual endpoint | Before apply | Yes |
| `disable_proxy_environment` | Remove managed values | Before delete | Yes |
| `restore_proxy_environment` | Restore the latest snapshot exactly | Uses existing | Yes |

The Windows System Proxy is a read-only discovery source. v0.1 never toggles it.

## Frontend boundary / 前端边界

Frontend dependencies flow `App.vue → app → features → shared`. `AppShell.vue` owns lifecycle, IPC orchestration, periodic read-only refresh, and cross-feature state. Feature components own presentation and local form interaction.

The home surface exposes three distinct layers:

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
