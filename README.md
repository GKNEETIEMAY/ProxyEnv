<p align="center"><img src="assets/icon.svg" width="92" height="92" alt="ProxyEnv icon"></p>
<h1 align="center">ProxyEnv</h1>
<p align="center">Windows Application Network Environment Diagnostic and Configuration Assistant.<br>Discover local proxy clients, understand which network layer an app may use, and choose an explicit, reversible action.</p>
<p align="center"><strong>English</strong> · <a href="README.zh-CN.md">简体中文</a></p>

<p align="center">
  <img alt="Platform" src="https://img.shields.io/badge/platform-Windows%2010%20%7C%2011-0078D4?style=flat-square">
  <img alt="Tauri" src="https://img.shields.io/badge/Tauri-2-24C8DB?style=flat-square&logo=tauri&logoColor=white">
  <img alt="Rust" src="https://img.shields.io/badge/Rust-stable-000000?style=flat-square&logo=rust">
  <img alt="License" src="https://img.shields.io/badge/license-MIT-22c55e?style=flat-square">
  <a href="https://github.com/GKNEETIEMAY/ProxyEnv/releases/latest"><img alt="Release" src="https://img.shields.io/badge/release-v0.1.0%20stable-22c55e?style=flat-square"></a>
</p>

> [!IMPORTANT]
> **ProxyEnv v0.1.0 is the first stable release.** Windows 10/11 x64 is the implemented and tested target. Linux and macOS remain architectural directions rather than supported platforms; other Unix variants are out of scope.

## Download

