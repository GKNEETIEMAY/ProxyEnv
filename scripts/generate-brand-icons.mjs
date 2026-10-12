import { createRequire } from "node:module";
import { copyFileSync, mkdirSync } from "node:fs";
import { resolve } from "node:path";

// Development-only resampling of the user's exact PNGs. No redraw, recolor,
// background removal, geometry changes or SVG substitute.
const require = createRequire(process.env.ROUTESPAN_ASSET_MODULES || import.meta.url);
const sharp = require("sharp");
const source = name => resolve(`assets/brand/routespan-${name}-source.png`);
const resize = (input, size, output) => sharp(input).resize(size, size, {
  fit: "contain", background: { r: 0, g: 0, b: 0, alpha: 0 }, kernel: "lanczos3",
}).png().toFile(output);
await resize(source("mark"), 256, "assets/brand/routespan-mark.png");
mkdirSync("public/brand", { recursive: true });
copyFileSync("assets/brand/routespan-mark.png", "public/brand/routespan-mark.png");
await resize(source("mark"), 1024, "assets/brand/routespan-appicon-1024.png");
for (const size of [20, 32]) {
  await resize(source("mark"), size, `src-tauri/icons/tray${size === 32 ? "" : "-20"}.png`);
}
if (process.argv.includes("--review")) {
  const output = resolve(".impeccable/review/brand");
  mkdirSync(output, { recursive: true });
  const layers = [{ input: await sharp({ create: { width: 840, height: 280, channels: 4, background: "#23211e" } }).png().toBuffer(), left: 0, top: 280 }];
  for (const [index, size] of [16, 20, 24, 32, 48, 64, 256].entries()) {
    const left = size === 256 ? 548 : 24 + index * 80;
    for (const row of [0, 1]) {
      const input = source("mark");
      layers.push({ input: await sharp(input).resize(size, size).png().toBuffer(), left, top: 32 + row * 280 });
    }
  }
  await sharp({ create: { width: 840, height: 560, channels: 4, background: "#fcfcfb" } })
    .composite(layers).png().toFile(resolve(output, "icon-sizes.png"));
}
console.log("Resized the latest user PNG for all RouteSpan icons; original colors and transparency preserved.");
