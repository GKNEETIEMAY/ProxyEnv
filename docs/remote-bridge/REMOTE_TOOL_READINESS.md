# Remote tool readiness — Stage 2

This stage retains the 境启 / ProxyEnv brand. Installation, route configuration and request verification are independent observations; none proves the others.

## Detection and lifetime

After Core Bridge becomes Connected, a separate post-connect job uses one authenticated `tool-discover` SSH request for both CLIs, followed by the existing read-only VS Code Remote extension inspector for both plugins. Reconnect schedules the same job without delaying transport recovery. Adding a target does not log in or detect tools. The `remote_bridge_discover_tools(targetId)` API can also support a later first-Check flow without persisting results.

The bundled `tool-discover` operation reuses trusted PATH expansion, including native, npm-global and NVM directories. It returns before configuration/Python dependencies. Each CLI is invoked only with `--version`, with an eight-second execution timeout, one-second forced-kill grace and bounded output. Node version probing has a two-second timeout; the SSH operation retains its 25-second whole-operation limit. No credentials, CLI configuration, shell startup files or executable paths are read or returned. No files or persistent PATH changes are made.

The Rust parser accepts only two ordered tools and the fixed installation-capability schema. Version output must match the Codex or Claude contract exactly. Unexpected text never crosses IPC or enters events. Compatibility is decided by the existing adapters:

| Tool | Supported versions |
|---|---|
| Codex CLI | major `0`, minor `>=134`, valid patch |
| Claude Code | major `2`, version `>=2.1.227` |

Observations live in memory. Target fingerprint changes invalidate them. Connection generation and per-discovery tickets discard old results, including an earlier request for the same target. Disabling AI routing does not uninstall the CLI or erase its installation observation.

## State and interaction

| Installation | Meaning | Enable new route configuration |
|---|---|---|
| Unknown | Not checked, or discovery unavailable | No |
| Checking | Read-only detection in progress | No |
| Installed | Runnable supported CLI, normalized version | Yes, when the AI route is available |
| Missing | Command not found | No |
| Unsupported | Runnable CLI outside current supported range | No |
| Broken | Command found, but failed/timed out/unexpected output | No |

The table describes standalone CLI evidence only. Each tool also has an independent `extensionState` and `extensionVersion`; a compatible remote VS Code plugin permits enabling the shared profile even when its CLI is Missing/Unsupported/Broken. Both pending checks disable the switch. Plugins installed only on the local desktop are not remote-runtime evidence. A unique remote server context and compatible candidates are required; conflicting contexts or unsafe directories do not authorize writes. Codex uses the extension's measured bundled runtime core (including known prerelease suffixes), not its marketplace version; Claude uses its extension package version. All coexisting versions must pass the compatibility gate, without guessing the active extension. Read-only extension discovery reuses the existing proven private-group permission exception; shared groups, unknown NSS, links and world-writable paths still fail closed.

Configured + Missing/Unsupported/Broken still permits disabling existing route configuration. Missing is a neutral discovery result, not a bridge error. Unsupported/Broken are warnings. Failures of discovery do not change Core Bridge status or prevent the other tool or General Proxy from working. CLI and plugin observations retain the same fingerprint/generation/ticket guard.

The existing shared-profile preview/apply path first checks the CLI. When it is missing or unsupported, the backend obtains fresh checked plugin evidence before using the shared configuration transaction. Apply and background sync independently recheck this evidence; no frontend-provided version can bypass it. Restore does not require either entry point to remain installed. No new extension configuration engine is introduced. Claude's explicit model probe still executes a CLI, so plugin-only users do not get a misleading executable probe; this does not prevent plugin conversations.

Overview displays installation and route configuration separately. Per the user's latest direction, CLI installation status is not repeated in Advanced; its existing layout and sections remain unchanged. Diagnostic reports include only allowlisted installation states and normalized versions. Events use `toolDiscovery`; timing uses `remoteTools.discovery`. Neither includes executable paths, HOME, PATH, npm prefix or raw output.

## Installation guidance

Plugin-only regression verification: Rust library tests 276 passed / 3 ignored, strict Clippy passed, Vue type-check and production build passed. Pure frontend/report tests passed (62 before the added plugin-report privacy case); the two isolated extension-only shell transactions both passed. Existing extension transactions passed 17 tests, with three Linux-only tests skipped on Windows. Edge synthetic IPC covered enabling Codex and Claude with no CLI, optional installation guidance, normal states and unchanged Advanced layout. Actual Remote SSH extension conversations and Linux private/shared-group behavior still require device acceptance; synthetic tests do not establish that a running plugin has reloaded its configuration.

The separate installation dialog offers a reviewed static CLI catalog, stable copy feedback, an existing terminal launcher and explicit Recheck of both entry points. A detected compatible plugin explicitly explains that standalone CLI installation is optional. It never executes, pastes, installs, updates, logs in or sends a model request.

