# Remote Bridge UI/UX — Simple and Advanced

**Status: proposed, not shipped.** The existing continuous setup/status page and shared warm ProxyEnv design remain the starting point. See `DESIGN.md` for tokens and accessibility rules.

## Simple (default)

Show one selected server; independent SSH, local-proxy and AI-route status; Codex/Claude access state; a concise Skills count **only when projection exists**; and clear Open VS Code/Open terminal actions. Use text plus icon, not color alone. A listener or saved config must not be labeled “ready” without the appropriate verification. Keep a conflict or failed operation visible with a safe next action.

Hide port numbers, `ssh -R`, `NO_PROXY`, profile/catalog hashes, provider IDs, ownership markers and session tokens in the default view. Never put a token in Advanced either. Authentication prompts remain focused, accessible dialogs. No redundant step rail or repeated connection title.

## Advanced

Disclose effective SSH source, General Proxy and AI Route separately, remote consumer vs local upstream port, runtime proxy match, `NO_PROXY`, tool versions, extension runtime, profile/catalog hash state, Skills ownership/conflicts and redacted diagnostics. Display the provenance and limits of each observation; independent checks update independently. Advanced inspection does not silently enable routes or write files.

Progressive disclosure must preserve in-progress state when switching Local/Remote, avoid remount/flicker, support keyboard/focus and reduced motion, and provide equivalent Chinese, English, Japanese and Korean copy. A future live UI pass should check narrow windows and dark theme against existing component tokens before acceptance.
