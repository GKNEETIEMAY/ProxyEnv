# Remote Bridge UI/UX — Simple and Advanced

**Status: implemented on `v0.2.0dev`; real-device release acceptance remains pending.** The continuous setup/status page and shared warm ProxyEnv design remain the foundation. See `DESIGN.md` for tokens and accessibility rules.

## Simple (default)

The connected page defaults to Overview. It shows one selected server; independent SSH, local-proxy and AI-route status; Codex/Claude access state; a concise synchronized Skills count; and clear Open VS Code/Open terminal actions. Text accompanies every semantic icon. A listener or saved config is never labeled “ready” without the appropriate verification. Conflicts and failed operations remain visible with a safe next action.

Hide port numbers, `ssh -R`, `NO_PROXY`, profile/catalog hashes, provider IDs, ownership markers and session tokens in the default view. Never put a token in Advanced either. Authentication prompts remain focused, accessible dialogs. No redundant step rail or repeated connection title.

## Advanced

The Advanced switch reveals effective SSH source, General Proxy and AI Route separately, remote consumer vs local upstream port, runtime proxy match, token-free environment exports, bridge testing, external-client actions, launch commands, Skills controls and connection-source actions. Protected tool dialogs continue to own tool versions, extension runtime, profile/catalog state and restore diagnostics. Display the provenance and limits of each observation; independent checks update independently. Advanced inspection does not silently enable routes or write files.

Overview/Advanced uses in-place visibility changes, so toggling does not remount tool controls, Skills state or operation feedback. The segmented control exposes pressed state and keyboard focus, follows reduced-motion behavior, and ships equivalent Chinese, English, Japanese and Korean copy. Desktop and narrow review previews are covered by the development harness; dark-theme and real-device behavior remain part of release acceptance.
