# Remote Bridge — v0.2 development

Remote Bridge connects a selected Windows local proxy or AI route to a non-root Linux SSH account through explicit, session-owned OpenSSH reverse forwards. It is not a general VPN, a proxy-client replacement, or a guarantee that every remote process can connect. **v0.2.0 is not a stable release.** Start with [scope](V0.2_SCOPE.md) and the [acceptance record](ACCEPTANCE_V0.2.md).

## Quick start (current development UI)

1. Select an available Active Proxy in Local Environment when General Proxy is needed.
2. In Remote Bridge, choose one target from the grouped OpenSSH, VS Code Remote, MobaXterm or Manual sections; keep the locally available General Proxy and/or AI Route capabilities enabled, then select **Connect**. First-use host fingerprints still require explicit confirmation.
3. If interactive SSH authentication succeeds, ProxyEnv automatically establishes the reverse forwards and verifies the remote loopback listeners. There is no separate review step.
4. Overview opens as soon as that core bridge is usable. Session environment, VS Code isolation, Codex, Claude and Skills preparation continue independently in the background and report their own state without blocking the tunnel.
5. Use the launch actions supported by the selected target: Open in VS Code, Open in MobaXterm, or Open terminal. Advanced contains technical diagnostics and token-free exports, not proxy credentials.
6. Disconnect to stop the owned tunnel. Enabled remote tool configuration is retained until explicitly disabled; it is not proof of a currently working route.

## Connection phases

- **Authentication:** native OpenSSH key/Agent/password/KBI flow and host-key confirmation.
- **Core Bridge:** the SSH process is alive, requested reverse forwards exist, local relays are alive and every requested remote listener is verified. This is the only gate for showing Connected.
- **Post-connect:** session environment, remote VS Code network isolation, Codex/Claude inspection or owned-route refresh, and Skills state are independent background jobs. A warning here never rewrites core transport readiness.

The bridge summary records bounded phase timings (`bridge.prepare`, `ssh.authenticate`, `bridge.forwardReady`, `coreBridge.total`, individual post-connect operations and `postConnect.total`) for local diagnostics. These measurements are not uploaded and must not be presented as real-device benchmarks unless captured from an actual target.

Current code supports separate local/remote pages, structured discovery, interactive OpenSSH authentication, reverse forwarding, direct shared Codex/Claude profile switches, lifecycle-bound local profile sync and pure Codex request/response relay. The user has confirmed Codex VS Code Remote conversation, file editing and tool use on one real target. Other targets and the Claude extension need separate acceptance.

## Limits and safety

- General proxy and AI route are independent. SSH success, configuration readback and listener presence are not model-request verification.
- The remote forwards bind loopback, but General Proxy deliberately has no username/password or per-UID isolation: another account on the same host may use it while the bridge is active. Codex and Claude use the separate session-authenticated AI Route. Validate both boundaries on a real shared host before release; see [relay authentication boundaries](AUTHENTICATED_RELAY.md).
- ProxyEnv validates a user-selected SSH private key with a bounded format-header read, without reading its body or persisting/uploading key contents. It does not read Moba credentials. Interactive responses are session-scoped; only an eligible plain server password may be held as Windows-user DPAPI ciphertext for the current bridge.
- Remote config writes are limited to reviewed Codex/Claude fields, with validation, backup, ownership and conflict-safe restore. Unknown or concurrently changed managed state fails closed. Secrets, provider URLs and `auth.json` are not projected.
- MCP, additional AI tool adapters, macOS Remote Bridge and automatic runtime installation are out of scope. Session-managed remote environment, manual SSH additions, compatible first-level MobaXterm launch, owned Skills projection and the Overview/Advanced connected UI are implemented on `v0.2.0dev`; real-device release acceptance remains pending.

The connected UI defaults to a concise Overview and preserves detailed port, route, tool and diagnostic controls behind Advanced progressive disclosure. The implemented behavior and remaining acceptance boundary are described in [UI/UX](UI_UX.md).

## Design and validation

| Area | Document |
| --- | --- |
| Release boundary | [v0.2 scope](V0.2_SCOPE.md) |
| Relay authentication boundaries and shared-host threat model | [Relay boundary design](AUTHENTICATED_RELAY.md) |
| Import, manual targets and Moba launch | [SSH connection manager](SSH_CONNECTION_MANAGER.md) |
| Session-owned shell environment | [Managed remote environment](MANAGED_REMOTE_ENVIRONMENT.md) |
| CC Switch-linked skills and remote projection | [Skills projection](SKILLS_REMOTE_PROJECTION.md) |
| Progressive disclosure and status language | [UI/UX](UI_UX.md) |
| Actual real-device and CI evidence | [Acceptance](ACCEPTANCE_V0.2.md) |
| Deferred MCP research | [MCP v0.3](MCP_V0.3.md) |

The former [implementation notes](../REMOTE_BRIDGE.md) are superseded by this entry and the current-state [architecture](../ARCHITECTURE.md). The dated [extension feasibility audit](../archive/REMOTE_BRIDGE_EXTENSION_AUDIT_2026-09-06.md) is historical, not a description of today's switches.
