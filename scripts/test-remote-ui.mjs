import { spawn } from "node:child_process";
import { mkdirSync, mkdtempSync } from "node:fs";
import { join, resolve } from "node:path";

const output = resolve(process.env.ROUTESPAN_REMOTE_UI_ARTIFACTS || ".impeccable/review/remote-ui");
mkdirSync(output, { recursive: true });
const site = mkdtempSync(join(output, "site-"));
// The development build exposes the existing synthetic review data to the test.
// Playwright serves only these assets through request interception, without a listener.
process.env.NODE_ENV = "development";
const { build } = await import("vite");
await build({ mode: "development", build: { outDir: site, emptyOutDir: false } });

const env = {
  ...process.env,
  PROXYENV_LAYOUT_URL: "http://127.0.0.1:1440",
  ROUTESPAN_REMOTE_UI_SITE: site,
  ROUTESPAN_M34_SCREENSHOTS: resolve(output, "windows"),
  ROUTESPAN_REMOTE_CAPABILITY_SCREENSHOTS: resolve(output, "capabilities"),
};
for (const script of ["test-windows-bridge-ui.mjs", "test-bridge-capabilities-ui.mjs"]) {
  await new Promise((resolveRun, reject) => {
    const child = spawn(process.execPath, [resolve("scripts", script), "--screenshots"], {
      env, windowsHide: true, stdio: "inherit", timeout: 180_000,
    });
    child.once("error", reject);
    child.once("close", (code, signal) => {
      if (code === 0) resolveRun();
      else reject(new Error(`${script} exited with ${code ?? signal}`));
    });
  });
}
