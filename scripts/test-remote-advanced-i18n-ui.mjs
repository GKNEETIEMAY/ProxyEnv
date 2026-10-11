import assert from "node:assert/strict";
import { createRequire } from "node:module";
import { mkdirSync } from "node:fs";
import { resolve } from "node:path";

const require = createRequire(process.env.PROXYENV_LAYOUT_PLAYWRIGHT || import.meta.url);
const { chromium } = require("playwright");
const browser = await chromium.launch({ headless:true, ...(process.env.PROXYENV_LAYOUT_BROWSER ? { executablePath:process.env.PROXYENV_LAYOUT_BROWSER } : {}) });
const output = process.argv.includes("--screenshots") ? resolve(".debug-tmp/advanced-i18n") : null;
if (output) mkdirSync(output,{recursive:true});
try {
  const page = await browser.newPage({ viewport:{width:1280,height:800}, reducedMotion:"reduce" });
  const errors = [];
  page.on("pageerror",error => errors.push(error.message));
  await page.goto(`${process.env.PROXYENV_LAYOUT_URL || "http://127.0.0.1:1420"}/?impeccable-review=remote-connected-advanced`);
  await page.locator(".bridge-advanced-view").waitFor();
  await page.evaluate(async () => {
    // Render the real component inside the existing app shell, with isolated
    // status fixtures. No SSH target, routing credentials or backend writes.
    const modulePath = "/src/features/remote-bridge/components/RemoteBridgeAdvancedView.vue";
    const source = await (await fetch(modulePath)).text();
    const vuePath = source.match(/from "([^\"]*\/vue\.js[^\"]*)"/)[1];
    const {createApp,reactive,nextTick,h} = await import(vuePath);
    const {default:Advanced} = await import(modulePath);
    const {remoteBridgeMessages} = await import("/src/shared/i18n/remote-bridge.ts");
    const host = document.createElement("div");
    document.querySelector(".bridge-advanced-view").replaceWith(host);
    const observation = {state:"notTested",checkedAt:null,errorCode:null,durationMs:null};
    const summary = {
      target:{id:"fixture",source:"openssh"},status:"connected",proxyStatus:"connected",ccStatus:"connected",
      proxy:{remotePort:7897},cc:{remotePort:15721},sshAuth:{method:"identityFile",authenticated:true,passwordStored:false},
      sessionEnvironmentState:"ready",vscodeState:"ready",runtimeExpectedProxyPort:7897,runtimeProxyMatch:"matched",
      codexConfigured:false,claudeConfigured:true,reconnectState:"idle",
      toolInstallations:[
        {tool:"codex",state:"missing",version:null,extensionState:"installed",extensionVersion:"26.825.2"},
        {tool:"claude",state:"installed",version:"2.1.284",extensionState:"installed",extensionVersion:"2.1.283"},
      ],
      diagnostics:{generalProxyEgress:observation,aiRouteVerification:observation},
      security:{remoteBindScope:"loopback",generalProxyAuth:"none",aiRouteAuth:"session",shellScope:"sessionOnly"},
    };
    const props = reactive({copy:remoteBridgeMessages["zh-CN"],summary,busy:false,connected:true,active:false,diagnosing:false,
      sessionCommand:'. "$HOME/.proxyenv/sessions/fixture/env.sh"',commandLoading:false,copying:false,copied:false});
    createApp({setup:() => () => h(Advanced,props)}).mount(host);
    window.setAdvancedLocale = async locale => { props.copy = remoteBridgeMessages[locale]; await nextTick(); };
    window.setAdvancedCopy = async value => { props.copied = value; await nextTick(); };
  });
  await page.evaluate(() => document.fonts.ready);
  let cases = 0;
  for (const theme of ["light","dark"]) {
    await page.evaluate(theme => document.documentElement.dataset.theme = theme,theme);
    for (const width of [1280,880,560]) {
      await page.setViewportSize({width,height:800});
      let reference;
      for (const locale of ["zh-CN","en","ja","ko"]) {
        await page.evaluate(locale => window.setAdvancedLocale(locale),locale);
        await page.evaluate(() => new Promise(resolve => requestAnimationFrame(resolve)));
        const result = await page.evaluate(() => {
          const view = document.querySelector(".bridge-advanced-view");
          const rect = element => { const {width,height} = element.getBoundingClientRect(); return {width,height}; };
          const rows = [...view.querySelectorAll(".advanced-status-row")];
          const bind = view.querySelector("#advanced-security-title + .advanced-status-row");
          const badge = bind.querySelector(".check-status");
          const installations = view.querySelector(".advanced-installations");
          const headers = [...installations.querySelectorAll("thead th")];
          if (headers.length !== 3 || !headers[0].textContent.trim()) throw new Error("AI tools heading must share the CLI and extension header row");
          if (headers.some(header => getComputedStyle(header).fontSize !== "10px" || header.getBoundingClientRect().top !== headers[0].getBoundingClientRect().top || header.getBoundingClientRect().height !== headers[0].getBoundingClientRect().height)) throw new Error("Installation headers must use the same compact font and row height");
          if (installations.querySelectorAll(".remote-tool-icon").length !== 2 || installations.querySelectorAll("tbody tr").length !== 2 || installations.querySelectorAll(".check-status").length !== 4) throw new Error("Each tool must occupy one row with CLI and extension columns");
          if (!["26.825.2","2.1.284","2.1.283"].every(version => installations.textContent.includes(version))) throw new Error("Installation versions are missing");
          if ([...installations.querySelectorAll('.check-status[data-state="healthy"]')].some(badge => !/^\d+(\.\d+)+$/.test(badge.textContent.trim()))) throw new Error("Installed tools should show only their version");
          if (innerWidth > 760) {
            const environment = view.querySelector(".advanced-environment").getBoundingClientRect();
            const links = view.querySelector(".advanced-links").getBoundingClientRect();
            if (Math.abs(environment.height-links.height) > 1) throw new Error("First-row cards must have equal heights");
          }
          const actionArea = view.querySelector(".advanced-links > .advanced-card-actions").getBoundingClientRect();
          const diagnosticAction = view.querySelector(".advanced-links > .advanced-card-actions button").getBoundingClientRect();
          if (Math.abs((diagnosticAction.top + diagnosticAction.height/2) - (actionArea.top + actionArea.height/2)) > 1) throw new Error("Run diagnostics must sit vertically centered in the remaining link-card space");
          return {
            badges:[...rows.map(row => row.querySelector(".check-status")),...installations.querySelectorAll(".check-status")].map(rect),
            buttons:[...view.querySelectorAll(".advanced-card button")].filter(button => button.getBoundingClientRect().height).map(rect),
            rowHeights:rows.map(row => rect(row).height),
            cards:[...view.querySelectorAll(".advanced-card")].map(rect),
            address:badge.textContent.includes("127.0.0.1"),detachedAddress:!!bind.querySelector("small"),
            fullAddressVisible:badge.querySelector("span").scrollWidth <= badge.querySelector("span").clientWidth + 1,
            horizontalOverflow:view.scrollWidth > view.clientWidth + 1,
            titles:rows.every(row => row.querySelector(".check-status").title === row.querySelector(".check-status").textContent.trim()),
          };
        });
        const label = `${theme}/${width}/${locale}`;
        assert.ok(result.address && !result.detachedAddress,`${label}: loopback address belongs inside its status badge`);
        assert.ok(result.fullAddressVisible,`${label}: loopback address must remain fully readable`);
        assert.equal(result.horizontalOverflow,false,`${label}: no horizontal overflow`);
        assert.ok(result.titles,`${label}: full status remains available on hover`);
        for (const badge of result.badges) assert.equal(badge.height,26,`${label}: status height is stable`);
        for (const button of result.buttons) assert.equal(button.height,32,`${label}: action height is stable`);
        if (!reference) reference = result;
        else {
          assert.deepEqual(result.badges,reference.badges,`${label}: badge geometry matches Chinese`);
          assert.deepEqual(result.buttons,reference.buttons,`${label}: button geometry matches Chinese`);
          assert.deepEqual(result.rowHeights,reference.rowHeights,`${label}: row geometry matches Chinese`);
          assert.deepEqual(result.cards,reference.cards,`${label}: card geometry matches Chinese`);
        }
        if (output && ["zh-CN","en"].includes(locale) && [1280,560].includes(width)) await page.screenshot({path:resolve(output,`${theme}-${width}-${locale}.png`)});
        cases++;
      }
      await page.evaluate(() => window.setAdvancedCopy(true));
      const copied = await page.locator(".advanced-tool-actions button").first().boundingBox();
      await page.evaluate(() => window.setAdvancedCopy(false));
      assert.deepEqual(await page.locator(".advanced-tool-actions button").first().boundingBox(),copied,"Copy feedback retains button geometry");
    }
  }
  assert.deepEqual(errors,[]);
  console.log(`PASS: ${cases} Advanced locale/theme/viewport cases; inline loopback address; stable status, row, card and button geometry; copy feedback.`);
} finally { await browser.close(); }
