# RouteSpan 内部契约

[English](INTERNAL_CONTRACTS.md) · 简体中文

本文规定长期产品/兼容约束，不记录实施流水账。[架构](ARCHITECTURE.zh-CN.md)解释机制，
[验收台账](RELEASE_QA_V0.2.zh-CN.md)记录证据。当前源码/测试事实优先于历史计划。

## 信息架构

| 层级 | 页面 | 契约 |
| --- | --- | --- |
| 一级 | 本机环境 | 本机能力与应用助手的唯一入口 |
| 一级 | 远程桥接 | 目标/能力设置，连接后进入工作区 |
| 上下文 | 应用助手 | 本机细分页面，返回本机环境 |
| 上下文 | 设置 | 返回打开设置前的主工作区 |
| 全局 | 诊断报告 | 快照弹窗，不替代导航 |
| 已连接远程 | 概述 / 高级 | 同一连接，共享页头与观察结果 |

保留两个主导航，不另建顶层 AI 供应商页面。主页面切换保持挂载。
远程状态为小型静态状态点；需要处理优先于连接中，再优先于已连接/空闲。
可选准备失败本身不改变已验证的 SSH 传输状态。状态同时用图标/文字与颜色表达，四种语言语义一致。

操作反馈统一使用 NotificationCenter，表单校验和持久工作流状态留在控件旁。
不恢复页面底部操作消息、重复设置步骤或额外桥接确认页。即使界面不再二次确认，后端仍必须重新验证。

## 品牌资产

展示品牌：**境枢 RouteSpan**；英文名：**RouteSpan**。
标语：**让本地能力，延伸到远程。** / **Extend local capabilities to remote environments.**

唯一源为 `assets/brand/routespan-mark-source.png`。Header/About 复用 BrandMark 和等比例衍生图；
原生/托盘图标来自同一源。没有明确品牌更新时，不重画几何、不用 SVG 替代、不修改颜色。
[资产规则](../assets/brand/README.zh-CN.md)定义复现；界面、窗口、托盘、终端与外部产物名使用展示品牌。

## 兼容标识

以下旧品牌 ProxyEnv 兼容标识有意保留：

| 标识 | 保留值 / 规则 |
| --- | --- |
| npm / Cargo package | `proxyenv` |
| Rust 库 / 内部二进制 | `proxyenv_lib` / `proxyenv.exe` |
| Tauri identifier | `io.github.gkneeteiemay.proxyenv` |
| 本机应用数据 | `%LOCALAPPDATA%\\ProxyEnv` |
| 远端应用管理目录 | `~/.proxyenv` |
| 浏览器存储键 | `proxyenv.remoteBridge.*` |
| AI 会话 Header | `X-ProxyEnv-Session` |
| 托管供应商 | `proxyenv_bridge` / 旧供应商展示值 |
| GitHub 仓库 | `GKNEETIEMAY/ProxyEnv` |
| 更新端点 | `https://github.com/GKNEETIEMAY/ProxyEnv/releases/latest/download/latest.json` |
| 更新校验 | 嵌入的公钥保持固定 |
| 发行说明分隔符 | 既有机器可读 `proxyenv-release:*` 标记 |

展示名变更不是数据迁移。不能为“完成品牌升级”机械替换这些值。
未来迁移需要明确设计、备份/恢复兼容、安装/更新验收与单独批准。
历史发行说明保留原产品字节记录。

## CC Switch 关系

**CC Switch 管理路由，RouteSpan 负责延伸。** 供应商凭据、订阅、OAuth 和上游路由策略属于 CC Switch/用户工具。
RouteSpan 提供选定传输、托管客户端配置和 Skills 映射，不成为竞争的供应商/路由控制平台。

检测服务不等于确认各工具路由开关。启用期间可同步模型/目录/配置，但运行中客户端可能需要重启/重载。
模型请求传输保持不透明，不将文本标记猜测为工具调用，不宣称协议转换。
AI 不依赖普通网络通道；两种远程能力均依赖 SSH 传输。

## 状态与修改契约

已安装 ≠ 已配置 ≠ 已验证。内置模型验证目前仅支持已安装的 Claude CLI；Codex 用实际客户端验证。
兼容的仅插件安装不因缺少 CLI 被拒绝，依赖 CLI 的验证仍可不可用。
AI 开启并完成检测后自动准备兼容工具；主动关闭工具的选择跨会话刷新/恢复保留。

后端共享摘要/generation 提供传输、诊断和会话导出。切换页、重新打开窗口或在其它页复制，
不得破坏有效观察/命令。复制成功不证明执行成功。

修改导入 SSH 建立应用保存副本，移除隐藏导入项，不重写来源配置/书签或导入私钥正文。
配置事务保留无关用户字段，恢复证据冲突时停止。取消 Skill 同步不删本机 Skill 或外来文件。

普通网络有意无凭据且不按 UID 隔离。AI Token 由后端管理，root/同账户信任不能靠品牌、
状态徽标或 Loopback 解决。默认 Shell 设置只属于会话，不改启动文件。
启用网络有已说明的自动出口检查；付费模型验证需主动执行；生成诊断报告不探测。

## 文档与验证词汇

- **已实现（Implemented）**：存在代码和适当范围的检查。
- **已验证/已验收（Verified/Accepted）**：记录匹配的环境、构建和证据。
- **已发布（Released）**：正式公开发布；Draft 不是公开发布。
- **稳定基线**：维护者指定的上一版，可能仍为 Draft。

截至 2026-10-30，RouteSpan v0.2.2 为最新公开稳定版，前版为 v0.2.1。旧品牌 ProxyEnv
v0.1.3 与 v0.1.4 均为历史公开发行。中英文 README、索引、路线图、安全/发行文档与 QA 的发布状态
应保持一致，但不上传私下保存的验收记录。
公开后同步更新双语 README、索引、路线图、安全/发行文档和验收台账。

## 当前事实清单

归并前已检查下列行为来源；它们不能证明所有真实设备已验收。

| 事实 | 当前来源 |
| --- | --- |
| 主/上下文导航与预加载 | `src/app/AppShell.vue`；`scripts/test-app-navigation.mjs` |
| 展示/原生品牌与兼容性 | `scripts/test-branding.mjs`；`src-tauri/tauri.conf.json` |
| 核心/连接后准备分离、断开、自动出口检查 | `src-tauri/src/features/remote_bridge/mod.rs` |
| SSH PTY 与主机信任 | `src-tauri/src/features/remote_bridge/ssh.rs` |
| 共享配置/版本/安装行为 | 远端后端的 `tool_adapter.rs`、`tool_discovery.rs`、`remote.sh` |
| 固定上游 AI 会话鉴权 | 远端后端的 `authenticated_relay.rs` |
| CC Switch 链接 Skill / 仅移除管理内容 | 远端后端的 `skills.rs` |
| 有界 JSONL 事件 / 安全报告 | `logging.rs`、`diagnostics.rs`；`scripts/test-diagnostic-report.mjs` |
| Draft 工作流、签名、不变更新来源 | `.github/workflows/release.yml`；`src-tauri/tauri.conf.json` |
