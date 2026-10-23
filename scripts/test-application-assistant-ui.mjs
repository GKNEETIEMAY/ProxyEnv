import assert from "node:assert/strict";
import { createRequire } from "node:module";
import { mkdirSync } from "node:fs";
import { resolve } from "node:path";
const require = createRequire(process.env.PROXYENV_LAYOUT_PLAYWRIGHT || import.meta.url);
const { chromium } = require("playwright");
const browser = await chromium.launch({ headless: true, ...(process.env.PROXYENV_LAYOUT_BROWSER ? { executablePath: process.env.PROXYENV_LAYOUT_BROWSER } : {}) });
const output = resolve('.impeccable/review/local-v0.2.2');
mkdirSync(output, { recursive: true });
const shots = process.argv.includes('--screenshots');
try {
  const page = await browser.newPage({ viewport: { width: 880, height: 760 }, reducedMotion: 'reduce' });
  await page.addInitScript(() => Object.defineProperty(navigator, 'clipboard', { value: { writeText: async text => { window.copiedText = text; } }, configurable: true }));
  const errors = []; page.on('pageerror', error => errors.push(error.message));
  await page.goto(`${process.env.PROXYENV_LAYOUT_URL || 'http://127.0.0.1:1422'}/scripts/fixtures/application-assistant.html`, { waitUntil: 'domcontentloaded' });
  await page.locator('.app-proxy-fields').waitFor();
  assert.deepEqual(await page.evaluate(() => fixture.calls), [], 'Opening the page does not enumerate, inspect, test, launch or write');
  assert.equal(await page.locator('.application-picker').count(), 0);
  assert.equal(await page.locator('.app-proxy-fields code').last().innerText(), 'http://[::1]:10808');
  await page.locator('.app-proxy-copy').first().click();
  assert.equal(await page.evaluate(() => window.copiedText), '::1');
  await page.locator('.app-proxy-copy').nth(1).click();
  assert.equal(await page.evaluate(() => window.copiedText), '10808');
  await page.locator('select').selectOption('socks5');
  await page.locator('.app-proxy-copy').last().click();
  assert.equal(await page.evaluate(() => window.copiedText), 'socks5://[::1]:10808');
  await page.evaluate(() => { fixture.state.active.candidate.protocol='http'; fixture.state.active.candidate.host='127.0.0.1'; fixture.state.active.revision++; });
  await page.waitForFunction(() => document.querySelector('.app-proxy-fields dd:last-child code')?.textContent !== '');
  assert.equal(await page.locator('select').isDisabled(), true);
  assert.equal(await page.locator('.app-proxy-fields code').last().innerText(), 'http://127.0.0.1:10808');
  await page.evaluate(() => { fixture.state.active.candidate.protocol='socks5'; fixture.state.active.revision++; });
  assert.equal(await page.locator('option').count(), 1);
  assert.equal(await page.locator('.app-proxy-fields code').last().innerText(), 'socks5://127.0.0.1:10808');
  await page.evaluate(() => { fixture.state.active.candidate.protocol='unknown'; fixture.state.active.revision++; });
  await page.locator('.app-proxy-unavailable').waitFor();
  assert.equal(await page.locator('.app-proxy-copy').count(), 0);
  await page.evaluate(() => { fixture.state.active.candidate.protocol='mixed'; fixture.state.active.revision=1; });
  await page.locator('select').selectOption('http');
  for (const locale of ['zh-CN','en','ja','ko']) for (const width of [560,880]) for (const mode of ['light','dark']) {
    await page.setViewportSize({ width, height: 760 });
    await page.evaluate(({ locale, mode }) => { fixture.state.locale=locale; fixture.theme('blue',mode); }, { locale,mode });
    await page.evaluate(() => new Promise(resolve => requestAnimationFrame(resolve)));
    assert.equal(await page.locator('.app-proxy-content').evaluate(node => node.scrollWidth > node.clientWidth + 1), false, `${locale}/${width}/${mode}: manual overflow`);
    if (shots && mode === 'light' && width === 560) await page.screenshot({ path: resolve(output, `app-setup-${locale}.png`) });
  }
  await page.evaluate(() => fixture.state.locale='zh-CN');
  await page.setViewportSize({ width:880,height:760 });
  if (shots) await page.screenshot({ path:resolve(output,'app-setup-desktop.png') });
  const expand = page.locator('.app-proxy-launch > header > button');
  await page.evaluate(() => fixture.state.failList=true);
  await expand.click();
  await page.locator('.notification-card[data-kind=error]').waitFor();
  assert.match(await page.locator('.notification-card[data-kind=error]').innerText(), /无法加载应用/);
  await page.evaluate(() => { fixture.state.failList=false; fixture.clearNotices(); });
  await page.locator('.application-picker-actions > button').first().click();
  await page.locator('.application-list button').first().waitFor();
  await page.locator('input[type=search]').fill('missing');
  assert.equal(await page.locator('.application-list').count(), 0);
  await page.locator('input[type=search]').fill('C:\\Apps\\Code');
  assert.equal(await page.locator('.application-list button').count(), 1);
  await page.locator('.application-list button').click();
  const launch = page.locator('.app-proxy-selected button');
  await launch.waitFor();
  assert.equal(await page.evaluate(() => fixture.calls.some(([c]) => c.startsWith('launch_') || c === 'diagnose_application')), false, 'Selection only selects');
  if (shots) await page.screenshot({ path:resolve(output,'app-setup-selection.png') });
  await page.evaluate(() => fixture.state.holdRenew=true);
  await launch.click();
  await page.waitForFunction(() => Boolean(fixture.releaseRenew));
  assert.equal(await launch.isDisabled(), true);
  await page.evaluate(() => { fixture.state.active.revision++; fixture.releaseRenew(); fixture.state.holdRenew=false; });
  await page.locator('.notification-card[data-kind=error]').waitFor();
  assert.equal(await page.evaluate(() => fixture.calls.some(([c]) => c.startsWith('launch_'))), false, 'Proxy change during authorization blocks launch');
  await page.evaluate(() => { fixture.state.expired=true; fixture.clearNotices(); });
  await launch.click();
  await page.locator('.app-proxy-result').waitFor();
  assert.match(await page.locator('.app-proxy-result').innerText(), /已启动应用.*回到应用验证连接/s);
  assert.deepEqual(await page.evaluate(() => fixture.calls.filter(([c]) => c.startsWith('launch_'))), [['launch_application_with_current_proxy', { applicationId:'code',expectedRevision:2 }]]);
  await page.evaluate(() => { fixture.state.expired=false; fixture.state.failLaunch=true; });
  await launch.click();
  await page.locator('.notification-card[data-kind=error]').waitFor();
  assert.equal(await page.locator('.app-proxy-result').count(), 0);
  await page.evaluate(() => { fixture.state.failLaunch=false; fixture.state.picked=true; fixture.clearNotices(); });
  await page.locator('.application-picker-actions > button').last().click();
  assert.match(await page.locator('.app-proxy-selected').innerText(), /Chat/);
  for (const locale of ['zh-CN','en','ja','ko']) for (const width of [560,880]) for (const mode of ['light','dark']) {
    await page.setViewportSize({ width, height:760 });
    await page.evaluate(({locale,mode}) => { fixture.state.locale=locale; fixture.theme('blue',mode); }, {locale,mode});
    await page.evaluate(() => new Promise(resolve => requestAnimationFrame(resolve)));
    const geometry = await page.evaluate(() => ({
      page: document.documentElement.scrollWidth > innerWidth,
      content: [...document.querySelectorAll('.app-proxy-content,.application-picker-actions')].some(node => node.scrollWidth > node.clientWidth + 1),
      controls: [...document.querySelectorAll('.application-proxy-page button')].filter(node => node.getClientRects().length).some(node => node.scrollWidth > node.clientWidth + 1),
    }));
    assert.deepEqual(geometry,{page:false,content:false,controls:false}, `${locale}/${width}/${mode}: picker overflow`);
  }
  await page.evaluate(() => { fixture.state.locale='zh-CN'; fixture.state.active.available=false; fixture.state.active.revision++; });
  assert.equal(await launch.isDisabled(), true);
  assert.equal(await page.locator('.app-proxy-copy').count(), 0);
  await page.locator('.app-proxy-unavailable button').click();
  assert.equal(await page.evaluate(() => fixture.state.back), true);
  const allowed = ['list_running_applications','renew_application_authorization','launch_application_with_current_proxy','pick_application'];
  assert.deepEqual(await page.evaluate(allowed => fixture.calls.filter(([c]) => !allowed.includes(c)), allowed), [], 'No diagnosis, rule editing, automatic health checks, proxy clearing or process termination');
  assert.deepEqual(errors,[]);
  console.log('Application proxy setup passed: lazy selection, protocol/IPv6 copying, search/browse, explicit launch, renewal recovery, revision race, failures/unavailable state, 32 four-language/width/theme layouts. Synthetic IPC only.');
} finally { await browser.close(); }
