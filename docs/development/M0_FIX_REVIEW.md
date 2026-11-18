# RouteSpan v0.3.0dev — M0-Fix Review

日期：2026-11-18（Asia/Shanghai）。基线：`7b5cee4e597ed82a9385125ab1487beb251141ff`，
本轮开始时本地与 `origin/v0.3.0dev` 一致、工作树干净。
本次仅做依赖安全、fixture 稳定性与 CI 分支监听整改，不进入 M1。
生产版本仍为 `0.2.2`；远程脚本、SSH 协议、relay、用户配置、更新来源及身份不变。

## 依赖核查与修复

### picomatch：没有可采用的正式补丁，继续阻断

核对来源：

- [GitHub 安全公告 GHSA-fc5q-39xc-vjpf](https://github.com/advisories/GHSA-fc5q-39xc-vjpf)：
  受影响 `<=4.0.7`，Patched versions 为 None。
- npm registry `pnpm view picomatch dist-tags versions --json`：最高已发布版本 `4.0.7`。
- 公告引用的 [PR #194](https://github.com/micromatch/picomatch/pull/194) 尚未合入，
  作者明确表示不是安全修复，单分支 `+(*a)` 问题仍未解决；不将其当作完整补丁。
- `pnpm audit` 仍返回 2 high，来自同一公告及两条路径；`<0.0.0` 不是有效修复版本。

实际依赖调用路径：

| 路径 | 源码调用点 | 输入来源 / 暴露条件 |
| --- | --- | --- |
| Vite 7.3.7 → picomatch 4.0.7 | `dist/node/chunks/config.js` 的 createFilter、glob/文件匹配逻辑 | 构建/开发时的 include/exclude、glob、导入模式；攻击需要恶意模式和被匹配内容进入匹配器 |
| Vite → tinyglobby 0.2.17 / fdir 6.5.0 → picomatch 4.0.7 | glob 遍历和匹配 | 受项目、插件及开发服务上下文影响；不据静态审阅宣称所有 HTTP 入口均不可达 |
| vue-tsc 3.3.10 → @vue/language-core 3.3.10 → picomatch 4.0.5 | `lib/codegen/template/elementProps.js` 的 isMatch | 模板属性名及 vueCompilerOptions 的 dataAttributes/htmlAttributes 模式；默认分别为 `[]`、`['aria-*']` |

本项目 tsconfig 没有自定义上述 Vue 模式；vite.config 的 watch ignored 为固定 `**/src-tauri/**`，
开发服务默认本机监听，但可由 `TAURI_DEV_HOST` 覆盖。仓库当前未直接导入 picomatch 到应用源码。
这些是缩小暴露范围的事实，不是安全豁免，也不证明所有第三方插件和不可信源码安全。

风险：不可信 glob 与输入可造成构建/开发 Node 进程 CPU 阻塞，影响可用性。
缓解建议：只构建可信配置/插件；开发服务器保持本机监听；不把外部文本作为 glob；
对不可信 PR 使用受限 CI runner 和现有 Job 超时；定期复核正式补丁。以上不能修复依赖本体，
也不能让审计通过。没有加 override、漏洞忽略或降低审计等级；未修改 pnpm 锁文件。
若需要维护本地补丁/替换匹配器，应独立审阅语义兼容与完整 ReDoS 覆盖，不能声称上游已修复。

### QUIC：精确锁文件补丁

| 包 | 变更 | 处理的官方公告 |
| --- | --- | --- |
| quinn-proto | 0.11.18 → 0.11.19 | [RUSTSEC-2026-0523](https://rustsec.org/advisories/RUSTSEC-2026-0523)、[RUSTSEC-2026-0524](https://rustsec.org/advisories/RUSTSEC-2026-0524) |
| quinn-udp | 0.5.15 → 0.5.16 | [RUSTSEC-2026-0538](https://rustsec.org/advisories/RUSTSEC-2026-0538) |

`quinn 0.11.11` 的 proto 约束为 `^0.11.15`、udp 为 `^0.5`，均接受所选正式补丁。
Cargo 精确解析、下载 crate 和 `--locked` Clippy 均成功。
差异只保留两个 package block：版本、registry checksum，以及 quinn-udp 新版本要求的
`windows-sys 0.52.0` 依赖。解析器带出的其他 Windows 依赖重选已排除，并重新通过真实
`--locked` 检查；没有新增包、改变 Cargo.toml 或升级生产版本。额外逐 package block 比较确认：
569 个包中只有这两个包变化，其余条目完全相同（`quinn-minimal-validation.log`）。

补丁后 `cargo audit` 退出 0、漏洞 0；保留 12 unmaintained、3 unsound、1 yanked warning，
不把它们写成“无任何安全告警”。本轮 JSON 输出没有此前 registry 撤包查询超时。
原 Windows 默认 normal 依赖树未显示 QUIC 两包；仍修复全锁文件而不添加审计豁免。

## Claude fixture：调查、整改与证明范围

M0 原失败发生在 Claude 第二次 apply（迁移到新端口）的 `spawnSync bash`，
达到既有 60 秒超时。原同轮还并行编译 cargo-audit，可能带来资源竞争，但没有当时的
逐进程 CPU/I/O 记录，不能把这一推测当作已证实根因。

本次未改动前的逐操作测量：完整事务约 33.1 秒（测试体约 31.6 秒），
第二次 apply 约 6.2 秒，其余步骤正常退出 0，没有复现 60 秒阻塞。
另对原 `sync -f` 做临时测量：8 次约 33–43 ms，无长阻塞证据；没有因此 mock 掉 sync。
脚本没有后台事务进程，主要是同步执行 stat/mv/Python/hash 等工具；stdout 为小型 JSON。
未发现超过输出缓冲上限的证据，实际 sync、hash、内容读回与恢复逻辑保留。

确认并修复的测试隔离问题：

1. 原 fixture 无条件继承开发者环境，BASH_ENV/ENV 可在 fixture 脚本设置 HOME/PATH 前执行，
   TEST_*、Python 和 MSYS 环境也可能污染用例。新增回归在旧 helper 上以 JSON 污染失败；
   新 helper 拒绝这些隐式输入，仍允许用例显式传入受控 TEST_* 覆盖。
2. 每个 fixture 使用独立 TMPDIR，避免多个用例共用全局暂存目录；清理只作用于该 fixture 根目录。
3. Windows stat 和 mv mock 最后的真实工具调用使用 exec，移除额外等待的 shell 层，
   保留真实所有者/文件大小/hash/写入/替换失败注入及恢复校验。Linux 权限由原生 Linux 测试验证。
4. 非零退出增加 tool/operation、耗时、退出信号/错误码诊断，便于定位后续阻塞步骤。
   60 秒超时及所有既有安全断言保持不变，无自动重试掩盖失败。

修复后的单项：隔离回归和 Claude 事务共同通过；事务连续两次约 28.1 / 26.8 秒。
这些证明具体隔离缺陷已修复及本轮单项稳定，不足以证明原 60 秒超时唯一根因已定位，
也不代表强杀场景的全部后代进程清理已验证。
同一代码的完整 `pnpm check` 随后连续两轮退出 0，远端组分别约 12 分 29 秒、12 分 7 秒，
两轮均包含原 Claude 事务及全部既有安全断言；不是只重复单项或替换成跳过 POSIX 的测试。

## CI 配置与推送边界

`.github/workflows/ci.yml` 仅在 push branches 增加 `v0.3.0dev`。
PyYAML 6.0.3 BaseLoader 解析新旧结构并比较：除该分支外结构相同，
原分支、tag/PR/manual triggers、四个 Job、pnpm audit 和 RustSec 步骤均保留。
本地结构校验不是 GitHub 执行结果，也不替代远端 Actions 验证。

本机虽有 `wsl.exe` 入口，`--list --quiet` 正确解码后退出 1，提示“系统找不到指定的文件”，
未取得可用 Linux 发行版；没有安装/初始化 WSL。原生 Linux 扩展验证由下述 Linux CI 完成，
不把 Git Bash 的权限 mock 或 Windows 上的通过项冒充该项验收。

维护者已在审阅四文件变更摘要后明确授权推送。整改源码与首次报告已推送至
`origin/v0.3.0dev`，冻结 SHA 为 `725aa02f179d14b46866f8ce75de63b87a549694`。
[实际 CI 44264475814](https://github.com/GKNEETIEMAY/ProxyEnv/actions/runs/44264475814)
已结束，head_sha 与该提交一致，整体为 failure。各 Job 结果：

| Job | 结果与证据范围 |
| --- | --- |
| Remote extension Linux filesystem tests | PASS，45 通过 / 0 失败 / 0 跳过，原生 Linux 文件系统事务 |
| RustSec audit / Security audit | PASS，云端保留的审计检查通过 |
| Windows local UI fixtures | PASS，仅模拟 UI，不等于原生 DPI/设备验收 |
| Windows build and test | BLOCKED，Audit frontend dependencies 因 picomatch 2 high 退出 1；其后构建、远端回归、Rust fmt/Clippy/测试和 release EXE 均跳过 |

失败日志明确给出同一 `GHSA-fc5q-39xc-vjpf` 及原两条依赖路径，没有降低审计门槛或改为 warning。
后续仅同步报告，不改变上述已验证的源码；报告提交自身的后续运行不能自动继承该 CI 结论。

只读查询开发分支旧 run 的两次 CLI 请求均遇到 API EOF，未取得可核对的运行结果；
此网络失败不算 CI 失败或成功。证据为 `ci-before-push.json`（实际为错误文本，不按 JSON 成功结果使用）。
推送后经只读重试成功取得实际 run 与 Job 日志；watch 曾因 EOF 结束，未将其当作 Job 状态。
本次最终结论来自重新查询的 completed 状态及完成后的 Job 日志。

## 本轮验证清单

本地证据目录为忽略目录 `artifacts/v0.3.0/M0-Fix/`，不上传原始私有路径或环境。

| 检查 | 实际结果 | 证据 |
| --- | --- | --- |
| 冻结安装 | PASS | install.log |
| pnpm check 第 1 轮 | PASS，退出 0；Node 313 通过 / 6 跳过，Rust 309 通过 / 3 忽略 | check-1.log |
| pnpm check 第 2 轮 | PASS，退出 0；Node 313 通过 / 6 跳过，Rust 309 通过 / 3 忽略 | check-2.log |
| pnpm audit --audit-level high | BLOCKED，2 high，同一 picomatch 公告两条路径 | pnpm-audit-after.log |
| Rust fmt | PASS | fmt.log |
| Rust all-target Clippy --locked -D warnings | PASS | clippy.log |
| Rust lib tests | PASS，309 通过 / 0 失败 / 3 忽略；另有 main/doc-tests 零项 | check-1.log |
| cargo audit | PASS（原审计策略），0 vulnerabilities / 16 warnings | cargo-audit-after.json |
| Claude 单项重复 | PASS，2 次，原 60 秒限制 | fixture-after-1.log / fixture-after-2.log |
| 隔离回归旧 helper | 预期 FAIL，确认测试可发现污染 | isolation-before.log |
| 隔离回归新 helper | PASS | fixture-after-1.log |
| workflow YAML 与结构比较 | PASS | workflow-validation.log |
| Linux 原生扩展事务 | PASS，45 通过 / 0 失败 / 0 跳过 | CI 44264475814，linux-ci-44264475814.log |
| Windows 上扩展检查 | PASS（执行范围），两轮均 41 通过 / 0 失败 / 4 跳过 | check-1.log / check-2.log |
| 原生 UI / DPI / 真 SSH / 模型请求 | PENDING，本轮未执行 | 不以静态或 fixture 代替 |
| CI Windows release EXE 与审计后步骤 | BLOCKED，前端 audit 失败后跳过 | ci-44264475814-failed.log |
| CI workflow / Linux runner / UI fixtures / RustSec | 独立 Job 通过；整体 CI 仍失败 | CI 44264475814 |

## 变更、回滚与 M0 结论

- `src-tauri/Cargo.lock`：两个兼容 QUIC 补丁及所需 Windows 依赖边。
- `scripts/test-remote-bridge.mjs`：fixture 环境/暂存隔离、mock exec、诊断及隔离回归。
- `.github/workflows/ci.yml`：增加开发分支监听。
- 本报告：公告、兼容性、测试证据、剩余阻断与授权状态。

已推送的分类提交摘要：

1. `9a334cf` — `fix(deps): patch QUIC security advisories in lockfile`：仅 Cargo.lock。
2. `338c04f` — `test(remote): isolate shell fixtures and enable dev branch CI`：fixture 与 ci.yml。
3. `725aa02` — `docs(v0.3): record M0 fix evidence and remaining blockers`：首次报告。

上一轮本地提交命令未能启动：自动权限审查超时，按工具允许重试一次后再次超时。
工具明确说明超时不表示动作不安全；当时没有形成新提交，HEAD 仍为基线 `7b5cee4`。
维护者随后明确授权推送，本轮按原四文件范围继续分类提交与推送；未绕过权限限制。
提交前回滚可逐文件撤回本轮差异（先确认没有后续用户改动）；提交后可 revert 对应开发提交。
无需用户数据迁移，撤回 QUIC 补丁会重新引入全锁文件告警。

**M0 仍不满足进入 M1 的条件。** picomatch 无正式补丁、pnpm 高危门槛仍失败；
开发分支 CI 已实际验证并被保留的高危审计门槛阻断。完整本地重复检查及原生 Linux 扩展 CI 已通过；
原超时根因仍有证据限制。
真实 Linux/Windows SSH 验收继续保留 PENDING，
不能将 Linux Node 文件系统 CI 当作实机 SSH 或 WindowsTarget 功能验收。
