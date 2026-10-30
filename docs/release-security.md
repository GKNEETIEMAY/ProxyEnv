# RouteSpan release security / 发行安全

English · [简体中文](release-security.zh-CN.md)

This defines the formal release trust model. RouteSpan v0.2.2 is the latest public
stable release, published on 2026-10-30. Its updater signatures, checksums and provenance
were independently verified. Native checks without new evidence remain unverified;
publication authorization does not imply device acceptance. The legacy brand was ProxyEnv; v0.1.4
is a public historical release. v0.2.1's updater signatures, checksums and provenance
were independently verified. The maintainer confirmed upgrade and remote-function acceptance.
Publication and evidence remain separate from local preview packaging.
See [Roadmap](ROADMAP.md) and [release QA](RELEASE_QA_V0.2.md).

## Trust mechanisms

| Mechanism | Requirement |
| --- | --- |
| Locked CI | Audit, build and test frozen npm/Cargo dependencies |
| SHA-256 | Checksums for downloaded release artifacts |
| GitHub Artifact Attestation | Provenance tied to intended repository/workflow/tag/commit |
| Tauri Updater signatures | Mandatory for every updater-enabled formal release |
| Embedded public key / fixed HTTPS source | Verification and endpoint are trusted configuration |
| Windows Authenticode | Not adopted; disclose Unknown Publisher/SmartScreen |

Checksums detect changed bytes, but are not a signature themselves. Provenance,
updater verification and clean-machine acceptance complement one another.
No paid certificate/service is required by project policy; this does not remove
the updater signing or manual QA requirements.

## Compatibility and updater

The display brand is RouteSpan. Legacy compatibility identifiers retained from
ProxyEnv include internal `proxyenv.exe`, `GKNEETIEMAY/ProxyEnv`, application identity,
release-note markers and the updater source:

```text
https://github.com/GKNEETIEMAY/ProxyEnv/releases/latest/download/latest.json
```

These do not migrate with a logo/display-name change.
[Internal contracts](INTERNAL_CONTRACTS.md) lists the protected identities.

Update checks read the fixed official `latest.json`; they do not call GitHub REST API.
Current-version notes are bundled locally. Automatic checks reuse a six-hour display
cache; manual checks bypass that TTL after a 30-second interval. Rate-limit cooldown
survives restart and cannot be bypassed manually. Cached data never authorizes installation.
Only an explicit Download and install action rechecks the live version, obtains an
official Tauri handle and fetches the signed artifact. Official Tauri Updater
verifies against the embedded key and uses the unchanged upgrade-only comparison.
Registered NSIS installations can be passively replaced and restarted; MSI and
portable variants use manual updates. No arbitrary URL, custom downloader or
execution of an unverified installer is authorized.

## Formal release workflow

### v0.2.2 upgrade evidence

The current transition is **v0.2.1 → v0.2.2 (Windows x64 NSIS)**. Downloaded
installer signatures were checked against the v0.2.1 public key, with matching
checksums, manifest and provenance. These checks do not execute the installer.
Native installation, restart and settings/SSH retention remain unverified without
new device evidence. See the current release QA entry; prior acceptance is historical.

### Historical v0.2.1 upgrade acceptance baseline

That release targeted **published v0.2.0 → v0.2.1 (Windows x64 NSIS)**.
The historical v0.1.4 upgrade failure is no longer in scope. Preserve v0.2.0's
embedded public key, update endpoint, application identity and per-user installation
mode; sign with the original matching private key. Paid Windows Authenticode is not required.

Before publication:

- Automated contracts: forward version transition, unchanged identities, paired installer / `.sig` / `latest.json`; missing or empty signatures block release.
- Real artifacts: verify the final installer signature against v0.2.0's public key and check downloaded checksums. Test fixtures are not real signature acceptance.
- Real device: upgrade a formal v0.2.0 NSIS installation, verify before installation, restart into v0.2.1, and retain settings and SSH entries.
- Failure protection: failed signatures or interrupted downloads must not install; v0.2.1 must not reinstall itself or downgrade to v0.2.0.

The v0.2.1 artifacts passed independent verification against the v0.2.0 public key.
The maintainer confirmed real-device upgrade and remote-function acceptance on 2026-10-20;
detailed device evidence remains private. See the release QA summary.

`.github/workflows/release.yml` verifies matching tag/package/Cargo/Tauri versions
and localized notes, installs locked dependencies, audits/builds/tests, packages
signed updater artifacts, collects installers/portable executable, checksums and
`latest.json`, attests public-repository artifacts and creates a Draft Release.
Formal tags must be stable versions; RC/prerelease tags are rejected rather than
changing the updater comparator. `pnpm test:release` checks versions, protected
identities, four-language notes and external `RouteSpan-…` artifact names before
packaging. NSIS/MSI signatures retain their paired names and the manifest points
to that same NSIS; the internal executable remains `proxyenv.exe` for compatibility.

```text
Locked checks → Signed artifacts → Checksums/provenance
             → Draft → External verification and manual QA → Publish
```

Missing private signing secret, installer signature or release-note structure
must fail the formal pipeline. Do not use `--no-sign` or suppress updater artifacts
to make an official release pass. The frontend high-severity audit failure was
repaired and passed full CI at `eee1873`; audit remains mandatory for every new
release change. See release QA for publication status and historical checkpoints,
not blanket acceptance claims for every future environment.

## Secret handling

Only the public verification key is committed. CI uses the authorized secrets
`TAURI_SIGNING_PRIVATE_KEY` and `TAURI_SIGNING_PRIVATE_KEY_PASSWORD` via Tauri's
official environment mechanism. A protected maintainer recovery copy belongs
outside tracked source. Never print, decode, package or commit private content,
prefixes, lengths, keys, passwords or certificate private material.

## External artifact verification

For each intended release, independently download the artifacts and retain evidence.
[Release QA](RELEASE_QA_V0.2.md) summarizes public status; detailed final v0.2
acceptance records are kept privately at the maintainer's request:

```powershell
Get-FileHash -LiteralPath <artifact> -Algorithm SHA256
gh attestation verify <artifact> -R GKNEETIEMAY/ProxyEnv
```

Confirm checksum agreement, expected repository/workflow/tag/commit, and identical
digests for the Release download and corresponding Actions artifact. Verify that
`latest.json` references the uploaded NSIS asset and its signature is accepted
by the installed trusted app. Green CI alone is not external verification evidence.

## Manual acceptance

Record clean Windows 10/11 x64 installation, first launch/uninstall, predecessor
upgrade with retained identity/data, portable launch, environment apply/restore
and conflict behavior, single-instance/tray/taskbar/Start Menu/Alt+Tab icons,
125%/150% native DPI, themes and the complete remote/reconnect/cleanup matrix.

Inspect packages for accidental private keys, logs, local configuration, debug
symbols or development-only files. Document SmartScreen/Unknown Publisher honestly.
Do not declare all these checks passed because the package was built.

## Local previews

A manual preview may be built with a temporary packaging override that disables
updater artifact creation, without changing tracked release configuration.
Label it preview, preserve source SHA and checksums, and disclose that it has no
updater signature/attestation and is not an official update release.
Do not upload its unsigned installer as `latest.json`'s target or silently promote
it to stable. The 2026-10-12 local preview is such a manual test package.
