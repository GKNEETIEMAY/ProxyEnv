<p align="center"><img src="assets/icon.svg" width="92" height="92" alt="ProxyEnv icon"></p>
<h1 align="center">ProxyEnv</h1>
<p align="center">A Windows-first desktop manager for proxy environment variables.<br>Discover local proxy clients and apply, diagnose, disable, or restore user-level proxy settings without guessing ports.</p>
<p align="center"><strong>English</strong> · <a href="README.zh-CN.md">简体中文</a></p>

<p align="center">
  <img alt="Platform" src="https://img.shields.io/badge/platform-Windows%2010%20%7C%2011-0078D4?style=flat-square">
  <img alt="Tauri" src="https://img.shields.io/badge/Tauri-2-24C8DB?style=flat-square&logo=tauri&logoColor=white">
  <img alt="Rust" src="https://img.shields.io/badge/Rust-stable-000000?style=flat-square&logo=rust">
  <img alt="License" src="https://img.shields.io/badge/license-MIT-22c55e?style=flat-square">
  <img alt="Status" src="https://img.shields.io/badge/status-v0.1%20development-f59e0b?style=flat-square">
</p>

> [!IMPORTANT]
> ProxyEnv is in v0.1 development and has no official release yet. Windows 10/11 is the implemented and tested target. Linux and macOS are architectural directions, not currently supported platforms; other Unix variants are out of scope.

## The problem

Many CLIs read `HTTP_PROXY`, `HTTPS_PROXY`, or `ALL_PROXY`, while some direct-connect software behaves incorrectly when those variables are present.

```text
HTTP_PROXY exists
├─ Claude Code / Codex / CLI    ✅
└─ Some direct-connect apps      ❌

HTTP_PROXY is missing
├─ Claude Code / Codex / CLI    ❌
└─ Some direct-connect apps      ✅
```

<p align="center"><img src="docs/assets/proxy-conflict.png" width="880" alt="Cute stick-figure illustration of the HTTP_PROXY conflict"></p>

ProxyEnv makes that state visible and turns registry editing into explicit, recoverable actions. It follows one safety rule:

> Automatic discovery, manual changes.

Detection and periodic refreshes never rewrite the Registry. When a client changes from port `7890` to `7897`, ProxyEnv reports a mismatch and waits for the user to choose **Sync to active proxy**.

## Capabilities

- Detect local proxy clients, listener PIDs, actual ports, and HTTP/SOCKS5/Mixed protocols.
- Show Windows System Proxy status without modifying it.
- Model the proxy environment as `Disabled`, `Partial`, `Enabled`, or `Mismatch`.
- Apply an automatically detected proxy address or a manually entered host, port, and protocol.
- Map protocols correctly: HTTP variables use `http://`; `ALL_PROXY` uses `socks5://` where applicable.
- Select which of `HTTP_PROXY`, `HTTPS_PROXY`, and `ALL_PROXY` the next Apply/Sync action manages.
- Keep `NO_PROXY` read-only.
- Snapshot before changes, delete values safely, restore exact previous values, broadcast `WM_SETTINGCHANGE`, and verify Registry results.
- Run from the system tray, honor startup/window preferences, and reuse the existing window on a second launch.
- Provide Simplified Chinese, English, Japanese, and Korean interfaces with light, dark, and system themes.

## Three proxy layers

ProxyEnv deliberately keeps these concepts separate:

| Layer | Meaning | ProxyEnv behavior |
| --- | --- | --- |
| Proxy client | A local process and listening address, such as v2rayN on `127.0.0.1:10809` | Detect and probe |
| Windows System Proxy | The Windows networking setting used by compatible applications | Read-only |
| Proxy environment | User variables inherited by newly started processes | Change only after an explicit action |

Changing environment variables does not reconfigure a proxy client, toggle Windows System Proxy, route all machine traffic, or alter an already-running process. New applications inherit the updated address; running applications must be restarted for it to take effect.

## Managed values

ProxyEnv v0.1 changes only the current user's values in:

```text
HKEY_CURRENT_USER\Environment
```

It never writes `HKLM` and normally needs no administrator privileges.

| Protocol | `HTTP_PROXY` | `HTTPS_PROXY` | `ALL_PROXY` |
| --- | --- | --- | --- |
| HTTP | `http://host:port` | `http://host:port` | removed |
| SOCKS5 | removed | removed | `socks5://host:port` |
| Mixed | `http://host:port` | `http://host:port` | `socks5://host:port` |

