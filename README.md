<p align="center">
  <img src="assets/icon.svg" width="92" height="92" alt="ProxyEnv icon">
</p>

<h1 align="center">ProxyEnv</h1>

<p align="center">
  A cross-platform proxy environment switch being built for Windows, Linux, and macOS<br>
  Discover proxy clients, actual listening ports, and protocols, then keep proxy variables synchronized
</p>

<p align="center">
  <strong>English</strong> · <a href="README.zh-CN.md">简体中文</a>
</p>

<p align="center">
  <img alt="Platform" src="https://img.shields.io/badge/targets-Windows%20%7C%20Linux%20%7C%20macOS-0078D4?style=flat-square">
  <img alt="Tauri" src="https://img.shields.io/badge/Tauri-2-24C8DB?style=flat-square&logo=tauri&logoColor=white">
  <img alt="Rust" src="https://img.shields.io/badge/Rust-stable-000000?style=flat-square&logo=rust">
  <img alt="License" src="https://img.shields.io/badge/license-MIT-22c55e?style=flat-square">
  <img alt="Status" src="https://img.shields.io/badge/status-v0.1%20in%20development-f59e0b?style=flat-square">
</p>

> [!IMPORTANT]
> ProxyEnv is currently in v0.1 development and does not have an official release yet. The Windows environment toggle, local proxy detection, system tray, and localized General / About settings are working. Linux and macOS backends and release packaging are still in progress.

| Platform            | Status                                 | Variable convention                                  |
| ------------------- | -------------------------------------- | ---------------------------------------------------- |
| Windows 10/11       | Implemented and tested on real systems | `HTTP_PROXY`, `HTTPS_PROXY`, `ALL_PROXY`, `NO_PROXY` |
| Linux               | Planned                                | `http_proxy`, `https_proxy`, `all_proxy`, `no_proxy` |
| macOS               | Planned                                | `http_proxy`, `https_proxy`, `all_proxy`, `no_proxy` |
| Other Unix variants | Not planned                            | —                                                    |

## Why ProxyEnv?

Desktop applications, CLIs, and networking libraries across Windows, Linux, and macOS do not always use the same proxy entry point. Claude Code, Codex, Git, npm, pip, and similar tools commonly read proxy environment variables, while some direct-connect applications can fail when those variables are present.

That creates a frustrating conflict:

```text
HTTP_PROXY exists
├─ Claude Code / Codex / CLI    ✅
└─ Some direct-connect apps      ❌

HTTP_PROXY is missing
├─ Claude Code / Codex / CLI    ❌
└─ Some direct-connect apps      ✅
```

<p align="center">
  <img src="docs/assets/proxy-conflict.png" width="880" alt="Cute stick-figure illustration of the HTTP_PROXY conflict">
</p>

ProxyEnv turns this workflow:

```text
Open the operating system's environment configuration
→ Find and remove proxy variables
→ Launch the direct-connect application
→ Recreate the original variables manually
```

into:

```text
Disable
→ Launch the target application
→ Enable
```

Before disabling anything, ProxyEnv saves a complete snapshot. Enabling restores the original values instead of replacing them with a hard-coded port.

## Core capabilities

| Capability             | Description                                                                         |
| ---------------------- | ----------------------------------------------------------------------------------- |
| One-click toggle       | Remove or restore user-level proxy variables; never write empty strings             |
| Live endpoint sync     | While enabled, follow the active proxy client instead of retaining a stale port     |
| Safe snapshots         | Persist the complete state before changes and atomically replace snapshot files     |
| Automatic discovery    | Combine the Windows system proxy, processes, TCP listener PIDs, and protocol probes |
| Actual ports           | Read real listening ports instead of assuming 7890 or 10808                         |
| Protocol detection     | Distinguish HTTP, SOCKS5, and mixed proxy ports                                     |
| Client identification  | Recognize popular proxy clients and display their icons                             |
| Read-back verification | Verify the registry after every Enable and Disable operation                        |
| Windows notification   | Broadcast environment changes through `WM_SETTINGCHANGE`                            |
| Selective variables    | Manage HTTP, HTTPS, and ALL proxy variables independently                           |
| Tray and preferences   | Open or toggle from the tray; persist language, theme, and window behavior           |
| Endpoint utilities     | Show protocol and confidence, and copy the active proxy address                      |
| About and updates      | Show the runtime version, changelog, and user-triggered GitHub Releases status        |
| Desktop shell          | Use a rounded frameless window with light, dark, and system appearance                |

## Positioning against environment-variable tools

Ratings reflect the scenario each tool serves best; they are not an absolute feature ranking. ProxyEnv focuses on automatic proxy discovery and switching, while the other tools are broader environment-variable or `PATH` managers.

