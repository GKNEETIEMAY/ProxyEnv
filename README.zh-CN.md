<p align="center">
  <img src="assets/brand/routespan-mark.png" width="88" alt="RouteSpan">
</p>

<h1 align="center">境枢 RouteSpan</h1>

<p align="center"><strong>让本地能力，延伸到远程。</strong></p>
<p align="center">本机环境 · 远程桥接 · CC Switch AI 路由延伸</p>
<p align="center"><a href="README.md">English</a> · 简体中文</p>

<p align="center">
  <a href="docs/GETTING_STARTED.zh-CN.md"><img alt="本机平台：Windows 10 / 11 x64" src="https://img.shields.io/badge/platform-Windows%2010%20%7C%2011%20x64-477da6?style=flat-square"></a>
  <a href="LICENSE"><img alt="MIT 许可证" src="https://img.shields.io/badge/license-MIT-34745b?style=flat-square"></a>
</p>

## 发布状态

**RouteSpan v0.2.1 是最新公开稳定版**，于 2026-10-20 发布。
请从 [v0.2.1 发行页](https://github.com/GKNEETIEMAY/ProxyEnv/releases/tag/v0.2.1)
下载 Windows x64 的 NSIS、MSI 或便携包。

旧品牌为 ProxyEnv：v0.1.3 与 v0.1.4 均为历史公开发行，v0.1.4 于 2026-10-13 发布。
RouteSpan 的上一稳定版为 v0.2.0。
已有 NSIS 用户可使用**关于 → 检查更新**；MSI 与便携版需手动更新。
手动预览包不属于稳定更新渠道；早期公开安装包不具备远程桥接能力。

## RouteSpan 是什么？

RouteSpan 是一个 Windows 桌面工具，帮助你理清本机代理环境，并将选定的本机能力
延伸到远程 Linux 开发账户。它连接你电脑上已经运行的服务与通过 SSH 使用的开发工具。

**适用平台**：本机为 Windows 10/11 x64，需要 WebView2；远端为非 root Linux 账户。
目前不支持将 Linux/macOS 作为本机桌面平台。安装包不要求用户安装 Node.js 或 Rust。

本机工作区帮助你选定一个活动代理，区分系统代理、TUN 与环境变量状态，并使用代理或
直连环境启动新应用。远程工作区复用这个选择提供网络访问，延伸 CC Switch 的本机
AI 路由，并为远端账户准备受支持的开发工具配置和选定的 Skills。

它不是代理内核、VPN、订阅管理器，也不是 AI 供应商服务。你仍然需要自行运行本机
代理客户端；使用 AI 路由时，还需要运行 CC Switch 并开启相应路由。
RouteSpan 不接管这些工具，而是减少它们与远程开发环境之间的重复配置。

## 为什么使用 RouteSpan？

<p align="center"><a href="docs/assets/routespan-workflow.png"><img src="docs/assets/routespan-workflow.png" width="880" alt="概念插图：分散的网络、AI 路由与 Skills 配置，通过 RouteSpan 整合为选定的远程工作区"></a></p>

概念插图：绿色勾选表示目标流程或配置准备，不代表网络访问与模型请求已经验证通过。

本机代理客户端已经切换，应用却可能继承着旧的代理环境变量；服务器可以 SSH 登录，
远端终端却可能无法访问需要的网站；AI 插件已经安装，仍可能与 CLI 使用不同的路由。
这些不是同一个状态，用一个“已连接”代替所有结论会让排查更加困难。

RouteSpan 将这些层次分别展示：先选择本机能力，再选择 SSH 目标，只开启本次会话需要的
功能。工具安装、配置完成与真实请求验证保持独立，不会将“检测到端口”说成“模型可用”。
你可以只桥接本机网络、只延伸 AI 路由，也可以同时开启两者。

这样，你既能继续使用自己熟悉的代理客户端、CC Switch 和编辑器，也能在切换机器、
服务器或工具时知道哪些配置已准备好，哪些结果需要实际验证，哪些操作会修改文件。

## 工作方式

<p align="center"><a href="docs/assets/routespan-architecture.png"><img src="docs/assets/routespan-architecture.png" width="880" alt="能力流向：Windows 本机代理与 CC Switch AI 路由，经 RouteSpan 的 SSH 反向转发延伸到远程 Linux 账户；选定的托管 Skills 独立进行文件同步"></a></p>

网络与 AI 路由使用 SSH 反向转发，选定的托管 Skills 使用文件同步。插图仅用于解释
使用场景与能力流向，不是界面截图、真实可用性证明，也不是逐包传输或 SSH 建立方向图。
窄窗口下可以点击图片查看大图。

桥接属于当前选中的账户与会话。它不会把服务器改造成新的 VPN 节点，不修改服务器的
系统网络，也不会替代 CC Switch 管理的供应商配置。远端使用的是被明确开启的入口，
而不是自动获得本机所有网络和文件访问能力。

## 核心能力

### 本机环境

- 检测受支持的本机代理客户端，并选择一个活动端点。
- 分别查看系统代理、TUN 观测结果与代理环境变量状态。
- 应用、清除或恢复选定的当前用户代理环境变量。
- 诊断选定的应用，以代理或直连环境启动一个新进程。

环境变量变更影响新启动的进程。已有终端或应用可能需要重新打开；RouteSpan 不向运行中
的进程注入新配置，也不替你切换 Windows 系统代理或 TUN。多个代理客户端同时运行时，
各模块复用同一个活动代理选择，不在后台悄悄换成另一个端口。

<p align="center"><a href="docs/assets/routespan-app-launch.png"><img src="docs/assets/routespan-app-launch.png" width="880" alt="应用启动选择：以活动代理或直连环境创建一个新进程"></a></p>

为**新进程**选择代理或直连环境，已有应用保持不变。启动成功不代表目标应用已经联网，
也不代表 Windows 系统代理或 TUN 被切换。

### 远程桥接

- 选择自动导入或手动保存的 SSH 连接，通过 OpenSSH 完成认证。
- 复用本机网络访问能力，并独立延伸 CC Switch AI 路由。
- 分别发现远端 CLI 与 VS Code 插件安装情况，再准备兼容工具。
- 打开远端终端、VS Code 或 MobaXterm，同步选定的托管 Skills。

连接后默认进入**概述**，集中展示日常操作；**高级**展示环境详情、独立链路诊断和
恢复工具。SSH 核心桥接确认后即可显示已连接，工具准备在后台继续，不会为了可选工具
让整个页面一直停留在连接中。某个工具准备失败也不代表其他能力必然失败。

### CC Switch AI 路由延伸

**CC Switch 负责路由，RouteSpan 负责延伸。** RouteSpan 不替代 CC Switch，而是把
CC Switch 管理的本机 AI 路由能力延伸到远程开发环境；不管理供应商、订阅或 OAuth。

请先在 **CC Switch → 设置 → 路由 → 本地路由 → 路由总开关 → 开启 Claude/Codex
路由**。RouteSpan 会检测本机路由服务，但监听端口被检测到，不代表每个工具的路由都
已打开，也不代表供应商已经接受真实的模型请求。

受支持的 Codex/Claude 设置会在远端接入启用期间跟随本机客户端配置。供应商密钥和
上游地址留在本机，不复制到服务器。正在运行的 CLI 或插件可能需要重启或重新加载，
才能使用更新后的模型设置；文件同步成功不能保证运行中的进程立即切换模型。

## 快速开始

1. 安装官方 v0.2.1 Windows x64 安装包。
2. 启动本机代理客户端，在**本机环境**确认活动端点。
3. 需要 AI 接入时，先启动 CC Switch 并开启相应工具的本机路由。
4. 在**远程桥接**选择或添加 SSH 连接，选择本次需要的能力。
5. 建立连接，明确确认可信的新主机指纹，并按提示完成认证。
6. 在**概述**查看工具准备结果，打开需要的终端或编辑器。

只使用本机环境时，不必连接服务器，也不必开启 AI 路由。只需要远端 AI 时，不必开启
普通网络桥接；若还需要下载依赖、访问外部网站，可以同时开启网络能力。
首次使用请先确认本机服务本身正常，不要把供应商密钥错误误认为 SSH 故障。

外部远端终端应按界面说明执行复制的终端配置命令，再启动新的 CLI；已经打开的
VS Code Remote 窗口可能需要执行 **Developer: Reload Window**，已有终端需要关闭重建。
首次连接的服务器可能尚未准备好 VS Code Server，等待 Remote - SSH 初始化后再检查。
如果仅一个工具不可用，无须重新安装已经能正常工作的其他工具。

查看[快速开始指南](docs/GETTING_STARTED.zh-CN.md)了解前置条件，查看
[远程桥接指南](docs/REMOTE_BRIDGE.zh-CN.md)了解当前连接流程、启动入口与常见问题。

## 受支持的远程开发工具

| 工具或入口 | v0.2.1 行为 |
| --- | --- |
| Codex CLI | 支持从 `0.134` 起的 `0.x` 版本，使用远端标准用户配置 |
| Claude Code CLI | 支持 `2.x`；会话 Header 接入要求 `2.1.227` 或更高 |
| VS Code Remote 插件 | Codex 与 Claude 分别检测，独立于 CLI；兼容的仅插件环境也可配置 |
| 终端 | RouteSpan 启动的终端自动加载会话环境；外部终端可能需执行复制的配置命令 |
| MobaXterm | 对受支持连接提供启动集成；外部 Shell 按说明加载会话环境 |
| Skills | 仅同步选定的 CC Switch 托管链接，Codex/Claude 可分别管理 |

安装引导提供由用户执行的命令，不静默安装 CLI、VS Code Server 或依赖。工具检测与
配置分别显示，缺少 CLI 不会自动阻止兼容插件接入；但依赖 CLI 的模型验证无法在纯插件
环境执行。自定义工具配置根目录和不受支持的远端运行时可能阻止自动准备，这并不等于
已经建立的 SSH 桥接失效。版本、安装位置与运行窗口都需要和实际使用的入口一致。
内置真实模型验证目前仅支持已安装的 Claude CLI；Codex 请通过实际客户端对话验证。

<p align="center"><a href="docs/assets/routespan-tool-readiness.png"><img src="docs/assets/routespan-tool-readiness.png" width="880" alt="工具准备状态：发现安装、准备路由配置与验证真实请求是三个独立事实"></a></p>

概念示例：**已安装**表示发现工具，**已配置**表示设置已准备，**已验证**需要真实请求证据。
CLI 与插件分别检测；兼容的仅插件环境不要求安装 CLI。图中的绿色勾选仅示意准备状态，
不能代替模型请求验证。

## 安全与边界

- 普通网络代理桥接明确**不使用代理账号密码认证**。回环监听**不能隔离共享 Linux
  主机的不同 UID**；同一服务器上的其他账户可能使用启用中的网络桥接。
- AI 路由独立使用后端管理的 `X-ProxyEnv-Session` 会话令牌，不复制供应商密钥。
  这项保护**不能防御服务器 root 管理员**，也不声称保护不可信的同账户进程。
- 首次信任主机指纹需要确认，已知主机指纹改变不会被绕过。符合条件的 SSH 密码仅在
  当前桥接期间加密保留于内存，切换目标、断开或退出后清除。
- 默认会话准备不修改 `.bashrc` 或 `.profile`，不接管 TUN，也不允许任意本机转发目的地。
- 启用网络桥接后会自动执行一次外网出口检查。模型验证需明确触发，可能消耗 token。
  生成诊断报告不会发起新的联网测试，也不会自动保存或上传报告内容。
- 受保护的配置写入保留备份，并拒绝覆盖冲突的用户修改。关闭隧道与恢复全部工具
  托管字段不是同一个操作；停用工具接入时才执行对应的配置恢复事务。

用于共享服务器之前，请阅读[安全策略](SECURITY.zh-CN.md)。安装包签名、校验与下载信任见
[发行安全](docs/release-security.zh-CN.md)。反馈时不要粘贴密码、Token、会话 Header 或完整私有配置。

## 当前 v0.2 状态

v0.2.1 已发布本机环境、远程桥接、CC Switch 路由延伸、工具准备、托管 Skills 与
概述/高级界面，保留旧品牌安装及更新身份。源码 CI 与签名发行流程通过，冻结产物
在公开发布前完成独立验证，维护者已确认升级和远端功能验收。
Tauri 更新签名不是 Windows Authenticode 签名。

[v0.2 发行验收](docs/RELEASE_QA_V0.2.zh-CN.md)展示发布状态与历史检查点，后续计划见
[路线图](docs/ROADMAP.zh-CN.md)。按维护者要求，详细验收记录仅私下保存，不上传仓库。
发行通过并不代表所有未来供应商、运行时或网络环境都兼容；重要工作环境仍需按实际
客户端和服务器条件验证，不应将安装、配置与请求验证混为一谈。

## 文档

- [文档地图](docs/README.zh-CN.md)
- [快速开始](docs/GETTING_STARTED.zh-CN.md)
- [远程桥接用户指南](docs/REMOTE_BRIDGE.zh-CN.md)
- [工程架构](docs/ARCHITECTURE.zh-CN.md)
- [内部契约](docs/INTERNAL_CONTRACTS.zh-CN.md)
- [安全策略](SECURITY.zh-CN.md)与[发行安全](docs/release-security.zh-CN.md)
- [路线图](docs/ROADMAP.zh-CN.md)与[发行验收](docs/RELEASE_QA_V0.2.zh-CN.md)

当前指南负责解释现在怎么用。历史设计记录不是另一套操作说明，也不覆盖当前行为；
请通过文档地图选择用户指南、技术边界或验收证据，避免把旧阶段设计当作当前承诺。

## 开发

桌面技术栈为 Tauri 2、Vue 3、TypeScript 与 Rust。使用 `package.json` 声明的
Node.js/pnpm 版本、Rust stable MSVC 和 Windows 构建工具。

```powershell
pnpm install --frozen-lockfile
pnpm build
pnpm test:docs
pnpm tauri dev
```

[贡献指南](CONTRIBUTING.zh-CN.md)列出完整回归命令。仓库与部分内部兼容标识仍保留 ProxyEnv
名称，以保持现有更新与用户数据连续性，这不是另一个产品。不要机械重命名包、存储目录
或 Updater 标识。本阶段文档整理不升级依赖、不更改连接协议、不迁移用户数据。

## 许可

[MIT](LICENSE)。
