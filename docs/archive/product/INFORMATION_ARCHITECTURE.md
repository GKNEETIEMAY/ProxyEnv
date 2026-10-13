> [!WARNING]
> Historical document. This file records an earlier design or implementation
> checkpoint and is not a source of truth for current RouteSpan behavior.

# v0.2 information architecture — Stage 4 freeze

Implementation baseline: `v0.2.0dev`, Stage 3 commit
`89df8059d305628a3f645f5080f560db4b74b449` (local and remote matched on 2026-10-12).
Stage 4 preserves the current ProxyEnv name, logo, slogan and design tokens.
This is an internal development contract, not a replacement user README.

## Frozen hierarchy

```text
ProxyEnv
├─ Local environment (default primary workspace)
│  ├─ Proxy discovery, Active Proxy, environment management, System Proxy / TUN
│  └─ Application Assistant (contextual task; Back returns to Local)
├─ Remote bridge (primary workspace)
│  ├─ Disconnected: SSH selection + current connection + capabilities + Connect
│  └─ Connected: Overview | Advanced (two secondary views)
└─ Global tools
   ├─ Diagnostic report (dialog)
   └─ Settings (contextual global page)
```

The two primary labels remain:

| Locale | Local | Remote |
| --- | --- | --- |
| zh-CN | 本机环境 | 远程桥接 |
| en | Local environment | Remote bridge |
| ja | ローカル環境 | リモートブリッジ |
| ko | 로컬 환경 | 원격 브리지 |

AI routing and Skills remain Remote Bridge capabilities, not primary navigation.
Overview provides capability/tool/Skills state and launch actions. Advanced explains
environment configuration, verification, security boundaries, events and recovery.
Diagnostic report remains a separate global output dialog; Settings remains an icon
action. Neither becomes a third workspace. Overview/Advanced are absent from the
disconnected setup workspace; existing reconnect and attention surfaces are retained.

## Navigation state and lifetime

`AppShell.vue` keeps `primaryView: local | remote` separate from
`view: local | remote | assistant | settings`. Both default to Local. Only explicit
primary navigation changes `primaryView`. Settings and Assistant return through the
existing Back handler; Assistant is entered from Local. Ctrl/Cmd + comma opens
Settings. Escape returns to the primary workspace except when an open dialog or
editable field owns it. A diagnostic dialog closes without changing that workspace.

RemoteBridgePage stays mounted using `v-show`, including during Settings and
Assistant. Its selection, pending authentication and Overview/Advanced choice are
not reset by workspace switches. Transport polling updates status, never navigates.
The existing explicit notification-open action remains a user navigation action.
No Router, URL route or history subsystem is introduced.

## Remote primary navigation indicator

`src/app/navigation.ts` projects only transport status and reconnect state. The
Header receives a `RemoteNavState`, not the full BridgeSummary. It renders a static
6px dot with a reserved slot, preventing label movement across periodic updates.

| Priority | Condition | Indicator |
| --- | --- | --- |
| 1 | reconnect attentionRequired, or stale / unavailable / error | Amber; needs attention |
| 2 | connecting, or reconnect waiting / retrying | Blue; connecting |
| 3 | connected | Green; connected |
| 4 | otherwise (normally disconnected) | Hidden; not connected |

Priority follows the Stage 4 design: attention > connecting > connected > idle.
For simultaneous stale/unavailable/error and retrying observations, transport
attention wins. A healthy connected snapshot with a retrying observation uses blue.
There is no red disconnected dot, pulse, spinning indicator or automatic navigation.

Optional preparing/partial outcomes, session environment, VS Code configuration,
AI tool readiness, missing CLI, Skills warnings, direct internet and model-request
verification do not affect the transport indicator. Stage 3's removed aggregate
preparation warning is not reintroduced. Model verification still requires an
explicit request; a green dot does not prove network egress or model availability.

The dot is decorative (`aria-hidden`). The Remote button's accessible name contains
its localized workspace name and transport state; its title gives the same state.
Existing keyboard focus, active underline and orange active text remain unchanged.

## Regression and acceptance

```powershell
node --test scripts/test-app-navigation.mjs
node node_modules/vue-tsc/bin/vue-tsc.js --noEmit
node node_modules/vite/bin/vite.js build
```

The navigation pure tests are also included in `pnpm test:remote`. Browser regression
uses the existing local review mode and synthetic IPC:

```powershell
node scripts/test-app-navigation-ui.mjs --screenshots
```

Coverage: default Local, exactly two primary actions, background transport mapping
and stable geometry, Local/Remote → Settings → Back/Escape, Local → Assistant → Back,
dialog-owned Escape, disconnected/connected secondary navigation, retained remote
view instance, four languages and light/dark scaled logical viewports.

Verification on 2026-10-12:

- Navigation unit regressions: 6 passed, no failures or skips.
- Combined navigation, remote bridge, installation, first-connect, notification
  and diagnostic-report regressions: 97 passed, 0 failed, 37 skipped. The skipped
  cases require POSIX shell fixtures excluded on this Windows run; they are not
  counted as verified.
- Vue TypeScript check and production Vite build passed. The existing bundle-size
  advisory remains; no new dependency was added.
- Browser navigation regression passed with synthetic IPC, including all four
  languages, light/dark themes and scaled logical viewports. Compact screenshots
  were visually reviewed without navigation clipping or indicator layout shifts.
- Existing Stage 3 first-connect browser regression passed: nonblocking
  preparation, manual diagnostics, launch availability and reconnect feedback
  remain intact, with no aggregate preparation warning.
- These results describe local pre-commit validation; GitHub CI results are
  recorded separately after pushing the Stage 4 commits.

Logical viewport checks at 125% and 150% are not real Windows DPI tests. Manual
acceptance must still cover actual Windows scaling, keyboard/screen-reader use,
and a live bridge changing status while Local or Settings is visible.

## Stage 5 boundary

The primary workspaces, contextual pages and global tool hierarchy are frozen for
the v0.2 branding stage. Stage 5 may update name, logo, icons, slogan, wordmark,
window/About/installer branding and brand tokens; it must not reorder primary
navigation or split Overview, AI tools, Skills and Advanced into new workspaces.
No backend, SSH authentication, Relay, discovery or configuration transaction
change is part of Stage 4.

Stage 5 display branding is recorded in [BRAND_MIGRATION.md](BRAND_MIGRATION.md).
It changes the visible name to 境枢 RouteSpan without changing this hierarchy or
the legacy compatibility identities.
