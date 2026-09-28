# Remote Bridge UI/UX — Simple and Advanced

The connected page explains AI-only, proxy-only and combined use. Action availability follows the required capability: AI verification follows AI health; proxy testing/managed terminal follows General Proxy health; Skills follows the live SSH session. An unrelated unavailable route must not disable healthy actions.

The default flow uses the managed terminal or Open in VS Code without manual proxy credential entry. A hint beside Open in VS Code explains that ProxyEnv temporarily isolates the remote account's VS Code Server from inherited proxy settings, keeps loopback AI routes direct, preserves local settings, and may require reloading an existing remote window. No third-party extension binary or user startup file is changed.

Advanced retains a token-free terminal setup command. Manual proxy-authentication help is a **closed-by-default disclosure** inside Advanced, with the actual remote listener, username `proxyenv`, and explicit copy buttons. It is a fallback for external clients, not a required step. Existing secondary buttons and hints are reused, without a new modal or visible password field. Four locales explain revocation, clipboard history, not remembering credentials, and the root/same-UID trust boundary. Successful copying does not claim that a running extension host has received new environment variables.

**Status: implemented on `v0.2.0dev`; real-device release acceptance remains pending.** The continuous setup/status page and shared warm ProxyEnv design remain the foundation. See `DESIGN.md` for tokens and accessibility rules.

## Simple (default)

The connected page defaults to Overview. It shows one selected server; independent SSH, local-proxy and AI-route status; Codex/Claude access state; a concise synchronized Skills count; and clear Open VS Code/Open terminal actions. Text accompanies every semantic icon. A listener or saved config is never labeled “ready” without the appropriate verification. Conflicts and failed operations remain visible with a safe next action.

Hide port numbers, `ssh -R`, `NO_PROXY`, profile/catalog hashes, provider IDs, ownership markers and session tokens in the default view. Never put a token in Advanced either. Authentication prompts remain focused, accessible dialogs. No redundant step rail or repeated connection title.

## Silent transport recovery

After an established bridge loses SSH transport, the backend attempts recovery without reopening the setup flow. It reuses the selected target, ports, live relays and session credentials. A small in-page reconnecting status is sufficient: temporary outages, retries and successful recovery produce no popup or sound. Retries back off to at most one attempt per minute. Interrupted application requests are not replayed; the user may need to retry the affected message after recovery.

Authentication failure, a changed host/configuration or a changed active proxy pauses recovery. Only this actionable pause produces one lower-right notice per recovery episode. The notice opens the remote bridge page, where the user can inspect the reason and reconnect. No password prompt opens automatically. An unfocused Windows app uses a native notification when available; the focused app uses an in-app notice. OS notification policy can suppress native delivery.

General Settings exposes **Notifications** (default on) and **Notification sound** (default off), persisted immediately. Disabling notifications also suppresses sound; the sound switch is disabled while notifications are off. The page retains the error and reconnect action regardless of notification preferences. Manual disconnect, target replacement and app exit cancel pending recovery; recovery must never recreate a user-disconnected bridge.

## Advanced

The Advanced switch reveals effective SSH source, General Proxy and AI Route separately, remote consumer vs local upstream port, runtime proxy match, token-free environment exports, bridge testing, external-client actions, launch commands, Skills controls and connection-source actions. Protected tool dialogs continue to own tool versions, extension runtime, profile/catalog state and restore diagnostics. Display the provenance and limits of each observation; independent checks update independently. Advanced inspection does not silently enable routes or write files.

Overview/Advanced uses in-place visibility changes, so toggling does not remount tool controls, Skills state or operation feedback. The segmented control exposes pressed state and keyboard focus, follows reduced-motion behavior, and ships equivalent Chinese, English, Japanese and Korean copy. Desktop and narrow review previews are covered by the development harness; dark-theme and real-device behavior remain part of release acceptance.
