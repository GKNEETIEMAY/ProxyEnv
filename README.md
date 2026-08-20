# ProxyEnv

> A lightweight Windows environment variable manager; v0.1 focuses on proxy environment toggling and automatic local proxy detection.

**ProxyEnv**, Chinese name **境启**, is a lightweight Windows desktop utility for viewing, enabling, and disabling proxy-related environment variables while automatically detecting popular local proxy clients and their actual listening ports.

ProxyEnv does not replace Clash, v2rayN, Hiddify, or other proxy clients. It does not manage subscriptions, nodes, routing rules, or proxy cores.

Version 0.1 starts with one focused problem:

> Some applications require `HTTP_PROXY`, `HTTPS_PROXY`, or `ALL_PROXY` to access the network, while other applications stop working correctly when those variables exist.

With ProxyEnv, you can temporarily remove proxy environment variables before launching an incompatible application, then restore them afterward without repeatedly editing Windows environment variables by hand.

---

## Why ProxyEnv?

Proxy behavior on Windows is inconsistent across applications.

For example:

```text
Browser
└─ Windows System Proxy
   └─ Clash / v2rayN
      └─ Works

Some CLI / API tools
└─ HTTP_PROXY / HTTPS_PROXY
   └─ Clash / v2rayN
      └─ Works

Some direct-connect applications
└─ Detect HTTP_PROXY
   └─ Force proxy usage
      └─ Connection fails
```

This creates a common conflict:

```text
HTTP_PROXY exists
├─ EasyCLIProxyAPI / CLI    ✅
└─ Some direct apps         ❌

HTTP_PROXY removed
├─ EasyCLIProxyAPI / CLI    ❌
└─ Some direct apps         ✅
```

ProxyEnv turns this workflow:

```text
Open Windows Environment Variables
→ Find HTTP_PROXY
→ Delete it
→ Launch application
→ Re-create the variable later
```

into:

```text
Disable Proxy Env
→ Launch application
→ Restore Proxy Env
```

---

## Core Features

### One-click Proxy Environment Toggle

Managed by default:

```text
HTTP_PROXY
HTTPS_PROXY
ALL_PROXY

http_proxy
https_proxy
all_proxy
```

Displayed but not removed by default:

```text
NO_PROXY
no_proxy
```

Disable removes the variables completely instead of setting empty strings.

Enable restores the last saved values whenever possible.

---

## Automatic Local Proxy Detection

ProxyEnv attempts to detect running proxy clients and their actual local listening ports.

Initial compatibility targets:

- Clash Verge Rev
- v2rayN
- FlClash
- Hiddify
- Clash Nyanpasu

Generic compatibility is also planned for clients using:

- Mihomo
- sing-box
- Xray

Detection does not rely only on hard-coded default ports.

The discovery pipeline is:

```text
Windows System Proxy
        ↓
Known Proxy Processes
        ↓
PID ↔ TCP Listening Ports
        ↓
HTTP / SOCKS5 Protocol Probe
        ↓
Client-specific Adapter
```

This allows ProxyEnv to discover custom ports whenever possible.

---

## Proxy Protocol Detection

Supported classifications:

```text
HTTP Proxy
SOCKS5 Proxy
Mixed Proxy
```

Example:

```text
Detected Proxy

Clash Verge Rev
127.0.0.1:7897
Mixed Proxy
● Listening
```

For a mixed proxy port, ProxyEnv can generate:

```text
HTTP_PROXY=http://127.0.0.1:7897
HTTPS_PROXY=http://127.0.0.1:7897
ALL_PROXY=socks5://127.0.0.1:7897
```

---

## Typical Workflow

### Normal state

```text
Proxy Environment

● ON
```

Applications that depend on proxy environment variables work normally.

### Temporarily disable proxy variables

Before launching an application that should not inherit proxy variables:

```text
ProxyEnv
→ Disable
→ Launch the application
```

After the application has started:

