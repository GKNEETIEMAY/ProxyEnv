# v0.2 Remote Bridge acceptance record

This records **observed evidence**, not implementation or design intent. Last documentation review: 2026-09-28. Stable remains v0.1.4; no v0.2 release decision has been made.

## Recorded real-device observation

The user reported that, on their Windows → Linux target, Codex CLI, Codex VS Code Remote, Claude CLI and the Claude VS Code extension could use the bridge during development. Codex VS Code Remote could converse, edit files and invoke tools after runtime proxy-port adaptation. The user also observed a CC Switch-linked Skill transfer. On 2026-09-28, restarting the new build and disconnecting/reconnecting the bridge both succeeded. Opening VS Code initially showed a repeating proxy-authentication dialog until `Developer: Reload Window`; this exposed a launch-before-network-isolation ordering bug. The implementation now applies and verifies the managed remote VS Code settings before launching an existing server, but that fix still needs a real-device retest. One Codex request also recovered through its first built-in stream retry; record whether that recurs after the ordering fix instead of treating the single retry as a completed stability result. These are observations from one target/scenario, not a completed compatibility matrix. The original target's full OS/client/provider/build matrix and reproducible logs were not captured here; repeat every row for release sign-off after the latest reconnect, authenticated-relay, VS Code network-isolation and Skill-projection changes.

| Scenario | Evidence | Status |
| --- | --- | --- |
| Codex VS Code Remote conversation | User real-device report | Observed on one target |
| Codex VS Code Remote file edit | User real-device report | Observed on one target |
| Codex VS Code Remote tool call | User real-device report | Observed on one target |
| Codex CLI model request | User real-device report after correcting the selected provider credential | Observed on one target; repeat after latest changes |
| Claude CLI and Claude VS Code extension request | User real-device report | Observed on one target; repeat after latest changes |
| CC Switch-linked Skill transfer | User real-device report | Transfer observed; Agent discovery still pending |
| Streaming and provider/model change | Not recorded as an acceptance run | Pending |
| SSH host key, password/KBI, Moba launch, shared-host relay isolation | No release matrix recorded | Pending |
| Disconnect/reconnect and conflict-safe restore | Restart and one disconnect/reconnect succeeded on 2026-09-28; conflict restore not exercised | Partial; extended reconnect and restore run pending |
| VS Code General Proxy isolation and restoration | Initial open reproduced a proxy-authentication loop until window reload; launch ordering was corrected and automated tests pass | Fixed in code; real-device retest pending |
| Codex stream continuity after reconnect | One request recovered on retry 1/5 after reconnect | Observe after VS Code ordering retest; repeated disconnects remain a blocker |

## Release matrix to complete

Record date, Windows build, local proxy client/Active Proxy, VS Code build/Remote SSH, Linux distribution/kernel/UID context, Codex Extension/bundled Codex/CLI and Claude versions, CC Switch/provider/model, General Proxy and AI Route ports (redacted when shared), streaming, tool calls, file edit, disconnect/reconnect, ownership restore, expected/actual result and safe evidence. Keep credentials, raw paths, tokens and SSH transcripts out of this file.

The release gate also requires Windows and Linux CI, RustSec and security audit green; live different-UID verification of the implemented M8 token isolation; all in-scope productization work; and explicit maintainer review of the full matrix. Local automated tests document implementation regressions only. The earlier `docs/REMOTE_BRIDGE.md` snapshots and mocked tests cannot substitute for live acceptance.