When the user has enabled the CC Switch AI routing capability, completed discovery automatically configures each compatible CLI or remote plugin using the existing shared-profile preview/apply/read-back/rollback transaction. Enabling the AI capability after discovery triggers the same setup. This includes explicit Recheck and reconnect discovery; it does not enable the AI capability itself. Missing/unsupported entries, route OFF/unavailable, stale target/generation/ticket and active manual transactions do not authorize a write. Configuration failures are recorded per tool without preventing the other tool or the network bridge from working. Successful setup seeds the existing local-profile watcher; it does not perform a model request. Explicit tool OFF is retained for this bridge session across refresh, recovery and AI capability OFF/ON; manual tool ON clears it, and a fresh connection starts a new automatic-setup session.

When General Proxy is connected, Open terminal uses the existing proxy terminal; otherwise it uses the manual terminal and explains that the server’s own network will be used. Native installation is recommended. npm and Homebrew options remain visible when prerequisites are absent. npm commands never include sudo. Both tool dialogs use the same shared npm capability snapshot. Claude's Node.js prerequisite is reported separately: an absent or old Node version never causes an installed npm command to be labeled missing.

Official commands reviewed on 2026-10-11:

| Tool | Native | npm | Homebrew |
|---|---|---|---|
| Codex | `curl -fsSL https://chatgpt.com/codex/install.sh \| sh` | `npm install -g @openai/codex` | `brew install --cask codex` |
| Claude | `curl -fsSL https://claude.ai/install.sh \| bash` | `npm install -g @anthropic-ai/claude-code` | `brew install --cask claude-code` |

