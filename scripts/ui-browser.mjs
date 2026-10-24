import { createRequire } from "node:module";

const require = createRequire(import.meta.url);
let playwright;
try {
  playwright = require("playwright");
} catch (error) {
  if (error.code !== "MODULE_NOT_FOUND" || !process.env.PROXYENV_LAYOUT_PLAYWRIGHT) throw error;
  playwright = createRequire(process.env.PROXYENV_LAYOUT_PLAYWRIGHT)("playwright");
}
export const { chromium } = playwright;
