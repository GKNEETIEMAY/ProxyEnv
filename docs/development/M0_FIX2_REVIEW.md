# RouteSpan v0.3.0dev — M0-Fix2 Review

日期：2026-11-18（Asia/Shanghai）。基线：`6a352ba`，分支 `v0.3.0dev`，开始时工作树干净。
已读取 M0 基线报告与 M0-Fix Review。本轮只调整 CI 的审计与回归调度，并评估依赖方案。

## 改动摘要

`9679c7d` 包含以下两个文件：

- `.github/workflows/ci.yml`：抽出独立前端安全审计 Job。
- `docs/development/M0_FIX2_REVIEW.md`：本报告。

新增 `frontend-security-audit`（显示名 `Frontend security audit`），沿用 Windows runner、
Node 20.19.0、pnpm 10.15.1、冻结安装及原命令 `pnpm audit --audit-level high`，超时 10 分钟。
`build-and-test` 仅移走这一审计步骤，其余步骤和先后顺序保持一致。
保留原有四个 Job、全部分支/触发器、权限、RustSec、UI、Linux 扩展以及所有 Windows 测试与 release EXE 构建。

全部 Job 均未添加 `needs`，审计和构建独立调度。未增加 `continue-on-error`、失败转换、
漏洞忽略、阈值变更、步骤条件跳过或矩阵 fail-fast。独立审计仍以真实退出码失败，
整个 CI workflow 仍失败；仅解除“审计失败使同一 Windows Job 后续步骤全部跳过”的问题。
这符合 [GitHub Job 依赖规则](https://docs.github.com/en/actions/how-tos/write-workflows/choose-what-workflows-do/use-jobs)。
调度独立不保证构建成功：依赖安装错误、构建/测试失败和 Job 超时仍各自导致失败。
新的审计 Job 和 Windows Job 各做一次冻结安装，增加少量 runner 开销。

`.github/workflows/release.yml` 的原发行前审计保持原样；本轮没有创建 Tag、Draft 或 Release，
没有修改应用版本 `0.2.2`、package/Cargo 配置、依赖锁文件、业务代码或 M1 平台架构。
未更改 GitHub 仓库分支保护规则，也未确认现有 required checks 设置；推送后若安全 Job
需要作为特定合并门槛，需检查仓库实际规则，不能把 Windows Job 成功等同于安全放行。

## picomatch 官方修复状态复核

2026-11-19 复核时，[GitHub 安全公告](https://github.com/advisories/GHSA-fc5q-39xc-vjpf)
已标记为 Withdrawn，撤回日期为 2026-11-18。npm registry 仍返回 `4.0.7`，公告未列出修复版本。
云端独立审计已通过，本地用原命令复核也退出 0。审核结果变化与公告撤回时间一致，锁定依赖和审计阈值相同。
下面保留拆分前的 High 评级、失败记录和有界性能评估。构建工具处理不可信 glob 时，
仍需考虑已观察到的匹配耗时风险。相关修补提议已关闭。见 [上游讨论](https://github.com/micromatch/picomatch/pull/235)。

2026-11-18 本地验证时记录如下：

- [官方发布页](https://github.com/micromatch/picomatch/releases)与 npm registry 的最高正式版本均为 `4.0.7`。
- [GHSA-fc5q-39xc-vjpf](https://github.com/advisories/GHSA-fc5q-39xc-vjpf)仍列受影响 `<=4.0.7`、无已发布补丁。
- [PR #194](https://github.com/micromatch/picomatch/pull/194)仍 Open；其作者明确不声称完整安全修复，
  `+(*a)` 仍在该 PR 范围之外。不使用未合入 PR 代替正式安全补丁。
- 本轮 `pnpm audit --audit-level high` 退出 1，仍为同一公告的两个 high 路径：
  Vite → picomatch、vue-tsc → @vue/language-core → picomatch。原版本分别为 4.0.7 / 4.0.5。

构建和开发工具的暴露条件及隔离建议见 [M0-Fix Review](M0_FIX_REVIEW.md)。

## 安全补丁与替代方案实测评估

本轮只在忽略的临时项目和独立子进程中实验，没有修改应用依赖。
每个匹配在独立 Node 子进程执行，限制 1500 ms / 4096 字节输出，超时由父进程结束；
固定输入为 32 个 `a` 加 `/` 或 `!`，避免无界 ReDoS 卡住当前开发进程。

| 实测方案 | 攻击样例 `+(*)` / `+(*a)` | 合法 extglob 与普通 glob | 结论 |
| --- | --- | --- | --- |
| 当前 picomatch 4.0.5 / 4.0.7 默认设置 | 两版本两样例均超过 1500 ms | `+(a\|b)` 匹配 `abba`，`**/*.{ts,vue}` 匹配 Vue 路径 | 确认当前问题，不是抽象风险 |
| 两版本加 `noextglob:true` | 两样例约 5–6 ms 返回 false | 普通 glob 正常，但合法 `+(a\|b)` 不再匹配 `abba` | 有界缓解有语义破坏，不能直接全局启用 |
| 正式 minimatch 11.0.9 | 两样例约 6–9 ms 结束 | 上述合法 extglob 与普通 glob 均匹配 | 可继续研究，尚不能直接替换 API |

minimatch 版本、Node engine 和依赖先经 registry 确认：11.0.9，Node `20 || >=22`，
依赖 brace-expansion 与 @isaacs/nonrecursive-glob-match。使用独立临时 package、精确版本、
`--ignore-scripts` 安装，候选项目高危审计退出 0（No known vulnerabilities found）。
这是候选小项目的结果，不是应用审计通过，也不证明所有 glob 攻击都已覆盖。
[官方 minimatch 说明](https://github.com/isaacs/minimatch)介绍了其非递归匹配实现；
本轮仅验证上述四个样例，不将上游安全声明等同于本项目已完成安全验证。

实际 API 差异：picomatch 默认导出可调用函数并返回 matcher，还使用 `.isMatch`、`.scan`、`.parse`；
minimatch 11.0.9 的 require 结果为 object、提供 `.minimatch` 函数，但无同名 isMatch/scan/parse。
Vite/tinyglobby/扫描逻辑和 Vue 的调用不能通过包名 override 无缝迁移。
此外，攻击样例 `+(*)` 加尾 `/` 在候选中返回 true，与原库超时及禁用 extglob 的 false 不同；
安全终止本身不证明匹配语义相同。

评估选择：

1. **正式上游补丁**：优先方案，但当前没有可采用版本，审计继续报错。
2. **本地补丁/禁用 extglob**：已有样例证明语义变化；仅让正则快速退出不够。
   需要完整嵌套/分支/转义/globstar/Windows 路径测试及上游相关回归，并覆盖两个已锁版本和所有调用点。
   当前不将未经完整验证的解析器补丁加入应用。
3. **minimatch 替换**：有正式版本且有限样例性能可行，下一步应先设计调用适配并测试 Vite、
   tinyglobby/fdir 与 Vue 类型生成，复核负向模式、路径、扫描元数据和生产构建；不能直接 override。
4. **本轮采用 CI 分离**：恢复完整构建/测试证据，同时保留安全失败。它不修复依赖本体，
   不解除 M0 安全门槛，也不是进入 M1 或发行的授权。

可复核样例参数：`+(*)` 对 `'a'.repeat(32)+'/'`，`+(*a)` 对 `'a'.repeat(32)+'!'`，
`+(a|b)` 对 `abba`，`**/*.{ts,vue}` 对 `src/view/App.vue`。
原始参数与逐项结果保存在下表本地证据中。

## 本地验证与证据

证据目录：本地忽略目录 `artifacts/v0.3.0/M0-Fix2/`。

| 检查 | 结果 | 证据 |
| --- | --- | --- |
| YAML 解析与全结构比较 | PASS；新增的只有审计 Job，Windows 仅移走原 audit，其余结构逐项相同 | workflow-validation.log |
| Job 依赖与失败配置 | PASS；五个 Job 均无 needs / continue-on-error，原审计命令和冻结环境相同 | workflow-validation.log |
| 冻结安装 | PASS，锁文件未变 | install.log |
| 前端类型检查与生产构建 | PASS，保留原 chunk 大小提示 | build.log |
| 文档与发行契约 | PASS，25 通过 / 0 失败 / 0 跳过 | contracts.log |
| 原高危审计 | BLOCKED，退出 1 / 2 high；真实失败未屏蔽 | pnpm-audit.log |
| 官方 npm 发布复核 | PASS（查询执行），最新 4.0.7 | picomatch-registry.json |
| picomatch 缓解实验 | 有界实验完成，含四个预期超时；不是测试套件 PASS | picomatch-evaluation.json |
| minimatch 候选实验与候选依赖审计 | 有界四样例结束，候选项目 audit 退出 0；非应用验收 | minimatch-evaluation.json / minimatch-audit.log |
| 2026-11-19 本地高危审计复核 | 退出 0，报告无已知漏洞 | pnpm-audit-2026-11-19.log |

最初 sandbox 内 Corepack 请求被本机代理访问权限拒绝。正常权限下冻结安装、构建及契约检查通过。
本地验证覆盖工作流结构、原审计命令和相关契约。完整平台回归结果见下方云端记录。

## 推送与实际 CI

2026-11-19 获授权后执行 `git fetch origin`。本地在 `v0.3.0dev`，工作区干净，远端基线
`6a352ba9233829d2f6b311c836bf96da72ddd1b2` 是授权提交的直接父提交。提交差异仅含上述两个文件，
提交范围和工作区的 `git diff --check` 均通过。

按授权执行 `git push origin 9679c7d:refs/heads/v0.3.0dev`。推送后 `git ls-remote` 返回
`9679c7dfac936daf10d8949fed9d6b8749086e62`，与 [CI 44459357514](https://github.com/GKNEETIEMAY/ProxyEnv/actions/runs/44459357514)
的 `head_sha` 一致。触发事件为 `push`，最终结论为 `success`，运行于北京时间 13:01 完成。

| Job | 实际结果 | 范围 |
| --- | --- | --- |
| [Frontend security audit](https://github.com/GKNEETIEMAY/ProxyEnv/actions/runs/44459357514/job/135342079006) | PASS | 日志记录了 `pnpm audit --audit-level high` 和 `No known vulnerabilities found` |
| [Windows build and test](https://github.com/GKNEETIEMAY/ProxyEnv/actions/runs/44459357514/job/135342079356) | PASS | 所有配置步骤均成功，包括前端构建、远程桥接与扩展事务回归、Rust fmt、Clippy、Rust 测试和 release EXE 构建 |
| [Remote extension Linux filesystem tests](https://github.com/GKNEETIEMAY/ProxyEnv/actions/runs/44459357514/job/135342079317) | PASS | 45 通过，0 失败，0 跳过 |
| [RustSec audit](https://github.com/GKNEETIEMAY/ProxyEnv/actions/runs/44459357514/job/135342079428) | PASS | RustSec Job 通过，保留 16 条 warning |
| [Security audit](https://github.com/GKNEETIEMAY/ProxyEnv/actions/runs/44459357514/job/135343214439) | PASS | RustSec 工作流生成的同次安全检查 |
| [Windows local UI fixtures](https://github.com/GKNEETIEMAY/ProxyEnv/actions/runs/44459357514/job/135342079427) | PASS | 模拟 UI 夹具 |

## 阶段判断

前端审计与 Windows 构建于北京时间 12:35:24 同时启动，Windows Job 的回归及构建步骤全部执行。
本次六项检查均通过，未发现新增 CI 回归。拆分前的审计仍留有失败记录，见
[M0-Fix CI 44264475814](https://github.com/GKNEETIEMAY/ProxyEnv/actions/runs/44264475814)。
本次审计结果随公告撤回而变化。已观察到的 glob 匹配性能风险和设备验收缺口继续跟踪。
本轮范围止于 M0-Fix2。
