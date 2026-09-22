# Authenticated local relay — M8 design

**Status: pending.** The present reverse forwards bind `127.0.0.1`, which does not isolate Unix users on a shared Linux host. A process running under a different UID may reach that listener. Until this boundary is implemented and tested, do not claim per-user isolation.

## Threat model and invariant

The remote consumer's request must prove possession of an unpredictable bridge-session credential before the relay connects to a fixed allowlisted upstream. A different UID without that credential is rejected; the same UID shares a trust domain, and root is outside user-space isolation guarantees. A port number, loopback binding, remote PID or provider key is not authentication.

## Required lifecycle

1. Generate a fresh ≥256-bit CSPRNG token for **each** bridge session. Define a delivery mechanism usable by the managed Codex/Claude clients without placing the token in WebView state, logs, diagnostics, `auth.json` or provider API-key fields. Review file permissions and any temporary remote material before implementation.
2. Authenticate each accepted request at the local relay boundary before reaching the fixed upstream. General proxy and AI route have independent allowlisted destinations; reject missing, wrong, expired or cross-session tokens and never fall back to an unauthenticated route.
3. Revoke on disconnect, target switch, process failure and shutdown. Reconnect rotates rather than reuses credentials. Confirm that stale remote listeners/configuration cannot authenticate against a new session.
4. Keep secrets out of IPC summaries and redacted reports; test byte-accurate passthrough only after authentication, including streaming, tool calls and errors. Do not reinterpret model or response semantics.

## Release tests

Different UID without token rejected; same session's authorized consumer succeeds; wrong and old tokens rejected; disconnect revokes immediately; reconnect rotates; failure paths never log or render token; fixed upstream cannot be replaced by frontend input; concurrent sessions cannot cross-authenticate. Exercise real shared-host loopback behavior, not only mocks.
