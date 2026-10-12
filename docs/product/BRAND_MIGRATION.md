# Stage 5 — 境枢 RouteSpan display-brand migration

Baseline: `v0.2.0dev`, commit `9a219e31d91d63142ec5c4dc20bf713bee51df2d`.
This is a display-brand migration, not a repository or storage migration.
Stage 4 navigation and bridge behavior remain frozen.

## Product relationship

RouteSpan is a local-first bridge: it extends existing local network access,
CC Switch-managed AI routes, Skills and development capabilities to remote
environments. CC Switch owns the route; RouteSpan supplies the span/bridge.
RouteSpan does not replace CC Switch or implement a new AI provider router.

| Locale | Display name | Slogan |
| --- | --- | --- |
| zh-CN | 境枢 RouteSpan | 让本地能力，延伸到远程。 |
| en | RouteSpan | Extend local capabilities to remote environments. |
| ja | RouteSpan | ローカルの能力を、リモート環境へ。 |
| ko | RouteSpan | 로컬의 기능을 원격 환경으로 확장하세요. |

## Implemented display changes

- Shared `BrandMark.vue` replaces the previous tile in Header and About.
- Four-language visible branding, diagnostic reports and existing notification
  text use RouteSpan. Dormant notification functionality remains dormant.
- Native product name is `RouteSpan`; the initial window title is
  `境枢 RouteSpan`. Tray tooltip and localized quit labels use RouteSpan.
- Terminal window title uses RouteSpan. SSH commands and authentication are unchanged.
- Release workflow external portable filename and artifact name use RouteSpan.
  Draft publication, signing, checksums and updater generation are unchanged.
- Manual connection metadata retains its legacy sentinel but does not render
  that sentinel as a configuration-file path. Real discovered paths remain visible.

## Logo system

The user's final instruction supersedes the SVG proposal and the subsequent
white App Icon selection: use the latest pale PNG for every branding icon
(`codex-clipboard-28130f12-f7b8-4ecd-973f-92242ecfc8e0.png`, SHA256
`98d69dcf748bafcbdc91deffffbfbe7867e2131fabac450104e36b1933506ff2`).
`assets/brand/routespan-mark-source.png` preserves its exact original bytes,
with the SHA256 hash pinned in the branding test.

`routespan-mark.png` is the 256px derivative used by Header/About and mirrored
in `public/brand/`. `routespan-appicon-1024.png` is a proportional derivative of
the same pale PNG and is the sole Tauri native icon source. Tray variants use
that PNG as well. No white/dark plate, other supplied image, authored SVG, mono
substitute, recoloring, redraw or background removal is used. Transparency and
original shading are retained.

`scripts/generate-brand-icons.mjs` resamples the originals and transparent tray PNGs;
Tauri CLI generates the native icon family. See `assets/brand/README.md` for
reproduction. No runtime dependency or lockfile change is required.

Six `--brand-*` colors are available in the shared stylesheet. Existing warm
surface, action, success, warning, danger and focus tokens are preserved; the
three logo colors do not replace semantic UI colors.

## Compatibility allowlist — intentionally NOT renamed

| Identity | Reason |
| --- | --- |
| npm/Cargo package `proxyenv`, library `proxyenv_lib`, executable `proxyenv.exe` | Build/internal identity |
| `io.github.gkneeteiemay.proxyenv` | Installation and updater identity |
| `%LOCALAPPDATA%\ProxyEnv` | Existing user settings and snapshots |
| remote `~/.proxyenv` | Sessions, managed copies and recovery state |
| `proxyenv.remoteBridge.*` localStorage keys | Remembered connection/UI state |
| `X-ProxyEnv-Session` | AI route session authentication contract |
| `proxyenv_bridge`, persisted `ProxyEnv Local Bridge` provider name | Existing managed configuration identity |
| Manual-target `source_label` / `config_path` sentinel `ProxyEnv` | Existing metadata contract; UI uses localized source label |
| Backend/IPC names, managed file suffixes and ownership markers | Existing transport and restore contracts |
| `GKNEETIEMAY/ProxyEnv`, updater endpoint/public key and release-note markers | Repository, trusted updates and historical records |
| v0.1.x release notes and existing README history | Historical facts; broader user-document refresh belongs to Stage 6 |

