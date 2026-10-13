# RouteSpan 品牌资产

[English](README.md) · 简体中文

展示品牌：**境枢 RouteSpan**。标语：**让本地能力，延伸到远程。** /
**Extend local capabilities to remote environments.**

## 唯一源文件

`routespan-mark-source.png` 是唯一透明 PNG 源，SHA-256 为
`98d69dcf748bafcbdc91deffffbfbe7867e2131fabac450104e36b1933506ff2`，
由 `scripts/test-branding.mjs` 检查。

## 衍生运行时资源

| 资源 | 用途 |
| --- | --- |
| [routespan-mark.png](routespan-mark.png) | 256px Header/About 标识 |
| [routespan-appicon-1024.png](routespan-appicon-1024.png) | 1024px 原生图标源 |
| [公开标识](../../public/brand/routespan-mark.png) | 字节一致的公开镜像 |
| [tray.png](../../src-tauri/icons/tray.png)、[tray-20.png](../../src-tauri/icons/tray-20.png) | 32px / 20px 透明托盘变体 |
| [原生图标](../../src-tauri/icons/icon.ico) | Tauri 派生的 Windows/商店/移动图标族 |

Header/About 复用 `BrandMark.vue`。未经明确品牌更新，不用底板或 SVG 替代，不重画几何、
不改颜色/透明度、不换来源。衍生资源仅等比例重采样。

## 复现

使用 Node.js 和仅开发时的 `sharp` 安装，或通过 `ROUTESPAN_ASSET_MODULES` 指向其 package.json：

```powershell
node scripts/generate-brand-icons.mjs --review
pnpm tauri icon assets/brand/routespan-appicon-1024.png -o src-tauri/icons
pnpm test:brand
```

资产生成不增加运行时依赖，审阅输出被 Git 忽略。
原生托盘/任务栏/图标缓存及 Windows 缩放渲染需独立实机验收；PNG 尺寸/hash 检查不足以证明。