Sources: [OpenAI Codex README](https://github.com/openai/codex/blob/main/README.md), [Claude Code setup](https://code.claude.com/docs/en/setup). Re-review before release; commands are not scraped at runtime. Per the user's confirmed preference, Claude npm capability checks retain the Stage 2 project baseline of Node.js 18+ with npm. The dialog separately labels the official recommendation as Node.js 22+ in all four languages. The reviewed official page currently lists 22+ as its npm requirement: retaining the project's lower guidance threshold does not guarantee that the latest package installs on Node.js 18. This does not change the CLI compatibility threshold or install Node automatically. Native installation remains recommended.

## UI contract

This is an extension of the incumbent warm desktop interface. It introduces no global tokens or visual identity changes; `DESIGN.md` and `.impeccable/design.json` remain the existing authority.

- Overview keeps compact tool tiles and a small installation/update guidance link. A supported Installed result shows its version separately from route configuration. Unknown, Checking, Missing, Unsupported and Broken remain distinct installation observations; Missing is neutral, Unsupported/Broken use warning treatment. Advanced retains its existing six cards without a CLI installation-status section; installation evidence and guidance remain in Overview.
- `RemoteToolInstallDialog.vue` uses the existing native `<dialog>`, warm semantic surfaces, fine ruled method sections and monospaced wrapping commands. Its width is `min(600px, calc(100vw - 32px))`; viewport-limited height and internal vertical scrolling keep all methods and actions reachable. Narrow layouts preserve reading order and wrap actions.
- Copy success changes the individual button to a green check with localized Copied label/title for two seconds. Opening focuses the dialog heading; native modal keyboard containment, Escape dismissal and return to the invoking control are retained. Changing the target closes the dialog. Failures appear as localized alerts. Interface labels and guidance cover Simplified Chinese, English, Japanese and Korean; catalog commands and technical values stay verbatim.
- Guidance commands are static reviewed content. Open terminal explicitly uses the existing proxy terminal when General Proxy is connected, otherwise the manual terminal with server-network guidance. Recheck is an explicit read-only action. Neither action installs, executes a catalog command, logs in, applies configuration or sends a model request. Existing configured tools can still enter protected disable/restore when installation is Missing, Unsupported or Broken; exact preview, backup and conflict-safe restore stay in the existing configuration flow.

Visual evidence: `.impeccable/review/remote-installation/dialog-light.png`, `dialog-dark.png` and `dialog-narrow.png`. These captures document layout with simulated IPC, not real SSH or installation acceptance. The earlier `advanced-installation.png` capture predates the user's decision to remove installation status from Advanced and no longer represents its current contents.

## Verification and remaining acceptance

- Rust: compatibility boundaries, strict schema, missing/broken states, fingerprint invalidation, target/generation/ticket isolation and non-error core bridge state.
- Shell fixtures: both missing/installed, failure/timeout/malformed output, native/npm-global/NVM discovery and no config creation.
- Frontend: independent tools, route enable/disable rules, four locales, static command dependencies and guidance-only boundaries.
- Browser: simulated IPC; copy/recheck, terminal choice, restored-missing configuration, keyboard focus/Esc and light/dark/narrow layouts. This is not a real SSH or installation test.

First-use host key + password/KBI, actual installation/recheck, reconnect during discovery and shared-host permissions still require manual device acceptance. Stage 3 can reuse the discovery API and snapshot without adding a second detection or configuration engine.

Existing baseline CI run `38081090890` failed at **Audit frontend dependencies** before Windows build/test. RustSec, Security audit and Remote extension Linux filesystem tests passed. Stage 2 does not modify audit or CI policy.

### Local verification record — 2026-10-11

| Check | Command | Result |
|---|---|---|
| TypeScript | `node node_modules/vue-tsc/bin/vue-tsc.js --noEmit` | Passed |
| Production bundle | `node node_modules/vite/bin/vite.js build` | Passed; existing >500 kB bundle advisory remains |
| Rust unit suite | `cargo test --manifest-path src-tauri/Cargo.toml --locked --lib` | 275 passed, 3 ignored |
| Strict Rust lint | `cargo clippy --manifest-path src-tauri/Cargo.toml --locked --all-targets -- -D warnings` | Passed |
| Rust format | `cargo fmt --manifest-path src-tauri/Cargo.toml --check` | Passed |
| Frontend/report regression | `node --test scripts/test-remote-bridge.mjs scripts/test-remote-tool-installation.mjs scripts/test-diagnostic-report.mjs` | 59 passed, 32 POSIX filesystem tests skipped in the non-POSIX run |
| Isolated CLI discovery shell fixtures | `node --test --test-name-pattern='CLI discovery' scripts/test-remote-bridge.mjs` | 2 passed using Git Bash; includes forged exit marker rejection |
| Installation UI | `node scripts/test-remote-tool-installation-ui.mjs` | Passed using Edge/Playwright with simulated IPC; light/dark/narrow, copy/recheck, independent tools, protected disable, proxy/manual terminal and keyboard focus |

Set `PROXYENV_TEST_SHELL` to a working POSIX shell for shell fixtures. The frontend-only run intentionally used an unavailable shell to separate pure tests from Linux filesystem semantics; it is not evidence that the skipped tests passed. Browser automation accepts `PROXYENV_LAYOUT_PLAYWRIGHT` and `PROXYENV_LAYOUT_BROWSER` runtime paths. Windows sandbox restrictions on Tauri build scripts, Vite realpath and Git Bash named objects required these build/shell checks to run with normal host access. No tests logged into a real remote target or installed a CLI.

### Changed-file map

Paths below are repository-relative. No changes to branding, SSH authentication, AI session authentication, release workflows or package versions are part of Stage 2.

| Area | Files | Purpose |
|---|---|---|
| Discovery and compatibility | `src-tauri/src/features/remote_bridge/tool_discovery.rs`, `tool_adapter.rs`, `remote.sh`, `ssh.rs` | Typed read-only observations, bounded version execution, strict parsing and existing compatibility gates |
| Bridge lifetime and diagnostics | `src-tauri/src/features/remote_bridge/mod.rs`, `reconnect.rs`, `events.rs` | Post-connect/reconnect discovery, fingerprint/generation/ticket isolation, cached report and safe event/timing integration |
| IPC | `src-tauri/src/commands/remote_bridge.rs`, `src-tauri/src/lib.rs` | Register asynchronous discovery command |
| Reviewed documentation links | `src-tauri/capabilities/default.json`, `src-tauri/gen/schemas/capabilities.json` | Allow only the two official documentation URLs; update generated capability metadata |
| Frontend model and rules | `src/features/remote-bridge/state.ts`, `tool-adapters.ts`, `overview-presentation.ts`, `install-guidance.ts` | Typed installation states, toggle/restore rules, localized presentation and static official command catalog |
| Frontend UI | `src/features/remote-bridge/components/RemoteToolInstallDialog.vue`, `RemoteBridgeOverview.vue`, `RemoteBridgePage.vue` | Separate guidance dialog and compact installation evidence in Overview; no new Advanced section |
| Existing event presentation | `src/features/remote-bridge/components/RemoteBridgeAdvancedView.vue` | Localize the new discovery event in the existing event list without changing Advanced layout |
| Localization and report | `src/shared/i18n/remote-bridge.ts`, `src/shared/types/index.ts`, `src/features/diagnostic-report/format.ts` | Four locale mappings and safe installation summaries |
| Regression | `scripts/remote-tool-test-modules.mjs`, `test-remote-tool-installation.mjs`, `test-remote-tool-installation-ui.mjs`, `test-remote-bridge.mjs`, `test-diagnostic-report.mjs`, `package.json` | Pure state/catalog tests, shell fixtures, browser harness, diagnostic privacy checks and normal test wiring |
| Documentation | `docs/remote-bridge/REMOTE_TOOL_READINESS.md`, `docs/remote-bridge/README.md`, `docs/ARCHITECTURE.md` | Stage 2 contract, integration map, official dependency review and outstanding real-device acceptance |