Changing a display name alone does not prove old-installation migration. Do not
rename these identities opportunistically during subsequent documentation work.

## Verification and acceptance boundary

Local automated checks cover localized names/slogans, exact user-source hashes,
PNG/ICO size coverage, native labels, compatibility identities and unchanged
historical release notes. Browser checks use synthetic IPC and cover four
languages, light/dark Header layouts down to 560px, About, reports, unchanged
primary/secondary navigation and stable background transport indicators.

Rust formatting, locked tests and Clippy remain required, as do frontend
TypeScript/build and existing bridge/configuration transaction regressions.
Skipped POSIX fixtures on Windows are not counted as verified.

Local results on 2026-10-12:

| Check | Result |
| --- | --- |
| Branding unit tests | 6 passed |
| Combined branding/navigation/bridge/install/notification/report/extension/network tests | 144 passed, 41 skipped, 0 failed |
| Rust locked tests | 282 passed, 3 ignored, 0 failed |
| Rust format / Clippy `-D warnings` | Passed |
| Vue TypeScript / production frontend build | Passed; existing large-bundle advisory remains |
| Brand and Stage 4 navigation browser checks | Passed with synthetic IPC |
| Independent branding screenshot review | Latest pale PNG Header/About and light/dark size board passed; no blocking screenshot defects |
| Native icon regeneration | Regenerated from latest pale PNG; PNG/ICO coverage and RGBA checks passed |
| Latest pale PNG Rust check | Passed; incremental-cache access warning does not affect compilation result |
| Final pale PNG installer build | User reported already built on 2026-10-12; redundant agent build stopped at user's request. Installer metadata, installation and upgrade were not independently verified |

The 41 frontend skips comprise 37 POSIX fixtures excluded from this Windows
run and 4 Linux-only filesystem cases. No additional runtime dependency was added.

Windows manual acceptance remains mandatory for:

- Actual tray 16/20px contrast on light/dark taskbars; use the transparent mark
  provisionally, not a claimed final native contrast result.
- Taskbar, Explorer shortcut, installer and Windows icon-cache behavior.
- A real v0.1.4 → v0.2.0 upgrade, startup entry, settings/snapshot reuse,
  updater trust, uninstall behavior and absence of duplicate installed products.
- Real 125%/150% DPI and live bridge authentication/reconnection.

Unsigned local installer smoke is a packaging check, not a signed release,
installation test or upgrade acceptance. GitHub CI runs only after a later push.

Stage 6 document work may proceed after the code/build checks; it must not claim
Stage 7 native upgrade/icon acceptance is complete.

## Stage 5 document regression — latest decisions take precedence

The implementation document's SVG, separate plated App Icon, mono variants and
small-size redraw requirements were superseded by the user's later instruction:
use only the final pale transparent PNG, proportionally resized. Header/About
compose this PNG with localized text rather than generating another lockup.
These are intentional requirement changes, not missing implementation.

| Requirement group | Acceptance evidence / status |
| --- | --- |
| Name, slogan, Header, About, diagnostic report and four languages | Implemented; branding tests and browser regression passed |
| Latest user artwork, native icon family and transparent tray | Implemented; source hash, RGBA, dimensions and ICO coverage passed; screenshot review passed |
| Native window, tray labels, dormant toast text and terminal title | Implemented; source assertions and Rust checks passed; no notification feature enabled |
| External portable/workflow names, productName | Implemented; release naming assertions passed; no tag or Release created |
| CC Switch relationship, warm UI and semantic status colors | Preserved; migration contract records local-first bridge positioning |
| Frozen Stage 4 hierarchy and remote status indicator | Preserved; navigation and synthetic browser regression passed |
| Package, executable, identifier, data paths, session header, repository and updater | Preserved; compatibility boundary tests passed |
| Broad README rewrite and historical release notes | Deferred to Stage 6; historical notes verified unchanged |
| Installer build | User reports complete; repeated packaging was cancelled, not counted as an agent success |
| Real Windows tray/taskbar, Start Menu, shortcut, Alt+Tab and installer appearance | Manual acceptance still open; screenshot board is not a substitute |
| Upgrade, startup, settings/snapshots, uninstall, updater and 125%/150% DPI | Stage 7 manual gate remains open |

Code implementation and automated regression are complete. This is not a claim
that the document's full Windows manual Definition of Done has been achieved.
