# Contributing to ProxyEnv

ProxyEnv targets Windows 10/11 x64 and uses Tauri 2, Vue 3, TypeScript, and Rust stable MSVC.

## Local checks

```powershell
pnpm install
pnpm build
cargo test --manifest-path src-tauri/Cargo.toml
cargo clippy --manifest-path src-tauri/Cargo.toml --all-targets -- -D warnings
```

Keep the Environment Core independent from proxy detection. Do not add system-proxy switching, per-app routing, full port scanning, bundled proxy cores, HKLM writes, or administrator-only behavior to the MVP.

