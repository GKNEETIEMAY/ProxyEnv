# RouteSpan brand assets

The display brand is **境枢 RouteSpan** (/ˈruːt.spæn/).
Slogan: **让本地能力，延伸到远程。** / **Extend local capabilities to remote environments.**

RouteSpan does not replace CC Switch. It extends CC Switch-managed AI routes
from your local machine to remote development environments.

## Asset roles

| Asset | Use |
| --- | --- |
| `routespan-mark-source.png` | Exact latest pale user PNG, transparent primary mark |
| `routespan-mark.png` | 256px proportional derivative; Header and About |
| `routespan-appicon-1024.png` | 1024px derivative of the latest pale PNG; sole native icon source |
| `../../public/brand/routespan-mark.png` | Identical primary PNG for public assets |
| `../../src-tauri/icons/tray.png`, `tray-20.png` | Transparent small-size tray variants |

The user's final selection on 2026-10-12 is the latest pale PNG
(`codex-clipboard-28130f12-f7b8-4ecd-973f-92242ecfc8e0.png`). It supplies Header,
About, tray and every native icon. Preserve its exact source bytes, colors,
shading, transparency and geometry; only proportional resampling is permitted.
No plate, authored SVG, other supplied image or monochrome substitute is used.
The original source hash is pinned in `scripts/test-branding.mjs`.

## Reproduction

Use Node.js and development-only `sharp` (or set `ROUTESPAN_ASSET_MODULES` to a
`package.json` beside an existing bundled `sharp` installation):

```powershell
node scripts/generate-brand-icons.mjs --review
pnpm tauri icon assets/brand/routespan-appicon-1024.png -o src-tauri/icons
```

Tauri CLI generates the Windows ICO, macOS ICNS, store and mobile variants; do
not stretch images out of proportion. No `sharp` package is added to the
application or lockfile. The optional review board is ignored test output.

Header/About use `BrandMark.vue` and the latest pale PNG, never an authored SVG or App Icon
plate. The tray uses the same PNG pending real Windows light/dark 16/20px acceptance.
Installer migration and icon cache behavior need real Windows validation.
