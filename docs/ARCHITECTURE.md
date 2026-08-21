# ProxyEnv architecture / 工程结构

ProxyEnv uses a feature-oriented Vue frontend and a layered Rust/Tauri backend. The structure is intentionally small, but every directory has one reason to change.

ProxyEnv 前端按功能域组织，Rust/Tauri 后端按职责分层。结构保持克制，但每个目录都只有一种变化原因。

## Directory map / 目录地图

```text
ProxyEnv/
├─ src/                              # Vue frontend / Vue 前端
│  ├─ app/                           # Application shell and desktop orchestration
│  │  ├─ components/                 # Shell-only components
│  │  └─ AppShell.vue                # Lifecycle, navigation and cross-feature state
│  ├─ features/
│  │  ├─ proxy/components/           # Proxy status and environment-variable UI
│  │  └─ settings/components/        # General settings and About UI
│  ├─ shared/
│  │  ├─ api/                        # Typed Tauri IPC boundary
│  │  ├─ i18n/                       # Locale resolution and copy
│  │  ├─ styles/                     # Global tokens and application styles
│  │  └─ types/                      # Types shared by multiple features
│  ├─ App.vue                        # Minimal Vue root
│  └─ main.ts                        # Browser entry
├─ src-tauri/src/                    # Rust backend / Rust 后端
│  ├─ commands/                      # Thin Tauri command adapters by domain
│  ├─ desktop/                       # Tray and native-window integration
│  ├─ environment/                   # Environment mutation, snapshots and broadcast
│  ├─ proxy/                         # Detection, listeners, probes and client identity
│  ├─ services/                      # Persistent application services
│  ├─ error.rs                       # Shared error contract
│  ├─ lib.rs                         # Tauri composition root
│  └─ main.rs                        # Native binary entry
├─ public/proxy-clients/             # Runtime client icons and attribution
└─ docs/                             # Maintainer-facing documentation
```

## Dependency rules / 依赖规则

Frontend dependencies flow inward: `App.vue → app → features → shared`. A feature may use `shared`, but one feature must not import another feature's private component. Cross-feature state and desktop lifecycle stay in `AppShell.vue`; presentation and feature-specific interaction stay in the feature component.

前端依赖方向固定为 `App.vue → app → features → shared`。功能域可以依赖 `shared`，但不能引用另一个功能域的私有组件。跨功能状态与桌面生命周期放在 `AppShell.vue`，展示和域内交互放在对应功能组件。

Backend commands deserialize IPC input, call domain or service code, then synchronize desktop state. They must not contain platform registry or proxy-detection implementation. Platform effects belong to `environment`, `proxy`, or `desktop`; durable preferences belong to `services`.

后端命令层只负责接收 IPC 参数、调用领域/服务代码并同步桌面状态，不放注册表或代理识别实现。平台副作用归入 `environment`、`proxy` 或 `desktop`，持久化偏好归入 `services`。

## Adding a feature / 新增功能

1. Add the user-facing component under `src/features/<feature>/components`.
2. Add shared IPC types to `src/shared/types` only when multiple features need them.
3. Add the typed invoke wrapper to `src/shared/api/backend.ts`.
4. Add a thin Rust adapter under `src-tauri/src/commands/<feature>.rs`.
5. Put reusable business or persistence logic in a domain module or `services`.
6. Register the command in `commands/mod.rs` and `lib.rs`, then run the frontend build and Rust tests.

新增功能时，依次建立功能组件、共享类型和 IPC 包装；Rust 侧新增薄命令适配器，并把可复用业务/持久化逻辑放入领域模块或 `services`。最后在 `commands/mod.rs` 与 `lib.rs` 注册，并执行前端构建及 Rust 测试。
