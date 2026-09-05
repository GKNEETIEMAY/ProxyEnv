<p align="center"><img src="assets/icon.svg" width="92" height="92" alt="ProxyEnv icon"></p>
<h1 align="center">ProxyEnv</h1>
<p align="center"><strong>Windows Application Network Environment Diagnostic and Configuration Assistant</strong></p>
<p align="center">Make “the browser works, but developer tools cannot connect” visible, understandable, and actionable.</p>
<p align="center"><strong>English</strong> · <a href="README.zh-CN.md">简体中文</a></p>

<p align="center">
  <img alt="Platform" src="https://img.shields.io/badge/platform-Windows%2010%20%7C%2011-0078D4?style=flat-square">
  <img alt="License" src="https://img.shields.io/badge/license-MIT-22c55e?style=flat-square">
  <a href="https://github.com/GKNEETIEMAY/ProxyEnv/releases/latest"><img alt="Release" src="https://img.shields.io/badge/release-v0.1.3%20stable-22c55e?style=flat-square"></a>
</p>

<p align="center">
  <a href="https://github.com/GKNEETIEMAY/ProxyEnv/releases/latest"><strong>Download for Windows</strong></a>
  · <a href="docs/ROADMAP.md">Roadmap</a>
  · <a href="SECURITY.md">Security</a>
</p>

> [!IMPORTANT]
> **v0.1.3** is the current stable release for Windows 10/11 x64. Features under development for **v0.1.4** are not included in the current download; see the [Roadmap](docs/ROADMAP.md) for exact scope.

## What is ProxyEnv?

ProxyEnv is not another proxy client. It is a Windows application network diagnostic and configuration assistant that brings scattered system and process-launch state into one view:

```text
Local proxy clients
        ↓
Windows System Proxy
        ↓
TUN / virtual-adapter evidence
        ↓
Proxy environment variables
        ↓
Target application launch environment
```

It helps identify a stopped proxy, changed port, missing or partial environment configuration, mismatches, multiple running clients, and application launch environments that differ from the active proxy.

## Why ProxyEnv?

Your browser can connect, but Git, Claude Code, Codex, npm, pip, or another application cannot? Your proxy client is running, yet different applications still produce different results?

Windows applications may independently use System Proxy, proxy environment variables, a TUN virtual adapter, or application-specific settings. One working route does not mean every application uses the same route.

<p align="center"><img src="docs/assets/proxy-conflict.png" width="880" alt="Illustration of opposite connectivity results with HTTP_PROXY enabled and disabled"></p>

## What can ProxyEnv do?

- Discover local proxy clients and identify their listening ports and HTTP, SOCKS5, or Mixed protocols.
- Observe Windows System Proxy and TUN / virtual-adapter evidence without changing either.
- Explain proxy-environment state as `Disabled`, `Partial`, `Enabled`, or `Mismatch`.
- Use one explicit active proxy across sync, connectivity tests, the application assistant, and new-process launches.
- Test current proxy connectivity and inspect the network environment an application may receive.
- Start a new application instance with the active proxy environment or a direct environment.
- Safely sync, clear, and restore current-user proxy environment variables.

Every modifying operation follows the same model:

```text
User initiates → result is shown clearly → recovery is available when needed
```

Automatic discovery and periodic refresh stay read-only and never silently rewrite the system environment.

## What ProxyEnv does not do

ProxyEnv does **not**:

- Provide proxy nodes or manage subscriptions, accounts, passwords, or tokens.
- Forward network traffic or act as a VPN / proxy client.
- Control TUN, routes, drivers, or Windows System Proxy.
- Automatically rewrite third-party application settings.
- Inject or rewrite the environment of a running process.
- Change the system environment without an explicit user action.

## Application network assistant

The assistant and rule engine are available in Current v0.1.3. The conservative diagnosis states described below are implemented for Next v0.1.4. The bundled rule directory currently contains only its schema: the initial reviewed production rules remain pending.

The assistant follows one short path:

```text
Choose an app → read local state → explain the likely path → recommend one action
              → preview and confirm if a file write is needed → verify the result
```

