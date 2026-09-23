# Managed remote environment — design

**Status: implemented on `v0.2.0dev`; release acceptance remains pending.** ProxyEnv creates a private, session-owned `env.sh`, loads it only in ProxyEnv-managed terminals, and removes it on a normal disconnect. Advanced token-free `export` commands remain available for external terminals.

## Session scope

Owned location: `~/.proxyenv/sessions/<session-id>/env.sh`, created after the authenticated relay and SSH reverse forward are verified. A reconnect creates a new session and rotates its credentials; an active-proxy change marks the current bridge stale instead of silently retargeting it. Only ProxyEnv-launched terminals explicitly source the verified file. ProxyEnv does not edit `.bashrc`, `.profile` or `/etc/environment`; an already-running shell or VS Code Server does not inherit new exports retroactively.

Generate authenticated `HTTP_PROXY`/`HTTPS_PROXY` for HTTP, authenticated `ALL_PROXY` with `socks5h` for SOCKS5, or all three for Mixed; clear stale managed values first. `NO_PROXY` protects loopback. Values and shell quoting derive from validated backend-owned endpoints, never free-form frontend commands. The directory and file are private, user-owned, non-symlink state. Each file carries a session-specific ProxyEnv ownership marker; update or removal fails closed on a missing marker or structural conflict. Disconnect revokes the local relay before best-effort remote cleanup, so a cleanup failure leaves no usable route token.

A future persistent shell mode would require separate advanced preview, backup, marker, apply, verification and conflict-safe restore. It is not part of the default workflow or this v0.2 plan.
