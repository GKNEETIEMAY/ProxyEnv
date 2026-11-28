import { readFile } from "node:fs/promises";
import { extname, resolve, sep } from "node:path";

const contentTypes = {
  ".html": "text/html", ".js": "application/javascript", ".css": "text/css",
  ".png": "image/png", ".svg": "image/svg+xml", ".ico": "image/x-icon",
  ".woff2": "font/woff2", ".ttf": "font/ttf",
};

export async function createRemoteFixturePage(browser, options) {
  const page = await browser.newPage(options);
  const site = process.env.ROUTESPAN_REMOTE_UI_SITE;
  if (!site) return page;
  const root = resolve(site);
  const origin = process.env.PROXYENV_LAYOUT_URL;
  await page.route(`${origin}/**`, async route => {
    const { pathname } = new URL(route.request().url());
    const file = resolve(root, pathname === "/" ? "index.html" : `.${pathname}`);
    if (!file.startsWith(root + sep)) return route.abort();
    try {
      return route.fulfill({ contentType: contentTypes[extname(file)], body: await readFile(file) });
    } catch (error) {
      if (error.code !== "ENOENT") throw error;
      return route.fulfill({ status: 404, body: "Fixture asset absent" });
    }
  });
  return page;
}
