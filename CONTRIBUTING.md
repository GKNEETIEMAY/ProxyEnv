# Contributing to ProxyEnv

ProxyEnv targets Windows 10/11 x64 and uses Tauri 2, Vue 3, TypeScript, and Rust stable MSVC.

The frontend is organized as `app → features → shared`; the Rust backend separates `commands`, `desktop`, domain modules, and `services`. See [`docs/ARCHITECTURE.md`](docs/ARCHITECTURE.md) before adding a feature or moving an IPC boundary.

## Local checks

```powershell
pnpm install
pnpm build
cargo test --manifest-path src-tauri/Cargo.toml
cargo clippy --manifest-path src-tauri/Cargo.toml --all-targets -- -D warnings
```

Desktop lifecycle changes require one additional integration check: launch the same built EXE twice and verify that the second process exits, the existing window is restored/focused, and only one process for that executable path remains.

`tauri-plugin-single-instance` must remain the first plugin registered in `src-tauri/src/lib.rs`. Its callback may restore the existing window and emit UI events, but must not change proxy state or navigate away from the user's current page.

Keep the Environment Core independent from proxy detection. Do not add system-proxy switching, per-app routing, full port scanning, bundled proxy cores, HKLM writes, or administrator-only behavior to the MVP.
