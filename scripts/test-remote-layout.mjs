import assert from "node:assert/strict";
import { createRequire } from "node:module";
import { mkdirSync } from "node:fs";
import { resolve } from "node:path";

// Optional browser tooling: no new production or locked development dependency.
const require = createRequire(process.env.PROXYENV_LAYOUT_PLAYWRIGHT || import.meta.url);
const { chromium } = require("playwright");
const browser = await chromium.launch({
  headless: true,
  ...(process.env.PROXYENV_LAYOUT_BROWSER ? { executablePath: process.env.PROXYENV_LAYOUT_BROWSER } : {}),
});
const base = process.env.PROXYENV_LAYOUT_URL || "http://localhost:1420";
const output = process.argv.includes("--screenshots") ? resolve(".debug-tmp/remote-layout") : null;
if (output) mkdirSync(output, { recursive: true });
const sizes = [[880,720], [1920,1080], [1280,720], [1024,640], [761,620], [760,620], [704,576], [587,480], [560,620]];
const routes = process.env.PROXYENV_LAYOUT_ROUTES?.split(",") || ["local", "remote", "remote-empty", "remote-add", "remote-connected", "remote-connected-advanced", "remote-auth", "remote-auth-rejected", "remote-auth-completing", "settings", "about", "assistant"];

