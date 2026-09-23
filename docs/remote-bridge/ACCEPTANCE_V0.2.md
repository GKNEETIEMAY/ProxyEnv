# v0.2 Remote Bridge acceptance record

This records **observed evidence**, not implementation or design intent. Last documentation review: 2026-09-23. Stable remains v0.1.4; no v0.2 release decision has been made.

## Recorded real-device observation

The user reported that, on their Windows → Linux VS Code Remote target, the Codex extension could converse, edit files and invoke tools after the runtime proxy-port adaptation. This is one target/scenario, not general extension compatibility. The original target's full OS/client/provider/build matrix and reproducible logs were not captured here; repeat for release sign-off.

| Scenario | Evidence | Status |
| --- | --- | --- |
| Codex VS Code Remote conversation | User real-device report | Observed on one target |
| Codex VS Code Remote file edit | User real-device report | Observed on one target |
| Codex VS Code Remote tool call | User real-device report | Observed on one target |
| Streaming and provider/model change | Not recorded as an acceptance run | Pending |
| Codex CLI, Claude CLI and Claude Extension model request | No full matrix recorded | Pending |
| SSH host key, password/KBI, Moba launch, shared-host relay isolation | No release matrix recorded | Pending |
| Disconnect/reconnect, conflict-safe restore and Skills | No full real-device run | Pending |

## Release matrix to complete

Record date, Windows build, local proxy client/Active Proxy, VS Code build/Remote SSH, Linux distribution/kernel/UID context, Codex Extension/bundled Codex/CLI and Claude versions, CC Switch/provider/model, General Proxy and AI Route ports (redacted when shared), streaming, tool calls, file edit, disconnect/reconnect, ownership restore, expected/actual result and safe evidence. Keep credentials, raw paths, tokens and SSH transcripts out of this file.

The release gate also requires Windows and Linux CI, RustSec and security audit green; live different-UID verification of the implemented M8 token isolation; all in-scope productization work; and explicit maintainer review of the full matrix. The earlier `docs/REMOTE_BRIDGE.md` snapshots and mocked tests cannot substitute for live acceptance.
