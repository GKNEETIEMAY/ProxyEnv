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
review passed; the required live GitHub rendering gate below remains pending.

## Remaining acceptance checklist

| Date | Build / commit | Environment | Expected | Actual | Evidence | Status |
| --- | --- | --- | --- | --- | --- | --- |
| 2026-10-12 review | Current docs change set | GitHub rendered pages | Review both READMEs, current index/guides/architecture/security/roadmap/QA | Changes not yet pushed; GitHub rendering not reviewed | Requires review after publication of this change set | PENDING |
| 2026-10-12 review | Next CI-triggering commit | GitHub Windows/Linux | Latest code CI passes with audits intact | No CI run exists for the unpushed changes | Trigger after authorized push; keep baseline result scoped | PENDING |

Release polish must resolve the frontend audit and all real-device/signing gates.
Do not change pending evidence to PASS solely to satisfy a documentation checklist.
