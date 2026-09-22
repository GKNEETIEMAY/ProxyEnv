# Managed remote environment — design

**Status: pending.** Today ProxyEnv can open a managed terminal with generated proxy environment and shows advanced `export` commands for an existing terminal. It does **not** create a remote session-owned `env.sh` file or automatically update one when the route changes.

## Session scope

Proposed owned location: `~/.proxyenv/sessions/<session-id>/env.sh`, created after connection, updated atomically when a selected route changes, and removed on disconnect. Only ProxyEnv-launched terminals/CLI explicitly source the verified file. Do not edit `.bashrc`, `.profile` or `/etc/environment` by default; an already-running shell or VS Code Server does not inherit new exports retroactively.

Generate `HTTP_PROXY`/`HTTPS_PROXY` for HTTP, `ALL_PROXY` with `socks5h` for SOCKS5, or all three for Mixed; clear stale managed values first. `NO_PROXY` protects loopback. Values and shell quoting must derive from validated backend-owned endpoints, not free-form frontend commands. Treat session IDs, symlinks, ownership, permissions, concurrent edits and disconnect failures as transaction inputs. Remove only verified ProxyEnv-owned state; never delete a foreign same-name file.

A future persistent shell mode would require separate advanced preview, backup, marker, apply, verification and conflict-safe restore. It is not part of the default workflow or this v0.2 plan.