| Tool                                        | Recommendation | Best for                                                                                       | Main characteristics                                                                                            |
| ------------------------------------------- | -------------- | ---------------------------------------------------------------------------------------------- | --------------------------------------------------------------------------------------------------------------- |
| **ProxyEnv**                                | ⭐⭐⭐⭐⭐     | Developers frequently switching between proxy and direct connections or multiple proxy clients | Rust + Tauri, cross-platform target, client/port/protocol discovery, snapshots, active-endpoint synchronization |
| Microsoft PowerToys – Environment Variables | ⭐⭐⭐⭐⭐     | Most developers                                                                                | Microsoft-maintained, modern UI, profiles, user/system variables                                                |
| EnvStudio                                   | ⭐⭐⭐⭐⭐     | Complex `PATH` values and multiple development environments                                    | Drag-and-drop `PATH`, deduplication, invalid-path and conflict detection, snapshot rollback                     |
| Envarly                                     | ⭐⭐⭐⭐½      | Open-source users who want safer changes                                                       | Open source, pre-change diff, snapshots/rollback, `PATH` drag-and-drop, PowerShell/Ansible export               |
| Rapid Environment Editor                    | ⭐⭐⭐⭐       | Traditional Windows development environments                                                   | Mature, tree-based `PATH` editing, error detection, backups, portable build                                     |
| envx                                        | ⭐⭐⭐⭐       | Terminal/TUI users                                                                             | Rust, cross-platform, snapshots/profiles, search, CLI, `.env`/JSON/YAML import and export                       |

## Supported proxy clients

| Client          | Windows process identification                         | Icon     | Status              |
| --------------- | ------------------------------------------------------ | -------- | ------------------- |
| Clash Verge Rev | `clash-verge.exe`, `verge-mihomo.exe`, service process | Official | Verified on Windows |
| v2rayN          | `v2rayN.exe` plus Xray / sing-box / Mihomo Core        | Official | Rules integrated    |
| FlClash         | `FlClash.exe` plus Mihomo Core                         | Official | Rules integrated    |
| Hiddify         | `Hiddify.exe` plus sing-box Core                       | Official | Rules integrated    |
| Clash Nyanpasu  | `clash-nyanpasu.exe` plus Mihomo / Clash RS            | Official | Rules integrated    |

ProxyEnv does not assign a client name from a generic core process alone. Shared processes such as `mihomo.exe` and `sing-box.exe` are correlated with a running frontend, the system proxy endpoint, and the listener PID before attribution.

Client icons come from their official repositories. Sources and licenses are documented in [`public/proxy-clients/ATTRIBUTION.md`](public/proxy-clients/ATTRIBUTION.md).

## Managed variables

Variable names follow the host platform instead of displaying duplicate aliases:

```text
Windows:        HTTP_PROXY      HTTPS_PROXY      ALL_PROXY
Linux / macOS:  http_proxy      https_proxy      all_proxy
```

Displayed but not removed by default:

```text
Windows:        NO_PROXY
Linux / macOS:  no_proxy
```

Windows environment variable names are case-insensitive, so upper- and lowercase aliases are the same value. Linux and macOS names are case-sensitive, so ProxyEnv follows the widely compatible lowercase proxy convention there. The v0.1 environment persistence and proxy discovery backend is currently implemented and tested on Windows; Linux and macOS will not be marked supported until their backends are complete.

`HTTP_PROXY` and `HTTPS_PROXY` are selected by default. `ALL_PROXY` is opt-in because it can affect more applications and package-search traffic; at least one managed proxy variable must remain selected. `NO_PROXY` / `no_proxy` stays read-only in the current UI.

ProxyEnv only modifies the current user:

```text
HKEY_CURRENT_USER\Environment
```

It does not write to `HKLM`, so v0.1 does not require administrator privileges by default.

## How it works

### Environment transaction

```text
Disable
Read → Persist snapshot → Delete → Broadcast → Read back and verify

Enable
Use detected active endpoint → Write selected values → Broadcast → Read back and verify
No active endpoint → Fall back to the saved snapshot
```

The snapshot is stored at:

```text
%LOCALAPPDATA%\ProxyEnv\env-snapshot.json
```

### Proxy discovery pipeline

```text
Windows System Proxy ─┐
Known Client Process ─┼─→ TCP Listener + PID
Local Listener Table ─┘            │
                                   ▼
                         HTTP / SOCKS5 Probe
                                   │
                                   ▼
                      Merge → Score → Recommend
```

Probing is limited to discovered local candidates. ProxyEnv does not scan ports `1–65535` and does not contact an external test website.

## Quick start

### Runtime requirements

- Windows 10 1803 or newer (x64), or Windows 11 x64
- Microsoft Edge WebView2 Runtime; it is normally preinstalled on current Windows 10/11 systems

Packaged builds do not require Node.js, pnpm, Rust, or Visual Studio on the user's computer.

### Development requirements

- Node.js 20.19+ or 22.12+; Node.js 22 LTS is recommended
- pnpm 10 through Corepack
- Rust stable MSVC toolchain
- Visual Studio Build Tools 2022 with Desktop development with C++

These minimums follow the actual Vite 7 and Tauri 2 toolchain requirements. Using the current LTS Node.js release is preferred; installing the newest Current release is not required.

### Run from source

```powershell
# After cloning the repository, enter the project directory
cd ProxyEnv
corepack enable
pnpm install
pnpm tauri dev
```

