# RouteSpan v0.2 发行验收

[English](RELEASE_QA_V0.2.md) · 简体中文

本文展示公开发布状态并保留历史检查点，不是另一套验收结论。状态仅为 **PASS**（通过）、
**PARTIAL**（部分）、**PENDING**（待验收）、**BLOCKED**（阻塞）。历史记录限定日期、构建、环境、
预期、实际和证据。截图不是网络证据，代码存在不是验收。按维护者要求，最终详细验收记录
私下保存，不上传 GitHub。

## 基线与发布状态

**当前状态（2026-10-13）：RouteSpan v0.2.0 已发布并设为最新稳定版，发行门槛已关闭。**
[公开发行](https://github.com/GKNEETIEMAY/ProxyEnv/releases/tag/v0.2.0)，冻结源码
`0a731b47d17bd5d2f8a4885cbf926a1bac68d35f`。
[源码 CI](https://github.com/GKNEETIEMAY/ProxyEnv/actions/runs/38266259096) 与
[签名发行构建](https://github.com/GKNEETIEMAY/ProxyEnv/actions/runs/38335015721) 均成功。
最终产物外部验证与正式自动更新验收完成，发布期间未替换安装器字节或 tag。
本仓库不包含私下保存的设备日志和详细验收记录。

下方基线、证据与预备矩阵是注明日期的历史快照，不是已发布 v0.2.0 的未关闭门槛。
保留旧构建当时的结果，不将它们事后改成“已测试”。

基线 HEAD：`6ab9ef65579e8c47e412e3e4863483035f8bec38`，分支 `v0.2.0dev`。
文档归并与插图变更基于此基线，没有修改运行时源码或依赖版本。
维护者指定前版为旧品牌 ProxyEnv v0.1.4。2026-10-12 GitHub API 显示 `isDraft=true`、
`publishedAt=null`，当时最新公开版为 v0.1.3、RouteSpan v0.2.0 尚未发布。
这些是历史状态，不代表当前发行。

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
本机中英文桌面/窄屏渲染和独立视觉复核通过；维护者随后确认实际 GitHub README 渲染正常，
证据见下方记录，不代表自动浏览器检查已通过。

## 剩余验收

| 日期 | 构建 / 提交 | 环境 | 预期 | 实际 | 证据 | 状态 |
| --- | --- | --- | --- | --- | --- | --- |
| 2026-10-12 审阅 | v0.2.0dev 的 eee1873；根 README/插图相对 3ca4e58 未变 | 维护者 GitHub 浏览器 | 双语 README 与四图加载且无排版溢出 | 维护者确认两份 README 与四图均显示正常 | 用户直接验收反馈；自动浏览器检查因 URL 安全检查被阻止 | PASS |
| 2026-10-12 审阅 | 3ca4e58 | GitHub Windows/Linux | 最新代码保留审计并通过 CI | Linux 扩展、RustSec、Security audit 通过；Windows 在前端审计处因两项高危停止，后续步骤跳过 | [CI 38258401095](https://github.com/GKNEETIEMAY/ProxyEnv/actions/runs/38258401095)，已核对 jobs API | BLOCKED |

## 发行收口跟踪

已推送文档基线更新为 `3ca4e58`，包括中文镜像。资产说明已压缩，原始生成输入经历史索引保留；
图片及运行时行为不变。自动检查被阻止后，维护者确认实际 GitHub README 渲染正常。
已授权依赖/CI 修复；未授权创建冻结分支、tag 或发布版本。

| 日期 | 构建 / 提交 | 环境 | 预期 | 实际 | 证据 | 状态 |
| --- | --- | --- | --- | --- | --- | --- |
| 2026-10-12 | 3ca4e58 + 资产说明变更 | 本机 Windows / Node | 链接、历史警告、中文一致性及受保护品牌身份 | 17 通过，0 失败 | `node --test scripts/test-docs.mjs scripts/test-branding.mjs` | PASS |
| 2026-10-12 | 40d0d2a + 依赖变更 | 本机 Windows / pnpm 10.15.1 | 冻结安装及高危审计，保持原审计策略 | 退出 0，无已知漏洞；Vue/renderer 3.5.43、source-map-js 1.2.2，无 override | `pnpm install --frozen-lockfile` 与 `pnpm audit --audit-level high` | PASS |
| 2026-10-12 | 40d0d2a + 依赖变更 | 本机 Windows / Node / TypeScript / Vite | 构建及全部前端/远端 package-script 回归 | 构建退出 0；154 通过、41 项 POSIX/Linux 跳过、0 失败；保留已有 chunk 提示 | `pnpm build`、brand/docs/report/remote/extensions scripts；同上使用不存在的 POSIX 测试 Shell | PASS |
| 2026-10-12 | 40d0d2a + 依赖变更 | 本机 Windows / 锁定 Rust MSVC | 格式、库测试及拒绝警告的 all-target Clippy | 282 通过、3 忽略、0 失败；格式与 Clippy 退出 0 | `cargo fmt --all -- --check`、`cargo test --lib --locked`、`cargo clippy --all-targets --locked -- -D warnings`，各命令带 `--manifest-path src-tauri/Cargo.toml` | PASS |
| 2026-10-12 | eee1873a22a9bc82ac0b0e9e9fb052e3c301d61a | GitHub Windows/Linux | 四项 CI jobs/checks 全绿 | Windows 构建/测试/release EXE、Linux 扩展、RustSec、Security audit 全部成功 | [CI 38261620424](https://github.com/GKNEETIEMAY/ProxyEnv/actions/runs/38261620424)，已通过 jobs API 核对准确 SHA 与四项结论 | PASS |
| 2026-10-12 | eee1873 + 发行预检变更 | 本机 Windows / Node / PowerShell 7 | 版本/身份、稳定 tag、说明及产物收集契约 | 7 通过；不可执行安装器/签名样本保留字节、匹配更名 NSIS manifest/校验和，并拒绝歧义安装器 | `pnpm test:release`；未使用真实签名、执行安装器或发布 | PASS |
| 2026-10-12 | eee1873 + 发行预检变更 | 离线四语说明校验 | 保留兼容分隔符与各语言实际变更章节 | 中/英/日/韩说明通过校验，章节/条目结构一致 | `scripts/validate-release-notes.mjs` 与发行契约测试 | PASS |
| 2026-10-12 | 928303e65fb1ef9bdfa27df9a2e9ea8dfcffe9b4 | GitHub Windows/Linux | 最新预检变更通过完整 CI | Windows（含发行契约/release EXE）、Linux 扩展、RustSec、Security audit 全部成功 | [CI 38264570889](https://github.com/GKNEETIEMAY/ProxyEnv/actions/runs/38264570889)，已核对准确 SHA 与四项结论 | PASS |

正式发行仅使用稳定版本 tag，不使用 RC/预发行 tag；更新器比较逻辑不变。
下载名称采用 `RouteSpan-…`，内部 binary 保持 `proxyenv.exe`。
产物收集样本测试不等于真实签名 Draft、原生安装、来源证明或生产更新验收。

发行收口仍需保持审计通过并关闭所有实机/签名门槛，不能为完成文档清单而把待验收改成 PASS。

## 安装包与真实链路验收矩阵

以下只是准备好的清单，不是测试结果；预检门槛通过后再开始。每行需补准确源码 SHA、
安装包 hash、OS/工具/供应商版本、目标别名及带日期的证据，并记录实际结果与 P0/P1 问题。
主目标执行完整远端矩阵，第二目标执行冒烟。不把密码、供应商密钥或会话 Header 写入证据。
较早用户成功反馈不能关闭这些行。

| 日期 | 构建 / 提交 | 环境 / 版本 | 预期 | 实际 | 证据 | 状态 |
| --- | --- | --- | --- | --- | --- | --- |
| 待验收 | 候选安装包待固定 | Windows 10 22H2 x64，OS/WebView2/包版本待记录 | 干净 NSIS 安装、启动、关于、关闭到托盘/恢复、单实例、设置、开机/静默启动、卸载 | 未对最终候选执行 | 需设备记录 | PENDING |
| 待验收 | 候选安装包待固定 | 受支持 Windows 11 x64，准确版本待记录 | 与 Windows 10 相同原生生命周期矩阵 | 未对最终候选执行 | 需设备记录 | PENDING |
| 待验收 | v0.1.3 → 候选版 | Windows x64，源/目标安装包 hash 待记录 | 保留身份/设置/快照；唯一安装/启动/卸载项；更新身份不变 | 未执行 | 需升级记录 | PENDING |
| 待验收 | v0.1.4 Draft → 候选版 | Windows x64，前版产物/hash 待记录 | 同升级矩阵，并明确记录 Draft 前版可用情况 | 未执行 | 需升级记录 | PENDING |
| 待验收 | 候选安装包待固定 | 两种 Windows、100%/125%/150%、浅/深色 | 任务栏、托盘 16/20px、Explorer、快捷方式、开始菜单、Alt+Tab、安装图标清晰，无异常边缘/缓存 | 未执行 | 需原生截图/生命周期记录 | PENDING |
| 待验收 | 候选安装包待固定 | 主 Linux 账户，OpenSSH/Agent 版本待记录 | 新/已知主机、密钥、Agent、密码、KBI 连接正确；取消、错误密码、超时可恢复 | 未执行 | 需限定范围的 SSH 认证记录 | PENDING |
| 待验收 | 候选安装包待固定 | 主目标的主机指纹变更样本 | 拒绝连接，不自动重置信任、yes 或关闭检查 | 未执行 | 需安全样本记录 | PENDING |
| 待验收 | 候选安装包待固定 | 主目标及本机代理/CC Switch 版本待记录 | 网络/AI OFF-OFF、ON-OFF、OFF-ON、ON-ON 独立；服务器直连不可用不等于桥接失败 | 未执行 | 需四种能力组合记录 | PENDING |
| 待验收 | 候选安装包待固定 | 主目标，实际代理协议/端口待记录 | 远端监听与外网出口成功/失败独立；本机代理停/恢复可恢复 | 未执行 | 需脱敏 HTTP/SOCKS 结果 | PENDING |
| 待验收 | 候选安装包待固定 | 主目标，CC Switch/供应商/模型版本待记录 | 总开关、各工具路由 ON/OFF、切供应商/模型、重启 CC Switch 后路由状态正确 | 未执行 | 需路由/客户端记录 | PENDING |
| 待验收 | 候选安装包待固定 | 主目标、两个非 root UID | AI 启用、Token 轮换、旧/缺少 Token 拒绝、断开撤销；另一 UID 可能访问普通代理；不保证防御 root/同账户 | 未执行 | 需仅状态的边界记录 | PENDING |
| 待验收 | 候选安装包待固定 | Codex CLI/Remote SSH 插件，版本待记录 | CLI 存在/缺少及仅插件发现；配置、真实对话、编辑/工具调用、重新加载后切配置/模型 | 未执行 | 需实际客户端记录 | PENDING |
| 待验收 | 候选安装包待固定 | Claude CLI/Remote SSH 插件，版本待记录 | CLI 存在/缺少及仅插件发现；配置、明确验证、真实对话 | 未执行 | 需实际客户端记录 | PENDING |
| 待验收 | 候选安装包待固定 | 主目标及本机托管 Skills 样本 | 首次同步、无变更刷新、元数据重同步、暂时失败/恢复；拒绝异主同名/嵌套链接，排除原生/系统 Skills；仅取消管理副本 | 未执行 | 需文件系统所有权记录 | PENDING |
| 待验收 | 候选安装包待固定 | 主目标及终端版本待记录 | 应用打开终端加载会话环境；无关已有 Shell 不变 | 未执行 | 需会话环境记录 | PENDING |
| 待验收 | 候选安装包待固定 | VS Code Remote/server/插件版本待记录 | 远端打开、Machine settings、重新加载、网络/AI、准确恢复管理内容 | 未执行 | 需 VS Code 记录 | PENDING |
| 待验收 | 候选安装包待固定 | MobaXterm 版本/书签来源待记录 | 导入及兼容非 Moba 目标仅从已连接概述启动；复制环境命令可用 | 未执行 | 需 Moba 启动记录 | PENDING |
| 待验收 | 候选安装包待固定 | 主目标及断网场景 | 结束管理 SSH、30s/2min 断网、代理/CC 重启、应用异常退出/重启后恢复传输、监听、Token、配置、Skills | 未执行 | 需带时间的恢复记录 | PENDING |
| 待验收 | 候选安装包待固定 | 主目标及非管理文件样本 | 主动断开清除管理 SSH/转发/环境，拒绝旧 Token，保留其它文件，UI 清除连接状态 | 未执行 | 需清理记录 | PENDING |
| 待验收 | 候选安装包待固定 | 第二非 root Linux 目标，版本待记录 | SSH、选定网络/AI、实际客户端、启动/断开冒烟 | 未执行 | 需第二目标记录 | PENDING |
| 待验收 | 冻结 tag/Draft 待固定 | 独立下载的签名 Draft | NSIS/MSI/便携版/签名/manifest/校验值匹配；核对仓库/工作流/tag/提交/摘要；Actions 与 Release 字节一致 | 未创建 tag/Draft | 需外部验证 | PENDING |
| 待验收 | 已发布发行待固定 | v0.1.3 及可用 v0.1.4 注册 NSIS | 生产更新检测版本/四语说明、验签、被动安装、重启、保留数据及唯一安装 | 未授权公开发布 | 需发布后冒烟；Draft 不等于 latest-channel 端到端 | PENDING |
