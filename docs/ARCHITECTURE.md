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
│  ├─ desktop/                       # Tray, single-instance and native-window integration
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

## Desktop lifecycle / 桌面生命周期

`tauri-plugin-single-instance` is registered before every other plugin in `lib.rs`. A second executable launch exits without creating another application instance. Its callback calls the shared tray/window helper to unminimize, show, and focus the existing main window, then emits `second-instance-opened` to the webview. `AppShell.vue` turns that event into a localized, polite live-region notice and removes it after 3.2 seconds.

`tauri-plugin-single-instance` 必须在 `lib.rs` 中早于其他插件注册。第二次启动 EXE 时，新进程不会创建另一套应用实例；回调复用托盘/窗口帮助函数，依次取消最小化、显示并聚焦现有主窗口，然后向 WebView 发送 `second-instance-opened`。`AppShell.vue` 将事件呈现为本地化的礼貌级 live region 提示，并在 3.2 秒后自动移除。

The callback must remain UI-state agnostic: it does not navigate away from the user's current page, mutate proxy state, or depend on frontend readiness during first launch. The single-instance plugin remains first in the builder chain because it establishes process ownership before autostart, tray, or window setup.

回调不得改变当前页面或代理状态，也不能参与首次启动的前端初始化。单实例插件必须保持在 Builder 插件链首位，确保在开机启动、托盘和窗口初始化前建立进程所有权。

## Adding a feature / 新增功能

1. Add the user-facing component under `src/features/<feature>/components`.
2. Add shared IPC types to `src/shared/types` only when multiple features need them.
3. Add the typed invoke wrapper to `src/shared/api/backend.ts`.
4. Add a thin Rust adapter under `src-tauri/src/commands/<feature>.rs`.
5. Put reusable business or persistence logic in a domain module or `services`.
6. Register the command in `commands/mod.rs` and `lib.rs`, then run the frontend build and Rust tests.

新增功能时，依次建立功能组件、共享类型和 IPC 包装；Rust 侧新增薄命令适配器，并把可复用业务/持久化逻辑放入领域模块或 `services`。最后在 `commands/mod.rs` 与 `lib.rs` 注册，并执行前端构建及 Rust 测试。

## Verification / 验证

Run `pnpm build`, `cargo test --manifest-path src-tauri/Cargo.toml`, and `cargo clippy --manifest-path src-tauri/Cargo.toml --all-targets` after changing either boundary. Desktop lifecycle changes also require launching the same built EXE twice: the second process must exit successfully, exactly one process for that executable path must remain, and the first instance must restore and receive the notice event.

修改前后端边界后需执行前端构建、Rust 测试和 Clippy。桌面生命周期变更还必须用同一构建产物连续启动两次：第二个进程应正常退出，同一 EXE 路径只保留一个进程，并且首个实例能够恢复窗口并收到提示事件。
