# 参与 RouteSpan 开发

[English](CONTRIBUTING.md) · 简体中文

RouteSpan 面向 Windows 10/11 x64，使用 Tauri 2、Vue 3、TypeScript 和 Rust stable MSVC。
仓库/package 为兼容保留旧品牌 ProxyEnv 标识。修改 IPC 或持久化边界前，请阅读
[架构](docs/ARCHITECTURE.zh-CN.md)、[内部契约](docs/INTERNAL_CONTRACTS.zh-CN.md)和[安全策略](SECURITY.zh-CN.md)。

RouteSpan v0.2.0 是公开稳定基线。`v0.2.1dev` 分支面向 v0.2.1 开发，不代表已发布。
代码完成不代表安装包实机验收，状态见[发行验收](docs/RELEASE_QA_V0.2.zh-CN.md)；详细设备记录仍私下保存。

## 本地检查

使用 package.json 声明的 Node.js/pnpm 版本、Windows 构建工具、WebView2 和 Rust stable MSVC，按锁文件安装。

```powershell
pnpm install --frozen-lockfile
pnpm build
pnpm test:brand
pnpm test:docs
pnpm test:release
pnpm test:updates
pnpm test:themes
pnpm test:tray
pnpm test:report
pnpm test:remote
pnpm test:extensions
cargo fmt --manifest-path src-tauri/Cargo.toml --all -- --check
cargo test --manifest-path src-tauri/Cargo.toml --lib --locked
cargo clippy --manifest-path src-tauri/Cargo.toml --all-targets --locked -- -D warnings
```

`pnpm check` 汇总前端、品牌/文档/发行/更新/主题/托盘/报告/远端/扩展检查和锁定依赖 Rust 测试。文档测试离线执行。
扩展测试包含 POSIX/Linux 文件系统用例；平台跳过不是对应行为通过的证据。
生成的远端助手必须可复现，`pnpm test:extensions` 在事务测试前检查它。
浏览器模拟数据测试只是 UI 辅助证据，不等于原生 Windows、真实 SSH 或模型验收。

## 修改原则

前端依赖方向为 app → features → shared，Rust 命令保持职责有限。
复用活动代理上下文、后端 generation/ticket 和共享状态。
文档修改不混入依赖升级、SSH 重写或数据迁移。
保留主机信任、固定上游、仅恢复管理内容和私有文件边界；前端检查不能授权任意命令/路径。

单实例插件必须先于托盘/窗口初始化注册。生命周期修改需双启动构建后的 EXE，
验证仅一个进程、恢复/聚焦窗口，第二次启动不修改状态或意外导航。

注册表、快照或广播修改需 Windows 集成证据，覆盖准确恢复、冲突保护、失败回滚、
WM_SETTINGCHANGE 和新进程继承。远端修改需明确范围的认证、重连、清理及用户文件保留证据。
不要从模拟截图推断验收通过。

## 文档修改

遵循[文档地图](docs/README.zh-CN.md)。当前文档说明当前行为；归档是历史，不是另一套操作指引。
双语同步章节顺序、能力数量、状态与安全声明，保留历史发行说明和兼容 ID。
QA 只依据带日期、构建/环境范围的证据更新；缺少验收使用 PENDING/BLOCKED。

## 发行与密钥

正式发行要求 tag/package/Cargo/Tauri 版本一致、多语言说明已审阅、冻结依赖、审计/构建/测试、
签名更新产物、校验值和来源证明，再创建 Draft 手动验收。Draft 不等于公开发布。
正式流程仅接受稳定版本 tag，不使用 RC/预发行 tag。
`pnpm test:release` 保护版本、兼容标识、产物名称和四语说明，不等于原生或签名产物验收。

只用官方 Tauri Updater、固定来源/公钥、默认只升级比较和用户明确检查/安装。
NSIS 替换与 MSI/便携版手动更新不同。不自建下载器或静默执行安装器，见[发行安全](docs/release-security.zh-CN.md)。

不要提交、打印或打包签名私钥、密钥前缀、密码、PFX/P12 或解码后的敏感材料。
CI 私钥只使用授权 Actions Secrets `TAURI_SIGNING_PRIVATE_KEY` 和 `TAURI_SIGNING_PRIVATE_KEY_PASSWORD`；
受保护恢复副本存于受跟踪源码之外。公开校验公钥不是秘密。
