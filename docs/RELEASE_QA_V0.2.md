# RouteSpan v0.2 release QA / 发行验收

English · [简体中文](RELEASE_QA_V0.2.zh-CN.md)

This is the sole current evidence ledger. Status values: **PASS**, **PARTIAL**,
**PENDING**, **BLOCKED**. Every record scopes date, build, environment, expectation,
actual observation and evidence. A screenshot is not network evidence; source
existence is not acceptance. No formal v0.2 release decision has been made.

## Baseline and release state

Baseline HEAD: `6ab9ef65579e8c47e412e3e4863483035f8bec38`, branch `v0.2.0dev`.
The documentation consolidation and illustration change set is based on that
baseline; no runtime source or dependency versions were changed.
Maintainer-designated predecessor: ProxyEnv v0.1.4. GitHub API on 2026-10-12 reports
`isDraft=true`, `publishedAt=null`; public latest is v0.1.3. Next RouteSpan v0.2.0
is unreleased. These are not interchangeable release states.

## Recorded evidence

| Date (Asia/Shanghai) | Build / commit | Environment | Expected | Actual | Evidence | Status |
| --- | --- | --- | --- | --- | --- | --- |
| 2026-10-12 | 6ab9ef6 | Local Windows, Node | Brand assets/display and protected identities remain correct | 6 tests passed during preview build | `node --test scripts/test-branding.mjs` output; source PNG hash is pinned | PASS |
| 2026-10-12 | 6ab9ef6 | Local Windows, Node | Generated remote helper matches source | Check completed with exit 0 | `node scripts/build-remote-extension.mjs --check` during preview build | PASS |
| 2026-10-12 17:39 | 6ab9ef6, clean source | Local Windows production build | Build a manual x64 NSIS preview from current code | Frontend and locked optimized Rust build succeeded; NSIS produced | `tauri build --ci --bundles nsis --config '{"bundle":{"createUpdaterArtifacts":false}}' -- --locked`; installer SHA-256 `9ff562d619db6e837af2b1b1e495d44a2579917629db9bac4a5a4c318c65fcad` | PASS |
| 2026-10-12 | 6ab9ef6 | Synthetic browser IPC, four locales, 560/980 CSS px | Header/navigation and final mark fit light/dark layouts | Prior browser geometry/branding checks passed; supporting evidence only | `scripts/test-branding-ui.mjs` review output; review captures remain outside the README presentation | PASS |
| 2026-10-12 17:04 | 6ab9ef6 | GitHub Linux extension job | Run actual Linux filesystem transaction tests | Job succeeded | [CI run 38227235963](https://github.com/GKNEETIEMAY/ProxyEnv/actions/runs/38227235963), Linux extension job | PASS |
| 2026-10-12 17:04 | 6ab9ef6 | GitHub RustSec job | Audit locked Rust dependencies | Job succeeded | Same CI run, RustSec job | PASS |
| 2026-10-12 17:04 | 6ab9ef6 | GitHub Security audit check | Pass configured security audit | Check succeeded | [Security audit check 114738512010](https://github.com/GKNEETIEMAY/ProxyEnv/runs/114738512010), verified by commit check-runs API | PASS |
| 2026-10-12 17:04 | 6ab9ef6 | GitHub Windows job | Complete audit, build and tests | Frontend `pnpm audit --audit-level high` failed; downstream build/tests/Rust steps skipped | Same CI run, Windows Audit frontend dependencies step | BLOCKED |
| Earlier user reports; exact build/date incomplete | Not fully pinned | User's tested Linux target and Windows clients | Use network, CLI/plugins, Skills and VS Code/Moba launch | User reported successful cases; not a reproducible full release matrix | Conversation acceptance reports; incomplete build/environment metadata | PARTIAL |
| 2026-10-12 | 6ab9ef6 preview | Second Windows machine | Install/use the new preview package | Package delivered; no new-machine result recorded yet | Manual preview directory and README/checksum; awaiting user test | PENDING |
| 2026-10-12 review | v0.1.4 → intended v0.2 package | Clean Windows 10/11 | Upgrade preserving identity/data, launch/uninstall | No scoped latest-package evidence | Maintainer must record source/destination builds and results | PENDING |
| 2026-10-12 review | Intended v0.2 package | Real Windows 125%/150% DPI | Taskbar/tray/Start Menu/Alt+Tab/native icons remain correct | Mock geometry and PNG checks do not cover native caches/DPI | Clean-device screenshots and lifecycle record required | PENDING |
| 2026-10-12 review | Intended v0.2 package | Live SSH/shared Linux/CLI/plugins | Full key/Agent/password/KBI, token, restore/reconnect matrix | Earlier successful cases insufficient for complete matrix | Repeatable target/build/provider-scoped record required | PENDING |
| 2026-10-12 review | Intended tagged release | External download and registered NSIS | Signed update, checksum/provenance, failure/downgrade handling | No current formal signed-release acceptance | Independent artifact/Updater verification required | PENDING |

The local preview preserves version 0.2.0 internally and compatibility identities.
It is unsigned by Authenticode, has no updater signature, no attestation and no
published update manifest. Build success is not clean-machine installation evidence.

## Documentation regression

All checks below ran for this documentation change set on 2026-10-12.
Platform skips are explicit and are not verified cases. Build output retains the
existing non-fatal large frontend chunk advisory and Windows linker messages.

| Date | Build / commit | Environment | Expected | Actual | Evidence | Status |
| --- | --- | --- | --- | --- | --- | --- |
| 2026-10-12 | 6ab9ef6 + docs delta | Local Windows / Node | Docs links, branding, archive boundaries, EN/ZH parity and README illustrations | 10 passed, 0 failed | `node --test scripts/test-docs.mjs` output; offline parser/contract tests | PASS |
| 2026-10-12 | 6ab9ef6 + docs delta | Local Edge, Markdown preview, EN/ZH at 1040/480 CSS px | All four conceptual illustrations load and layout fits; additions use the original white line-art style | All four language/width combinations passed; not a live GitHub-rendering result | `.impeccable/review/readme/` screenshots and inspection script | PASS |
| 2026-10-12 | 6ab9ef6 + docs delta | Local Windows / Node | Preserve brand and protected identities | 6 passed, 0 failed | `pnpm test:brand` output | PASS |
| 2026-10-12 | 6ab9ef6 + docs delta | Local Windows / Node | Keep safe report localization and non-probing behavior | 11 passed, 0 failed | `pnpm test:report` output | PASS |
| 2026-10-12 | 6ab9ef6 + docs delta | Local Windows; POSIX fixtures excluded | Remote/UI/installation/navigation/notification regressions | 86 passed, 37 skipped, 0 failed | `pnpm test:remote`; `PROXYENV_TEST_SHELL` points at an absent fixture shell | PASS |
| 2026-10-12 | 6ab9ef6 + docs delta | Local Windows; Linux-only cases excluded | Generated helper and config/network transactions | 41 passed, 4 skipped, 0 failed | `pnpm test:extensions` output | PASS |
| 2026-10-12 | 6ab9ef6 + docs delta | Local Windows / TypeScript / Vite | Production frontend build | Type check and build succeeded | `pnpm build` exit 0; large-chunk advisory remains | PASS |
| 2026-10-12 | 6ab9ef6 + docs delta | Local Windows / locked Rust MSVC | Library regressions | 282 passed, 3 ignored, 0 failed | `cargo test --manifest-path src-tauri/Cargo.toml --lib --locked` output | PASS |
| 2026-10-12 | d3c670e + Chinese documentation delta | Local Windows / Node | All current document translations, links, command/section parity, QA status parity and brand identities | 17 passed, 0 failed | `node --test scripts/test-docs.mjs scripts/test-branding.mjs`; no runtime change | PASS |

Submission recheck used the equivalent direct Node entry points for the package
scripts above: docs/brand/report/remote combined 113 passed, 37 skipped, 0 failed;
extensions 41 passed, 4 skipped; TypeScript/Vite build and locked Rust library tests
passed again. Sandbox-denied realpath, temporary-file and loopback tests were rerun
outside the sandbox; those environment restrictions were not product regressions.

## Documentation design review

The implementation follows the documentation map, current-versus-archive separation,
EN/ZH parity, compatibility identities and evidence vocabulary in the consolidation
design. Consolidation established eight current Markdown documents including its
index; each now has a Chinese mirror without adding topics or evidence sources. Fourteen
superseded subject files are retained under the archive with historical warnings;
two existing historical evidence documents also have warnings and repaired links.
No historical release notes were rewritten.

Later maintainer requests supersede two original presentation assumptions: README
uses four white-background explanatory illustrations instead of UI screenshots/
ASCII flow, with platform and MIT badges restored; v0.1.4 remains the designated
predecessor but is explicitly a GitHub Draft, not the latest public release.
The source brand PNG is unchanged. Link, stale-brand and compatibility-context
checks pass. Local EN/ZH rendering at desktop/narrow widths and independent visual
review passed; live GitHub README rendering was subsequently confirmed by the
maintainer as recorded below (not by automated browser inspection).

## Remaining acceptance checklist

| Date | Build / commit | Environment | Expected | Actual | Evidence | Status |
| --- | --- | --- | --- | --- | --- | --- |
| 2026-10-12 review | v0.2.0dev at eee1873; root README/artwork unchanged from 3ca4e58 | Maintainer's GitHub browser | EN/ZH README and four images load with no layout overflow | Maintainer confirmed both READMEs and all four illustrations render normally | Direct user acceptance reply; automated browser inspection was blocked by URL safety checks | PASS |
| 2026-10-12 review | 3ca4e58 | GitHub Windows/Linux | Latest code CI passes with audits intact | Linux extension, RustSec and Security audit passed; Windows stopped at frontend audit (two high findings), later steps skipped | [CI run 38258401095](https://github.com/GKNEETIEMAY/ProxyEnv/actions/runs/38258401095), jobs API verified | BLOCKED |

## Release polish tracking

The publication baseline is now `3ca4e58`, including the Chinese mirrors.
Asset notes were shortened and original generation inputs retained in the historical
index; artwork and runtime behavior are unchanged. The maintainer confirmed live
GitHub README rendering after automated inspection was blocked. Dependency/CI
remediation is authorized; no freeze branch, tag or release is authorized.

| Date | Build / commit | Environment | Expected | Actual | Evidence | Status |
| --- | --- | --- | --- | --- | --- | --- |
| 2026-10-12 | 3ca4e58 + asset-note delta | Local Windows / Node | Links, archival warnings, Chinese parity and protected brand identity | 17 passed, 0 failed | `node --test scripts/test-docs.mjs scripts/test-branding.mjs` | PASS |
| 2026-10-12 | 40d0d2a + dependency delta | Local Windows / pnpm 10.15.1 | Frozen installation and high-severity audit with unchanged audit policy | Exit 0; no known vulnerabilities; Vue/renderer 3.5.43, source-map-js 1.2.2, no override | `pnpm install --frozen-lockfile` and `pnpm audit --audit-level high` | PASS |
| 2026-10-12 | 40d0d2a + dependency delta | Local Windows / Node / TypeScript / Vite | Build and all package-script frontend/remote regressions | Build exit 0; 154 passed, 41 POSIX/Linux skips, 0 failed; existing chunk warning remains | `pnpm build`, brand/docs/report/remote/extensions scripts; absent POSIX fixture shell as above | PASS |
| 2026-10-12 | 40d0d2a + dependency delta | Local Windows / locked Rust MSVC | Formatting, library tests and all-target Clippy with warnings denied | 282 passed, 3 ignored, 0 failed; formatting and Clippy exit 0 | `cargo fmt --all -- --check`, `cargo test --lib --locked`, `cargo clippy --all-targets --locked -- -D warnings`, each with `--manifest-path src-tauri/Cargo.toml` | PASS |
| 2026-10-12 | eee1873a22a9bc82ac0b0e9e9fb052e3c301d61a | GitHub Windows/Linux | All four CI jobs/checks green | Windows build/test/release EXE, Linux extension, RustSec and Security audit all succeeded | [CI 38261620424](https://github.com/GKNEETIEMAY/ProxyEnv/actions/runs/38261620424), exact SHA and all four conclusions checked through jobs API | PASS |
| 2026-10-12 | eee1873 + release-preflight delta | Local Windows / Node / PowerShell 7 | Version/identity, stable-tag policy, notes and artifact collection contracts | 7 passed; inert installer/signature fixtures preserved bytes, matched renamed NSIS manifest and checksums, rejected ambiguous installers | `pnpm test:release`; no real signatures, installer execution or publication | PASS |
| 2026-10-12 | eee1873 + release-preflight delta | Offline four-language release-note validation | Retain compatible delimiters and substantive localized change sections | Chinese/English/Japanese/Korean blocks validated with matching section/item structure | `scripts/validate-release-notes.mjs` and release-contract tests | PASS |
| 2026-10-12 | 928303e65fb1ef9bdfa27df9a2e9ea8dfcffe9b4 | GitHub Windows/Linux | Latest preflight changes pass full CI | Windows including release contracts/release EXE, Linux extension, RustSec and Security audit all succeeded | [CI 38264570889](https://github.com/GKNEETIEMAY/ProxyEnv/actions/runs/38264570889), exact SHA and four conclusions verified | PASS |

Formal releases use stable version tags only, not RC/prerelease tags; the updater
comparator remains unchanged. Download names use `RouteSpan-…` and the internal
binary remains `proxyenv.exe`. Collector fixture tests are not verification of
signed Draft artifacts, native installation, attestation or production updates.

Release polish must keep audits green and close all real-device/signing gates.
Do not change pending evidence to PASS solely to satisfy a documentation checklist.

## Packaged and live acceptance matrix

This is a prepared checklist, not a test result. Start only after the preflight
gate is green. For every row, replace the candidate with the exact source SHA,
installer hash, OS/tool/provider versions, target alias and dated evidence; record
actual observations and any P0/P1 issue. Primary target runs the full remote matrix;
secondary target runs a smoke test. Never paste passwords, provider keys or session
headers into evidence. Earlier user success reports do not close these rows.

| Date | Build / commit | Environment / versions | Expected | Actual | Evidence | Status |
| --- | --- | --- | --- | --- | --- | --- |
| Pending | Candidate package TBD | Windows 10 22H2 x64, exact OS/WebView2/package versions TBD | Clean NSIS install, launch, About, close-to-tray/restore, single instance, settings, startup/silent start, uninstall | Not run for final candidate | Device record required | PENDING |
| Pending | Candidate package TBD | Supported Windows 11 x64, exact versions TBD | Same native lifecycle matrix as Windows 10 | Not run for final candidate | Device record required | PENDING |
| Pending | v0.1.3 → candidate | Windows x64, source/destination installer hashes TBD | Retain identity/settings/snapshots; one install/startup/uninstall entry; updater identity unchanged | Not run | Upgrade record required | PENDING |
| Pending | v0.1.4 Draft → candidate | Windows x64, predecessor artifact/hash TBD | Same upgrade matrix; availability of Draft predecessor recorded explicitly | Not run | Upgrade record required | PENDING |
| Pending | Candidate package TBD | Both Windows editions, 100%/125%/150%, light/dark | Taskbar, tray 16/20px, Explorer, shortcut, Start Menu, Alt+Tab and installer icons readable, no unintended edges/cache artifacts | Not run | Native screenshots/lifecycle record required | PENDING |
| Pending | Candidate package TBD | Primary Linux account; OpenSSH/Agent versions TBD | New/known host, key, Agent, password and KBI connect correctly; cancellation, wrong password and timeout recover | Not run | Scoped SSH auth record required | PENDING |
| Pending | Candidate package TBD | Primary target with changed host-key fixture | Fail closed; no automatic trust reset, yes or disabled checking | Not run | Safe fixture record required | PENDING |
| Pending | Candidate package TBD | Primary target and local proxy/CC Switch versions TBD | General/AI OFF-OFF, ON-OFF, OFF-ON, ON-ON remain independent; unavailable server direct internet is not bridge failure | Not run | Four capability cases required | PENDING |
| Pending | Candidate package TBD | Primary target, actual proxy protocol/ports TBD | Remote listener and external egress success/failure are independent; local proxy stop/resume recovers | Not run | Redacted HTTP/SOCKS results required | PENDING |
| Pending | Candidate package TBD | Primary target, CC Switch/provider/model versions TBD | Master and per-tool route ON/OFF, provider/model switch and CC Switch restart preserve correct route state | Not run | Route/client record required | PENDING |
| Pending | Candidate package TBD | Primary target, two non-root UIDs | AI enable, token rotation, stale/missing token rejection and disconnect revocation; other UID may reach general proxy; no root/same-account guarantee | Not run | Status-only boundary record required | PENDING |
| Pending | Candidate package TBD | Codex CLI and Remote SSH plugin, versions TBD | CLI present/missing and plugin-only discovery; configuration, real conversation, file edit/tool call, profile/model change after reload | Not run | Actual-client record required | PENDING |
| Pending | Candidate package TBD | Claude CLI and Remote SSH plugin, versions TBD | CLI present/missing and plugin-only discovery; configuration, explicit verification, real conversation | Not run | Actual-client record required | PENDING |
| Pending | Candidate package TBD | Primary target, local managed Skills fixtures | First projection, no-op refresh, metadata resync, transient failure/recovery; refuse foreign same-name/nested links, exclude native/system Skills; cancel only owned copies | Not run | Filesystem ownership record required | PENDING |
| Pending | Candidate package TBD | Primary target and terminal versions TBD | App-opened terminal loads session env; unrelated existing shells unchanged | Not run | Session-only environment record required | PENDING |
| Pending | Candidate package TBD | VS Code Remote, server/plugin versions TBD | Remote open, Machine settings, reload, general/AI access and exact owned restoration | Not run | VS Code record required | PENDING |
| Pending | Candidate package TBD | MobaXterm version/bookmark source TBD | Imported and compatible non-Moba target launch only from connected Overview; copied environment command works | Not run | Moba launch record required | PENDING |
| Pending | Candidate package TBD | Primary target and interrupted network | Kill owned SSH, 30s/2min outage, proxy/CC restart and abrupt app exit/restart recover transport, listener, token, profile and Skills | Not run | Timed recovery record required | PENDING |
| Pending | Candidate package TBD | Primary target with foreign-file fixture | Explicit disconnect removes owned SSH/forwards/env, rejects old token, preserves foreign files, clears UI connected state | Not run | Cleanup record required | PENDING |
| Pending | Candidate package TBD | Secondary non-root Linux target, versions TBD | SSH, selected network/AI, actual client and launch/disconnect smoke | Not run | Secondary target record required | PENDING |
| Pending | Frozen tag/Draft TBD | Independently downloaded signed Draft | NSIS/MSI/portable/signatures/manifest/checksums match; repo/workflow/tag/commit/digests verified; Actions and Release bytes match | No tag/Draft created | External verification required | PENDING |
| Pending | Published release TBD | v0.1.3 and available v0.1.4 registered NSIS | Production update detects version/four-language notes, verifies signature, installs passively, restarts, preserves data and one install | No publication authorized | Post-publication smoke required; Draft is not latest-channel E2E | PENDING |
