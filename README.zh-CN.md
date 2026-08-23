<p align="center"><img src="assets/icon.svg" width="92" height="92" alt="ProxyEnv 图标"></p>
<h1 align="center">境启 ProxyEnv</h1>
<p align="center">Windows-first 的代理环境与应用联网桌面助手。<br>自动发现本机代理客户端，解释应用可能使用的网络层，并让用户选择明确、可逆的操作。</p>
<p align="center"><a href="README.md">English</a> · <strong>简体中文</strong></p>

<p align="center">
  <img alt="平台" src="https://img.shields.io/badge/平台-Windows%2010%20%7C%2011-0078D4?style=flat-square">
  <img alt="Tauri" src="https://img.shields.io/badge/Tauri-2-24C8DB?style=flat-square&logo=tauri&logoColor=white">
  <img alt="Rust" src="https://img.shields.io/badge/Rust-stable-000000?style=flat-square&logo=rust">
  <img alt="许可证" src="https://img.shields.io/badge/许可证-MIT-22c55e?style=flat-square">
  <img alt="状态" src="https://img.shields.io/badge/状态-v0.2%20开发中-f59e0b?style=flat-square">
</p>

> [!IMPORTANT]
> ProxyEnv 仍处于 v0.2 开发阶段，尚无正式 Release。当前已经实现并实机验证的目标平台是 Windows 10/11。Linux 与 macOS 是后续架构方向，不属于当前支持平台；其他 Unix 变体不在计划内。在本开发线完成兼容性测试前，安装包元数据仍保持 `0.1.0`。

## 要解决的问题

许多 CLI 会读取 `HTTP_PROXY`、`HTTPS_PROXY` 或 `ALL_PROXY`，但一些需要直连的软件会在这些变量存在时出现异常。

```text
HTTP_PROXY 存在
├─ Claude Code / Codex / CLI    ✅
└─ 某些直连软件                  ❌

HTTP_PROXY 不存在
├─ Claude Code / Codex / CLI    ❌
└─ 某些直连软件                  ✅
```

<p align="center"><img src="docs/assets/proxy-conflict.png" width="880" alt="HTTP_PROXY 冲突的可爱火柴人简笔画"></p>

ProxyEnv 把这些状态清晰展示出来，并把手工编辑注册表变成明确、可恢复的操作。产品遵循一个安全原则：

> 自动发现，手动修改。

检测与定时刷新不会写注册表。当代理客户端从端口 `7890` 切换到 `7897` 时，ProxyEnv 会显示 `Mismatch`，等待用户点击“同步到活动代理”。

## 核心能力

- 识别本机代理客户端、监听 PID、真实端口及 HTTP/SOCKS5/Mixed 协议。
- 只读显示 Windows 系统代理，不修改其开关或地址。
- 用 `Disabled`、`Partial`、`Enabled`、`Mismatch` 四种状态描述代理环境。
- 支持应用自动检测到的代理地址，也支持手动输入主机、端口与协议。
- 正确映射协议：HTTP 变量使用 `http://`，适用时 `ALL_PROXY` 使用 `socks5://`。
- 选择下一次应用/同步要管理的 `HTTP_PROXY`、`HTTPS_PROXY` 与 `ALL_PROXY`。
- `NO_PROXY` 保持只读。
- 修改前保存快照，安全删除，精确恢复，广播 `WM_SETTINGCHANGE`，并读回注册表验证。
- 支持系统托盘、开机启动/窗口偏好，以及第二次启动时唤醒现有窗口。
- 提供简体中文、英文、日语与韩语，以及浅色、深色、跟随系统主题。
- 可从可见的运行中应用选择或浏览可执行文件，并给出面向新手的网络诊断。
- 将虚拟隧道证据表示为“未检测到 / 可能存在 / 已检测到 / 未知”，但不控制 TUN。
- 用活动代理或清除代理变量的环境启动新进程，绝不向运行中进程注入或修改环境。
- 当存在经过评审的内置应用规则时，先预览并确认，再备份、写入、验证，并支持冲突保护恢复。

## 应用网络助手

应用助手保持一条短流程：

```text
选择应用 → 读取本机状态 → 解释可能的网络路径 → 推荐一个操作
         → 如需写文件则预览并确认 → 验证结果
```