It reads the active local proxy, Windows System Proxy, proxy environment, virtual-adapter evidence, and the bundled application-rule catalog. Selecting an already-running app only identifies its executable and does **not** change that process by itself. “Launch with proxy” and “Launch directly” always start a new process with an explicit child environment. The manual-proxy guide additionally offers a clearly destructive restart action: after warning about unsaved work and receiving a second confirmation, the backend revalidates the selected PID against the authorized executable, closes that one process, and starts its replacement without inherited proxy variables.

Application rules are declarative data, not executable adapters. A rule may name exact process names, fixed user-profile-relative configuration paths, one existing field, a supported format (`JSON`, `YAML`, `TOML`, or `INI`), and a typed proxy value. Only a reviewed rule whose current value is correct can confirm that an application is ready; enabled proxy environment variables mean only “environment configured, application behavior unknown.” Without a reviewed rule, ProxyEnv does not scan unknown configuration files and recommends a proxy-environment launch only when a usable local proxy exists and the environment is disabled.

## Multiple proxy clients and network observation

**Next — v0.1.4 (implemented, unreleased): unified active proxy selection.**

When multiple clients coexist, Current active proxy initially recommends one usable client and lets you explicitly switch. Environment sync, mismatch checks, connectivity tests, the assistant, proxy launches, and rule preview/apply all share that selection. Refresh never switches it: a missing proxy keeps its last-known details, becomes unavailable, and prompts reselection. Selecting a client does not write environment variables; applying a manual endpoint also makes it the global target. Selection lasts for this session; restarting ProxyEnv starts a fresh recommendation.

ProxyEnv deliberately keeps these concepts separate:

| Layer | Meaning | ProxyEnv behavior |
| --- | --- | --- |
| Proxy client | A local process and listening address, such as v2rayN on `127.0.0.1:10809` | Detect and probe |
| Windows System Proxy | The Windows networking setting used by compatible applications | Read-only |
| Proxy environment | User variables inherited by newly started processes | Change only after an explicit action |
| TUN / virtual adapter | A possible OS-level traffic path that may affect apps without proxy variables | Read-only, evidence-based observation |

Windows System Proxy and TUN are independent: either one, both, or neither may be active. Windows exposes an authoritative System Proxy setting but no universal client-independent TUN switch, so TUN remains an evidence-based observation using adapter identity, operational state, and default/split-default routes. Changing environment variables does not reconfigure a proxy client, toggle Windows System Proxy, control TUN, route all machine traffic, or alter an already-running process. New applications inherit the updated address; running applications must be restarted for it to take effect. A listening proxy port only proves that the client process is available—it does not prove that Windows System Proxy or TUN routing is enabled.

### Supported proxy clients

ProxyEnv identifies Clash Verge Rev, v2rayN, FlClash, Hiddify, Clash Nyanpasu, Clash Party, Mihomo Party, NekoBox/NekoRay, Clash for Windows, and GUI.for.Clash. Every recognized client uses an attributed upstream icon; unknown clients fall back to the shared proxy icon. Detection failure can be bypassed with a manual host, port, and protocol.

Icons come from official upstream repositories; see [`public/proxy-clients/ATTRIBUTION.md`](public/proxy-clients/ATTRIBUTION.md).

## Environment management and recovery

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

### Safe operation semantics

```text
Disable: read → snapshot → delete → broadcast → read back → verify
Restore: load latest snapshot → restore exact values → broadcast → verify
Sync:    validate loopback endpoint → build plan → snapshot → write/delete → broadcast → verify
Failure: rollback every changed value → broadcast → verify the rollback
Refresh: detect and compare only; never write
```

Snapshots store both the prior and applied state atomically under `%LOCALAPPDATA%\ProxyEnv\snapshots\latest.json`. Restore stops without writing if another program changed a managed value. Snapshot files are size/schema/allowlist checked and links or Windows reparse points are rejected; legacy v1 snapshots are not restored.

## Security and privacy design

### Diagnostic reports

**Next — v0.1.4:** implemented and locally tested, not included in the v0.1.3 download; packaged Windows acceptance remains pending.

Open **Diagnostic report** in the title bar to preview a safe, read-only snapshot and copy it into a GitHub Issue or feedback message. The report defaults to the interface language; Simplified Chinese, English, Japanese, and Korean can be selected independently. Changing language does not rerun diagnostics. **Refresh report** collects a new snapshot.

