import { build, preview } from "vite";
import { spawn } from "node:child_process";
import { appendFileSync, mkdirSync, writeFileSync } from "node:fs";
import { resolve } from "node:path";

const output = resolve(".impeccable/review/local-v0.2.2");
mkdirSync(output, { recursive: true });
const log = resolve(output, "ui-run.log");
writeFileSync(log, "Synthetic component tests only; no real proxy or environment writes.\n");
const site = resolve(output, "site");
await build({
  build: { outDir: site, rollupOptions: { input: [resolve("scripts/fixtures/local-proxy.html"), resolve("scripts/fixtures/application-assistant.html")] } },
});
const server = await preview({
  build: { outDir: site },
  preview: { host: "127.0.0.1", port: 1422, strictPort: true },
});
try {
  console.log("Compiled fixture server listening; checking readiness…");
  const url = "http://127.0.0.1:1422";
  const response = await fetch(`${url}/scripts/fixtures/local-proxy.html`, { signal: AbortSignal.timeout(30000) });
  if (!response.ok) throw Error(`Fixture readiness failed: HTTP ${response.status}`);
  for (const script of ["test-local-proxy-ui.mjs", "test-application-assistant-ui.mjs"]) {
    console.log(`Running ${script}`);
    await new Promise((resolveRun, reject) => {
      const child = spawn(process.execPath, [resolve("scripts", script), "--screenshots"], {
        env: { ...process.env, PROXYENV_LAYOUT_URL: url }, windowsHide: true,
        stdio: ["ignore", "pipe", "pipe"], timeout: 600000,
      });
      for (const stream of [child.stdout, child.stderr]) stream.on("data", data => {
        process.stdout.write(data);
        appendFileSync(log, data);
      });
      child.once("error", reject);
      child.once("close", (code, signal) => code === 0 ? resolveRun() : reject(Error(`${script}: exit ${code}, signal ${signal}`)));
    });
  }
} finally {
  await new Promise((resolveClose, reject) => server.httpServer.close(error => error ? reject(error) : resolveClose()));
}
