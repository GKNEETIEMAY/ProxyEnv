<p align="center">
  <img src="assets/icon.svg" width="92" height="92" alt="ProxyEnv 图标">
</p>

<h1 align="center">境启 ProxyEnv</h1>

<p align="center">
  面向 Windows、Linux 与 macOS 开发的跨平台代理环境变量开关<br>
  自动发现代理客户端、实际监听端口与协议，并同步代理环境变量
</p>

<p align="center">
  <a href="README.md">English</a> · <strong>简体中文</strong>
</p>

<p align="center">
  <img alt="Platform" src="https://img.shields.io/badge/目标平台-Windows%20%7C%20Linux%20%7C%20macOS-0078D4?style=flat-square">
  <img alt="Tauri" src="https://img.shields.io/badge/Tauri-2-24C8DB?style=flat-square&logo=tauri&logoColor=white">
  <img alt="Rust" src="https://img.shields.io/badge/Rust-stable-000000?style=flat-square&logo=rust">
  <img alt="License" src="https://img.shields.io/badge/许可证-MIT-22c55e?style=flat-square">
  <img alt="Status" src="https://img.shields.io/badge/状态-v0.1%20开发中-f59e0b?style=flat-square">
</p>

> [!IMPORTANT]
> ProxyEnv 目前处于 v0.1 开发阶段，尚未提供正式 Release。Windows 核心环境变量开关、本机代理识别、系统托盘与多语言设置已经可用；Linux 与 macOS 后端和安装包仍在开发中。

| 平台           | 当前状态             | 变量命名                                             |
| -------------- | -------------------- | ---------------------------------------------------- |
| Windows 10/11  | 已实现并进行实机验证 | `HTTP_PROXY`、`HTTPS_PROXY`、`ALL_PROXY`、`NO_PROXY` |
| Linux          | 计划支持             | `http_proxy`、`https_proxy`、`all_proxy`、`no_proxy` |
| macOS          | 计划支持             | `http_proxy`、`https_proxy`、`all_proxy`、`no_proxy` |
| 其他 Unix 变体 | 不在支持计划内       | —                                                    |

## 为什么需要 ProxyEnv？

Windows、Linux 与 macOS 上的桌面软件、CLI 和网络库并不总是使用同一套代理入口。Claude Code、Codex、Git、npm、pip 等工具通常会读取代理环境变量，而某些需要直连的软件可能因此无法正常连接。

这会形成一个让人反复打开“环境变量”窗口的典型冲突：

```text
HTTP_PROXY 存在
├─ Claude Code / Codex / CLI    ✅
└─ 某些直连软件                  ❌

HTTP_PROXY 不存在
├─ Claude Code / Codex / CLI    ❌
└─ 某些直连软件                  ✅
```

<p align="center">
  <img src="docs/assets/proxy-conflict.png" width="880" alt="HTTP_PROXY 开关冲突的可爱火柴人简笔画">
</p>

ProxyEnv 把原本繁琐的操作：

```text
打开操作系统的环境变量配置
→ 找到并删除代理变量
→ 启动需要直连的软件
→ 手动重建原来的变量
```

简化为：

```text
Disable
→ 启动目标软件
→ Enable
```

Disable 前会保存完整快照；Enable 时恢复原值，而不是覆盖成固定端口。

## 核心能力

| 能力         | 说明                                                 |
| ------------ | ---------------------------------------------------- |
| 一键启停     | 删除或恢复用户级代理环境变量，不写入空字符串         |
| 实时端点同步 | 环境变量开启时跟随当前活动代理，不继续保留失效旧端口 |
| 安全快照     | 修改前持久化完整快照，使用原子替换避免半写入文件     |
| 自动发现     | 结合 Windows 系统代理、进程、TCP 监听 PID 和协议探测 |
| 实际端口     | 读取真实监听端口，不假设客户端永远使用 7890 或 10808 |
| 协议识别     | 区分 HTTP、SOCKS5 与 Mixed 代理端口                  |
| 客户端识别   | 识别主流代理客户端并展示对应图标                     |
| 写回验证     | 每次 Enable / Disable 后重新读取注册表确认结果       |
| 环境广播     | 通过 `WM_SETTINGCHANGE` 通知 Windows 环境已更新      |
| 变量选择     | 可分别选择是否管理 HTTP、HTTPS 与 ALL 代理变量       |
| 托盘与偏好   | 从托盘打开或切换，并持久化语言、主题和窗口行为       |

## 与环境变量工具的定位对比

推荐度按各工具最擅长的使用场景评估，并非通用功能排名。ProxyEnv 专注代理变量自动发现与切换；其余工具更适合通用环境变量或 `PATH` 管理。

