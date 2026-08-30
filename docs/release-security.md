# Release security / 发布安全

This document defines the open-source release trust model for ProxyEnv. The project intentionally does not use Windows Authenticode; future Tauri Updater signing is a separate, free mechanism that becomes mandatory only when automatic updates are enabled.

本文定义 ProxyEnv 的开源发行信任模型。项目明确不采用 Windows Authenticode；未来的 Tauri Updater 签名是另一套免费机制，仅在启用自动更新时成为强制要求。

## Current release state / 当前状态

- ProxyEnv does **not** currently include `@tauri-apps/plugin-updater` or `tauri-plugin-updater`.
- **Check for updates** is a read-only, user-triggered request to the fixed GitHub Releases API URL. It reads `tag_name`, compares versions, and never downloads or executes an installer.
- Tag pushes matching `v*` run `.github/workflows/release.yml`. The workflow verifies versions, installs from lockfiles, audits/builds/tests, creates the Windows bundles, generates `SHA256SUMS.txt`, attests public-repository artifacts, and uploads everything to a draft GitHub Release.
- Windows artifacts are intentionally not Authenticode-signed and may show **Unknown Publisher**, **Microsoft Defender SmartScreen**, or **Windows protected your PC**. This does not prevent a stable release under the project policy.

- ProxyEnv 当前没有接入 updater 插件。
- “检查更新”只会在用户点击后访问固定的 GitHub Releases API、读取 `tag_name` 并比较版本，不下载或执行安装程序。
- `v*` 标签会触发 Release 工作流；工作流使用锁文件构建、生成 `SHA256SUMS.txt`，在公开仓库中生成 Artifact Attestation，并把产物放入草稿 Release 等待人工验收。
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
- if automatic updates are enabled: Tauri Updater signature verification, a pinned HTTPS source, default anti-downgrade behavior, and signed updater artifacts.

Windows Authenticode, OV/EV certificates, PFX files, signtool integration, Azure signing, and other paid code-signing services are not part of the ProxyEnv release plan. Users must download only from the official GitHub Releases page and verify the published files.

## Mechanism comparison / 机制对比

| Mechanism / 机制 | Status / 状态 |
| --- | --- |
| GitHub Actions | Required; locked build, audit, test, package, and draft release / 必须：锁定依赖并完成审计、测试、打包与草稿发布 |
| SHA-256 | Required; `SHA256SUMS.txt` is generated for every tag release / 必须：每次标签发布生成校验文件 |
| GitHub Artifact Attestation | Configured; empirical verification is required for the first formal tag / 已配置，首次正式标签必须完成实际验证 |
| Tauri Updater signing | Mandatory only when automatic updates are enabled / 仅在启用自动更新时必须 |
| Windows Authenticode | Not adopted; does not block stable releases / 不采用，不阻止稳定版发布 |

SHA-256 detects file changes but is not a digital signature by itself. GitHub Artifact Attestation binds artifact digests to the GitHub Actions build identity using signed provenance. Users can verify an official download with `gh attestation verify <file> -R GKNEETIEMAY/ProxyEnv`. Future updater signing will independently verify update artifacts against a public key embedded in the trusted application.

## First-tag attestation verification / 首次标签 Attestation 验证

The workflow configuration is complete, but configuration alone is not release evidence. After the first formal `v*` tag finishes, a maintainer must download an actual release artifact and verify it outside the build job:

工作流配置已经完成，但“已配置”不等于“已经过真实发行验证”。首次正式 `v*` 标签构建完成后，维护者必须在构建任务之外下载真实 Release 产物，并使用已安装且已登录 GitHub 的 `gh` CLI 执行：

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

This empirical verification is pending until the first formal tag exists. A successful workflow run without the external verification above does not complete this checklist item.

该项在首次正式标签产生前保持“待验证”。仅看到 Workflow 成功，不代表这项验收已经完成。

## Future Tauri Updater design / 未来自动更新设计

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

When automatic updates are implemented, use the official Tauri 2 updater and its signature format. Do not build a custom downloader, signature algorithm, minisign parser, or `Command::new(downloaded_exe)` flow.

