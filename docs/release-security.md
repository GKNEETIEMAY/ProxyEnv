# Release security / 发布安全

This document defines the release trust model for ProxyEnv. Tauri Updater signing and Windows Authenticode are separate mechanisms and must not be presented as substitutes for one another.

本文定义 ProxyEnv 的发行信任模型。Tauri Updater 签名与 Windows Authenticode 是两套独立机制，不能互相替代或混称。

## Current release state / 当前状态

- ProxyEnv does **not** currently include `@tauri-apps/plugin-updater` or `tauri-plugin-updater`.
- **Check for updates** is a read-only, user-triggered request to the fixed GitHub Releases API URL. It reads `tag_name`, compares versions, and never downloads or executes an installer.
- Tag pushes matching `v*` run `.github/workflows/release.yml`. The workflow verifies that the tag and package versions match, installs from lockfiles, audits/builds/tests, creates the current Tauri Windows bundles, generates `SHA256SUMS.txt`, and uploads everything to a draft GitHub Release.
- Current Windows artifacts are not Authenticode-signed and may show **Unknown Publisher**, **Microsoft Defender SmartScreen**, or **Windows protected your PC**.

- ProxyEnv 当前没有接入 updater 插件。
- “检查更新”只会在用户点击后访问固定的 GitHub Releases API、读取 `tag_name` 并比较版本，不下载或执行安装程序。
- `v*` 标签会触发 Release 工作流；产物先进入草稿 Release，供维护者核对后发布。
- 当前 Windows 产物未签 Authenticode，可能出现“未知发布者”、SmartScreen 或“Windows 已保护你的电脑”提示。

## Priority / 优先级

### P0 — release blockers / 发布阻断项

- environment apply transaction and verified rollback;
- restore conflict detection;
- strict production CSP;
- backend IPC trust boundary;
- snapshot schema, allowlist, link/reparse-point, and size validation;
- lockfile-enforced CI and Release pipeline;
- if automatic updates are enabled: Tauri Updater signature verification, a pinned HTTPS source, default anti-downgrade behavior, and signed updater artifacts.

### P1 — future public-release hardening / 后续公开发行增强

- Windows Authenticode through Microsoft Artifact Signing or a trusted OV code-signing certificate;
- timestamped signatures for the executable and installers;
- post-build `Get-AuthenticodeSignature` verification.

An open-source Windows build can be published without Authenticode. This affects publisher identity and SmartScreen reputation; it does not replace or weaken the signature verification required for future automatic updater artifacts.

## Mechanism comparison / 机制对比

| Mechanism | Purpose | Cost | Status |
| --- | --- | --- | --- |
| Tauri Updater signing | Update artifact authenticity and integrity before installation | No CA certificate cost | Mandatory before automatic update is enabled; not implemented yet |
| SHA-256 | Manual artifact-integrity verification and release audit trail | Free | Generated for every tag release |
| Windows Authenticode | Windows publisher identity, SmartScreen reputation, Unknown Publisher mitigation | May require a paid certificate or service | P1; not a current blocker |
| GitHub Actions Release | Controlled, repeatable build/test/package/upload path | Repository CI cost only | Implemented as a tag-triggered draft release |

SHA-256 is not a digital signature. A checksum hosted beside a compromised artifact can also be replaced, while updater signing verifies the artifact against a public key embedded in the trusted application.

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

The private signing key must never appear in Git, `.env.example`, `src-tauri`, `public`, documentation examples as real content, build artifacts, or CI output. The public verification key is intentionally distributable and may be committed.

Official reference: [Tauri 2 Updater documentation](https://v2.tauri.app/plugin/updater/). CC Switch is an architectural reference only; ProxyEnv does not copy its private-key compatibility scripts.

## Release workflow / 发布流程

The current workflow intentionally passes `--no-sign` to avoid implying Authenticode. It produces the portable executable plus whatever installer types the existing Tauri bundle configuration creates; it does not force an installer migration. Each release remains a draft until a maintainer checks:

1. the tag, `package.json`, and `tauri.conf.json` versions match;
2. CI and dependency audits passed;
3. `SHA256SUMS.txt` matches every uploaded binary;
4. the installer and portable executable launch on a clean Windows 10/11 x64 machine;
5. unsigned-artifact warnings are described in the release notes;
6. no updater claim is made until signed updater artifacts and `latest.json` are actually enabled.
