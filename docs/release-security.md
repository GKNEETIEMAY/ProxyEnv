# Release security / 发布安全

```yaml
Current Stable: v0.1.4
Next: v0.2.0
```

Release scope and unreleased implementation progress are maintained in [ROADMAP.md](ROADMAP.md). Publishing a stable version does not, by itself, prove completion of every external verification below.

发布范围和未发布开发进度见路线图。已有稳定版本不等于下列每项外部验收都有可追溯记录。

This document defines the open-source release trust model for ProxyEnv. The project intentionally does not use Windows Authenticode. Current Stable v0.1.4 supports user-initiated signed updates through Tauri Updater's separate, free signature mechanism; every updater-enabled release must be signed.

本文定义 ProxyEnv 的开源发行信任模型。项目明确不采用 Windows Authenticode。当前稳定版 v0.1.4 已支持用户主动触发的 Tauri 签名更新；签名机制独立且免费，所有启用 Updater 的版本都必须签名。

## Current release state / 当前状态

- ProxyEnv includes the official Tauri Updater and Process plugins with only check, download-and-install, and restart permissions.
- **Check for updates** reads the fixed GitHub Releases API for localized notes and checks the pinned HTTPS `latest.json`. Download and installation begin only after the user selects **Download and install**.
- The NSIS setup edition verifies every update against the public key embedded in `tauri.conf.json`, uses Tauri's default upgrade-only comparison, installs in passive mode, replaces the registered installation, and restarts after success. MSI and portable packages remain manual-update variants.
- Tag pushes matching `v*` run `.github/workflows/release.yml`. The workflow verifies versions and localized notes, installs from lockfiles, audits/builds/tests, requires the updater private-key secret, creates signed Windows bundles and `latest.json`, generates `SHA256SUMS.txt`, attests public-repository artifacts, and uploads everything to a draft GitHub Release.
- Windows artifacts are intentionally not Authenticode-signed and may show **Unknown Publisher**, **Microsoft Defender SmartScreen**, or **Windows protected your PC**. This does not prevent a stable release under the project policy.

- ProxyEnv 已接入官方 Tauri Updater 与 Process 插件，只开放检查、下载并安装、重启所需的最小权限。
- “检查更新”读取固定 GitHub Releases API 中的本地化日志，并检查固定 HTTPS `latest.json`；只有用户点击“下载并安装更新”后才开始下载与安装。
- NSIS 安装版使用 `tauri.conf.json` 内置公钥验证每个更新，沿用 Tauri 默认的仅升级版本比较，以被动模式替换已登记安装并在成功后重启。MSI 与 Portable 仍采用手动更新。
- `v*` 标签会触发 Release 工作流；工作流验证版本与多语言日志，使用锁文件完成审计、测试和构建，强制读取 Updater 私钥 Secret，生成签名 Windows 安装包、`latest.json`、`SHA256SUMS.txt` 与 Artifact Attestation，再放入草稿 Release 等待人工验收。
- Windows 产物按项目策略不签 Authenticode，可能出现“未知发布者”、SmartScreen 或“Windows 已保护你的电脑”提示，但这不阻止稳定版发布。

## Priority / 优先级

### Release requirements / 发行要求

- environment apply transaction and verified rollback;
- restore conflict detection;
- strict production CSP;
- backend IPC trust boundary;
- snapshot schema, allowlist, link/reparse-point, and size validation;
- lockfile-enforced CI and Release pipeline;
- SHA-256 checksums and GitHub Artifact Attestation for public release artifacts;
- Tauri Updater signature verification, a pinned HTTPS source, default anti-downgrade behavior, signed updater artifacts, and a valid `latest.json`.

Windows Authenticode, OV/EV certificates, PFX files, signtool integration, Azure signing, and other paid code-signing services are not part of the ProxyEnv release plan. Users must download only from the official GitHub Releases page and verify the published files.

## Mechanism comparison / 机制对比