`HTTP_PROXY` and `HTTPS_PROXY` are selected by default. `ALL_PROXY` is opt-in because its broader fallback behavior can affect package search, LAN discovery, or software with incomplete SOCKS support.

## Safe operations

```text
Disable: read → snapshot → delete → broadcast → read back → verify
Restore: load latest snapshot → restore exact values → broadcast → verify
Sync:    build plan from selected proxy address → snapshot → write/delete → broadcast → verify
Refresh: detect and compare only; never write
```

Snapshots are stored atomically under `%LOCALAPPDATA%\ProxyEnv\snapshots\latest.json`. Legacy `env-snapshot.json` files remain readable.

## Supported proxy clients

ProxyEnv identifies Clash Verge Rev, v2rayN, FlClash, Hiddify, Clash Nyanpasu, Clash Party, Mihomo Party, NekoBox/NekoRay, Clash for Windows, and GUI.for.Clash. The first five use attributed upstream icons; the additional and unknown clients use the shared proxy icon. Detection failure can be bypassed with a manual host, port, and protocol.

Icons come from official upstream repositories; see [`public/proxy-clients/ATTRIBUTION.md`](public/proxy-clients/ATTRIBUTION.md).

## Requirements

### Running a packaged build

- Windows 10 version 1803 or later (x64), or Windows 11 x64
- Microsoft Edge WebView2 Runtime, normally preinstalled on supported Windows versions

Packaged builds do not require Node.js, pnpm, Rust, or Visual Studio.

### Developing from source

- Node.js `20.19+` or `22.12+`; Node.js 22 LTS is recommended
- pnpm 10 via Corepack
- Rust stable with the MSVC toolchain
- Visual Studio Build Tools 2022 with **Desktop development with C++**

These are the minimums required by the current Vite 7 and Tauri 2 toolchain; using the newest Node Current release is unnecessary.

```powershell
cd ProxyEnv
corepack enable
pnpm install
pnpm tauri dev
```

In VS Code, install the recommended extensions, select `ProxyEnv: Tauri Debug`, and press `F5`.

## Verification

```powershell
pnpm build
cargo test --manifest-path src-tauri/Cargo.toml
cargo clippy --manifest-path src-tauri/Cargo.toml --all-targets -- -D warnings
pnpm tauri build
```

## Repository structure

```text
ProxyEnv/
├─ src/
│  ├─ app/                       # Vue shell and desktop orchestration
│  ├─ features/                  # Proxy and settings UI
│  └─ shared/                    # IPC, i18n, types, and visual tokens
├─ src-tauri/src/
│  ├─ commands/                  # Thin Tauri IPC adapters
│  ├─ desktop/                   # Tray, single instance, native window
│  ├─ environment/               # Generic mutation, snapshot, broadcast, verification
│  ├─ features/proxy/            # Proxy detection, plans, state, sync/restore/disable
│  └─ services/                  # Persistent application settings
├─ public/proxy-clients/         # Runtime icons and attribution
└─ docs/                         # Architecture and README artwork
```

The generic Environment Core has no proxy-client or proxy-variable knowledge. Dependency rules are documented in [`docs/ARCHITECTURE.md`](docs/ARCHITECTURE.md).

## Scope and privacy

ProxyEnv is not a proxy client, VPN, subscription manager, or traffic router. v0.1 does not control Clash/v2rayN APIs, TUN, nodes, rules, Windows System Proxy, per-app routing, or system-level environment variables.

Detection, protocol probing, and environment management stay on the local machine. ProxyEnv never reads subscriptions, nodes, tokens, passwords, or traffic. Only a user-triggered update check contacts GitHub Releases. See [`SECURITY.md`](SECURITY.md).

## Contributing

Contributions are welcome. Read [`CONTRIBUTING.md`](CONTRIBUTING.md) and preserve the dependency direction: Proxy Feature may depend on Environment Core; Environment Core must never depend on Proxy Feature.

## Contributors

- ProxyEnv maintainers and community contributors
- OpenAI Codex — implementation, testing, design, and documentation assistance

## License

ProxyEnv is released under the [MIT License](LICENSE). Third-party icons retain their upstream licenses.