Required enablement steps:

1. Add the official Rust and frontend updater plugins and only the minimum capabilities.
2. Generate the Tauri key pair outside the repository.
3. Commit only the public key in `tauri.conf.json` and pin an HTTPS endpoint such as `https://github.com/GKNEETIEMAY/ProxyEnv/releases/latest/download/latest.json`.
4. Set `bundle.createUpdaterArtifacts` to `true`.
5. Store the private key and optional password only as GitHub Actions secrets named `TAURI_SIGNING_PRIVATE_KEY` and `TAURI_SIGNING_PRIVATE_KEY_PASSWORD`.
6. Let Tauri create and verify `.sig` data. Never print, decode, transform, or package the private key.
7. Keep Tauri's default version comparison (`remote_version > current_version`). Do not install older or equal versions; a future manual rollback must be a separate, explicit design.
8. Keep update endpoints in trusted Tauri/Rust configuration. Frontend IPC must never accept `update_url`, `download_url`, or `signature_url`.
9. Define the complete download, verification, installation, restart, failure-reporting, and recovery flow before enabling automatic installation.

The private signing key must never appear in Git, `.env.example`, `src-tauri`, `public`, documentation examples as real content, build artifacts, or CI output. The public verification key is intentionally distributable and may be committed.

Official reference: [Tauri 2 Updater documentation](https://v2.tauri.app/plugin/updater/). CC Switch is an architectural reference only; ProxyEnv does not copy its private-key compatibility scripts.

Automatic updating is currently deferred and is not a release blocker. Until the work above is complete, the product behavior remains:

```text
Check for updates → GitHub Releases → compare versions → notify the user
```

ProxyEnv does not automatically download or execute an installer. Once automatic updates are enabled, the updater plugin, embedded public key, Tauri-signed artifacts, fixed HTTPS source, downgrade prevention, installation flow, and failure verification all become P0 requirements.

自动更新当前未启用，也不是本阶段发布阻断项。现阶段只允许“检查更新 → GitHub Releases → 比较版本 → 提示用户”，不会自动下载或执行安装程序。未来真正启用时，Updater 插件、公钥、Tauri 签名产物、固定 HTTPS 更新源、防降级、安装流程以及失败处理与验证将同时升级为 P0。

## Release workflow / 发布流程

The workflow intentionally passes `--no-sign` because Authenticode is not part of the project strategy. It produces the portable executable and the installer types configured by Tauri. The release sequence is:

```text
CI Build → SHA-256 → Artifact Attestation → Draft Release → Manual QA → Publish Stable
```

Artifact Attestation runs once the repository is public; GitHub does not provide it for ordinary private repositories without GitHub Enterprise Cloud. The first formal Windows release remains a draft until a maintainer completes this clean-machine checklist:

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
13. no updater claim is made until signed updater artifacts and `latest.json` are actually enabled.

首次正式 Windows Release 必须在干净的 Windows 10/11 x64 环境完成安装、启动、卸载、Portable、环境变量应用/恢复、冲突保护和应用助手验收；同时核验 SmartScreen/未知发布者提示、SHA-256、Artifact Attestation、Release 与 CI 产物一致性，以及发布包中不存在私钥、调试文件或本机敏感数据。

## Current conclusion / 当前结论

There is no release blocker that requires a paid certificate or paid signing service. ProxyEnv may publish a stable unsigned Windows release after the first-release verification above is completed.

目前不存在必须依赖付费证书或付费签名服务才能解决的发布阻断项。完成上述首次发行验收后，ProxyEnv 可以正式发布未签 Authenticode 的 Windows 稳定版本。

```text
Windows Authenticode
❌ 不购买
❌ 不采用
❌ 不作为 P0
❌ 不阻止 Stable Release
```

Official references: [GitHub Artifact Attestations](https://docs.github.com/en/actions/how-tos/secure-your-work/use-artifact-attestations/use-artifact-attestations) and [Tauri 2 Updater](https://v2.tauri.app/plugin/updater/).
