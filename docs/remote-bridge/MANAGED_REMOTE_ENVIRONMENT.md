# Managed remote environment — design

**Status: implemented on `v0.2.0dev`; release acceptance remains pending.** ProxyEnv creates a private, session-owned `env.sh`, loads it in ProxyEnv-managed terminals, and removes it on a normal disconnect. Advanced can copy a token-free command that explicitly sources this file in an external remote terminal.

## Session scope

Owned location: `~/.proxyenv/sessions/<session-id>/env.sh`, created after the General Proxy relay and SSH reverse forward are verified. Silent recovery of the same SSH transport retains the session and ports so existing consumers keep working. An active-proxy change pauses recovery instead of silently retargeting it. Only ProxyEnv-launched terminals explicitly source the verified file. ProxyEnv does not edit `.bashrc`, `.profile` or `/etc/environment`; an already-running shell or VS Code Server does not inherit new exports retroactively.

Generate credential-free `HTTP_PROXY`/`HTTPS_PROXY` loopback URLs for HTTP, `ALL_PROXY` with `socks5h` for SOCKS5, or all three for Mixed; clear stale managed values first. `NO_PROXY` protects loopback. Values and shell quoting derive from validated backend-owned endpoints, never free-form frontend commands. The directory and file are private, user-owned, non-symlink state. Each file carries a session-specific ProxyEnv ownership marker; update or removal fails closed on a missing marker or structural conflict. Disconnect revokes the local relay before best-effort remote cleanup.

A future persistent shell mode would require separate advanced preview, backup, marker, apply, verification and conflict-safe restore. It is not part of the default workflow or this v0.2 plan.

Both uppercase and lowercase proxy names are set consistently. Unsupported protocol variables are cleared in both cases. Existing uppercase/lowercase bypass lists are combined with loopback and exported as both `NO_PROXY` and `no_proxy`, so the separately authenticated AI route is not sent through the General Proxy by clients honoring these variables.

### Usage

Remote VS Code network setup accepts group-write permissions only for a verified user-private primary group: the group name matches the user, no other account has that primary group, and no other explicit group member exists. Bounded NSS lookups support local `files`/`systemd` sources; lookup failures, directory-backed sources, and shared groups retain strict rejection. The exception is scoped to network apply/restore and is not supplied by the client. Owner, symlink, hardlink, size, and world-write checks remain enforced, including readback and replacement. Parent directory permissions are not changed; new transaction files retain private modes. AI configuration transactions keep their existing strict policy.

Core bridge readiness does not imply background shell setup has finished. Before launching a managed proxy terminal or returning its setup command, ProxyEnv prepares the session environment if it is still pending or failed; setup errors are returned instead of handing off an unusable command. AI-only bridges can launch a plain remote terminal without requiring General Proxy.

Open in MobaXterm starts the selected supported bookmark, or opens the application for other SSH sources; it does not inject into an existing MobaXterm shell or copy SSH credentials. The connected overview now keeps the shared Copy terminal setup command action beside the MobaXterm instructions. Connect the same host and account, source the command in each new terminal, then launch tools. Codex/Claude access must be enabled independently for AI routing; opening a terminal does not enable or verify it.

| Mode | Enable | Next step |
| --- | --- | --- |
| AI only | AI Route; leave General Proxy off | Enable Codex/Claude access and use its managed configuration. Existing unrelated proxy environment in the consumer still needs loopback bypass. |
| Network only | General Proxy; leave AI Route off | Launch the managed proxy terminal. New programs inherit credential-free loopback proxy URLs. |
| Both | General Proxy and AI Route | Enable the desired AI access and launch CLI programs from the managed terminal. General requests use the credential-free proxy; loopback AI requests bypass it and use their separate session header. |

For an external terminal, copy and run the terminal setup command before starting a new CLI. **Open in VS Code** temporarily disables inherited proxy fields for the detected remote account's VS Code Server and adds loopback bypasses, without changing local VS Code settings. Codex and Claude use the separately authenticated AI Route. Reload an existing remote window. This reversible transaction is separate from the shell environment: see [Remote relay authentication boundaries](AUTHENTICATED_RELAY.md#automatic-vs-code-network-setup).

The General Proxy has no username/password flow or Advanced credential fallback. SSH login authentication and the Codex/Claude AI session header remain separate security boundaries.
