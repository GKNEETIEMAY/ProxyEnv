# v0.2.0dev closeout comparison — 2026-10-05

Baseline: `7ff1dda4db877720221f74235dc35316bea597a5` on `v0.2.0dev`, plus the current responsive-layout worktree. Stable remains v0.1.4. This is an implementation/evidence comparison with the final UI/UX plan and performance supplement, **not release approval**.

## Latest implementation takes precedence

- Setup is the latest left SSH list / right current connection and capabilities layout, not the older stacked three-step flow. Source order and per-row provenance remain OpenSSH / VS Code / MobaXterm / ProxyEnv; duplicate names are not merged.
- Add/edit share one validated form. Imported edits create ProxyEnv-owned copies; imported removals hide the list entry without rewriting source configuration. New manual connections default to password authentication.
- MobaXterm is a connected-target action only, never a setup/pre-connect launch. A supported selected bookmark opens that bookmark; another compatible target opens the detected application without injection.
- Authentication uses the latest compact, content-sized dialog rather than the old oversized fixed-height design. Terminal, response field and actions remain mounted through authentication/core bridge establishment; actual secrets clear after submit while a fixed mask preserves the visual state. Status stays at the top right.
- CC Switch detection starts with the application and caches the shared local routing service/URL, refreshing every two seconds independently of setup visibility. Setup consumes that cache without restarting detection; an established bridge keeps its own endpoint. It does not claim individual Claude/Codex route toggles are enabled; the shared tooltip/reminder explains how to enable them. An explicit user-off choice survives detection refresh.
- Temporary transport recovery stays inline and silent. Notification/sound settings remain dormant. General Proxy remains credential-free; SSH login and AI session authentication remain separate.

## Closure status

| Goal | Current evidence | Closure |
| --- | --- | --- |
| SSH source management, safe edit/hide/remove, remembered selection | Current components/store and targeted regression tests | Implemented; latest-build CRUD acceptance still required |
| Two-step capability selection → authentication → automatic bridge | Current page flow and tests; user reported one-password bridge success | Implemented and observed on one target; full key/Agent/KBI/host-key matrix pending |
| Stable authentication controls, loading action, selectable text, error treatment | Current dialog and targeted tests | Implemented; latest Windows scaling/IME/error-state review pending |
| Connected Overview/Advanced, independent capabilities and target launch actions | In-place visibility/state, capability checks and Moba tests | Implemented; latest-build Moba and full view-switch lifecycle acceptance pending |
| Maximize/restore/narrow layout | Full-width shell, remaining-height layout, contained scrolling, optional browser geometry regression | Fixed in this worktree; real Windows display-scaling acceptance remains separate |
| General Proxy without username/password | No General Proxy credential workflow; latest user reports describe removal decision | Implemented; Linux loopback is not cross-UID isolation |
| AI session boundary and Skills discovery | Existing safe relay/projection tests; user reported shared-account and Agent discovery checks | Observed on the tested target only; root/same-UID remain privileged trust boundaries |
| Full disconnect/restore, reconnect, old-token rejection, preserving user files | Ownership/recovery implementation and earlier partial real-device reports | Full repeatable release matrix pending |
| Documentation | Current UI, security and architecture docs; this comparison resolves older design wording | Updated, not proof of runtime acceptance |
| CI and security gate | Most recent visible run is an older commit, not current local HEAD | Not closed; see CI below |

## Performance supplement

| Requirement | Current implementation | Remaining work/evidence |
| --- | --- | --- |
| Core readiness independent from optional tool setup | `mod.rs` finalizes the verified core, publishes its summary, then starts `start_post_connect` | Real target T0–T8 measurements pending |
| Structured post-connect/tool states | Backend Summary exposes post-connect/environment/VS Code/tool/Skills states; UI displays them independently | Complete real-device partial-failure matrix pending |
| Phase timings without credentials/config contents | Prepare/authenticate/forward/core/tool status-preview-apply/environment/VS Code/post-connect timing records | No measured before/after or 1–2-second performance claim is available |
| Independent work in parallel | Session environment, VS Code refresh and tool refresh use separate jobs; Codex/Claude status checks run in parallel | Owned tool preview/apply operations remain serial after statuses; do not describe all writes as parallel |
| Skip unnecessary writes | Only an unconfigured, already-owned matching route enters transactional refresh | Validate unchanged versus rotated-token/profile scenarios on a target |
| Reduce remote round trips with a safe batch operation | No new combined `bridge-finalize` remote operation found | Not implemented; assess only if timings justify it, preserving transaction/ownership boundaries |
| Skills do not gate Connected | Skills management remains separate from core readiness | Skills lifecycle/visibility matrix still required for release |
| No ControlMaster/core transport rewrite | Existing OpenSSH protocol/host-key protections retained | Intentional, not a missing release feature |

## Actual CI state

The latest visible branch run at inspection was [CI 36521248220](https://github.com/GKNEETIEMAY/ProxyEnv/actions/runs/36521248220), commit `5988ceb455ddae048fe8d40109a88701a6cdf6e0` (2026-09-29). Windows failed at Rust tests and skipped the release executable build; Linux extension filesystem tests failed; RustSec and Security audit passed **for that older commit**. Access to detailed failed logs returned HTTP 403. At this review, the current local work had not yet been pushed; latest-code CI must be checked after push. Do not transfer the old audit successes or failures to unrun current code.

## Remaining release gates

1. Latest-code Windows/Linux CI and security audits green.
2. Latest-build MobaXterm launch, key/Agent/KBI/host-key authentication, and complete disconnect/restore/reconnect/token rotation matrix.
3. Actual Windows maximize/restore, 125%/150% scaling and theme checks; browser CSS-size checks are supporting evidence only.
4. Actual SSH timing samples and observed round-trip counts, with secrets/paths redacted; batch optimization is still an unimplemented supplement item.
5. Maintainer sign-off on the reproducible acceptance matrix. No push, tag or release is implied by this UI repair.
