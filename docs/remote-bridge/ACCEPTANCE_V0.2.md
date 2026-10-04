# v0.2 Remote Bridge acceptance record

This records **observed evidence**, not implementation or design intent. Last documentation review: 2026-09-29. Stable remains v0.1.4; no v0.2 release decision has been made.

## Recorded real-device observation

The user reported that, on their Windows → Linux target, Codex CLI, Codex VS Code Remote, Claude CLI and the Claude VS Code extension could use the bridge during development. Codex VS Code Remote could converse, edit files and invoke tools after runtime proxy-port adaptation. The user also observed a CC Switch-linked Skill transfer. On 2026-09-28, restarting the new build and disconnecting/reconnecting the bridge both succeeded. Opening VS Code initially showed a repeating proxy-authentication dialog until `Developer: Reload Window`; this exposed a launch-before-network-isolation ordering bug. The implementation now applies and verifies the managed remote VS Code settings before launching an existing server, but that fix still needs a real-device retest. One Codex request also recovered through its first built-in stream retry; record whether that recurs after the ordering fix instead of treating the single retry as a completed stability result. These are observations from one target/scenario, not a completed compatibility matrix. The original target's full OS/client/provider/build matrix and reproducible logs were not captured here; repeat every row for release sign-off after the latest reconnect, authenticated-relay, VS Code network-isolation and Skill-projection changes.

| Scenario | Evidence | Status |
| --- | --- | --- |
| Codex VS Code Remote conversation | User real-device report | Observed on one target |
| Codex VS Code Remote file edit | User real-device report | Observed on one target |
| Codex VS Code Remote tool call | User real-device report | Observed on one target |
| Codex CLI model request | User real-device report after correcting the selected provider credential | Observed on one target; repeat after latest changes |
| Claude CLI and Claude VS Code extension request | User real-device report | Observed on one target; repeat after latest changes |
| CC Switch-linked Skill transfer and Agent discovery | User real-device report on 2026-09-29 | Completed on the tested target; broader matrix still required |
| Streaming and provider/model change | Not recorded as an acceptance run | Pending |
| Shared-host account boundary | User reported dual-user verification on 2026-09-29 | AI Route/account-owned files observed isolated; General Proxy remains intentionally unauthenticated and root remains a privileged administrator |
| SSH host key and password/KBI automatic bridge construction | Implementation and automated coverage present | Real-device acceptance still pending |
| MobaXterm launch | Application-level detection exposes a connected-Overview action for imported bookmarks and compatible non-Moba targets; setup remains selection-only | Real-device launch from the latest build still pending |
| Disconnect/reconnect and conflict-safe restore | Restart and one disconnect/reconnect succeeded on 2026-09-28; the 2026-09-29 several-minute recovery regression led to direct tunnel recovery and a bounded 1/2/4/8/15/30-second backoff | Partial; repeat outage/recovery and restore run pending |
| Dark theme and narrow-window layout | User real-device report on 2026-09-29 | Completed for the tested build |
| VS Code General Proxy isolation and restoration | Initial open reproduced a proxy-authentication loop until window reload; General Proxy credentials were removed and automated tests pass | Fixed in code; real-device retest pending |
| Codex stream continuity after reconnect | One request recovered on retry 1/5 after reconnect | Observe after VS Code ordering retest; repeated disconnects remain a blocker |

## Current implementation regression evidence

The 2026-09-29 UI/UX closeout adds grouped targets, remembered target selection, safe local display copies for imported connections, one-surface capability selection, one Connect action, automatic bridge construction after interactive SSH authentication, a stable compact reduced-motion-aware authentication dialog, application-aware MobaXterm actions, silent bounded reconnect state, and Overview/Advanced disclosure. Core readiness is now separated from post-connect configuration: the verified tunnel becomes usable first, while independent optional jobs update their own status and timing in the background. Relevant targeted frontend regression tests, production frontend build, Rust library tests and extension-bundle tests pass locally on Windows. Linux-only extension-helper cases remain covered by the Ubuntu CI job rather than the local Windows run. This evidence verifies implementation behavior only; it does not complete the real-device rows below.

## Release matrix to complete

Record date, Windows build, local proxy client/Active Proxy, VS Code build/Remote SSH, Linux distribution/kernel/UID context, Codex Extension/bundled Codex/CLI and Claude versions, CC Switch/provider/model, General Proxy and AI Route ports (redacted when shared), streaming, tool calls, file edit, disconnect/reconnect, ownership restore, expected/actual result and safe evidence. Keep credentials, raw paths, tokens and SSH transcripts out of this file.

The release gate still requires Windows and Linux CI, RustSec and security audit green; a real MobaXterm launch from setup and connected Overview; password/KBI authentication flowing directly into bridge construction; full disconnect/restore and reconnect cycles; repeatable timing evidence from an actual SSH target; and explicit maintainer review of the full matrix. The tested Skills discovery, shared-account boundary, dark theme and narrow layout observations remain scoped to the reported target/build. Local automated tests document implementation regressions only. The earlier `docs/REMOTE_BRIDGE.md` snapshots and mocked tests cannot substitute for live acceptance.