| Mechanism / 机制 | Status / 状态 |
| --- | --- |
| GitHub Actions | Required; locked build, audit, test, package, and draft release / 必须：锁定依赖并完成审计、测试、打包与草稿发布 |
| SHA-256 | Required; `SHA256SUMS.txt` is generated for every tag release / 必须：每次标签发布生成校验文件 |
| GitHub Artifact Attestation | Configured; retain verification evidence for the released tag / 已配置，须保留对应发布标签的实际验证证据 |
| Tauri Updater signing | Implemented in Current v0.1.4; required for every updater-enabled release / 当前 v0.1.4 已实现，每个启用 Updater 的版本都必须使用 |
| Windows Authenticode | Not adopted; does not block stable releases / 不采用，不阻止稳定版发布 |

SHA-256 detects file changes but is not a digital signature by itself. GitHub Artifact Attestation binds artifact digests to the GitHub Actions build identity using signed provenance. Users can verify an official download with `gh attestation verify <file> -R GKNEETIEMAY/ProxyEnv`. Tauri Updater independently verifies update artifacts against the public key embedded in the trusted application.

## Release attestation verification / 发行 Attestation 验证

The workflow configuration is complete, but configuration and tag existence alone are not verification evidence. For every released tag, a maintainer must download an actual release artifact and verify it outside the build job:

工作流已配置，但“已配置、已有标签”不等于“已保留实际验证证据”。维护者应为每个发布标签在构建任务之外下载真实 Release 产物，并使用 `gh` CLI 执行：

```powershell
gh attestation verify <artifact> -R GKNEETIEMAY/ProxyEnv
```

Record the command output or link it from the release QA record, then confirm all of the following:

- verification succeeds for each public installer and portable executable;
- the attestation identifies `GKNEETIEMAY/ProxyEnv` as the source repository;
- the workflow identity, tag, and commit SHA match the intended release run;
- the downloaded Release file has the same SHA-256 digest listed in `SHA256SUMS.txt`;
- the GitHub Release download and the same-named GitHub Actions workflow artifact have identical SHA-256 digests.

需要保存命令输出或在发行验收记录中附上链接，并确认：安装包与 Portable 均可验证；Attestation 指向正确仓库；Workflow、标签与 Commit SHA 正确；Release 下载文件与 `SHA256SUMS.txt` 一致；同名 Release 文件和 CI Workflow Artifact 的 SHA-256 完全一致。

Release-specific verification evidence must be linked from the release QA record before this checklist item is marked complete. Do not infer success solely from a green workflow.

每个版本都必须在发行验收记录中附上外部验证证据后，才能将该项标为完成；仅看到 Workflow 成功不能替代实际验证。

## Tauri Updater design / 自动更新设计

```text
GitHub Actions
    ↓ locked build and tests
Tauri creates updater artifact
    ↓ TAURI_SIGNING_PRIVATE_KEY signs it
artifact + artifact.sig + latest.json
    ↓ fixed HTTPS GitHub Release endpoint
ProxyEnv updater
    ↓ embedded public key verification
valid and remote_version > current_version ? install : reject
```

ProxyEnv uses the official Tauri 2 updater and its signature format. It does not use a custom downloader, signature algorithm, minisign parser, or `Command::new(downloaded_exe)` flow.

Implemented controls:

1. Official Rust and frontend updater plugins expose only the minimum capabilities.
2. The Tauri key pair is generated outside tracked source; only its public key is committed.
3. `tauri.conf.json` pins `https://github.com/GKNEETIEMAY/ProxyEnv/releases/latest/download/latest.json` and enables updater artifacts.
4. The private key and optional password exist only as GitHub Actions secrets named `TAURI_SIGNING_PRIVATE_KEY` and `TAURI_SIGNING_PRIVATE_KEY_PASSWORD`.
5. Tauri creates and verifies `.sig` data; CI never prints, decodes, transforms, or packages the private key.
6. Tauri's default version comparison (`remote_version > current_version`) remains unchanged. Older or equal versions are not installed.
7. Update endpoints stay in trusted Tauri configuration. Frontend IPC never accepts `update_url`, `download_url`, or `signature_url`.
8. The About page exposes check, download progress, signature/manifest failure, passive installation, and restart states. A failed update leaves the current installation in place.
9. Automatic replacement is limited to the NSIS setup edition. Other bundle types are directed to the official Release instead of leaving an unmanaged old file behind.

