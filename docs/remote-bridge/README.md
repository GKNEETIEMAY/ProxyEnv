# Remote Bridge — v0.2 development

Remote Bridge connects a selected Windows local proxy or AI route to a non-root Linux SSH account through explicit, session-owned OpenSSH reverse forwards. It is not a general VPN, a proxy-client replacement, or a guarantee that every remote process can connect. **v0.2.0 is not a stable release.** Start with [scope](V0.2_SCOPE.md) and the [acceptance record](ACCEPTANCE_V0.2.md).

## Quick start (current development UI)

1. Select an available Active Proxy in Local Environment.
2. Choose a discovered OpenSSH, VS Code Remote SSH or compatible MobaXterm target in Remote Bridge. First-use host fingerprints require explicit confirmation.
3. Check SSH, inspect independent server-network, local-proxy and AI-route results, then explicitly connect the desired route. Port availability is checked again before forwarding.
4. Use the managed PowerShell/OpenSSH terminal or the advanced copyable exports for an already-open terminal. Codex and Claude each have an explicit, independently reversible shared-profile access switch.
5. Disconnect to stop the owned tunnel. Enabled remote tool configuration is retained until explicitly disabled; it is not proof of a currently working route.

Current code supports separate local/remote pages, structured discovery, interactive OpenSSH authentication, reverse forwarding, direct shared Codex/Claude profile switches, lifecycle-bound local profile sync and pure Codex request/response relay. The user has confirmed Codex VS Code Remote conversation, file editing and tool use on one real target. Other targets and the Claude extension need separate acceptance.

## Limits and safety

- General proxy and AI route are independent. SSH success, configuration readback and listener presence are not model-request verification.
- The remote forwards bind loopback, but **current development code has no per-bridge token isolation between different Unix users**. Do not treat loopback binding as a shared-host security boundary. [Authenticated relay](AUTHENTICATED_RELAY.md) is a v0.2 release gate.
- ProxyEnv does not read SSH private-key contents or Moba credentials. Interactive responses are session-scoped; only an eligible plain server password may be held as Windows-user DPAPI ciphertext for the current bridge.
- Remote config writes are limited to reviewed Codex/Claude fields, with validation, backup, ownership and conflict-safe restore. Unknown or concurrently changed managed state fails closed. Secrets, provider URLs and `auth.json` are not projected.
- MCP, additional AI tool adapters, macOS Remote Bridge and automatic runtime installation are out of scope. Skills projection, managed remote environment, manual SSH additions, Moba launch and Simple/Advanced UI are **planned, not present**.

The present UI exposes advanced port, route and diagnostic information. The proposed Simple/Advanced split is described in [UI/UX](UI_UX.md), not advertised as shipped.

## Design and validation

| Area | Document |
| --- | --- |
| Release boundary | [v0.2 scope](V0.2_SCOPE.md) |
| Session authentication and shared-host threat model | [Authenticated relay](AUTHENTICATED_RELAY.md) |
| Import, manual targets and Moba launch | [SSH connection manager](SSH_CONNECTION_MANAGER.md) |
| Session-owned shell environment | [Managed remote environment](MANAGED_REMOTE_ENVIRONMENT.md) |
| Effective local skills and remote projection | [Skills projection](SKILLS_REMOTE_PROJECTION.md) |
| Progressive disclosure and status language | [UI/UX](UI_UX.md) |
| Actual real-device and CI evidence | [Acceptance](ACCEPTANCE_V0.2.md) |
| Deferred MCP research | [MCP v0.3](MCP_V0.3.md) |

The former [implementation notes](../REMOTE_BRIDGE.md) are superseded by this entry and the current-state [architecture](../ARCHITECTURE.md). The dated [extension feasibility audit](../archive/REMOTE_BRIDGE_EXTENSION_AUDIT_2026-09-06.md) is historical, not a description of today's switches.
