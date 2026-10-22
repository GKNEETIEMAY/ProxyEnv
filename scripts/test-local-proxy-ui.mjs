import assert from "node:assert/strict";
import { createRequire } from "node:module";
import { mkdirSync, writeFileSync } from "node:fs";
import { resolve } from "node:path";
const require = createRequire(process.env.PROXYENV_LAYOUT_PLAYWRIGHT || import.meta.url);
const { chromium } = require("playwright");
const browser = await chromium.launch({ headless: true, ...(process.env.PROXYENV_LAYOUT_BROWSER ? { executablePath: process.env.PROXYENV_LAYOUT_BROWSER } : {}) });
const output = resolve(".impeccable/review/local-v0.2.2"); mkdirSync(output, { recursive: true });
const shots = process.argv.includes("--screenshots");
try {
  const page = await browser.newPage({ viewport: { width: 880, height: 760 }, reducedMotion: "reduce" });
  const errors = []; page.on("pageerror", error => { errors.push(error.message); console.error(error.message); });
  page.on("console", message => { if (message.type() === "error") console.error(message.text()); });
  await page.goto(`${process.env.PROXYENV_LAYOUT_URL || "http://127.0.0.1:1422"}/scripts/fixtures/local-proxy.html`);
  await page.locator(".local-view-switch").waitFor();
  const views = page.locator(".local-view-switch button"), overview = views.first(), advanced = views.last();
  assert.equal(await overview.getAttribute("aria-pressed"), "true");
  assert.equal(await page.locator(".local-candidates").isVisible(), false);
  assert.equal(await page.evaluate(() => fixture.calls.length), 0, "Mounting does not test or mutate");
  assert.equal(await page.locator('.local-workspace-content').evaluate(node => getComputedStyle(node).scrollbarWidth), 'none', 'Overview has no space-consuming scroll rail');
  await advanced.click(); assert.equal(await page.locator(".local-candidate-list li").count(), 3, "Same-process ports are individually selectable");
  await page.locator(".local-candidate-list button").nth(1).click();
  assert.equal(await page.locator(".local-active-address > code").textContent(), "127.0.0.1:10808");
  assert.equal(await page.locator(".local-active-address > span").textContent(), "socks5");
  assert.deepEqual(await page.evaluate(() => fixture.calls.map(c => c[0])), ["select"], "Selection does not sync");
  assert.equal(await page.locator(".local-candidate-list button").last().isDisabled(), true);
  await overview.click(); await page.locator(".local-sync-scope button.local-text-button").click();
  assert.equal(await advanced.getAttribute("aria-pressed"), "true");
  const choices = page.locator('.variable-choice input');
  await choices.nth(1).uncheck(); assert.equal(await choices.first().isDisabled(), true, "At least one variable stays enabled");
  await choices.nth(2).check();
  assert.match(await page.locator('.local-sync-scope').innerText(), /HTTP_PROXY.*ALL_PROXY/s);
  await page.evaluate(() => { fixture.state.environment.entries[3].value = Array.from({ length: 12 }, (_, i) => `long-internal-service-${i}.example.test`).join(','); });
  const variable = page.locator('.variable-value').last();
  assert.equal(await variable.evaluate(node => getComputedStyle(node).whiteSpace), 'normal', 'Advanced variables expose their full values');
  const manual = page.locator('.local-manual');
  await manual.locator('input').first().fill('example.com'); await manual.locator('form.manual-endpoint button').click();
  assert.equal(await manual.locator('.field-error').count(), 1);
  assert.equal(await page.evaluate(() => fixture.calls.some(c => c[0] === 'inspect')), false);
  await manual.locator('input').first().fill('127.0.0.1');
  await manual.locator('form.manual-endpoint button').click();
  await page.locator('dialog[open]').waitFor();
  assert.match(await page.locator('dialog[open]').innerText(), /手动代理并写入用户环境变量/);
  await page.keyboard.press('Escape'); assert.equal(await page.evaluate(() => fixture.calls.some(c => c[0] === 'manual')), false);
  await manual.locator('form.manual-endpoint button').click(); await page.locator('dialog[open] .primary-action').click();
  assert.equal(await page.evaluate(() => fixture.calls.filter(c => c[0] === 'manual').length), 1);
  await overview.click();
  await page.locator('.local-connectivity .secondary-action').click();
  assert.equal(await page.evaluate(() => fixture.calls.filter(c => c[0] === 'test').length), 1);
  await page.evaluate(() => { fixture.state.connectivity.result = {
    state: 'reachable', endpoint: { host: '127.0.0.1', port: 10808, protocol: 'socks5' }, localListenerOk: true, protocolOk: true,
    testedAt: new Date().toISOString(), totalDurationMs: 400,
    targets: ['Google','GitHub','Cloudflare'].map((name, i) => ({ name, success: i !== 1, url: 'https://example.test', statusCode: i === 1 ? 403 : 200, durationMs: 110, errorKind: i === 1 ? 'remoteRejected' : null }))
  }; fixture.state.now = Date.now(); });
  assert.match(await page.locator('.local-connectivity-summary').innerText(), /2\/3/);
  assert.equal(await page.locator('.local-connectivity-summary .check-status').getAttribute('data-state'), 'warning');
  await page.locator('.local-connectivity-summary button').click();
  assert.equal(await page.locator('.local-targets li').count(), 3);
  assert.match(await page.locator('.local-targets').innerText(), /远端服务拒绝请求/);
  await page.evaluate(() => fixture.state.now = Date.now() + 301000);
  assert.match(await page.locator('.local-connectivity-summary').innerText(), /过期/);
  assert.equal(await page.locator('.local-connectivity-summary .check-status').getAttribute('data-state'), 'idle');
  await page.evaluate(() => { fixture.state.now = Date.now(); fixture.state.connectivity.result = null; });
  await overview.click();
  const checks = [];
  let index = 0;
  for (const locale of ['zh-CN','en','ja','ko']) for (const palette of ['warm','blue','teal','white']) for (const mode of ['light','dark']) {
    const width = [560,880,1280][index++ % 3];
    await page.setViewportSize({ width, height: 760 });
    await page.evaluate(({ locale,palette,mode }) => { fixture.state.locale=locale; fixture.theme(palette,mode); }, { locale,palette,mode });
    for (const view of [overview,advanced]) {
      await view.click();
      const geometry = await page.evaluate(() => ({ overflow: document.documentElement.scrollWidth > innerWidth,
        contentOverflow: [...document.querySelectorAll('.local-workspace-content')].some(node => node.scrollWidth > node.clientWidth + 1),
        buttons: [...document.querySelectorAll('.home-page button')].filter(node => node.getClientRects().length && !node.closest('dialog')).map(node => ({ width: node.clientWidth, scroll: node.scrollWidth })) }));
      assert.equal(geometry.overflow, false, `${locale}/${palette}/${mode}/${width}: page overflow`);
      assert.equal(geometry.contentOverflow, false, `${locale}/${palette}/${mode}/${width}: content overflow`);
      assert.ok(geometry.buttons.every(b => b.width > 0 && b.scroll <= b.width + 1), `${locale}/${width}: clipped control`);
    }
    checks.push({ locale,palette,mode,width });
  }
  await page.evaluate(() => { fixture.state.locale='zh-CN'; fixture.theme('warm','light'); });
  await page.evaluate(() => {
    fixture.state.selected=['http','https'];
    fixture.state.environment.state='enabled';
    fixture.state.environment.matchesActiveProxy=true;
    fixture.state.environment.activeProxy.candidate=fixture.state.candidates[0];
    fixture.state.environment.activeProxy.selectedCandidateId=fixture.state.candidates[0].id;
  });
  await page.setViewportSize({ width: 880, height: 760 }); await overview.click();
  await page.waitForFunction(() => document.querySelector('.local-view-switch button')?.getAttribute('aria-pressed') === 'true');
  await page.evaluate(() => new Promise(resolve => requestAnimationFrame(() => requestAnimationFrame(resolve))));
  if (shots) await page.screenshot({ path: resolve(output, 'overview.png') });
  await advanced.click();
  await page.waitForFunction(() => document.querySelector('.local-view-switch button:last-child')?.getAttribute('aria-pressed') === 'true');
  await page.evaluate(() => new Promise(resolve => requestAnimationFrame(() => requestAnimationFrame(resolve))));
  if (shots) await page.screenshot({ path: resolve(output, 'advanced.png') });
  await page.setViewportSize({ width: 560, height: 760 }); await overview.click();
  await page.waitForFunction(() => document.querySelector('.local-view-switch button')?.getAttribute('aria-pressed') === 'true');
  await page.evaluate(() => new Promise(resolve => requestAnimationFrame(() => requestAnimationFrame(resolve))));
  if (shots) await page.screenshot({ path: resolve(output, 'compact.png') });
  const scroller = page.locator('.local-workspace-content');
  await scroller.focus();
  await page.keyboard.press('End');
  await page.waitForFunction(() => document.querySelector('.local-workspace-content').scrollTop > 0);
  assert.ok(await scroller.evaluate(node => node.scrollTop > 0), 'Overview remains keyboard-scrollable without a visible rail');
  await page.locator('.local-sync-action').click();
  await page.locator('.local-environment .environment-actions button').nth(1).click();
  await page.locator('.local-environment .environment-actions button').last().click();
  assert.deepEqual(await page.evaluate(() => fixture.calls.slice(-3).map(c => c[0])), ['sync','disable','restore'], 'Reorganized actions retain their original events');
  await page.evaluate(() => fixture.state.busy=true);
  assert.equal(await page.locator('.local-sync-action').isDisabled(), true);
  assert.equal(await page.locator('.local-environment .environment-actions button').nth(1).isDisabled(), true);
  await page.evaluate(() => fixture.state.busy=false);
  await page.evaluate(() => { fixture.state.environment.activeProxy.available=false; fixture.state.environment.activeProxy.candidate.listening=false; });
  assert.equal(await page.locator('.local-connectivity .secondary-action').isDisabled(), true);
  assert.match(await page.locator('.local-active').innerText(), /10809/);
  await page.evaluate(() => { fixture.state.candidates=[]; fixture.state.environment.activeProxy.candidate=null; });
  assert.equal(await page.locator('.local-active').getByRole('button', { name: '手动配置' }).isVisible(), true);
  assert.equal(await page.evaluate(() => fixture.calls.filter(c => c[0] === 'test').length), 1, 'Expiry, view changes and unavailable state never start another test');
  assert.deepEqual(errors, []);
  writeFileSync(resolve(output,'results.json'), JSON.stringify({ checks, passed: true, evidence: 'Vue component fixture with synthetic data; no real environment writes or external tests' },null,2));
  console.log('Local UI passed: 64 locale/theme/width/view combinations; endpoint selection, variable scope, manual validation/confirmation, partial connectivity, expiry, unavailable state and no automatic test. Synthetic component data only.');
} finally { await browser.close(); }
