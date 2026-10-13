# RouteSpan brand assets

English · [简体中文](README.zh-CN.md)

Display brand: **境枢 RouteSpan**. Slogan: **让本地能力，延伸到远程。** /
**Extend local capabilities to remote environments.**

## Canonical source

`routespan-mark-source.png` is the canonical transparent PNG. Its SHA-256 is
`98d69dcf748bafcbdc91deffffbfbe7867e2131fabac450104e36b1933506ff2`,
enforced by `scripts/test-branding.mjs`.

## Derived runtime assets

| Asset | Role |
| --- | --- |
| [routespan-mark.png](routespan-mark.png) | 256px Header/About mark |
| [routespan-appicon-1024.png](routespan-appicon-1024.png) | 1024px native icon source |
| [Public mark](../../public/brand/routespan-mark.png) | Byte-identical public mirror |
| [tray.png](../../src-tauri/icons/tray.png), [tray-20.png](../../src-tauri/icons/tray-20.png) | 32px / 20px transparent tray variants |
| [Native icons](../../src-tauri/icons/icon.ico) | Tauri-derived Windows/store/mobile family |

Header/About reuse `BrandMark.vue`. Do not substitute a background plate or SVG,
redraw geometry, change colors/transparency, or use another source without an
explicit brand update. Derivatives use proportional resampling only.

## Reproduction

Use Node.js and a development-only `sharp` installation, or point
`ROUTESPAN_ASSET_MODULES` at its package.json:

```powershell
node scripts/generate-brand-icons.mjs --review
pnpm tauri icon assets/brand/routespan-appicon-1024.png -o src-tauri/icons
pnpm test:brand
```

No runtime dependency is added for asset generation. Review output is ignored.
Native tray/taskbar/icon caches and scaled Windows rendering require separate
real-device acceptance; PNG size/hash tests alone do not prove that behavior.
