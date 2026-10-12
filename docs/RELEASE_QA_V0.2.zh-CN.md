# RouteSpan v0.2 发行验收

[English](RELEASE_QA_V0.2.md) · 简体中文

本文是唯一当前证据台账的中文镜像，不是另一套验收结论。状态仅为 **PASS**（通过）、
**PARTIAL**（部分）、**PENDING**（待验收）、**BLOCKED**（阻塞）。每条记录限定日期、构建、环境、
预期、实际和证据。截图不是网络证据，代码存在不是验收；尚未决定正式发布 v0.2。

## 基线与发布状态

基线 HEAD：`6ab9ef65579e8c47e412e3e4863483035f8bec38`，分支 `v0.2.0dev`。
文档归并与插图变更基于此基线，没有修改运行时源码或依赖版本。
维护者指定前版为旧品牌 ProxyEnv v0.1.4。2026-10-12 GitHub API 显示 `isDraft=true`、
`publishedAt=null`，最新公开版为 v0.1.3。下一版 RouteSpan v0.2.0 未发布，不能混用这些状态。

## 已记录证据

| 日期（Asia/Shanghai） | 构建 / 提交 | 环境 | 预期 | 实际 | 证据 | 状态 |
| --- | --- | --- | --- | --- | --- | --- |
| 2026-10-12 | 6ab9ef6 | 本机 Windows、Node | 品牌资产/展示与受保护身份正确 | 预览构建时 6 项通过 | `node --test scripts/test-branding.mjs` 输出，源 PNG hash 固定 | PASS |
| 2026-10-12 | 6ab9ef6 | 本机 Windows、Node | 生成远端助手匹配源码 | 退出码 0 | 预览构建中的 `node scripts/build-remote-extension.mjs --check` | PASS |
| 2026-10-12 17:39 | 6ab9ef6，干净源码 | 本机 Windows 生产构建 | 构建手动 x64 NSIS 预览 | 前端及锁定优化 Rust 构建成功，产出 NSIS | `tauri build --ci --bundles nsis --config '{"bundle":{"createUpdaterArtifacts":false}}' -- --locked`；安装包 SHA-256 `9ff562d619db6e837af2b1b1e495d44a2579917629db9bac4a5a4c318c65fcad` | PASS |
| 2026-10-12 | 6ab9ef6 | 模拟浏览器 IPC，四语言，560/980 CSS px | 页头/导航/最终标识适配浅色与深色 | 先前几何/品牌检查通过，仅辅助证据 | `scripts/test-branding-ui.mjs` 审阅输出，截图不作为 README 展示 | PASS |
| 2026-10-12 17:04 | 6ab9ef6 | GitHub Linux 扩展任务 | 真实 Linux 文件系统事务测试 | 任务成功 | [CI 38227235963](https://github.com/GKNEETIEMAY/ProxyEnv/actions/runs/38227235963)，Linux 扩展任务 | PASS |
| 2026-10-12 17:04 | 6ab9ef6 | GitHub RustSec 任务 | 审计锁定 Rust 依赖 | 任务成功 | 同一 CI，RustSec 任务 | PASS |
| 2026-10-12 17:04 | 6ab9ef6 | GitHub Security audit | 配置的安全审计通过 | 检查成功 | [检查 114738512010](https://github.com/GKNEETIEMAY/ProxyEnv/runs/114738512010)，提交 check-runs API 已确认 | PASS |
| 2026-10-12 17:04 | 6ab9ef6 | GitHub Windows 任务 | 完成审计/构建/测试 | `pnpm audit --audit-level high` 失败，后续构建/测试/Rust 跳过 | 同一 CI 的 Windows 前端依赖审计步骤 | BLOCKED |
| 较早用户反馈，准确构建/日期不完整 | 未完整固定 | 用户 Linux 目标与 Windows 客户端 | 网络、CLI/插件、Skills、VS Code/Moba 启动 | 用户反馈部分成功，不是可复现完整矩阵 | 对话验收反馈，构建/环境元数据不完整 | PARTIAL |
| 2026-10-12 | 6ab9ef6 预览 | 第二台 Windows | 安装/使用预览包 | 已提供包，尚无新机器结果 | 手动预览目录、README/校验值，等待测试 | PENDING |
| 2026-10-12 审阅 | v0.1.4 → 预期 v0.2 包 | 干净 Windows 10/11 | 升级保留身份/数据、启动/卸载 | 无限定范围的最新包证据 | 需维护者记录源/目标构建与结果 | PENDING |
| 2026-10-12 审阅 | 预期 v0.2 包 | 真实 Windows 125%/150% DPI | 任务栏/托盘/开始菜单/Alt+Tab/原生图标正确 | 模拟几何/PNG 未覆盖原生缓存/DPI | 需干净机器截图与生命周期记录 | PENDING |
| 2026-10-12 审阅 | 预期 v0.2 包 | 真实 SSH/共享 Linux/CLI/插件 | 完整密钥/Agent/密码/KBI、Token、恢复/重连矩阵 | 早期成功不足以覆盖全部 | 需可复现目标/构建/供应商记录 | PENDING |
| 2026-10-12 审阅 | 预期 tag 发行 | 外部下载与注册 NSIS | 签名更新、校验值/来源、失败/降级处理 | 无当前正式签名发行验收 | 需独立产物/Updater 验证 | PENDING |

本地预览内部版本仍为 0.2.0，保留兼容身份。未做 Authenticode 签名，没有更新签名、
来源证明或公开更新 manifest。构建成功不是干净机器安装证据。

## 文档回归

以下检查在 2026-10-12 对文档变更集执行。平台跳过明确列出，不能算对应行为已验证。
构建保留已有、非致命的大前端 chunk 提示和 Windows 链接器消息。

| 日期 | 构建 / 提交 | 环境 | 预期 | 实际 | 证据 | 状态 |
| --- | --- | --- | --- | --- | --- | --- |
| 2026-10-12 | 6ab9ef6 + docs delta | 本机 Windows / Node | 链接、品牌、归档、双语一致性、README 插图 | 10 通过，0 失败 | `node --test scripts/test-docs.mjs`，离线解析/契约测试 | PASS |
| 2026-10-12 | 6ab9ef6 + docs delta | 本机 Edge，中英文 1040/480 CSS px | 四图加载/布局适配，新增图使用原白底线稿 | 四种语言/宽度组合通过，不是 GitHub 实际渲染 | `.impeccable/review/readme/` 截图与检查脚本 | PASS |
| 2026-10-12 | 6ab9ef6 + docs delta | 本机 Windows / Node | 品牌与受保护身份 | 6 通过，0 失败 | `pnpm test:brand` 输出 | PASS |
| 2026-10-12 | 6ab9ef6 + docs delta | 本机 Windows / Node | 报告本地化与不探测行为 | 11 通过，0 失败 | `pnpm test:report` 输出 | PASS |
| 2026-10-12 | 6ab9ef6 + docs delta | 本机 Windows，排除 POSIX 用例 | 远端/UI/安装/导航/通知 | 86 通过，37 跳过，0 失败 | `pnpm test:remote`；`PROXYENV_TEST_SHELL` 指向不存在的测试 Shell | PASS |
| 2026-10-12 | 6ab9ef6 + docs delta | 本机 Windows，排除 Linux 专用用例 | 生成助手及配置/网络事务 | 41 通过，4 跳过，0 失败 | `pnpm test:extensions` 输出 | PASS |
| 2026-10-12 | 6ab9ef6 + docs delta | 本机 Windows / TypeScript / Vite | 生产前端构建 | 类型检查/构建成功 | `pnpm build` 退出 0，保留大 chunk 提示 | PASS |
| 2026-10-12 | 6ab9ef6 + docs delta | 本机 Windows / 锁定 Rust MSVC | 库回归 | 282 通过，3 忽略，0 失败 | `cargo test --manifest-path src-tauri/Cargo.toml --lib --locked` 输出 | PASS |
| 2026-10-12 | d3c670e + 中文文档变更 | 本机 Windows / Node | 全部当前文档翻译、链接、命令/章节、QA 状态一致性与品牌身份 | 17 通过，0 失败 | `node --test scripts/test-docs.mjs scripts/test-branding.mjs`；未改运行时 | PASS |

提交前使用以上 package script 的等效 Node 入口重测：文档/品牌/报告/远端共 113 通过、37 跳过、0 失败；
扩展 41 通过、4 跳过；TypeScript/Vite 和锁定 Rust 库测试再次通过。
沙箱拒绝 realpath、临时文件和 Loopback 测试后在沙箱外重跑；环境限制不是产品回归。

## 文档设计复核

实现遵循文档地图、当前/归档分离、双语一致性、兼容身份和证据用语。
归并建立八个当前 Markdown 文档（含索引）；后续各文档补充中文镜像，不增加主题或事实来源。
十四个旧主题移入归档并标记历史；两份原有历史证据也加警告并修复链接。
未重写历史发行说明。

后续维护者要求取代原展示假设：README 使用四张白底说明图而非截图/ASCII，恢复平台/MIT 徽标；
旧品牌 v0.1.4 保留指定前版身份，但明确为 GitHub Draft，不是最新公开发行。
品牌源 PNG 未变；链接、旧品牌和兼容上下文检查通过。
本机中英文桌面/窄屏渲染和独立视觉复核通过，所要求的实际 GitHub 渲染仍待验收。

## 剩余验收

| 日期 | 构建 / 提交 | 环境 | 预期 | 实际 | 证据 | 状态 |
| --- | --- | --- | --- | --- | --- | --- |
| 2026-10-12 审阅 | 当前文档变更集 | GitHub 渲染页面 | 复核双语 README、索引/指南/架构/安全/路线图/QA | 未推送，未复核 GitHub 渲染 | 推送后复核 | PENDING |
| 2026-10-12 审阅 | 下一次触发 CI 的提交 | GitHub Windows/Linux | 最新代码保留审计并通过 CI | 未推送变更尚无 CI | 授权推送后触发，保持基线结论范围 | PENDING |

发行收口仍需解决前端审计与所有实机/签名门槛，不能为完成文档清单而把待验收改成 PASS。