| 工具                                        | 推荐度     | 适合什么情况                          | 主要特点                                                                   |
| ------------------------------------------- | ---------- | ------------------------------------- | -------------------------------------------------------------------------- |
| **ProxyEnv**                                | ⭐⭐⭐⭐⭐ | 经常切换代理/直连、使用多个代理客户端 | Rust + Tauri、目标跨平台、自动发现客户端/端口/协议、快照回滚、活动端点同步 |
| Microsoft PowerToys – Environment Variables | ⭐⭐⭐⭐⭐ | 大多数开发者                          | 微软官方、界面现代、支持 Profile、User/System 变量                         |
| EnvStudio                                   | ⭐⭐⭐⭐⭐ | `PATH` 很复杂、多开发环境             | 拖拽 `PATH`、去重、失效路径检测、冲突检测、快照回滚                        |
| Envarly                                     | ⭐⭐⭐⭐½  | 喜欢开源、希望修改更安全              | 开源、修改前 Diff、快照/回滚、`PATH` 拖拽、PowerShell/Ansible 导出         |
| Rapid Environment Editor                    | ⭐⭐⭐⭐   | 传统 Windows 开发环境                 | 很成熟、`PATH` 树状管理、错误检测、备份、便携版                            |
| envx                                        | ⭐⭐⭐⭐   | 喜欢终端/TUI                          | Rust、跨平台、快照/Profile、搜索、CLI、`.env`/JSON/YAML 导入导出           |

## 支持的代理客户端

| 客户端          | Windows 进程识别                                | 图标     | 当前状态              |
| --------------- | ----------------------------------------------- | -------- | --------------------- |
| Clash Verge Rev | `clash-verge.exe`、`verge-mihomo.exe`、服务进程 | 官方图标 | 已在 Windows 实机验证 |
| v2rayN          | `v2rayN.exe` + Xray / sing-box / Mihomo Core    | 官方图标 | 规则已接入            |
| FlClash         | `FlClash.exe` + Mihomo Core                     | 官方图标 | 规则已接入            |
| Hiddify         | `Hiddify.exe` + sing-box Core                   | 官方图标 | 规则已接入            |
| Clash Nyanpasu  | `clash-nyanpasu.exe` + Mihomo / Clash RS        | 官方图标 | 规则已接入            |

检测不会仅凭一个通用 Core 进程武断判断客户端。对于 `mihomo.exe`、`sing-box.exe` 等可能被多个 GUI 使用的进程，ProxyEnv 会结合正在运行的前端进程、系统代理端点和监听 PID 再做归属。

客户端图标来自各自官方仓库，许可与来源见 [`public/proxy-clients/ATTRIBUTION.md`](public/proxy-clients/ATTRIBUTION.md)。

## 管理哪些变量？

变量名根据操作系统选择，不再同时展示重复的大小写别名：

```text
Windows：       HTTP_PROXY      HTTPS_PROXY      ALL_PROXY
Linux / macOS： http_proxy      https_proxy      all_proxy
```

仅展示、默认不删除：

```text
Windows：       NO_PROXY
Linux / macOS： no_proxy
```

Windows 环境变量名不区分大小写，因此大小写别名实际指向同一个值；Linux 与 macOS 环境变量名区分大小写，ProxyEnv 采用兼容性更广的小写代理变量约定。当前 v0.1 的环境持久化与代理发现后端仅在 Windows 上实现并完成测试；Linux/macOS 后端完成前不会标记为正式支持。

默认勾选 `HTTP_PROXY` 与 `HTTPS_PROXY`。`ALL_PROXY` 需要用户主动启用，因为它可能影响更多应用及包搜索流量；至少需要保留一个代理变量。当前界面仅只读展示 `NO_PROXY` / `no_proxy`。

ProxyEnv 只操作当前用户：

```text
HKEY_CURRENT_USER\Environment
```

它不会写入 `HKLM`，v0.1 默认不需要管理员权限。

## 工作原理

### 环境变量事务

```text
Disable
读取当前值 → 保存快照 → 删除变量 → 广播变更 → 读回验证

Enable
使用检测到的活动端点 → 写入所选变量 → 广播变更 → 读回验证
没有活动端点 → 回退到最近保存的快照
```

快照保存在：

```text
%LOCALAPPDATA%\ProxyEnv\env-snapshot.json
```

### 代理发现链

```text
Windows System Proxy ─┐
Known Client Process ─┼─→ TCP Listener + PID
Local Listener Table ─┘            │
                                   ▼
                         HTTP / SOCKS5 Probe
                                   │
                                   ▼
                      Merge → Score → Recommend
```

探测仅访问已经发现的本机候选端点，不扫描 `1–65535` 全端口，也不访问外部测试站点。

## 快速开始

### 用户运行要求

- Windows 10 1803 或更高版本（x64），或 Windows 11 x64
- Microsoft Edge WebView2 Runtime；较新的 Windows 10/11 通常已经预装

安装打包后的 ProxyEnv 不需要用户安装 Node.js、pnpm、Rust 或 Visual Studio。

### 源码开发要求

