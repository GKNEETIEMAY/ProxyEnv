# VS Code AI 扩展桥接实现可行性审计

审计日期：2026-09-06。代码基线：`v0.2.0dev` / `338c5fc`。

本轮只审计，不修改功能代码、用户配置或远程环境，不发送模型请求。增补需求是待实现的产品目标，不作为已验证的技术事实。

## 结论

**有条件可行，但当前实现不足以宣称支持两个 VS Code 图形化扩展。** 隧道、会话和确认机制可复用；扩展运行位置识别、版本识别、共享配置增量事务及真实请求验收需要新增设计。不能只给现有 CLI 按钮增加“Extension”标签。

| 目标 | 当前代码 | 审计判断 |
| --- | --- | --- |
| Codex CLI | 独立 profile 文件和启动命令 | 保留；真实模型调用尚未验收 |
| Claude Code CLI | 用户级 `~/.claude/settings.json` 接管，原文件备份与冲突保护恢复 | 已替代独立 `--settings` 文件；普通 `claude` 可直接读取，真实模型调用仍待验收 |
| Codex VS Code 扩展 | 无专用适配 | 用户级配置路线有依据；独立 profile 选择没有足够依据 |
| Claude Code VS Code 扩展 | 无专用适配 | 网关入口有明确依据；不能仅依赖共享 settings 的子进程环境 |
| Remote - SSH | 读取 SSH 配置、发起打开窗口 | 不证明连接已完成，也不证明某个扩展正在远端运行 |

产品目标可采用：“通过受控 SSH Bridge，让远程 Linux 中的 Codex、Claude Code，以及 VS Code Remote 环境中的 Codex / Claude Code 扩展，安全复用本机代理与 CC Switch 路由。”四条真实链路验收前，仅作为开发目标。

## 1. Codex：配置可延伸，CLI profile 不可等同于扩展配置

