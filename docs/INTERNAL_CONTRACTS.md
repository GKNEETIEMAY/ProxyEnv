# RouteSpan internal contracts / 内部契约

English · [简体中文](INTERNAL_CONTRACTS.zh-CN.md)

These are durable product/compatibility constraints, not an implementation diary.
[Architecture](ARCHITECTURE.md) explains mechanisms; [release QA](RELEASE_QA_V0.2.md)
records evidence. Current source/test facts take precedence over historical plans.

## Information architecture

| Level | Surface | Contract |
| --- | --- | --- |
| Primary | Local Environment | Local capabilities and one entry to Application Assistant |
| Primary | Remote Bridge | Target/capability setup, then connected workspace |
| Context | Application Assistant | Local drill-down; return to Local Environment |
| Context | Settings | Return to the primary context that opened it |
| Global | Diagnostic Report | Modal snapshot; does not replace navigation |
| Connected remote | Overview / Advanced | Same connection, shared header and observations |

Keep two primary navigation buttons, not a separate top-level AI provider page.
Primary surfaces remain mounted across navigation. Remote presence is a small
static state dot; attention outranks connecting, which outranks connected/idle.
Optional preparation failures do not reclassify verified SSH transport by themselves.
Status requires icon/text as well as color, and all four locales share semantics.

Operational feedback uses the one NotificationCenter; form validation and persistent
workflow state stay inline. Do not resurrect page-bottom operation messages,
duplicate setup steps or an extra bridge-confirmation page. Backend revalidation
is still mandatory even when the UI does not ask for another confirmation screen.

## Brand assets

Display Brand: **境枢 RouteSpan**; English name: **RouteSpan**.
Slogan: **让本地能力，延伸到远程。** /
**Extend local capabilities to remote environments.**

The canonical source is `assets/brand/routespan-mark-source.png`. Header/About
reuse BrandMark and a proportional derivative; native/tray assets derive from the
same source. No geometry redraw, SVG substitute or color change without an explicit
brand update. [Asset rules](../assets/brand/README.md) define reproduction.
UI strings, window, tray, terminal and external artifact names use the display brand.

## Compatibility identities

Legacy compatibility identifiers retained from ProxyEnv are intentional:

| Identity | Retained value / rule |
| --- | --- |
| npm / Cargo package | `proxyenv` |
| Rust library / binary | `proxyenv_lib` / `proxyenv.exe` |
| Tauri identifier | `io.github.gkneeteiemay.proxyenv` |
| Local application data | `%LOCALAPPDATA%\\ProxyEnv` |
| Remote owned directory | `~/.proxyenv` |
| Browser storage keys | `proxyenv.remoteBridge.*` |
| AI session header | `X-ProxyEnv-Session` |
| Owned provider | `proxyenv_bridge` / legacy provider display value |
| GitHub repository | `GKNEETIEMAY/ProxyEnv` |
| Updater endpoint | `https://github.com/GKNEETIEMAY/ProxyEnv/releases/latest/download/latest.json` |
| Updater verification | Embedded public key stays pinned |
| Release-note delimiters | Existing machine-readable `proxyenv-release:*` markers |

A display-name change is not a data migration. Never mechanically replace these
values to “finish branding”. A future migration needs an explicit design, backup/
restore compatibility, installer/updater acceptance and separate approval.
Historical release notes remain byte-preserved records of their original products.

## CC Switch relationship

**CC Switch owns the route. RouteSpan supplies the span.** Provider credentials,
subscriptions, OAuth and upstream route policy belong to CC Switch/user tooling.
RouteSpan offers selected transport, managed client-profile projection and Skills
projection. It never becomes a competing provider/router control plane.

Service detection is not per-tool route-switch confirmation. Models/catalog/profile
can synchronize while enabled, but running clients may need restart/reload.
Model request transport remains opaque; do not turn text markup into guessed tool
calls or claim protocol translation. AI access does not require the general network
lane; both remote capabilities require the SSH transport.

## State and mutation contracts

Installed ≠ Configured ≠ Verified. Built-in model verification currently supports
installed Claude CLI only; Codex is checked with its actual client. A compatible plugin-only installation is not
rejected merely because a standalone CLI is missing. CLI-dependent verification
can remain unavailable. Enabled AI plus completed discovery auto-prepares compatible
tools; explicit per-tool OFF survives session refresh/recovery.

One backend-owned summary/generation provides transport, diagnostics and session
exports. Tab changes, reopening the window or copying in another view must not
destroy a valid observation/command. Copy success is not proof of execution.

Imported SSH edits create owned saved copies; removal hides imported entries.
Source config/bookmarks and private-key bodies are not rewritten/imported.
Owned config transactions preserve unrelated user fields and stop on conflicting
recovery evidence. Skill cancellation never deletes local Skills or foreign files.

Network bridging is intentionally credential-free and not per-UID isolated.
AI tokens are backend-managed; root/same-account trust cannot be solved by branding,
UI badges or loopback binding. Default shell setup is session-only, not startup-file
modification. Network enablement has a documented automatic egress check; paid
model verification is explicit. Diagnostic report generation is non-probing.

## Documentation and verification vocabulary

- **Implemented:** code exists with appropriate scoped checks.
- **Verified/Accepted:** matching environment/build/evidence was recorded.
- **Released:** a formal public release was published; a Draft is not public.
- **Stable baseline:** maintainer-designated predecessor, which may still be a Draft.

RouteSpan v0.2.0 is the latest public stable release as of 2026-10-13. The legacy
ProxyEnv v0.1.3 is the previous public release; v0.1.4 remains a Draft predecessor
baseline. Keep release status aligned in both READMEs, index, roadmap, security/release
docs and QA without uploading private acceptance records.

## Current fact inventory

The inventory was checked before consolidation; these are behavior sources,
not evidence that every real device has passed acceptance.

| Fact | Current source anchor |
| --- | --- |
| Primary/context navigation and preload | `src/app/AppShell.vue`; `scripts/test-app-navigation.mjs` |
| Display/native branding and compatibility | `scripts/test-branding.mjs`; `src-tauri/tauri.conf.json` |
| Core/post-connect separation, disconnect, automatic egress | `src-tauri/src/features/remote_bridge/mod.rs` |
| SSH PTY and host trust | `src-tauri/src/features/remote_bridge/ssh.rs` |
| Shared profile/version/installation behavior | `tool_adapter.rs`, `tool_discovery.rs`, `remote.sh` in the remote backend |
| Fixed-upstream AI session authentication | `authenticated_relay.rs` in the remote backend |
| CC Switch-linked Skills / owned removal | `skills.rs` in the remote backend |
| Bounded JSONL events / safe reports | `logging.rs`, `diagnostics.rs`; `scripts/test-diagnostic-report.mjs` |
| Draft workflow, signatures, unchanged update source | `.github/workflows/release.yml`; `src-tauri/tauri.conf.json` |
