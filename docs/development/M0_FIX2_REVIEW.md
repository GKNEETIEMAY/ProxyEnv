# RouteSpan v0.3.0dev — M0-Fix2 Review

日期：2026-11-18（Asia/Shanghai）。基线：`6a352ba`，分支 `v0.3.0dev`，开始时工作树干净。
已读取 M0 基线报告与 M0-Fix Review。本轮只调整 CI 的审计与回归调度，并评估依赖方案。

## 改动摘要

本轮待提交文件仅为：

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

本轮重新查询，不沿用历史状态作为最新事实：

- [官方发布页](https://github.com/micromatch/picomatch/releases)与 npm registry 的最高正式版本均为 `4.0.7`。
- [GHSA-fc5q-39xc-vjpf](https://github.com/advisories/GHSA-fc5q-39xc-vjpf)仍列受影响 `<=4.0.7`、无已发布补丁。
- [PR #194](https://github.com/micromatch/picomatch/pull/194)仍 Open；其作者明确不声称完整安全修复，
  `+(*a)` 仍在该 PR 范围之外。不使用未合入 PR 代替正式安全补丁。
- 本轮 `pnpm audit --audit-level high` 退出 1，仍为同一公告的两个 high 路径：
  Vite → picomatch、vue-tsc → @vue/language-core → picomatch。原版本分别为 4.0.7 / 4.0.5。

正式修复版本尚不存在，不能猜测新版本、降级到同样受影响版本，或以 audit ignore 宣称已修复。
构建/开发工具的暴露条件及隔离建议继续见 M0-Fix Review；本轮没有扩大其信任边界。

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
| GitHub Actions 实际执行 | PENDING，等待本轮变更摘要获准推送 | 尚无本轮 run 链接 |
| Windows 完整回归 / Rust / release EXE | 本轮未重跑；原所有命令保留，实际 CI 结果等待推送 | 不沿用旧 CI 为本轮 PASS |
| Linux 扩展与 UI Job | 配置原样保留，本轮实际 CI 等待推送 | 不把上轮 45 项通过写成本轮通过 |

最初 sandbox 内 Corepack 请求被本机代理访问权限拒绝；正常权限下冻结安装、构建及契约检查通过。
本轮没有运行时/测试源码变化，因此没有无条件重复上轮两次完整回归；重点验证工作流结构、
安全退出码和受影响契约。云端完整 Windows 与 Linux 运行仍必须按实际结果记账。

## 提交、推送与阶段判断

本轮本地提交范围为上述两个文件；本次请求明确要求摘要获准后推送，因此当前未推送。
提交说明：`ci: separate frontend security audit from Windows regression`。
回滚只需撤回本轮 workflow 与报告提交；不涉及配置、身份或数据迁移。

推送后的核对清单：

1. run 的 head_sha 与本次提交一致。
2. Frontend security audit 因当前漏洞失败，workflow 整体仍为 failure。
3. Windows build and test 已独立启动，实际执行全部原构建/测试步骤；任一失败需单独定位。
4. Linux 扩展、UI、RustSec 与原版本/发行契约按各自结果记录；未运行/跳过不得写 PASS。

**结论：本地已完成 CI 解耦与方案评估；真实云端解耦效果待推送验证。**
picomatch 高危漏洞仍未修复，M0 不满足安全放行条件，本轮不进入 M1。