它会读取活动本机代理、Windows 系统代理、代理环境变量、虚拟网卡证据与内置应用规则目录。选择运行中应用只用于确定其可执行文件，不会修改该进程；“使用代理启动”和“直连启动”都会创建带有明确子进程环境的新进程。

应用规则是声明式数据，不是可执行 Adapter。规则只能声明精确进程名、固定的用户目录配置路径、一个已有字段、受支持的格式（`JSON`、`YAML`、`TOML` 或 `INI`）以及有类型的代理值。没有经过评审的规则时，ProxyEnv 只提供启动回退方案，不扫描未知配置文件。

## 四层可观测网络状态

ProxyEnv 明确区分四个容易混淆的概念：

| 层级 | 含义 | ProxyEnv 的行为 |
| --- | --- | --- |
| 代理客户端 | v2rayN 等本机进程及其监听地址，例如 `127.0.0.1:10809` | 检测与探测 |
| Windows 系统代理 | 供兼容软件读取的 Windows 网络设置 | 只读 |
| 代理环境变量 | 新启动进程继承的用户变量 | 仅在用户明确操作后修改 |
| TUN / 虚拟网卡 | 可能让应用在没有代理变量时也改变网络路径的系统层通道 | 只读、基于多项证据观察 |

修改环境变量不会配置代理客户端、不会切换 Windows 系统代理、不会控制 TUN、不会让整台电脑的流量自动改道，也不会改变已经运行进程的环境。新启动的应用会继承此代理地址，已运行的应用需要重启才生效。端口正在监听只说明代理客户端进程可用，不代表 Windows 系统代理或 TUN 路由已经开启。

## 管理范围

ProxyEnv 只修改当前用户的：

```text
HKEY_CURRENT_USER\Environment
```

不会写入 `HKLM`，通常不需要管理员权限。

| 协议 | `HTTP_PROXY` | `HTTPS_PROXY` | `ALL_PROXY` |
| --- | --- | --- | --- |
| HTTP | `http://host:port` | `http://host:port` | 删除 |
| SOCKS5 | 删除 | 删除 | `socks5://host:port` |
| Mixed | `http://host:port` | `http://host:port` | `socks5://host:port` |

默认选择 `HTTP_PROXY` 与 `HTTPS_PROXY`。`ALL_PROXY` 需要用户主动选择，因为它的回退范围更广，可能影响包搜索、局域网发现或 SOCKS 支持不完整的软件。

## 安全操作语义

```text
关闭：读取 → 保存快照 → 删除 → 广播 → 读回 → 验证
恢复：读取最近快照 → 精确恢复旧值 → 广播 → 验证
同步：根据选中的代理地址生成计划 → 保存快照 → 写入/删除 → 广播 → 验证
刷新：只检测与比较，绝不写入
```

快照原子保存在 `%LOCALAPPDATA%\ProxyEnv\snapshots\latest.json`，旧版 `env-snapshot.json` 仍可读取。

## 支持的代理客户端

当前可识别 Clash Verge Rev、v2rayN、FlClash、Hiddify、Clash Nyanpasu、Clash Party、Mihomo Party、NekoBox/NekoRay、Clash for Windows 与 GUI.for.Clash。前五项使用已注明来源的上游图标，新增客户端与未知客户端使用通用代理图标；自动识别失败时可手动填写主机、端口与协议。

图标来自官方上游仓库，来源与许可见 [`public/proxy-clients/ATTRIBUTION.md`](public/proxy-clients/ATTRIBUTION.md)。

## 与环境变量工具对比

下列工具解决的是相邻问题。ProxyEnv 刻意保持更窄的范围，专注代理发现、网络层解释，以及安全的新进程启动或经过评审的规则修改。