Download the latest stable version from [GitHub Releases](https://github.com/GKNEETIEMAY/ProxyEnv/releases/latest):

- `ProxyEnv_x.x.x_x64-setup.exe` — recommended installer for most users
- `ProxyEnv_x.x.x_x64_en-US.msi` — MSI installer for managed Windows environments
- `ProxyEnv-x.x.x-windows-x64-portable.exe` — portable executable

In these file-name templates, `x.x.x` represents the release version, such as `0.1.0`.

ProxyEnv intentionally ships without Windows Authenticode. Windows may therefore show **Unknown Publisher**, **Microsoft Defender SmartScreen**, or **Windows protected your PC**. Download only from this repository and verify the files using the accompanying `SHA256SUMS.txt` and GitHub Artifact Attestation. The **About** page can check GitHub Releases for a newer version; ProxyEnv does not automatically download or execute updates.

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

- Detect local proxy clients, listener PIDs, actual ports, and HTTP/SOCKS5/Mixed protocols; when several clients exist, group endpoint candidates by process, show the active/total client count, and page each client through the same primary view.
- Show Windows System Proxy status without modifying it.
- Model the proxy environment as `Disabled`, `Partial`, `Enabled`, or `Mismatch`.
- Apply an automatically detected proxy address or a manually entered host, port, and protocol.
- Map protocols correctly: HTTP variables use `http://`; `ALL_PROXY` uses `socks5://` where applicable.
- Select which of `HTTP_PROXY`, `HTTPS_PROXY`, and `ALL_PROXY` the next Apply/Sync action manages.
- Keep `NO_PROXY` read-only.
- Snapshot before changes, delete values safely, restore exact previous values, broadcast `WM_SETTINGCHANGE`, and verify Registry results.
- Run from the system tray, honor startup/window preferences, and reuse the existing window on a second launch.
- Provide Simplified Chinese, English, Japanese, and Korean interfaces with light, dark, and system themes.
- Choose a visible running application or browse to an executable, then receive a beginner-oriented network diagnosis.
- Observe TUN virtual-adapter evidence as `Not detected`, `Possible`, `Detected`, or `Unknown` without controlling TUN. A TUN-only setup with no local proxy listener is shown as a suspected proxy client and is never treated as an environment-variable endpoint.
- Launch a new application process with the active proxy or with proxy variables cleared; never inject into a running process.
- Preview, confirm, back up, verify, and conflict-safely restore a known application setting when a reviewed bundled rule exists.

## Application network assistant

The assistant follows one short path:

```text
Choose an app → read local state → explain the likely path → recommend one action
              → preview and confirm if a file write is needed → verify the result
```

It reads the active local proxy, Windows System Proxy, proxy environment, virtual-adapter evidence, and the bundled application-rule catalog. Selecting an already-running app identifies its executable; it does **not** change that process. “Launch with proxy” and “Launch directly” always start a new process with an explicit child environment.

Application rules are declarative data, not executable adapters. A rule may name exact process names, fixed user-profile-relative configuration paths, one existing field, a supported format (`JSON`, `YAML`, `TOML`, or `INI`), and a typed proxy value. If no reviewed rule exists, ProxyEnv uses the launch fallback and does not scan for unknown configuration files.

## Four observable network layers

ProxyEnv deliberately keeps these concepts separate:

| Layer | Meaning | ProxyEnv behavior |
| --- | --- | --- |
| Proxy client | A local process and listening address, such as v2rayN on `127.0.0.1:10809` | Detect and probe |
| Windows System Proxy | The Windows networking setting used by compatible applications | Read-only |
| Proxy environment | User variables inherited by newly started processes | Change only after an explicit action |
| TUN / virtual adapter | A possible OS-level traffic path that may affect apps without proxy variables | Read-only, evidence-based observation |

Windows System Proxy and TUN are independent: either one, both, or neither may be active. Windows exposes an authoritative System Proxy setting but no universal client-independent TUN switch, so TUN remains an evidence-based observation using adapter identity, operational state, and default/split-default routes. Changing environment variables does not reconfigure a proxy client, toggle Windows System Proxy, control TUN, route all machine traffic, or alter an already-running process. New applications inherit the updated address; running applications must be restarted for it to take effect. A listening proxy port only proves that the client process is available—it does not prove that Windows System Proxy or TUN routing is enabled.

## Managed values

ProxyEnv changes only the current user's values in:

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
Sync:    validate loopback endpoint → build plan → snapshot → write/delete → broadcast → verify
Failure: rollback every changed value → broadcast → verify the rollback
Refresh: detect and compare only; never write
```

Snapshots store both the prior and applied state atomically under `%LOCALAPPDATA%\ProxyEnv\snapshots\latest.json`. Restore stops without writing if another program changed a managed value. Snapshot files are size/schema/allowlist checked and links or Windows reparse points are rejected; legacy v1 snapshots are not restored.

## Supported proxy clients

ProxyEnv identifies Clash Verge Rev, v2rayN, FlClash, Hiddify, Clash Nyanpasu, Clash Party, Mihomo Party, NekoBox/NekoRay, Clash for Windows, and GUI.for.Clash. The first five use attributed upstream icons; the additional and unknown clients use the shared proxy icon. Detection failure can be bypassed with a manual host, port, and protocol.

Icons come from official upstream repositories; see [`public/proxy-clients/ATTRIBUTION.md`](public/proxy-clients/ATTRIBUTION.md).

## Compared with environment-variable tools

These tools solve adjacent problems. ProxyEnv is deliberately narrower: it focuses on proxy discovery, network-layer explanation, and safe per-launch or reviewed-rule actions.

| Tool | Recommendation | Best for | Main characteristics |
| --- | --- | --- | --- |
| **ProxyEnv** | ⭐⭐⭐⭐⭐ | Proxy conflicts and app connectivity | Detects active proxy endpoints, explains proxy/system-proxy/TUN layers, supports per-launch environments and reversible reviewed rules |
| Microsoft PowerToys – Environment Variables | ⭐⭐⭐⭐⭐ | Most Windows developers | Microsoft-maintained, modern UI, profiles, User/System variables |
| EnvStudio | ⭐⭐⭐⭐⭐ | Complex PATH and multiple dev environments | PATH drag-and-drop, deduplication, invalid-path and conflict checks, snapshot rollback |
| Envarly | ⭐⭐⭐⭐½ | Open-source and safer general edits | Diff before changes, snapshots/rollback, PATH ordering, PowerShell/Ansible export |
| Rapid Environment Editor | ⭐⭐⭐⭐ | Traditional Windows development | Mature PATH tree, error detection, backup, portable edition |
| envx | ⭐⭐⭐⭐ | Terminal/TUI users | Rust, cross-platform, snapshots/profiles, search, CLI, `.env`/JSON/YAML import and export |

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
cargo test --manifest-path src-tauri/Cargo.toml --locked
cargo clippy --manifest-path src-tauri/Cargo.toml --all-targets --locked -- -D warnings
pnpm tauri build
```

Official Windows releases intentionally do not use Authenticode under the project's open-source release policy, so Windows may show Unknown Publisher, SmartScreen, or Windows protected your PC. Download ProxyEnv only from the [official GitHub Releases page](https://github.com/GKNEETIEMAY/ProxyEnv/releases), then verify the files with `SHA256SUMS.txt` and the GitHub Artifact Attestation generated by the public-repository workflow. Builds use frozen lockfiles; automatic updates remain disabled until official Tauri Updater signature verification is configured. Manual proxy endpoints accept only `localhost`, `127.0.0.1`, or `::1`. See [release security](docs/release-security.md).

## Repository structure

```text
ProxyEnv/
├─ src/
│  ├─ app/                       # Vue shell and desktop orchestration
│  ├─ features/                  # Proxy, application-assistant, and settings UI
│  └─ shared/                    # IPC, i18n, types, and visual tokens
├─ src-tauri/src/
│  ├─ commands/                  # Thin Tauri IPC adapters
│  ├─ desktop/                   # Tray, single instance, native window
│  ├─ environment/               # Generic mutation, snapshot, broadcast, verification
│  ├─ features/proxy/            # Proxy detection, plans, state, sync/restore/disable
│  ├─ features/network_observation/ # Read-only virtual-adapter evidence
│  ├─ features/application_assistant/ # Process selection, diagnosis, launch, declarative rules
│  └─ services/                  # Persistent application settings
├─ public/proxy-clients/         # Runtime icons and attribution
└─ docs/                         # Architecture and README artwork
```

The generic Environment Core has no proxy-client or proxy-variable knowledge. Dependency rules are documented in [`docs/ARCHITECTURE.md`](docs/ARCHITECTURE.md).

## Scope and privacy

ProxyEnv is not a proxy client, VPN, subscription manager, traffic forwarder, or TUN controller. It does not control Clash/v2rayN APIs, nodes, subscriptions, proxy-client rules, Windows System Proxy, routes, drivers, or system-level environment variables. It does not inject into, terminate, or rewrite the environment of running processes.

Detection, protocol probing, TUN observation, application enumeration, rule preview, and environment management stay on the local machine. ProxyEnv never reads, saves, or manages proxy user names or passwords, subscription tokens, node credentials, other proxy authentication material, or traffic. Runtime diagnostics pass through a shared redaction boundary that removes local paths, proxy endpoints, and process details; configuration values are treated as fully sensitive. It performs no external connectivity test unless the user explicitly requests the existing proxy test, and only a user-triggered update check contacts GitHub Releases. See [`SECURITY.md`](SECURITY.md).

## Contributing

Contributions are welcome. Read [`CONTRIBUTING.md`](CONTRIBUTING.md) and preserve the dependency direction: Proxy Feature may depend on Environment Core; Environment Core must never depend on Proxy Feature.

## Contributors

- ProxyEnv maintainers and community contributors
- OpenAI Codex — implementation, testing, design, and documentation assistance

## License

ProxyEnv is released under the [MIT License](LICENSE). Third-party icons retain their upstream licenses.
