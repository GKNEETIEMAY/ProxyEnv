# Managed remote environment — design

**Status: implemented on `v0.2.0dev`; release acceptance remains pending.** ProxyEnv creates a private, session-owned `env.sh`, loads it in ProxyEnv-managed terminals, and removes it on a normal disconnect. Advanced can copy a token-free command that explicitly sources this file in an external remote terminal.

## Session scope

Owned location: `~/.proxyenv/sessions/<session-id>/env.sh`, created after the authenticated relay and SSH reverse forward are verified. An explicit replacement connection creates a new session and rotates its credentials; silent recovery of the same SSH transport retains the session, ports and credentials so its existing consumers keep working. An active-proxy change pauses recovery instead of silently retargeting it. Only ProxyEnv-launched terminals explicitly source the verified file. ProxyEnv does not edit `.bashrc`, `.profile` or `/etc/environment`; an already-running shell or VS Code Server does not inherit new exports retroactively.

Generate authenticated `HTTP_PROXY`/`HTTPS_PROXY` for HTTP, authenticated `ALL_PROXY` with `socks5h` for SOCKS5, or all three for Mixed; clear stale managed values first. `NO_PROXY` protects loopback. Values and shell quoting derive from validated backend-owned endpoints, never free-form frontend commands. The directory and file are private, user-owned, non-symlink state. Each file carries a session-specific ProxyEnv ownership marker; update or removal fails closed on a missing marker or structural conflict. Disconnect revokes the local relay before best-effort remote cleanup, so a cleanup failure leaves no usable route token.

A future persistent shell mode would require separate advanced preview, backup, marker, apply, verification and conflict-safe restore. It is not part of the default workflow or this v0.2 plan.

Both uppercase and lowercase proxy names are set consistently. Unsupported protocol variables are cleared in both cases. Existing uppercase/lowercase bypass lists are combined with loopback and exported as both `NO_PROXY` and `no_proxy`, so the AI route is not sent through the authenticated General Proxy by clients honoring these variables.

### Usage

| Mode | Enable | Next step |
| --- | --- | --- |
| AI only | AI Route; leave General Proxy off | Enable Codex/Claude access and use its managed configuration. No general-proxy password or export is needed. Existing unrelated proxy environment in the consumer still needs loopback bypass. |
| Network only | General Proxy; leave AI Route off | Launch the managed proxy terminal. New programs inherit session authentication. |
| Both | General Proxy and AI Route | Enable the desired AI access and launch CLI programs from the managed terminal. General requests use the authenticated proxy; loopback AI requests bypass it and use their separate session header. |

For an external terminal, copy and run the terminal setup command before starting a new CLI. **Open in VS Code** temporarily disables inherited proxy fields for the detected remote account's VS Code Server and adds loopback bypasses, without changing local VS Code settings or storing a proxy username/password. Codex and Claude use the separately authenticated AI Route. Reload an existing remote window. This reversible transaction is separate from the shell environment: see [Authenticated relay](AUTHENTICATED_RELAY.md#automatic-vs-code-network-setup).

Manual General Proxy credentials appear only in a collapsed Advanced fallback for other clients. Use `proxyenv` and the explicitly copied session password **only when the dialog refers to this remote listener**. Never substitute an SSH password or AI provider key; do not remember the credential. VS Code and AI tools do not require this fallback.