The private signing key must never appear in Git, `.env.example`, `src-tauri`, `public`, documentation examples as real content, build artifacts, or CI output. CI reads it only from the authorized Actions secret; a maintainer recovery copy may exist only outside tracked source in protected offline storage. The public verification key is intentionally distributable and may be committed.

Official reference: [Tauri 2 Updater documentation](https://v2.tauri.app/plugin/updater/). CC Switch is an architectural reference only; ProxyEnv does not copy its private-key compatibility scripts.

The updater-enabled release flow is:

```text
User checks → fixed latest.json → compare version → user confirms download/install → download and verify signature → passive install → restart
```

The updater plugin, embedded public key, Tauri-signed artifacts, fixed HTTPS source, downgrade prevention, installation flow, and failure handling are P0 requirements. A tag build fails when its signing secret, installer signature, or localized release-note structure is missing.

Updater 插件、内置公钥、Tauri 签名产物、固定 HTTPS 更新源、防降级、安装流程与失败处理现均为 P0。缺少私钥 Secret、安装包签名或规范化多语言更新日志时，标签构建必须失败。

## Release workflow / 发布流程

The workflow does not configure an Authenticode certificate, thumbprint, PFX, or sign command because Authenticode is not part of the project strategy. It intentionally does not pass `--no-sign`, because that flag would also suppress required Tauri Updater signatures. The release sequence is:

```text
CI Build → SHA-256 → Artifact Attestation → Draft Release → Manual QA → Publish Stable
```

Artifact Attestation is configured for public-repository release runs. For v0.1.4 and later releases, retain a draft until a maintainer completes and records this clean-machine checklist:

1. the tag, `package.json`, and `tauri.conf.json` versions match;
2. CI, dependency audits, tests, and release packaging passed;
3. installation, first launch, and uninstallation succeed on clean Windows 10 and Windows 11 x64 environments;
4. the portable executable starts normally without installation;
5. proxy environment apply and exact restore both succeed;
6. restore conflict protection stops rather than overwriting an external change;
7. the application assistant's selection, diagnosis, launch, and protected configuration flows behave as documented;
8. Unknown Publisher and SmartScreen behavior matches the transparent warning in the release notes;
9. `SHA256SUMS.txt` matches every downloaded Release binary;
10. `gh attestation verify <artifact> -R GKNEETIEMAY/ProxyEnv` succeeds and reports the expected repository, workflow, and commit;
11. each GitHub Release download is byte-identical to the corresponding CI workflow artifact, as proven by SHA-256;
12. installers, portable archives, and uploaded artifacts contain no private keys, debug symbols, development-only files, logs, or local sensitive data;
13. `latest.json` references the same signed NSIS asset uploaded to the Release, and the embedded public key verifies its `.sig` content.

v0.1.4 及后续 Windows Release 应在干净的 Windows 10/11 x64 环境完成并记录上述验收后再发布；缺少的验收证据必须明确标记，不能通过文档修改补记为已通过。

## Current conclusion / 当前结论

There is no release blocker that requires a paid certificate or paid signing service. Current Stable is v0.1.4; Next is v0.2.0. Stable Windows releases remain intentionally unsigned by Authenticode, while signed updater artifacts and per-release verification remain required.

目前不存在必须依赖付费证书或付费签名服务才能解决的发布阻断项。当前稳定版为 v0.1.4，下一版为 v0.2.0；正式版不签 Authenticode，但仍要求 Updater 签名和逐版验收。

```text
Windows Authenticode
❌ 不购买
❌ 不采用
❌ 不作为 P0
❌ 不阻止 Stable Release
```

Official references: [GitHub Artifact Attestations](https://docs.github.com/en/actions/how-tos/secure-your-work/use-artifact-attestations/use-artifact-attestations) and [Tauri 2 Updater](https://v2.tauri.app/plugin/updater/).