```text
ProxyEnv
→ Enable
```

Already-running processes generally keep the process environment they received when they were created.

---

## System Tray

ProxyEnv is designed to live primarily in the Windows system tray.

Example:

```text
🟢 Proxy Env ON
```

Left click:

```text
ON ↔ OFF
```

Context menu:

```text
ProxyEnv
────────────────
● Proxy Env ON

Detected:
Clash Verge Rev
127.0.0.1:7897

Enable
Disable
Refresh
Open
Exit
```

---

## Non-Goals

ProxyEnv is not a proxy client.

The MVP will not include:

- Node management
- Subscription management
- Clash API control
- v2rayN control
- TUN control
- Windows System Proxy switching
- Per-app proxy rules
- Automatic application launching
- Packet capture
- VPN/TUN drivers
- Domain-based routing
- Bundled proxy cores

The product boundary is intentionally narrow:

> **Manage proxy environment variables and help the user discover the active local proxy endpoint.**

---

---

## Long-term Direction

ProxyEnv is not intended to remain limited to proxy variables.

After the proxy environment toggle becomes stable, the project may grow into a general-purpose Windows environment variable manager, including:

```text
Environment variable browsing and search
PATH visualization, ordering, and invalid-entry detection
Snapshots and rollback
Developer environment profiles
JAVA_HOME / CUDA_PATH / Node / Python environment switching
API Base URL and developer-tool environment configuration
```

Design principle:

> **Stabilize the Environment Core first, then expand gradually without making the v0.1 experience heavier.**


## Tech Stack

```text
Tauri 2
Vue 3
TypeScript
Rust
windows-rs
NSIS
GitHub Actions
```

Target platforms:

```text
Windows 10 22H2 x64
Windows 11 x64
```

Planned later:

```text
Windows 11 ARM64
```

---

## Repository Layout

```text
proxyenv/
├─ README.md
├─ README.zh-CN.md
├─ PRD.md
├─ TECHNICAL_DESIGN.md
├─ src/
├─ src-tauri/
└─ .github/
```

---

## Development Roadmap

### Phase 1 — Environment Core

Implement first:

```text
Read Environment
→ Snapshot
→ Disable
→ Enable
→ WM_SETTINGCHANGE
→ Read-back Verification
```

### Phase 2 — Generic Proxy Detection

Implement:

```text
Windows System Proxy
Process Scan
TCP PID Mapping
HTTP Probe
SOCKS5 Probe
Candidate Merge
Confidence Scoring
```

### Phase 3 — Client Identification

Add identification support for:

```text
Clash Verge Rev
v2rayN
FlClash
Hiddify
Clash Nyanpasu
```

### Phase 4 — UI & Tray

Implement:

```text
Status
Toggle
Refresh
Detected Proxy
Warnings
Tray
```

### Phase 5 — Release

Implement:

```text
NSIS Installer
Portable ZIP
SHA256
GitHub Actions Release
```

---

## Development Principles

- No administrator privileges required by default.
- Only modify `HKCU\Environment`.
- Always create a snapshot before removing variables.
- Never silently overwrite the user's existing proxy values.
- Never assume a client always uses a fixed default port.
- Never scan all ports from 1–65535.
- Probe only local candidate endpoints.
- Keep all detection local.
- Never collect or upload proxy subscriptions, nodes, credentials, or API tokens.
- The main window and tray must share the same Rust core logic.

---

## Releases

Planned GitHub Release assets:

```text
ProxyEnv_x.x.x_x64-setup.exe
ProxyEnv_x.x.x_x64-portable.zip
SHA256SUMS.txt
```

Users should be able to download, install, and run ProxyEnv directly from GitHub Releases.

---

## License

Recommended license:

```text
MIT License
```

---

## Project Statement

**ProxyEnv / 境启**

> Start with one-click proxy environment toggling, then grow into a simpler and more reliable Windows environment variable manager.
