# Contributing to RouteSpan

English · [简体中文](CONTRIBUTING.zh-CN.md)

RouteSpan targets Windows 10/11 x64 using Tauri 2, Vue 3, TypeScript and Rust stable
MSVC. Repository/package identities retain the legacy ProxyEnv name for compatibility.
Read [Architecture](docs/ARCHITECTURE.md), [Internal contracts](docs/INTERNAL_CONTRACTS.md)
and [Security](SECURITY.md) before changing an IPC or persistence boundary.

RouteSpan v0.2.2 is the public stable baseline, built from `v0.2.2dev` at tag `v0.2.2`.
The previous RouteSpan v0.2.1 release followed v0.2.0 and was
built from `v0.2.1dev` at tag `v0.2.1`. Code completion does not prove
packaged-device acceptance. See [release QA](docs/RELEASE_QA_V0.2.md);
detailed device records remain private.

## Local checks

Use the Node.js/pnpm versions declared in package.json, Windows build tools,
WebView2 and Rust stable MSVC. Install from the lockfile.

```powershell
pnpm install --frozen-lockfile
pnpm build
pnpm test:brand
pnpm test:docs
pnpm test:release
pnpm test:updates
pnpm test:themes
pnpm test:tray
pnpm test:report
pnpm test:local
pnpm test:remote
pnpm test:extensions
cargo fmt --manifest-path src-tauri/Cargo.toml --all -- --check
cargo test --manifest-path src-tauri/Cargo.toml --lib --locked
cargo clippy --manifest-path src-tauri/Cargo.toml --all-targets --locked -- -D warnings
```

`pnpm check` combines frontend, brand/docs/release/update/theme/tray/report/local/remote/extension checks and locked
Rust tests. Documentation tests are offline. Extension tests include POSIX and
Linux filesystem cases; platform-skipped tests are not PASS evidence for those cases.
The generated remote helper must stay reproducible: `pnpm test:extensions` checks
it before transaction tests. Browser fixture tests are supporting UI evidence,
not native Windows, live SSH or model acceptance.

For local workspace browser checks, start `pnpm dev --host 127.0.0.1 --port 1422`
and run `pnpm test:local:ui`. Like the other UI scripts, this needs an available
Playwright runtime; `PROXYENV_LAYOUT_PLAYWRIGHT` can point to its package anchor
and `PROXYENV_LAYOUT_BROWSER` to an installed Chromium executable. Set
`PROXYENV_LAYOUT_URL` when using another preview port. The fixture uses synthetic
data and never writes real proxy settings or tests external sites.

## Change discipline

Frontend dependencies flow app → features → shared; Rust commands are narrow.
Reuse one active-proxy context, backend generation/tickets and shared UI state.
Do not mix dependency upgrades, SSH rewrites or data migrations into documentation
changes. Preserve host trust, fixed upstreams, owned-only restoration and private
file boundaries. No arbitrary shell commands or paths authorized by frontend checks.

Single-instance plugin registration must precede tray/window setup. Lifecycle
changes require a built-EXE double-launch check: one process, restored/focused window,
no state mutation or unwanted navigation from the second launch.

Registry, snapshot or broadcast changes need Windows integration evidence for
exact restore, conflict protection, failure rollback, WM_SETTINGCHANGE and new
process inheritance. Remote changes need scoped authentication, reconnect, cleanup
and preserved-user-file evidence. Do not infer acceptance from mock screenshots.

## Documentation changes

Use [the documentation map](docs/README.md). Current docs explain current behavior;
archive records are historical, not alternative instructions. Maintain EN/ZH
section order, capability count, status and security claims together. Preserve
historical release notes and compatibility IDs. Update QA only from dated,
build/environment-scoped evidence; use PENDING/BLOCKED for missing gates.

## Releases and secrets

The formal release flow requires matching tag/package/Cargo/Tauri versions,
reviewed multilingual notes, frozen dependencies, audit/build/test, signed updater
assets, checksums and provenance, then a Draft for manual QA. Draft ≠ publication.
The formal workflow accepts stable version tags only (no RC/prerelease tags).
`pnpm test:release` protects versions, compatibility identities, artifact naming
and the four-language notes; it is not native or signed-artifact acceptance.

Use only official Tauri Updater with pinned source/public key, default upgrade-only
comparison and explicit user check/install. NSIS replacement is distinct from
MSI/portable manual updates. No custom downloader or silent installer execution.
See [release security](docs/release-security.md).

Never commit/print/package signing keys, key prefixes, passwords, PFX/P12 or decoded
secret material. CI private keys use only authorized Actions secrets
`TAURI_SIGNING_PRIVATE_KEY` and `TAURI_SIGNING_PRIVATE_KEY_PASSWORD`; protected
recovery copies remain outside tracked source. Public verification keys are not secret.