官方说明 IDE 和 CLI 共享 `config.toml` 配置层；`chatgpt.*` 属于编辑器设置。IDE 提供 Open config.toml 入口。[官方开发设置](https://learn.chatgpt.com/docs/developer-settings?surface=ide)

Codex 0.134.0 起，CLI profile 使用独立文件；旧的顶层 `profile` 选择器已不受支持。不能向主配置添加 `profile = "proxyenv_bridge"` 来假定扩展会切换。[官方高级配置](https://learn.chatgpt.com/docs/config-file/config-advanced)

候选适配是：确认扩展使用远程账户的配置目录后，预览修改用户级 `model_provider`，增添独立命名的 `model_providers.proxyenv_bridge`。端点保持 `http://127.0.0.1:<port>/v1`，采用 `responses`。不覆盖内置 provider，不选择或覆盖 model，不触碰 sandbox、approval_policy、MCP 等字段。若命名已存在且不是本工具管理的内容，停止并报告冲突。字段依据见[配置参考](https://learn.chatgpt.com/docs/config-file/config-reference)。

必须在预览中说明：**修改用户级 model_provider 会影响同账户其他使用默认配置的 Codex 会话。** 因而“只配置 Extension”只能承诺只执行扩展适配步骤，不能承诺 CLI 默认行为完全不变。若产品要求严格隔离，目前证据不足，不能交付这一承诺。

配置层及受管理约束仍可能改变有效配置；项目级配置不是可靠的 provider 覆盖入口。需验证实际运行时读到的 provider，而不是仅验证文件内容。[配置层级](https://learn.chatgpt.com/docs/config-file/config-basic)

`~/.codex/.env` 在官方 Bedrock 指南中有桌面/IDE 使用说明，并要求重启后新建会话；这不证明它能选择自定义 profile，也不证明本桥接需要写它。当前无 Secret 的自定义 provider 路线不应新增 `.env` 写入。[官方 Bedrock 指南](https://learn.chatgpt.com/docs/amazon-bedrock)

扩展重启或窗口 Reload 后，用新会话验收；不能假设已有会话热更新。远端 CODEX_HOME、扩展自带 app-server 的配置加载方式及具体版本仍待实机确认。

## 2. Claude：共享配置成立，但网关登录检查另有入口

官方确实说明 `~/.claude/settings.json` 在 CLI 和扩展之间共享，同时扩展有 `claudeCode.environmentVariables`。[VS Code 指南](https://code.claude.com/docs/en/vs-code)

更具体的官方网关指南说明：扩展在启动前从 `claudeCode.environmentVariables` 检查凭据；共享 settings 的值能够到达子进程，却不能覆盖扩展自身的登录检查。因此优先验证 **Remote Settings 中的扩展专用环境变量**，不能宣布仅改 `~/.claude/settings.json` 即完成接入。[网关配置指南](https://code.claude.com/docs/en/llm-gateway-connect)

候选最小改动：在确认的远端设置作用域，仅增量维护数组中 `ANTHROPIC_BASE_URL` 和 `ANTHROPIC_AUTH_TOKEN` 两项；端点不加 `/v1`。保留其他数组项、未知字段和注释。公开占位值 `PROXY_MANAGED` 仅在实际 CC Switch 路由确实接受时使用，它不是通用网关凭据。已有真实凭据项不读取展示、不静默替换；遇到冲突停止。不要自动设置跳过权限、隐藏 onboarding 或强制注销。

CLI 接入已统一采用共享用户配置：Codex 管理 `~/.codex/config.toml` 中的 provider 选择与 ProxyEnv 专用 provider 表，Claude 管理 `~/.claude/settings.json` 中的路由/认证占位字段。预览只显示托管字段，远端先保存完整原文件，再合并路由字段并读回校验。桥接端口变化时只更新 ProxyEnv 托管字段；后续写入的其它受支持字段会保留，并在字段级停用时继续保留。托管字段被改动、结构异常或恢复证据不一致时仍失败关闭。Codex 的 `auth.json` 不修改；项目、受管理配置及进程环境来源仍可能覆盖用户级设置，需要验证有效 endpoint 和认证来源，不导出凭据。[Claude 配置层级](https://code.claude.com/docs/en/settings)

## 3. 本机版本证据与未确认项

仅只读检查本机默认 VS Code 扩展清单，并对 Codex 自带二进制执行 `--version`，没有读取用户 AI 配置或认证文件：

| 组件 | 实际观察 | 能证明什么 |
| --- | --- | --- |
| Windows `openai.chatgpt` | `26.825.51511` | 本机安装版本，不代表远端版本 |
| 上述扩展内置 Codex | `0.151.0-alpha.7.2` | 扩展 runtime 可为预发布版，不能按终端 CLI 版本推定 |
| Windows `Anthropic.claude-code` | `2.1.252` | 本机安装版本，不代表远端版本 |
| Claude 扩展配置 schema | `environmentVariables` 为 `machine` scope；项使用 `name` / `value` | 与 Remote Settings 候选方案一致，仍需实机验证优先级 |
| 两个 manifest 的 extensionKind | 均未显式声明 | 不能仅凭清单确定真实执行位置 |

现有 `remote.sh` 对 Codex 的版本格式要求是纯 `0.x.y`，上述 alpha runtime 会被拒绝；更重要的是它检查 PATH 中的 `codex`，不检查扩展自带 runtime。扩展可自带运行时，不应要求用户额外安装 CLI。后续应分别记录 VS Code、Remote - SSH、扩展、扩展 runtime、独立 CLI、CC Switch 的版本，并建立经过验收的组合，不能简单放宽正则后宣称全面兼容。

未检查远端安装和运行进程，未确认远端配置实际路径，未验证登录状态、provider 路由、扩展请求或恢复行为。本机证据不得迁移为远端验收结果。

## 4. 扩展运行位置是写入前置条件

VS Code 可以同时存在本地与远端 Extension Host；安装位置、扩展类型和运行环境共同影响选择。官方提供 Developer: Show Running Extensions 查看真实位置。[Extension Host](https://code.visualstudio.com/api/advanced-topics/extension-host)、[远程扩展指南](https://code.visualstudio.com/api/advanced-topics/remote-extensions)

推荐状态分开表示：`已检测安装`、`运行位置未知 / 本地 / 远端`、`配置未应用 / 已写入待重载 / 待验证 / 已验收 / 冲突`。启动 Code.exe 成功只表示“已发起打开”，不能立即显示“远程连接成功”。

第一阶段可通过官方 Running Extensions 视图由用户确认远程主机与账户，记录为“用户确认”，并在目标或窗口变化后失效。远端安装清单与只读进程元数据只能作辅助证据，不能单独证明特定扩展已激活。不读取进程完整环境或日志全文，以免带出 Token。

纯 Tauri 外部应用尚无已验证的、稳定的官方接口直接取得其他窗口每个扩展的运行位置。若要求全自动强证明，需要另行评估受支持集成；不能解析 VS Code 私有数据库、修改其他扩展或自动安装伴随插件。无法确认时禁止扩展配置写入，但可保留 CLI 能力。

Remote Settings 应通过目标窗口的官方入口确认；不要把 Windows User Settings 当作远端文件，也不要硬编码所有版本都使用同一个 `.vscode-server` 目录。[Remote - SSH 设置说明](https://code.visualstudio.com/docs/remote/ssh)

## 5. 共享配置编辑是主要工程缺口

现有代码在 `remote.sh` 只接受与生成模板完全一致的专用文件，返回端口和哈希；这限制了 Secret 外流。它不能安全编辑任意 JSONC/TOML。现有 `vscode.rs::parse_settings` 会去除注释，只适用于读取，不能直接用于保留格式的写回。

本地 Rust 已有 `toml` / `serde_json`，但把完整远端配置下载后再用它们解析会突破“不复制完整配置”的边界；Shell 的 sed 替换也不能可靠处理 TOML 的点键、多行值、内联表和 JSONC 注释、转义、重复键。

实施前应完成配置编辑器的独立技术验证：

1. 解析与备份留在远端，向 Windows 返回允许字段的脱敏语义 diff、路径身份和哈希，不返回全文或未知值。
2. 保留非目标字节、注释及未知字段；重复键、畸形文件、非预期类型、敏感目标字段冲突均停止。
3. 预览令牌绑定 SSH 身份、账户、配置路径、运行位置证据、版本和文件哈希；确认后再次验证。
4. 同目录临时文件、权限检查、锁、原子 rename、读回验证；恢复仅撤销自身改动，第三方变化不得被整文件旧备份覆盖。
5. CLI 与 Extension 可涉及多个文件；提供逐文件结果和恢复记录。跨文件操作不能称为单次原子写，部分失败需回滚或明确保留可恢复状态。

技术栈保持 Vue / TypeScript / Tauri / Rust，不新增 Python、远程守护进程或软件安装。候选是用目标已安装且验证过的 VS Code Node 运行一次性、固定的 TypeScript 编译产物及保留格式的解析代码；不修改 VS Code Server。此路线尚未验证 Node 路径稳定性、运行时兼容和解析库许可，不应先写产品功能依赖它。依赖不满足时停止，不自动安装。另一种“下载全文到本地 Rust 处理”的路线不推荐。

需求还需明确：若“不读取 API Key”指不提取、不回传、不使用 Secret，远端进程可把未知字节作为不透明内容保留；若连包含 Secret 的文件字节都禁止经过读写/备份，则通用共享配置增量编辑本身不可实现，只能拒绝这类文件并提供手工方案。不能悄悄把此歧义解释成允许下载秘密。

## 6. 路由与断连边界

SSH 隧道可复用，但本地端口监听不证明服务就是 CC Switch，也不证明对应工具路由已开启。[CC Switch 路由说明](https://github.com/farion1231/cc-switch/blob/main/docs/user-manual/en/4-proxy/4.2-routing.md)

断连后保留指向远端 loopback 的配置，让目标模型请求失败；不要自动恢复原 provider，否则下一次请求可能绕过桥接。Restore 是用户单独操作，预览应说明恢复后使用原路由。

需要区分“模型推理请求不自动绕路”和“远端所有网络流量都被强制封锁”。本设计不控制其他插件、MCP、遥测、用户修改、同账户进程或端口后来被其他服务占用；在禁止修改防火墙和隔离网络的约束下，不能保证系统级强制断网。产品声明应限于已验证的目标模型请求链路。

## 7. 实施顺序与真实验收

保留增补需求 M1–M9 的发布门槛；在 M6/M7 功能实现前增加两个技术验证：远端运行位置取证、共享配置无损事务。Claude 的配置路线较明确，可先做隔离实验；这不意味着可以跳过 Codex 或四场景验收。

| 验收 | 必要证据 | 当前状态 |
| --- | --- | --- |
| 远程代理与 CC Bridge | 真实 SSH、loopback 监听、身份、服务路由 | 未执行 |
| Codex CLI | 新会话实际响应及 CC Switch 对应请求记录 | 未执行 |
| Claude CLI | 同上，并确认有效 endpoint | 未执行 |
| Codex 图形化扩展 | 远端运行位置、新会话响应、对应路由记录 | 未执行 |
| Claude 图形化扩展 | 同上，包含扩展登录检查通过 | 未执行 |
| Provider A → B | 两个 provider 的实际路由证据；远端未复制新 Secret | 未执行 |
| 断开桥接 | 四种入口的新模型请求失败，未改回直接 provider | 未执行 |
| 恢复与故障 | 第三方编辑、备份变化、中断、重载、部分失败 | 扩展方案未执行 |

真实验收使用无敏感内容的最小 Prompt，单独记录场景、版本组合、时间、结果与脱敏请求关联信息；不采集原始认证头、Prompt 正文或完整日志。curl 成功只能证明代理协议/网络的一部分，不能代替图形化扩展验收。

进入实机验证需要明确的测试 SSH 别名、远端非 root 账户、已安装的两个扩展和 CC Switch 测试路由，并由用户触发或明确授权付费模型测试。本轮未使用这些资源。

## 审计决议

可以继续设计和小范围技术验证；暂不进入“两个扩展已支持”的实现完成/稳定发布声明。优先解决 Claude 扩展前置登录配置、Codex 默认 provider 的共享影响、远端执行位置证明、远端无损配置事务四项，再按版本组合完成真实验收。本轮唯一新增物是这份审计文档。