| 工具 | 推荐度 | 适合什么情况 | 主要特点 |
| --- | --- | --- | --- |
| **ProxyEnv** | ⭐⭐⭐⭐⭐ | 代理冲突与应用联网问题 | 识别活动代理端点，解释代理/系统代理/TUN 层，支持独立启动环境与可恢复的评审规则 |
| Microsoft PowerToys – Environment Variables | ⭐⭐⭐⭐⭐ | 大多数 Windows 开发者 | 微软维护、界面现代、支持 Profile、User/System 变量 |
| EnvStudio | ⭐⭐⭐⭐⭐ | PATH 很复杂、多开发环境 | 拖拽 PATH、去重、失效路径与冲突检测、快照回滚 |
| Envarly | ⭐⭐⭐⭐½ | 喜欢开源、希望通用修改更安全 | 修改前 Diff、快照/回滚、PATH 拖拽、PowerShell/Ansible 导出 |
| Rapid Environment Editor | ⭐⭐⭐⭐ | 传统 Windows 开发环境 | 成熟、PATH 树状管理、错误检测、备份、便携版 |
| envx | ⭐⭐⭐⭐ | 喜欢终端/TUI | Rust、跨平台、快照/Profile、搜索、CLI、`.env`/JSON/YAML 导入导出 |

## 运行要求

### 使用打包版本

- Windows 10 1803 或更高版本（x64），或 Windows 11 x64
- Microsoft Edge WebView2 Runtime；受支持的较新 Windows 通常已预装

打包版本不要求用户安装 Node.js、pnpm、Rust 或 Visual Studio。

### 源码开发

- Node.js `20.19+` 或 `22.12+`；推荐 Node.js 22 LTS
- 通过 Corepack 使用 pnpm 10
- Rust stable MSVC 工具链
- Visual Studio Build Tools 2022，并安装 **Desktop development with C++**

这些版本是当前 Vite 7 与 Tauri 2 工具链的实际最低要求，无需追随最新 Node Current 版本。

```powershell
cd ProxyEnv
corepack enable
pnpm install
pnpm tauri dev
```

在 VS Code 中安装工作区推荐扩展，选择 `ProxyEnv: Tauri Debug`，按 `F5` 即可。

## 验证与构建

```powershell
pnpm build
cargo test --manifest-path src-tauri/Cargo.toml
cargo clippy --manifest-path src-tauri/Cargo.toml --all-targets -- -D warnings
pnpm tauri build
```

## 工程结构

```text
ProxyEnv/
├─ src/
│  ├─ app/                       # Vue 外壳与桌面编排
│  ├─ features/                  # 代理、应用助手与设置界面
│  └─ shared/                    # IPC、i18n、类型和视觉令牌
├─ src-tauri/src/
│  ├─ commands/                  # 轻量 Tauri IPC 适配器
│  ├─ desktop/                   # 托盘、单实例、原生窗口
│  ├─ environment/               # 通用 mutation、快照、广播、验证
│  ├─ features/proxy/            # 代理检测、计划、状态、同步/恢复/关闭
│  ├─ features/network_observation/ # 只读虚拟网卡证据
│  ├─ features/application_assistant/ # 应用选择、诊断、启动、声明式规则
│  └─ services/                  # 持久化应用设置
├─ public/proxy-clients/         # 运行时图标与归属说明
└─ docs/                         # 架构文档与 README 插图
```

通用 Environment Core 不包含代理客户端或代理变量知识。完整依赖规则见 [`docs/ARCHITECTURE.md`](docs/ARCHITECTURE.md)。

## 边界与隐私

ProxyEnv 不是代理客户端、VPN、订阅管理器、流量转发器或 TUN 控制器。它不控制 Clash/v2rayN API、节点、订阅、代理客户端规则、Windows 系统代理、路由、驱动或系统级环境变量，也不会向运行中进程注入、结束进程或改写其环境。

代理检测、协议探测、TUN 观测、应用枚举、规则预览和环境变量管理均在本机完成。软件不会读取订阅、节点、Token、密码或流量；除非用户明确触发现有代理测试，否则不会进行外部联网测试，只有用户主动检查更新时才访问 GitHub Releases。详见 [`SECURITY.md`](SECURITY.md)。

## 贡献

欢迎贡献。请先阅读 [`CONTRIBUTING.md`](CONTRIBUTING.md)，并保持依赖方向：Proxy Feature 可以依赖 Environment Core，Environment Core 不能反向依赖 Proxy Feature。

## 贡献者

- ProxyEnv 维护者与社区贡献者
- OpenAI Codex——参与实现、测试、设计与文档编写的 AI 编程助手

## 许可证

ProxyEnv 基于 [MIT License](LICENSE) 发布，第三方图标继续遵循各自上游许可证。