For VS Code, install the recommended workspace extensions, select `ProxyEnv: Tauri Debug`, and press `F5`. The workspace task reuses an existing ProxyEnv Vite server and stops only stale debug instances from this repository before rebuilding.

### Check and build

```powershell
# Frontend type check and production build
pnpm build

# Rust tests
cargo test --manifest-path src-tauri/Cargo.toml

# Strict static analysis
cargo clippy --manifest-path src-tauri/Cargo.toml --all-targets -- -D warnings

# Build the Windows NSIS installer
pnpm tauri build
```

## Repository layout

```text
ProxyEnv/
├─ src/                         # Feature-oriented Vue frontend
│  ├─ app/                      # App shell and desktop orchestration
│  ├─ features/                 # Proxy and settings feature modules
│  └─ shared/                   # Typed IPC, i18n, styles, and shared types
├─ src-tauri/                   # Layered Rust / Tauri backend
│  └─ src/
│     ├─ commands/              # Thin Tauri command adapters by domain
│     ├─ desktop/               # Tray and native-window integration
│     ├─ environment/           # Registry, snapshots, broadcast, transactions
│     ├─ proxy/                 # System proxy, processes, listeners, probes
│     └─ services/              # Persistent application services
├─ public/proxy-clients/        # Client icons and attribution
├─ docs/                        # Architecture notes and README artwork
└─ .vscode/                     # Local debugging configuration
```

See [`docs/ARCHITECTURE.md`](docs/ARCHITECTURE.md) for dependency rules and the feature-extension workflow.

## Security and privacy

- Detection and environment operations stay on the local machine.
- Proxy nodes, subscription URLs, passwords, tokens, and traffic are never read.
- Process names, ports, and proxy settings are never uploaded.
- Only a user-triggered update check contacts the GitHub Releases API; it sends no proxy or environment data.
- The Windows system proxy is not modified.
- System-level environment variables and `HKLM` are not modified.
- Protocol probes use short timeouts and connect only to local candidate ports.
- Disable stops before deletion if the snapshot cannot be saved successfully.

See [`SECURITY.md`](SECURITY.md) for the security policy.

## Non-goals

ProxyEnv is not a proxy client. The v0.1 scope excludes:

- Node, subscription, and rule management
- Clash or v2rayN control APIs
- TUN, VPN, or driver management
- Windows System Proxy switching
- Per-app proxies or process injection
- Full port scanning, packet capture, or traffic uploads
- Bundled Mihomo, sing-box, Xray, or other proxy cores
- Other Unix variants such as AIX or FreeBSD

## Roadmap

- [x] Vue 3 + Tauri 2 project foundation
- [x] User environment variable reads
- [x] Snapshot, delete, broadcast, and verification for Disable
- [x] Exact restoration and verification for Enable
- [x] Windows System Proxy parsing
- [x] TCP listener-to-PID correlation
- [x] HTTP / SOCKS5 / mixed protocol probes
- [x] Popular client identification and official icons
- [x] Selective HTTP / HTTPS / ALL proxy controls
- [x] System tray toggle, open, and exit actions
- [x] Localized settings, startup, and window behavior
- [x] Endpoint copy, General / About settings, update status, and changelog
- [x] Rounded frameless desktop shell
- [ ] Multiple-candidate selection UI
- [ ] Dynamic ON / OFF / Warning tray artwork
- [ ] Windows integration compatibility matrix
- [ ] Linux environment persistence, process, and listener backend
- [ ] macOS environment persistence, system proxy, and process backend
- [ ] Linux/macOS packaging and integration test matrix
- [ ] Automated NSIS and portable releases

## FAQ

### Why does an already-running application not change immediately?

Windows processes usually copy their parent's environment when they are created. ProxyEnv broadcasts the environment update, but it cannot force existing terminals or applications to rebuild their environment blocks. Launch the target application after switching, and reopen the terminal when necessary.

### Does Disable set variables to empty strings?

No. ProxyEnv removes managed values from `HKCU\Environment` after saving a snapshot.

### Will Enable update my custom proxy address?

Yes, when a verified active local proxy is detected. While the environment switch is enabled, ProxyEnv keeps the managed variables aligned with that endpoint and only writes when the endpoint changes. If no active endpoint is available during Enable, it falls back to the latest saved snapshot.

### Why is a running proxy client not shown as a candidate?

A running frontend does not prove that its proxy core is listening. ProxyEnv only displays candidates backed by the system proxy, a listener PID, or a successful protocol probe; it does not guess a port from the client name.

## Contributing

Contributions and issue reports are welcome. Read [`CONTRIBUTING.md`](CONTRIBUTING.md) first and keep the Environment Core independent from Proxy Detection.

## Contributors

- ProxyEnv maintainers and community contributors
- OpenAI Codex — AI coding assistant for implementation, testing, and documentation

## License

ProxyEnv source code is available under the [MIT License](LICENSE). Third-party client icons remain subject to their upstream licenses; see the icon attribution document for details.