Reports include version metadata, client counts and the current selection, independent system-proxy/environment/TUN states, cached connectivity results, and a summary for the application currently selected in the assistant. Untested connectivity remains **Not tested**. No usernames, application/configuration paths, proxy addresses, credentials, node/subscription information, raw configuration values or PIDs are included. Generation does not start an external connectivity test, save a file, or upload anything; review the preview before sharing.

### Privacy boundaries

ProxyEnv is not a proxy client, VPN, subscription manager, traffic forwarder, or TUN controller. It does not control Clash/v2rayN APIs, nodes, subscriptions, proxy-client rules, Windows System Proxy, routes, drivers, or system-level environment variables. It never injects into or rewrites a running process's environment. The only process-closing exception is the manual guide's explicitly confirmed, identity-checked restart described above.

Detection, protocol probing, TUN observation, application enumeration, rule preview, and environment management stay on the local machine. ProxyEnv never reads, saves, or manages proxy user names or passwords, subscription tokens, node credentials, other proxy authentication material, or traffic. Runtime diagnostics pass through a shared redaction boundary that removes local paths, proxy endpoints, and process details; configuration values are treated as fully sensitive. It performs no external connectivity test unless the user explicitly requests the existing proxy test. A user-triggered update check contacts the fixed official GitHub endpoints; a separate Download and install action may then fetch the manifest-selected, signature-verified installer. See [`SECURITY.md`](SECURITY.md).

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

## Installation and use

Download the latest stable release from [GitHub Releases](https://github.com/GKNEETIEMAY/ProxyEnv/releases/latest):

- `ProxyEnv_x.x.x_x64-setup.exe` — recommended for most users.
- `ProxyEnv_x.x.x_x64_en-US.msi` — for managed Windows environments.
- `ProxyEnv-x.x.x-windows-x64-portable.exe` — portable edition.

`x.x.x` represents the actual version. ProxyEnv intentionally ships without Windows Authenticode, so Windows may show **Unknown Publisher**, **Microsoft Defender SmartScreen**, or **Windows protected your PC**. Download only from this repository and verify files with `SHA256SUMS.txt` and GitHub Artifact Attestation.

### Requirements

- Windows 10 version 1803 or later (x64), or Windows 11 x64
- Microsoft Edge WebView2 Runtime, normally preinstalled on supported Windows versions

Packaged builds do not require Node.js, pnpm, Rust, or Visual Studio.

The installed edition can check for and install Tauri-signed updates from About. MSI and portable editions continue to update manually through the official Release page.

## Technical architecture

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

## Development environment

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

### Verification

```powershell
pnpm build
cargo test --manifest-path src-tauri/Cargo.toml --locked
cargo clippy --manifest-path src-tauri/Cargo.toml --all-targets --locked -- -D warnings
pnpm tauri build
```

Official Windows releases intentionally do not use Authenticode under the project's open-source release policy, so Windows may show Unknown Publisher, SmartScreen, or Windows protected your PC. Download ProxyEnv only from the [official GitHub Releases page](https://github.com/GKNEETIEMAY/ProxyEnv/releases), then verify files with `SHA256SUMS.txt` and the GitHub Artifact Attestation generated by the public workflow. Release builds use frozen lockfiles and require Tauri Updater signatures. The NSIS setup edition accepts only the pinned HTTPS `latest.json`, verifies the installer with the embedded public key, rejects older or equal versions by Tauri's default comparison, installs in passive mode, and restarts after success. The private updater key exists only in the authorized Actions secret store. Manual proxy endpoints accept only `localhost`, `127.0.0.1`, or `::1`. See [release security](docs/release-security.md).

## Roadmap

```yaml
Current Stable: v0.1.3
Next: v0.1.4
```

Development status, release scope, and future direction are tracked in [`docs/ROADMAP.md`](docs/ROADMAP.md). Linux and macOS remain architectural directions with no committed delivery date; other Unix variants are out of scope.

## Contributing

Contributions are welcome. Read [`CONTRIBUTING.md`](CONTRIBUTING.md) and preserve the dependency direction: Proxy Feature may depend on Environment Core; Environment Core must never depend on Proxy Feature.

## Contributors

- ProxyEnv maintainers and community contributors
- OpenAI Codex — implementation, testing, design, and documentation assistance

## License

ProxyEnv is released under the [MIT License](LICENSE). Third-party icons retain their upstream licenses.
