# Contributing to ProxyEnv

```yaml
Current Stable: v0.1.4
Next: v0.2.0
```

See the [Roadmap](docs/ROADMAP.md) for released capabilities and unreleased development progress. Implementation completion does not imply release or clean-machine acceptance. Update these version markers together when publishing; this documentation cleanup does not bump package versions or create a tag.

For a release, review `README.md`, `README.zh-CN.md`, `CONTRIBUTING.md`, `SECURITY.md`, `docs/ARCHITECTURE.md`, `docs/release-security.md`, and `docs/ROADMAP.md` together. Move only actually released features from Next to Current, and distinguish automated tests from recorded packaged-Windows acceptance. Linux/macOS have no committed implementation date.

ProxyEnv targets Windows 10/11 x64 and uses Tauri 2, Vue 3, TypeScript, and Rust stable MSVC.

The frontend is organized as `app → features → shared`; the Rust backend separates `commands`, `desktop`, domain modules, and `services`. See [`docs/ARCHITECTURE.md`](docs/ARCHITECTURE.md) before adding a feature or moving an IPC boundary.

## Local checks

```powershell
pnpm install
pnpm build
pnpm test:report
cargo test --manifest-path src-tauri/Cargo.toml --locked
cargo clippy --manifest-path src-tauri/Cargo.toml --all-targets --locked -- -D warnings
```

Desktop lifecycle changes require one additional integration check: launch the same built EXE twice and verify that the second process exits, the existing window is restored/focused, and only one process for that executable path remains.

`tauri-plugin-single-instance` must remain the first plugin registered in `src-tauri/src/lib.rs`. Its callback may restore the existing window and emit UI events, but must not change proxy state or navigate away from the user's current page.

Keep the Environment Core independent from proxy detection. Do not add system-proxy switching, per-app routing, full port scanning, bundled proxy cores, HKLM writes, or administrator-only behavior to the MVP.

## Release secrets

Never commit a signing private key, certificate private key, `.pfx`, `.p12`, or `.key` file. Tauri Updater private material used by CI belongs only in GitHub Actions secrets named `TAURI_SIGNING_PRIVATE_KEY` and `TAURI_SIGNING_PRIVATE_KEY_PASSWORD`; any maintainer recovery copy must stay outside tracked source in protected offline storage. Workflows must never print the secret, its prefix, length, decoded form, or file contents. The updater public verification key is not secret and is reviewed and committed in `tauri.conf.json`.

Current Stable v0.1.4 supports signed updates for the NSIS setup edition. The user first selects **Check for updates**, then explicitly selects **Download and install**; the official Tauri Updater verifies the signed installer from the pinned HTTPS source, retains its default upgrade-only version comparison, replaces the registered installation, and restarts after success. MSI and portable packages remain manual-update variants. Do not introduce a custom downloader, arbitrary update URLs, direct execution of an unverified installer, or silent background installation. See [`docs/release-security.md`](docs/release-security.md).

Keep the public product boundary narrow: ProxyEnv observes and explains the current Windows proxy environment, manages selected user environment variables, and launches new processes with an explicit proxy or direct environment. Do not present third-party application configuration maintenance as a shipped integration or roadmap commitment.