- Node.js 20.19+ 或 22.12+；推荐 Node.js 22 LTS
- 通过 Corepack 使用 pnpm 10
- Rust stable MSVC 工具链
- Visual Studio Build Tools 2022（Desktop development with C++）

以上最低版本来自当前 Vite 7 与 Tauri 2 工具链的实际要求。建议使用仍受支持的 Node.js LTS，不要求追随最新 Current 版本。

### 从源码运行

```powershell
# 克隆仓库后进入项目目录
cd ProxyEnv
corepack enable
pnpm install
pnpm tauri dev
```

也可以使用 VS Code：安装工作区推荐扩展，选择 `ProxyEnv: Tauri Debug`，然后按 `F5`。

### 检查与构建

```powershell
# 前端类型检查与生产构建
pnpm build

# Rust 测试
cargo test --manifest-path src-tauri/Cargo.toml

# 严格静态检查
cargo clippy --manifest-path src-tauri/Cargo.toml --all-targets -- -D warnings

# 构建 Windows NSIS 安装包
pnpm tauri build
```

## 项目结构

```text
ProxyEnv/
├─ src/                         # Vue 3 前端
│  ├─ App.vue
│  ├─ services/                 # Tauri IPC 封装
│  └─ types/
├─ src-tauri/                   # Rust / Tauri 后端
│  └─ src/
│     ├─ environment/           # 注册表、快照、广播与事务
│     └─ proxy/                 # 系统代理、进程、监听端口与协议探测
├─ public/proxy-clients/        # 客户端图标及归属说明
├─ docs/assets/                 # README 插图
└─ .vscode/                     # 本地调试配置
```

## 安全与隐私

- 所有检测和环境变量操作均在本机完成。
- 不读取代理节点、订阅 URL、密码、Token 或用户流量。
- 不上传进程、端口或代理配置。
- 不修改 Windows 系统代理。
- 不写入系统级环境变量或 `HKLM`。
- 协议探测使用短超时，并且只连接本机候选端口。
- Disable 前必须成功保存快照，失败则不会继续删除变量。

安全问题请参阅 [`SECURITY.md`](SECURITY.md)。

## 不会做什么

ProxyEnv 不是代理客户端。v0.1 不提供：

- 节点、订阅或规则管理
- Clash / v2rayN 控制 API
- TUN、VPN 或驱动管理
- Windows System Proxy 开关
- Per-App Proxy 或进程注入
- 全端口扫描、抓包或流量上传
- 内置 Mihomo、sing-box、Xray 等代理核心
- AIX、FreeBSD 等其他 Unix 变体

## 开发路线

- [x] Vue 3 + Tauri 2 项目骨架
- [x] 用户级环境变量读取
- [x] Disable 快照、删除、广播与验证
- [x] Enable 原值恢复与验证
- [x] Windows System Proxy 解析
- [x] TCP 监听端口与 PID 关联
- [x] HTTP / SOCKS5 / Mixed 协议探测
- [x] 主流客户端识别与官方图标
- [x] HTTP / HTTPS / ALL 代理变量选择
- [x] 系统托盘切换、打开与退出操作
- [x] 多语言设置、开机启动与窗口行为
- [ ] 多候选代理选择界面
- [ ] 托盘 ON / OFF / Warning 动态图标
- [ ] Windows 集成测试矩阵
- [ ] Linux 环境持久化、进程与监听端口后端
- [ ] macOS 环境持久化、系统代理与进程后端
- [ ] Linux/macOS 安装包与集成测试矩阵
- [ ] NSIS 与 Portable 自动发布

## 常见问题

### 为什么切换后，已经运行的软件没有立即变化？

Windows 进程通常在创建时复制父进程的环境变量。ProxyEnv 会广播环境变更，但无法强制已经运行的终端或应用重建自己的 environment block。请在切换后启动目标软件，必要时重新打开终端。

### Disable 会把变量设置为空字符串吗？

不会。ProxyEnv 会从 `HKCU\Environment` 中真正删除受管理变量，并在此之前保存快照。

### Enable 会更新我自己的代理地址吗？

会，但前提是检测到了经过验证的本机活动代理。环境变量开关开启期间，ProxyEnv 会让受管理变量跟随该端点，并且只在端点变化时写入；Enable 时如果没有活动端点，则回退到最近保存的快照。

### 为什么能看到代理客户端，却没有候选端口？

客户端进程存在不代表代理 Core 已经监听。ProxyEnv 只展示经过系统代理、监听 PID 或协议探测支持的候选，不根据客户端名称猜端口。

## 贡献

欢迎提交问题和改进。开始前请阅读 [`CONTRIBUTING.md`](CONTRIBUTING.md)，并保持 Environment Core 与 Proxy Detection 解耦。

## 贡献者

- ProxyEnv 维护者与社区贡献者
- OpenAI Codex——参与实现、测试与文档编写的 AI 编程助手

## 许可证

ProxyEnv 源代码基于 [MIT License](LICENSE) 发布。第三方客户端图标适用各自上游许可证，详见图标归属说明。
