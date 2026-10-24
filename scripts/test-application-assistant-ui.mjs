import assert from "node:assert/strict";
import { chromium } from "./ui-browser.mjs";
import { mkdirSync } from "node:fs";
import { resolve } from "node:path";
const browser = await chromium.launch({ headless: true, ...(process.env.PROXYENV_LAYOUT_BROWSER ? { executablePath: process.env.PROXYENV_LAYOUT_BROWSER } : {}) });
const output = resolve('.impeccable/review/local-v0.2.2');
mkdirSync(output, { recursive: true });
const shots = process.argv.includes('--screenshots');
try {
  const page = await browser.newPage({ viewport: { width: 880, height: 760 }, reducedMotion: 'reduce' });
  await page.addInitScript(() => Object.defineProperty(navigator, 'clipboard', { value: { writeText: async text => { window.copiedText = text; } }, configurable: true }));
  await page.route("**/*", route => new URL(route.request().url()).hostname === "127.0.0.1" ? route.continue() : route.abort());
  const errors = []; page.on('pageerror', error => errors.push(error.message));
  await page.goto(`${process.env.PROXYENV_LAYOUT_URL || 'http://127.0.0.1:1422'}/scripts/fixtures/application-assistant.html`, { waitUntil: 'domcontentloaded' });
  await page.locator('.app-proxy-address-row').waitFor();
  assert.deepEqual(await page.evaluate(() => fixture.calls), [], 'Opening the page does not enumerate, inspect, test, launch or write');
  assert.equal(await page.locator('.application-picker').count(), 0);
  assert.equal(await page.locator('.app-proxy-address-row code').innerText(), 'http://[::1]:10808');
  assert.equal(await page.locator('.app-proxy-fields').isVisible(), true, 'Server and port are visible without expanding');
  assert.match(await page.locator('.app-proxy-status .check-status').innerText(), /本机代理环境已配置/);
  await page.locator('.app-proxy-status [role=button]').click();
  await page.locator('[role=tooltip]').waitFor();
  assert.match(await page.locator('[role=tooltip]').innerText(), /是否读取.*不代表应用已经使用代理/);
  await page.keyboard.press('Escape');
  for (const [state,label] of [['disabled','未配置'],['partial','尚未同步'],['mismatch','尚未同步']]) {
    await page.evaluate(state => { fixture.state.environment.state=state; fixture.state.environment.matchesActiveProxy=false; }, state);
    assert.ok((await page.locator('.app-proxy-status .check-status').innerText()).includes(label));
  }
  await page.evaluate(() => { fixture.state.environment.state='enabled'; fixture.state.environment.matchesActiveProxy=false; });
  assert.match(await page.locator('.app-proxy-status .check-status').innerText(), /尚未同步/, 'Configured variables must match the selected proxy');
  await page.evaluate(() => { fixture.state.environment.matchesActiveProxy=true; fixture.state.stale=true; });
  assert.match(await page.locator('.app-proxy-status .check-status').innerText(), /待刷新/);
  assert.equal(await page.locator('.app-proxy-copy-address').count(), 0, 'Stale observations do not offer outdated addresses');
  await page.evaluate(() => fixture.state.stale=false);
  await page.locator('.app-proxy-copy').first().click();
  assert.equal(await page.evaluate(() => window.copiedText), '::1');
  await page.locator('.app-proxy-copy').nth(1).click();
  assert.equal(await page.evaluate(() => window.copiedText), '10808');
  await page.getByRole('button', { name:'SOCKS5', exact:true }).click();
  await page.locator('.app-proxy-copy-address').click();
  assert.equal(await page.evaluate(() => window.copiedText), 'socks5://[::1]:10808');
  await page.evaluate(() => { fixture.state.active.candidate.protocol='http'; fixture.state.active.candidate.host='127.0.0.1'; fixture.state.active.revision++; });
  await page.waitForFunction(() => document.querySelector('.app-proxy-address-row code')?.textContent === 'http://127.0.0.1:10808');
  assert.equal(await page.locator('.app-proxy-protocols').count(), 0);
  assert.equal(await page.locator('.app-proxy-protocol-label').innerText(), 'HTTP');
  assert.equal(await page.locator('.app-proxy-address-row code').innerText(), 'http://127.0.0.1:10808');
  await page.evaluate(() => { fixture.state.active.candidate.protocol='socks5'; fixture.state.active.revision++; });
  assert.equal(await page.locator('.app-proxy-protocol-label').innerText(), 'SOCKS5');
  assert.equal(await page.locator('.app-proxy-address-row code').innerText(), 'socks5://127.0.0.1:10808');
  await page.evaluate(() => { fixture.state.active.candidate.protocol='unknown'; fixture.state.active.revision++; });
  await page.locator('.app-proxy-back').waitFor();
  assert.equal(await page.locator('.app-proxy-copy').count(), 0);
  await page.evaluate(() => { fixture.state.active.candidate.protocol='mixed'; fixture.state.active.revision=1; });
  await page.getByRole('button', { name:'HTTP', exact:true }).click();
  for (const locale of ['zh-CN','en','ja','ko']) for (const palette of ['warm','blue','teal','neutral']) for (const width of [560,880,1280]) for (const mode of ['light','dark']) {
    await page.setViewportSize({ width, height: 760 });
    await page.evaluate(({ locale, palette, mode }) => { fixture.state.locale=locale; fixture.theme(palette,mode); }, { locale,palette,mode });
    assert.equal(await page.locator("html").getAttribute("data-palette"), palette);
    await page.evaluate(() => new Promise(resolve => requestAnimationFrame(resolve)));
    assert.equal(await page.locator('.app-proxy-content').evaluate(node => node.scrollWidth > node.clientWidth + 1), false, `${locale}/${width}/${mode}: manual overflow`);
    if (shots && palette === 'blue' && mode === 'light' && width === 560) await page.screenshot({ path: resolve(output, `app-setup-${locale}.png`) });
  }
  await page.evaluate(() => { fixture.state.locale='zh-CN'; fixture.theme('blue','light'); });
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
  const launch = page.locator('.app-proxy-selected .primary-action');
  await launch.waitFor();
  assert.equal(await page.locator('.application-picker').count(), 0, 'Selection collapses the list into one action row');
  const selectionCalls = await page.evaluate(() => fixture.calls.length);
  await expand.click(); await expand.click();
  assert.equal(await page.evaluate(() => fixture.calls.length), selectionCalls, 'Reopening a selected app does not enumerate again');
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
  await page.locator('.app-proxy-selected .app-proxy-text-action').click();
  await page.locator('.application-picker-actions > button').last().click();
  assert.match(await page.locator('.app-proxy-selected').innerText(), /Chat/);
  for (const locale of ['zh-CN','en','ja','ko']) for (const palette of ['warm','blue','teal','neutral']) for (const width of [560,880,1280]) for (const mode of ['light','dark']) {
    await page.setViewportSize({ width, height:760 });
    await page.evaluate(({locale,palette,mode}) => { fixture.state.locale=locale; fixture.theme(palette,mode); }, {locale,palette,mode});
    assert.equal(await page.locator("html").getAttribute("data-palette"), palette);
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
  await page.locator('.app-proxy-back').click();
  assert.equal(await page.evaluate(() => fixture.state.back), true);
  const allowed = ['list_running_applications','renew_application_authorization','launch_application_with_current_proxy','pick_application'];
  assert.deepEqual(await page.evaluate(allowed => fixture.calls.filter(([c]) => !allowed.includes(c)), allowed), [], 'No diagnosis, rule editing, automatic health checks, proxy clearing or process termination');
  assert.deepEqual(errors,[]);
  console.log('Application proxy setup passed: lazy selection, protocol/IPv6 copying, search/browse, explicit launch, renewal recovery, revision race, failures/unavailable state, 192 four-language/palette/width/theme layouts. Synthetic IPC only.');
} catch (error) {
  const page = browser.contexts().flatMap(context => context.pages())[0];
  if (page) await page.screenshot({ path: resolve(output, 'failure-application-assistant.png') }).catch(() => {});
  throw error;
} finally { await browser.close(); }
