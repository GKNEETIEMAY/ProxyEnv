# ProxyEnv Roadmap

```yaml
Current Stable: v0.1.4
Next: v0.2.0
```

Checked **Current** items are shipped. The development branch `v0.2.0dev` is not a stable release; implemented code and a single successful user test do not make every target supported. See the [Remote Bridge scope](remote-bridge/V0.2_SCOPE.md) and [acceptance record](remote-bridge/ACCEPTANCE_V0.2.md). No release date or Linux/macOS desktop-adapter schedule is promised.

## Current — v0.1.4

- [x] Proxy-client discovery, actual listener/protocol detection and unified ActiveProxyContext.
- [x] Safe user proxy-variable sync, snapshots, verified recovery and conflict protection.
- [x] Independent, read-only Windows System Proxy and TUN/virtual-adapter observation.
- [x] Conservative Application Assistant diagnosis, explicit proxy/direct new-process launch and confirmed manual-guide restart.
- [x] Redacted diagnostic report in Chinese, English, Japanese and Korean.
- [x] User-triggered signed NSIS update; MSI/Portable update manually.

## Next — v0.2.0 Remote Bridge

The development branch already has structured OpenSSH/VS Code/Moba bookmark discovery, interactive authentication, independent General Proxy/AI Route forwarding, direct shared Codex/Claude profile switches, lifecycle-bound safe profile synchronization and pure Codex traffic relay. Codex VS Code Remote conversation, file editing and tool calls have been observed by the user on one real target. Those observations do not prove every provider, model, extension or SSH configuration.

Release work still open:

- [x] M8 per-session authenticated local relay, credential delivery, rotation/revocation and automated security tests. Real different-UID shared-host acceptance remains open.
- [ ] SSH connection manager: import management, manual add, automatic OpenSSH or selected identity file; compatible MobaXterm launch.
- [ ] Managed remote session environment create/update/remove without default shell-startup edits.
- [ ] Effective local Skills detection and conflict-safe remote projection/restore.
- [ ] Simple default / Advanced UI with secure progressive disclosure.
- [ ] Real-device Claude and Codex matrix, provider changes, streaming, disconnect/reconnect and recovery.
- [ ] Windows/Linux CI, RustSec/security audit, documentation and release acceptance.

The Codex VS Code Remote observation is **accepted for those three behaviors on one target**; Claude Extension remains pending. Moba launch, M8 shared-host acceptance, Skills and Simple/Advanced UI are pending. Do not describe both extensions as wholly untested or either as universally accepted.

## Future — v0.3 research

MCP remote bridging (HTTP/SSE/Streamable HTTP before stdio compatibility) and further CC Switch-compatible tool adapters may be designed later. Runtime auto-installation, secret copying, protocol conversion and dependency on CC Switch's database are not planned. Linux/macOS desktop platform adapters have no committed date; other Unix variants remain outside scope.

ProxyEnv's stable local product boundary remains narrow: make Windows proxy layers visible and controllable, with reversible actions and explicit proxy/direct launches. Remote Bridge extends that boundary only through reviewed, user-initiated SSH forwarding and owned remote configuration.
