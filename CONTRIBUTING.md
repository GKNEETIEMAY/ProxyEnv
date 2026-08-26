# Contributing to ProxyEnv

ProxyEnv targets Windows 10/11 x64 and uses Tauri 2, Vue 3, TypeScript, and Rust stable MSVC.

The frontend is organized as `app → features → shared`; the Rust backend separates `commands`, `desktop`, domain modules, and `services`. See [`docs/ARCHITECTURE.md`](docs/ARCHITECTURE.md) before adding a feature or moving an IPC boundary.

## Local checks

```powershell
pnpm install
pnpm build
cargo test --manifest-path src-tauri/Cargo.toml --locked
cargo clippy --manifest-path src-tauri/Cargo.toml --all-targets --locked -- -D warnings
```

Desktop lifecycle changes require one additional integration check: launch the same built EXE twice and verify that the second process exits, the existing window is restored/focused, and only one process for that executable path remains.

`tauri-plugin-single-instance` must remain the first plugin registered in `src-tauri/src/lib.rs`. Its callback may restore the existing window and emit UI events, but must not change proxy state or navigate away from the user's current page.

Keep the Environment Core independent from proxy detection. Do not add system-proxy switching, per-app routing, full port scanning, bundled proxy cores, HKLM writes, or administrator-only behavior to the MVP.

## Release secrets

Never commit a signing private key, certificate private key, `.pfx`, `.p12`, or `.key` file. Future Tauri Updater private material belongs only in GitHub Actions secrets named `TAURI_SIGNING_PRIVATE_KEY` and `TAURI_SIGNING_PRIVATE_KEY_PASSWORD`; workflows must never print the secret, its prefix, length, decoded form, or file contents. A Tauri updater public verification key is not secret and may be reviewed and committed when the official updater is enabled.

ProxyEnv currently performs only a manual GitHub Releases version check. Do not add direct installer downloads or execute downloaded files. Future automatic updates must use the official Tauri Updater with signed artifacts, a pinned HTTPS endpoint, and its default anti-downgrade comparison. See [`docs/release-security.md`](docs/release-security.md).
