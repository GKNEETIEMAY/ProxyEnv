> [!WARNING]
> Historical document. This file records an earlier design or implementation
> checkpoint and is not a source of truth for current RouteSpan behavior.

# First connection and environment preparation

Stage 3 builds on the Stage 2 CLI/extension discovery and PATH guidance. It does
not change SSH transport, host-key validation, credential lifetime, proxy relay,
AI session authentication, or the Advanced page's diagnostic behavior.

## Connection lifecycle

```text
Choose SSH target and capabilities
→ Establish bridge
→ Check SSH / allocate ports
→ If needed: one OpenSSH dialog
   Host key confirmation → Password / Keyboard Interactive → Building
→ Core bridge connected
→ Overview immediately; optional preparation continues in the background
```

Only the backend's connected summary completes the connection. SSH authentication
success alone does not. Noninteractive and interactive success use the same
`finishConnectionUi()` cleanup: release busy state, close the dialog, clear response
and mask, return to Overview, and focus the bridge heading.

The existing authentication revision isolates late check, submit, poll and finish
results after cancellation. Cancelling calls `sshAuthCancel()`, stops polling,
clears local authentication state and releases controls immediately. It preserves
the chosen target and capability preferences. A stale cancellation response cannot
reset a newer connection's state. Check-to-connect authentication continues in the
same dialog, retaining the terminal presentation and mask until a new prompt arrives.

## Core versus optional preparation

The backend already uses generation-scoped parallel preparation. Stage 3 derives a
frontend-only `RemoteReadiness` from the current `BridgeSummary`; it adds no IPC,
persistence, independent polling, probes, credentials or configuration writes.

| Preparation item | Applicable when | Existing evidence |
| --- | --- | --- |
| Terminal environment | General Proxy enabled | `sessionEnvironmentState` |
| VS Code environment check | Target supports opening VS Code | `vscodeState` |
| AI tools | AI Route enabled | Codex/Claude runtime states and CLI/extension discovery |
| Proxy actual egress | General Proxy enabled | `diagnostics.generalProxyEgress` |

Pending/preparing maps to preparing, ready to ready, and warning to attention.
An egress test in progress stays preparing independently of post-connect completion.
Failed or still-unverified egress after preparation needs attention, never “network
normal”. Non-applicable items are omitted. A missing CLI is neutral: users may only
use its extension, or neither entry. Server direct internet and model-request tests
are independent observations, not readiness gates. Skills warnings may require
attention but never prevent the core bridge from connecting.

Overall projection: preparing takes priority; then attention; otherwise ready.
It is not a copy of the backend aggregate `postConnectStatus`. Following the latest
product decision, this projection only gates once-only completion feedback; it is
not another overview panel or warning banner.

## Interaction and notifications

- Overview keeps its existing capability, AI-tool and Skills statuses, with no
  aggregate preparing/partial warning strip. Optional preparation is not a bridge
  failure and does not produce a generic “some setup needs attention” notification.
- Real failures remain in the existing operation notifications, feature-specific
  statuses and recent events. Reconnection retains its existing dedicated feedback.
- Advanced retains its existing layout and does not add installation panels.
- **Recheck** uses the existing unified diagnostic command. Its availability depends
  on an active SSH connection and the current explicit operation, not a cached local
  CC Switch “checking” flag. Cached CC observations update even while connected,
  without changing that bridge's selected capability or endpoint.
- Terminal, VS Code and MobaXterm launch buttons remain usable after core connection.
  The managed terminal retains existing lazy environment repair. External terminals
  still require explicitly copying/executing the session setup command.
- Capability and AI configuration switches retain their existing preparation guards.
- No automatic installation dialog, model request, shell-profile edit or new real
  network request is introduced by readiness rendering.
- Overview separates configuration from installation: the shared colored status
  appears above the independent CLI and VS Code extension version rows, without a
  tinted badge background. A restart/conflict/unavailable state remains a warning,
  not a successful model verification. Programmatic heading focus has no black
  outline; actionable controls keep their keyboard-focus indicators.

An explicit connect/retry arms one in-memory readiness announcement for its target.
The first stable ready snapshot uses the existing notification center once. An
attention snapshot only disarms it, without an additional generic warning. Polling,
tab changes, background configuration updates and automatic reconnection do not
replay it. Cancel, failure, disconnect and target change reset the announcement.

## Regression checks

```powershell
pnpm build
pnpm test:remote
```

`scripts/test-remote-first-connect.mjs` covers capability applicability, neutral
missing CLI/direct-internet/model states, egress independence, reconnect priority,
shared completion, stale submit/finish results, and once-only readiness feedback.
The existing remote tests retain first-host confirmation and post-auth failure
recovery checks.

With a local Vite server and Playwright/Edge configured:

```powershell
node scripts/test-remote-readiness-ui.mjs --screenshots
node scripts/test-ssh-auth-recovery-ui.mjs
node scripts/test-remote-tool-installation-ui.mjs
```

These use synthetic IPC, not a real SSH server. They check preparation/attention/
ready transitions without an aggregate warning panel, launch availability, neutral
missing CLI, AI-only mode, failure recovery, and light/dark narrow layouts.

Manual acceptance remains necessary: new host-key confirmation followed by one
password/KBI login, key/Agent login, real cancellation/retry, actual proxy/AI routes,
first VS Code Server setup, missing CLI with installed extension, and network-loss
recovery. These checks are not replaced by mocked tests. No private server settings
are changed by automated UI tests.

## Local regression record — 2026-10-12

The original Stage 3 proposal's aggregate preparing/partial notice was explicitly
removed by the later product decision. It is not an unfinished UI requirement.
The later request also removes the configuration badge's tinted background. These
decisions take precedence over the initial reference design.

| Check | Result |
| --- | --- |
| TypeScript (`vue-tsc --noEmit`) | Passed |
| Production bundle (`vite build`) | Passed; existing >500 kB bundle advisory remains |
| Remote, installation, first-connect, notifications and report tests | 91 passed, 37 POSIX fixtures skipped with an intentionally unavailable shell |
| Browser regression | All three scripts above passed with Edge/Playwright and synthetic IPC: preparation and manual recheck, post-auth recovery, configuration colors/order, focus and light/dark/narrow layouts |
| Real SSH / first host / password / KBI / key / Agent acceptance | Pending user testing; not claimed by browser mocks |
| GitHub CI | Not triggered by local commits; no push in this batch |

No Rust backend code, authentication boundary, release workflow or version number
is changed in this batch. This record is not a claim that the full Linux filesystem
suite or all Stage 3 real-device acceptance has completed.
