<p align="center">
  <img src="assets/icon.svg" width="92" height="92" alt="ProxyEnv icon">
</p>

<h1 align="center">ProxyEnv</h1>

<p align="center">
  A lightweight proxy environment switch for Windows<br>
  Detect local proxy clients, actual listening ports, and proxy protocols automatically
</p>

<p align="center">
  <strong>English</strong> · <a href="README.zh-CN.md">简体中文</a>
</p>

<p align="center">
  <img alt="Platform" src="https://img.shields.io/badge/platform-Windows%2010%20%7C%2011-0078D4?style=flat-square">
  <img alt="Tauri" src="https://img.shields.io/badge/Tauri-2-24C8DB?style=flat-square&logo=tauri&logoColor=white">
  <img alt="Rust" src="https://img.shields.io/badge/Rust-stable-000000?style=flat-square&logo=rust">
  <img alt="License" src="https://img.shields.io/badge/license-MIT-22c55e?style=flat-square">
  <img alt="Status" src="https://img.shields.io/badge/status-v0.1%20in%20development-f59e0b?style=flat-square">
</p>

> [!IMPORTANT]
> ProxyEnv is currently in v0.1 development and does not have an official release yet. The environment toggle and local proxy detection core are working; tray integration and release packaging are still in progress.

## Why ProxyEnv?

Windows applications do not share one proxy mechanism. Browsers and many desktop applications use the Windows system proxy, while Claude Code, Codex, Git, npm, pip, and other CLI tools or networking libraries often prefer `HTTP_PROXY`, `HTTPS_PROXY`, and `ALL_PROXY`.

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
Open Windows Environment Variables
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

| Capability | Description |
|---|---|
| One-click toggle | Remove or restore user-level proxy variables; never write empty strings |
| Safe snapshots | Persist the complete state before changes and atomically replace snapshot files |
| Automatic discovery | Combine the Windows system proxy, processes, TCP listener PIDs, and protocol probes |
| Actual ports | Read real listening ports instead of assuming 7890 or 10808 |
| Protocol detection | Distinguish HTTP, SOCKS5, and mixed proxy ports |
| Client identification | Recognize popular proxy clients and display their icons |
| Read-back verification | Verify the registry after every Enable and Disable operation |
| Windows notification | Broadcast environment changes through `WM_SETTINGCHANGE` |

## Supported proxy clients

| Client | Windows process identification | Icon | Status |
|---|---|---|---|
| Clash Verge Rev | `clash-verge.exe`, `verge-mihomo.exe`, service process | Official | Verified on Windows |
| v2rayN | `v2rayN.exe` plus Xray / sing-box / Mihomo Core | Official | Rules integrated |
| FlClash | `FlClash.exe` plus Mihomo Core | Official | Rules integrated |
| Hiddify | `Hiddify.exe` plus sing-box Core | Official | Rules integrated |
| Clash Nyanpasu | `clash-nyanpasu.exe` plus Mihomo / Clash RS | Official | Rules integrated |

ProxyEnv does not assign a client name from a generic core process alone. Shared processes such as `mihomo.exe` and `sing-box.exe` are correlated with a running frontend, the system proxy endpoint, and the listener PID before attribution.

Client icons come from their official repositories. Sources and licenses are documented in [`public/proxy-clients/ATTRIBUTION.md`](public/proxy-clients/ATTRIBUTION.md).

## Managed variables

Managed by default:

```text
HTTP_PROXY      HTTPS_PROXY      ALL_PROXY
http_proxy      https_proxy      all_proxy
```

Displayed but not removed by default:

```text
NO_PROXY        no_proxy
```

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
Load snapshot → Restore exact values → Broadcast → Read back and verify
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

### Requirements

- Windows 10 22H2 x64 or Windows 11 x64
- Microsoft Edge WebView2 Runtime
- Node.js 22 or newer
- pnpm 10
- Rust stable MSVC toolchain
- Visual Studio Build Tools 2022 with Desktop development with C++

### Run from source

```powershell
# After cloning the repository, enter the project directory
cd ProxyEnv
corepack enable
pnpm install
pnpm tauri dev
```

For VS Code, install the recommended workspace extensions, select `ProxyEnv: Tauri Debug`, and press `F5`.

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
├─ src/                         # Vue 3 frontend
│  ├─ App.vue
│  ├─ services/                 # Tauri IPC wrappers
│  └─ types/
├─ src-tauri/                   # Rust / Tauri backend
│  └─ src/
│     ├─ environment/           # Registry, snapshots, broadcast, transactions
│     └─ proxy/                 # System proxy, processes, listeners, probes
├─ public/proxy-clients/        # Client icons and attribution
├─ docs/assets/                 # README artwork
└─ .vscode/                     # Local debugging configuration
```

## Security and privacy

- Detection and environment operations stay on the local machine.
- Proxy nodes, subscription URLs, passwords, tokens, and traffic are never read.
- Process names, ports, and proxy settings are never uploaded.
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

## Roadmap

- [x] Vue 3 + Tauri 2 project foundation
- [x] User environment variable reads
- [x] Snapshot, delete, broadcast, and verification for Disable
- [x] Exact restoration and verification for Enable
- [x] Windows System Proxy parsing
- [x] TCP listener-to-PID correlation
- [x] HTTP / SOCKS5 / mixed protocol probes
- [x] Popular client identification and official icons
- [ ] Multiple-candidate selection UI
- [ ] System tray Enable / Disable
- [ ] Settings, startup, and start-minimized behavior
- [ ] Windows integration compatibility matrix
- [ ] Automated NSIS and portable releases

## FAQ

### Why does an already-running application not change immediately?

Windows processes usually copy their parent's environment when they are created. ProxyEnv broadcasts the environment update, but it cannot force existing terminals or applications to rebuild their environment blocks. Launch the target application after switching, and reopen the terminal when necessary.

### Does Disable set variables to empty strings?

No. ProxyEnv removes managed values from `HKCU\Environment` after saving a snapshot.

### Will Enable overwrite my custom proxy address?

Enable restores the exact values captured before the latest Disable operation. ProxyEnv does not silently overwrite existing variables during startup when no snapshot is available.

### Why is a running proxy client not shown as a candidate?

A running frontend does not prove that its proxy core is listening. ProxyEnv only displays candidates backed by the system proxy, a listener PID, or a successful protocol probe; it does not guess a port from the client name.

## Contributing

Contributions and issue reports are welcome. Read [`CONTRIBUTING.md`](CONTRIBUTING.md) first and keep the Environment Core independent from Proxy Detection.

## License

ProxyEnv source code is available under the [MIT License](LICENSE). Third-party client icons remain subject to their upstream licenses; see the icon attribution document for details.