try {
  for (const theme of ["light", "dark"]) {
    const context = await browser.newContext({ reducedMotion: "reduce" });
    const page = await context.newPage();
    for (const route of routes) {
      await page.goto(`${base}/?impeccable-review=${["remote-empty","remote-add"].includes(route) ? "remote" : route}`);
      await page.locator(".app-header").waitFor();
      await page.evaluate(() => document.fonts.ready);
      if (route.startsWith("remote-auth")) await page.locator(".remote-auth-dialog[open]").waitFor();
      if (["remote","remote-empty","remote-add"].includes(route)) await page.locator(".remote-current-summary").waitFor();
      if (route === "remote-empty") await page.locator(".remote-setup-actions .secondary-action").click();
      if (route === "remote-add") await page.locator(".remote-panel-actions button").first().click();
      await page.evaluate(value => document.documentElement.dataset.theme = value, theme);
      for (const [width,height] of sizes) {
        await page.setViewportSize({width,height});
        await page.waitForFunction(({width,height}) => {
          const frame = document.querySelector(".app-frame").getBoundingClientRect();
          return Math.abs(frame.width - width) < 1 && Math.abs(frame.height - height) < 1;
        },{width,height});
        await page.evaluate(() => new Promise(done => requestAnimationFrame(() => requestAnimationFrame(done))));
        const result = await page.evaluate(() => {
          const rect = selector => {
            const element = document.querySelector(selector);
            if (!element) return null;
            const bounds = element.getBoundingClientRect();
            return {left:bounds.left,right:bounds.right,top:bounds.top,bottom:bounds.bottom,width:bounds.width};
          };
          const stage = document.querySelector(".view-stage");
          const setup = document.querySelector(".remote-setup-page");
          const list = document.querySelector(".remote-connection-list-scroll");
          const rows = list ? [...list.querySelectorAll(".remote-target")] : [];
          const listBounds = list?.getBoundingClientRect();
          return {
            frame:rect(".app-frame"), header:rect(".header-actions"), stage:rect(".view-stage"),
            horizontalOverflow:stage.scrollWidth > stage.clientWidth + 1,
            setup:setup ? rect(".remote-workspace-configure") : null,
            stageOverflow:setup && stage.scrollHeight > stage.clientHeight + 1,
            actions:setup ? rect(".remote-setup-actions") : null,
            left:setup ? rect(".remote-connections-panel") : null,
            right:setup ? rect(".remote-current-panel") : null,
            content:setup ? rect(".remote-current-content") : null,
            summary:setup ? rect(".remote-current-summary") : null,
            capabilityPanel:setup ? rect(".remote-setup-panel") : null,
            lastCapability:setup ? rect(".remote-capability-choice:last-child") : null,
            rowCount:rows.length,
            completeVisibleRows:rows.filter(row => {
              const bounds = row.getBoundingClientRect();
              return bounds.top >= listBounds.top - 1 && bounds.bottom <= listBounds.bottom + 1;
            }).length,
            dialog:rect("dialog[open]"),
            authField:rect("dialog[open] .remote-auth-field input"),
          };
        });
        const label = `${theme}/${route}/${width}x${height}`;
        assert.equal(result.frame.width,width,`${label}: window shell must fill viewport`);
        assert.ok(result.header.right <= width + 1,`${label}: window controls overflow`);
        assert.equal(result.horizontalOverflow,false,`${label}: page has horizontal overflow`);
        if (route === "remote-connected") {
          const overview = await page.evaluate(() => {
            const cards = [...document.querySelectorAll('.bridge-capability')].map(element => element.getBoundingClientRect().toJSON());
            const stage = document.querySelector('.view-stage');
            const bar = document.querySelector('.bridge-launch-bar');
            const launch = [...bar.querySelectorAll('button')];
            return { cards, actions: launch.length, mobaDisabled: launch[2].disabled, terminalVisible: launch[0].getBoundingClientRect().bottom <= stage.getBoundingClientRect().bottom, capabilityInteractive: [...document.querySelectorAll('.capability-switch')].every(input => !input.disabled) };
          });
          assert.equal(overview.actions,3,`${label}: launch actions must stay visible even when unavailable`);
          assert.equal(overview.mobaDisabled,true,`${label}: unavailable MobaXterm must stay disabled`);
          assert.equal(overview.capabilityInteractive,true,`${label}: connected capabilities must be interactive`);
          const mappingTypography=await page.evaluate(() => {
            const font=element=>{
              const style=getComputedStyle(element);
              return { family:style.fontFamily, size:style.fontSize, weight:style.fontWeight, lineHeight:style.lineHeight };
            };
            return { reference:font(document.querySelector('.remote-target-metadata > span')), mapping:[...document.querySelectorAll('.bridge-mapping span,.bridge-mapping code,.bridge-mapping small')].map(font) };
          });
          for (const font of mappingTypography.mapping) assert.deepEqual(font,mappingTypography.reference,`${label}: mapping typography must match SSH metadata`);
          const mappingLayout=await page.evaluate(() => {
            const mappings=[...document.querySelectorAll('.bridge-mapping')].map(element=>{
              const style=getComputedStyle(element);
              const children=[...element.children].map(child=>child.getBoundingClientRect());
              const bounds=element.getBoundingClientRect();
              const rect=selector=>element.querySelector(selector).getBoundingClientRect();
              const localLabel=rect('.bridge-mapping-local-label'),remoteLabel=rect('.bridge-mapping-remote-label');
              const localAddress=rect('.bridge-mapping-local-address'),remoteAddress=rect('.bridge-mapping-remote-address');
              const client=rect('.bridge-mapping-client'),arrow=rect('.bridge-mapping-arrow');
              const center=rect=>rect.top+rect.height/2;
              return {display:style.display,labelsAligned:Math.abs(center(localLabel)-center(remoteLabel))<1,addressesAligned:Math.abs(center(localAddress)-center(remoteAddress))<1 && Math.abs(center(localAddress)-center(arrow))<1,clientBelow:client.top>=localAddress.bottom,contained:element.scrollWidth<=element.clientWidth+1 && children.every(rect=>rect.left>=bounds.left-1 && rect.right<=bounds.right+1),plainArrow:element.querySelector('.bridge-mapping-arrow').children.length===1};
            });
            return {mappings,globe:!!document.querySelector('.remote-server-direct > .remote-server-globe'),directLabel:document.querySelector('.remote-server-direct > span')?.textContent};
          });
          assert.ok(mappingLayout.globe,`${label}: server direct network must show a globe icon`);
          assert.equal(mappingLayout.directLabel,'服务器直连网络',`${label}: overview must use the direct-network title`);
          for (const mapping of mappingLayout.mappings) {
            assert.equal(mapping.display,'grid',`${label}: mapping must keep local and remote columns`);
            assert.ok(mapping.labelsAligned && mapping.addressesAligned && mapping.clientBelow && mapping.contained && mapping.plainArrow,`${label}: mapping labels and addresses must align with a plain arrow and separate client details ${JSON.stringify(mapping)}`);
          }
          if (width >= 880) for (const card of overview.cards) assert.ok(card.height <= 126,`${label}: desktop connected capability card must stay compact`);
          if (width > 760) assert.ok(overview.cards[1].left >= overview.cards[0].right,`${label}: capability cards must be side by side`);
          else assert.ok(overview.cards[1].top >= overview.cards[0].bottom,`${label}: narrow capability cards must stack`);
          if (width >= 1280 && height >= 720) assert.ok(overview.terminalVisible,`${label}: desktop launch bar must fit the first screen`);
        }
        if (result.setup) {
          assert.equal(result.stageOverflow,false,`${label}: setup must not scroll outside its panels`);
          assert.ok(result.setup.bottom <= height + 1,`${label}: workspace border is clipped ${JSON.stringify(result)}`);
          assert.ok(result.actions.bottom <= result.right.bottom,`${label}: actions escape the current-connection panel`);
          assert.ok(result.actions.top >= result.right.top,`${label}: action bar is not visible`);
          if (width > 760) assert.ok(result.right.left >= result.left.right,`${label}: desktop must use two columns`);
          else assert.ok(result.right.top >= result.left.bottom,`${label}: narrow panes must stack`);
          assert.ok(result.completeVisibleRows >= Math.min(2,result.rowCount),`${label}: at least two complete SSH connections must remain visible ${JSON.stringify(result)}`);
          if (width > 760 && height >= 620 && result.lastCapability) assert.ok(result.lastCapability.bottom <= result.content.bottom + 1,`${label}: both desktop capabilities should be visible without scrolling ${JSON.stringify(result)}`);
          if (result.lastCapability) {
            const reachable = await page.evaluate(() => {
              const content = document.querySelector(".remote-current-content");
              const previous = content.scrollTop;
              content.scrollTop = content.scrollHeight;
              const last = content.querySelector(".remote-capability-choice:last-child").getBoundingClientRect();
              const bounds = content.getBoundingClientRect();
              const visible = last.bottom <= bounds.bottom + 1 && last.bottom > bounds.top;
              content.scrollTop = previous;
              return visible;
            });
            assert.ok(reachable,`${label}: capabilities must remain reachable in the current-panel scroller`);
          }
        }
        if (result.dialog) {
          assert.ok(result.dialog.left >= 0 && result.dialog.right <= width + 1,`${label}: dialog width`);
          assert.ok(result.dialog.top >= 0 && result.dialog.bottom <= height + 1,`${label}: dialog height`);
          if (result.authField) assert.ok(result.authField.top < result.dialog.bottom,`${label}: retained authentication field`);
        }
        if (route === "remote-add") {
          await page.locator('dialog[open] label:has(> input[type="number"]) .help-tooltip').hover();
          const tooltip = page.getByRole("tooltip");
          await tooltip.waitFor();
          assert.match(await tooltip.innerText(),/22/,`${label}: SSH port help explains the default`);
          const bounds = await tooltip.boundingBox();
          assert.ok(bounds.x >= 0 && bounds.x + bounds.width <= width + 1 && bounds.y >= 0 && bounds.y + bounds.height <= height + 1,`${label}: SSH port help stays inside the viewport`);
          await page.mouse.move(0,0);
          await tooltip.waitFor({state:"hidden"});
        }
        if (route === "remote-empty") {
          assert.equal(await page.locator(".remote-current-empty .pixel-mascot").count(),1,`${label}: one decorative mascot below the instruction`);
          assert.equal(await page.locator(".pixel-mascot__actor").evaluate(element => getComputedStyle(element).animationName),"none",`${label}: reduced motion keeps the mascot static`);
        }
        if (route === "remote-connected" && width === 880) {
          const help=page.locator('.bridge-capability .help-tooltip').first();
          await help.hover();
          await page.getByRole('tooltip').waitFor();
          await help.click();
          await page.mouse.move(0,0);
          assert.equal(await page.getByRole('tooltip').count(),1,`${label}: clicking help pins it`);
          await page.keyboard.press('Escape');
          await page.getByRole('tooltip').waitFor({state:'hidden'});
          await help.focus();
          await page.keyboard.press('Enter');
          assert.equal(await help.getAttribute('aria-expanded'),'true',`${label}: keyboard can pin help`);
          await page.locator('.remote-workspace-heading h1').click();
          await page.getByRole('tooltip').waitFor({state:'hidden'});
          const originalTools=await page.locator('.bridge-tool strong').allTextContents();
          await page.locator('.bridge-skills-heading button').click();
          await page.locator('.remote-skills').waitFor({state:'visible'});
          assert.equal(await page.locator('.remote-skills').evaluate(element=>element===document.activeElement),true,`${label}: Manage focuses existing Skills controls`);
          await page.locator('.remote-view-switch button').first().click();
          assert.deepEqual(await page.locator('.bridge-tool strong').allTextContents(),originalTools,`${label}: view changes preserve tools`);
          await page.locator('.view-stage').evaluate(element=>element.scrollTop=0);
        }
        if (output && theme === "light" && route === "remote" && [880,1920,560,587].includes(width)) {
          await page.screenshot({path:resolve(output,`${route}-${width}x${height}.png`)});
        }
        if (output && route === 'remote-connected' && [880,1280,560].includes(width)) {
          await page.screenshot({path:resolve(output,`${route}-${theme}-${width}x${height}.png`)});
        }
      }
    }
    await context.close();
  }
  const startupContext = await browser.newContext({viewport:{width:880,height:720},reducedMotion:"reduce"});
  const startup = await startupContext.newPage();
  await startup.addInitScript(() => {
    window.ccCalls = 0;
    window.ccComplete = false;
    window.isTauri = true;
    // Synthetic IPC only: no real SSH target, route server or user credentials.
    window.__TAURI_INTERNALS__ = {invoke:async command => {
      if (command === "remote_bridge_detect_cc") {
        window.ccCalls++;
        await new Promise(resolve => setTimeout(resolve,100));
        window.ccComplete = true;
        return {state:"confirmed",localPort:15822};
      }
      if (command === "remote_bridge_summary") return {status:"disconnected",target:null,proxy:null,cc:null,proxyStatus:null,ccStatus:null,activeProxyRevision:null,environment:"",codexConfigured:false,claudeConfigured:false,error:null,sshAuth:{mode:"nonInteractive",method:"unknown",authenticated:false,passwordStored:false}};
      throw new Error("Unsupported preview command");
    }};
  });
  await startup.goto(`${base}/?impeccable-review=local`);
  await startup.waitForFunction(() => window.ccComplete);
  assert.equal(await startup.locator(".remote-bridge-page").count(),1,"Startup must preload the SSH page");
  assert.equal(await startup.locator(".remote-bridge-page").isVisible(),false,"Preloading must not change the initial local page");
  await startup.locator(".remote-target-main").first().waitFor({state:"attached"});
  await startup.waitForFunction(() => document.querySelector('#bridge-ai-route')?.checked && document.querySelector('#bridge-local-network')?.checked);
  await startup.evaluate(() => { window.preloadedRemote = document.querySelector(".remote-bridge-page"); window.preloadedTarget = document.querySelector(".remote-target-main"); });
  await startup.evaluate(() => {
    window.ccFrames = [];
    const sample = () => {
      const route = document.querySelector(".remote-capability-choice:last-child");
      if (route) window.ccFrames.push({text:route.textContent,enabled:route.querySelector("input").checked});
      if (window.ccFrames.length < 6) requestAnimationFrame(sample);
    };
    requestAnimationFrame(sample);
  });
  await startup.locator(".primary-nav button").nth(1).click();
  await startup.waitForFunction(() => window.ccFrames.length >= 6);
  const firstPaint = await startup.evaluate(() => ({frames:window.ccFrames,calls:window.ccCalls,samePage:window.preloadedRemote === document.querySelector(".remote-bridge-page"),sameTarget:window.preloadedTarget === document.querySelector(".remote-target-main")}));
  assert.equal(firstPaint.samePage,true,"First navigation must reveal the preloaded page without remounting");
  assert.equal(firstPaint.sameTarget,true,"First navigation must retain the preloaded SSH list");
  assert.equal(firstPaint.calls,1,"Entering the remote page must not issue a duplicate discovery");
  for (const frame of firstPaint.frames) {
    assert.match(frame.text,/127\.0\.0\.1:15822/,"Every initial frame uses the startup-discovered address");
    assert.equal(frame.enabled,true,"The available default route renders enabled from the first frame");
  }
  await startup.locator(".primary-nav button").first().click();
  await startup.locator(".primary-nav button").nth(1).click();
  assert.equal(await startup.evaluate(() => window.preloadedRemote === document.querySelector(".remote-bridge-page")),true,"Later navigation also retains the same page instance");
  await startupContext.close();
  const mascotContext = await browser.newContext({viewport:{width:880,height:720},reducedMotion:"no-preference"});
  const mascotPage = await mascotContext.newPage();
  const actions = ["idle","coding","sleep","music","coffee","reading","plant","game"];
  // Seed only the empty-state creation, not application/library startup.
  async function enterEmptyState(sample) {
    await mascotPage.emulateMedia({reducedMotion:"no-preference"});
    await mascotPage.goto(`${base}/?impeccable-review=remote`);
    await mascotPage.locator(".remote-current-summary").waitFor();
    await mascotPage.evaluate(value => { window.originalRandom = Math.random; Math.random = () => value; },sample);
    await mascotPage.locator(".remote-setup-actions .secondary-action").click();
    await mascotPage.locator(".remote-empty-mascot").waitFor();
    await mascotPage.evaluate(() => { Math.random = window.originalRandom; delete window.originalRandom; });
    await mascotPage.waitForFunction(() => !document.querySelector(".remote-empty-mascot").classList.contains("is-paused"));
  }
  for (const [kindIndex,kind] of ["claude","codex"].entries()) {
    for (const [actionIndex,action] of actions.entries()) {
      await enterEmptyState((kindIndex*actions.length+actionIndex+.5)/(2*actions.length));
      const mascot = mascotPage.locator(`.pixel-mascot--${kind}.pixel-mascot--${action}`);
      await mascot.waitFor();
      assert.notEqual(await mascot.locator(".pixel-mascot__actor").evaluate(element => getComputedStyle(element).animationName),"none",`${kind}/${action}: sprite action is animated`);
      assert.notEqual(await mascot.locator(".pixel-mascot__actor").evaluate(element => getComputedStyle(element).animationIterationCount),"infinite",`${kind}/${action}: action completes before switching`);
      await mascotPage.evaluate(value => document.documentElement.dataset.theme = value,kindIndex ? "dark" : "light");
      if (output && ((kind === "claude" && action === "coding") || (kind === "codex" && action === "music"))) {
        await mascotPage.screenshot({path:resolve(output,`empty-${kind}-${action}.png`)});
      }
      await mascotPage.emulateMedia({reducedMotion:"reduce"});
      assert.equal(await mascot.locator(".pixel-mascot__actor").evaluate(element => getComputedStyle(element).animationName),"none",`${kind}/${action}: reduced-motion alternative`);
      await mascotPage.emulateMedia({reducedMotion:"no-preference"});
      await mascotPage.locator(".primary-nav button").first().click();
      await mascotPage.waitForFunction(() => document.querySelector(".remote-empty-mascot").classList.contains("is-paused"));
      await mascotPage.locator(".primary-nav button").nth(1).click();
      assert.equal(await mascot.count(),1,`${kind}/${action}: returning does not reroll the character`);
      await mascotPage.locator(".remote-target-main").first().click();
      assert.equal(await mascotPage.locator(".pixel-mascot").count(),0,`${kind}/${action}: selection removes the decorative animation`);
    }
  }
  await enterEmptyState((3+.5)/16); // Music: six .65s cycles, then a real animationend.
  await mascotPage.waitForTimeout(500);
  assert.equal(await mascotPage.locator(".pixel-scene").getAttribute("data-scene"),"0","No mid-action switching");
  const stageBefore = await mascotPage.locator(".remote-empty-mascot").boundingBox();
  const footerBefore = await mascotPage.locator(".remote-setup-actions").boundingBox();
  await mascotPage.locator('.pixel-scene[data-scene="1"]').waitFor({timeout:7000});
  await mascotPage.waitForFunction(() => !document.querySelector(".pixel-scene-enter-active"));
  assert.deepEqual(await mascotPage.locator(".remote-empty-mascot").boundingBox(),stageBefore,"Completed-action transition does not resize the stage");
  assert.deepEqual(await mascotPage.locator(".remote-setup-actions").boundingBox(),footerBefore,"Scene transition does not shift the buttons");
  await mascotPage.locator('.pixel-scene[data-scene="2"]').waitFor({timeout:7000});
  assert.equal(await mascotPage.locator(".pixel-scene--duo .pixel-mascot").count(),2,"Every third completed scene pairs the characters");
  // Exercise all duet props and verify that both actors must finish.
  for (const [index,action] of ["coding","sleep","plant","game","music"].entries()) {
    await enterEmptyState(.01);
    await mascotPage.locator(".pixel-mascot__actor").evaluate(element => element.dispatchEvent(new Event("animationend")));
    await mascotPage.waitForFunction(() => document.querySelector('.pixel-scene[data-scene="1"]') && !document.querySelector(".pixel-scene-enter-active"));
    await mascotPage.evaluate(value => { window.originalRandom = Math.random; Math.random = () => value; },(index+.5)/5);
    await mascotPage.locator(".pixel-mascot__actor").evaluate(element => element.dispatchEvent(new Event("animationend")));
    await mascotPage.locator(`.pixel-scene--duo .pixel-mascot--${action}`).first().waitFor();
    await mascotPage.waitForFunction(() => !document.querySelector(".pixel-scene-enter-active"));
    await mascotPage.evaluate(() => { Math.random = window.originalRandom; delete window.originalRandom; });
    const actors = mascotPage.locator(".pixel-mascot__actor");
    await actors.first().evaluate(element => element.dispatchEvent(new Event("animationend")));
    assert.equal(await mascotPage.locator(".pixel-scene").getAttribute("data-scene"),"2","First duet actor cannot interrupt the second");
    await mascotPage.evaluate(value => document.documentElement.dataset.theme = value,index%2 ? "dark" : "light");
    const geometry = await mascotPage.evaluate(() => {
      const stage = document.querySelector(".remote-empty-mascot");
      const scene = document.querySelector(".pixel-scene");
      const signal = document.querySelector(".pixel-duet-signal");
      const rect = signal.getBoundingClientRect(), stageRect = stage.getBoundingClientRect();
      return { display:getComputedStyle(stage).display, sceneDisplay:getComputedStyle(scene).display,
        signalWidth:parseFloat(getComputedStyle(signal).width), signalHeight:parseFloat(getComputedStyle(signal).height),
        fill:getComputedStyle(signal).fill, inside:rect.top >= stageRect.top && rect.bottom <= stageRect.bottom,
        codexColor:getComputedStyle(document.querySelector(".codex-blob")).fill };
    });
    assert.equal(geometry.display,"grid","Fixed stage CSS is applied");
    assert.equal(geometry.sceneDisplay,"flex","Duet CSS is applied");
    assert.equal(geometry.signalWidth,32,"Duet accent is a small decoration, not an unstyled SVG");
    assert.equal(geometry.signalHeight,20);
    assert.notEqual(geometry.fill,"rgb(0, 0, 0)");
    assert.equal(geometry.inside,true,"Duet accent stays inside the reserved stage");
    assert.equal(geometry.codexColor,index%2 ? "rgb(135, 156, 203)" : "rgb(145, 168, 223)","Codex uses the active theme palette");
    if (output && (action === "coding" || action === "plant")) await mascotPage.screenshot({path:resolve(output,`empty-duet-${action}.png`)});
    await actors.last().evaluate(element => element.dispatchEvent(new Event("animationend")));
    await mascotPage.locator('.pixel-scene[data-scene="3"]').waitFor();
    assert.equal(await mascotPage.locator(".pixel-mascot").count(),1,"Completed duet returns to a solo scene");
  }
  await mascotContext.close();
  console.log(`PASS: ${sizes.length * routes.length * 2} responsive layout cases; cached startup frames; 16 solo and 5 duet scenes, natural completion, stable transitions, reduced motion, hidden-page pause and selection cleanup.`);
} finally {
  await browser.close();
}
