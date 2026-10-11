import assert from "node:assert/strict";
import {createRequire} from "node:module";
import {mkdirSync} from "node:fs";
import {resolve} from "node:path";
const require=createRequire(process.env.PROXYENV_LAYOUT_PLAYWRIGHT || import.meta.url);
const {chromium}=require("playwright");
const browser=await chromium.launch({headless:true,...(process.env.PROXYENV_LAYOUT_BROWSER?{executablePath:process.env.PROXYENV_LAYOUT_BROWSER}:{})});
try {
  const page=await browser.newPage({viewport:{width:980,height:800},reducedMotion:"reduce"});
  const errors=[];page.on("pageerror",error=>errors.push(error.message));
  await page.goto(`${process.env.PROXYENV_LAYOUT_URL||"http://127.0.0.1:1420"}/?impeccable-review=remote-connected`);
  await page.getByRole("heading",{name:"桥接状态",exact:true}).waitFor();
  await page.evaluate(async()=>{
    // Use Vite's exact dependency URL, including any HMR timestamp, not a second module instance.
    const source=await (await fetch('/src/shared/components/NotificationCenter.vue')).text();
    const path=source.match(/from "([^"]*\/state\/notifications\.ts[^"]*)"/)[1];
    window.notice=await import(path);
  });
  const box=await page.locator('.remote-connected-page').boundingBox();
  const beforeFocus=await page.evaluate(()=>document.activeElement?.tagName);
  await page.evaluate(()=>window.notice.notify("代理环境已同步。","success","test"));
  const card=page.locator('.notification-card');
  try {await card.waitFor({timeout:2000});}
  catch(cause){console.log(await page.evaluate(()=>({notices:window.notice.notifications,host:document.querySelector('.notification-center')?.outerHTML})),errors);throw cause;}
  assert.deepEqual(await page.locator('.remote-connected-page').boundingBox(),box,"Notifications never shift page layout");
  assert.equal(await page.evaluate(()=>document.activeElement?.tagName),beforeFocus,"Notifications do not steal focus");
  const directory=resolve('.impeccable/review/notifications');mkdirSync(directory,{recursive:true});
  for(const [name,width,height,theme] of [["light",980,800,"light"],["dark",980,800,"dark"],["narrow",620,620,"light"]]) {
    await page.setViewportSize({width,height});await page.evaluate(theme=>document.documentElement.dataset.theme=theme,theme);
    const bounds=await card.boundingBox();assert.ok(bounds.x>=0&&bounds.y>=0&&bounds.x+bounds.width<=width&&bounds.y+bounds.height<=height);
    if(process.env.PROXYENV_LAYOUT_CAPTURE!=="0")await page.screenshot({path:resolve(directory,`${name}.png`)});
  }
  await page.evaluate(()=>{
    const modal=document.createElement('dialog');modal.id='notification-test-modal';modal.innerHTML='<input aria-label="Test input"><button>Close test modal</button>';document.body.append(modal);modal.showModal();
    window.notice.notify("本次认证未通过，请重新输入。","error","test");
  });
  await card.getByText("本次认证未通过，请重新输入。",{exact:true}).waitFor();
  await card.getByRole('button',{name:'关闭通知',exact:true}).click({timeout:3000});
  await card.waitFor({state:'hidden'});
  await page.waitForFunction(()=>document.querySelectorAll('.notification-card').length===0);
  assert.equal(await card.count(),0,"Notice remains clickable above a modal dialog");
  await page.evaluate(()=>{document.querySelector('#notification-test-modal').close();document.querySelector('#notification-test-modal').remove();
    for(let i=0;i<4;i++)window.notice.notify(`提示 ${i}`,'info');});
  await page.waitForFunction(()=>document.querySelectorAll('.notification-card').length===3);
  await page.evaluate(()=>window.notice.clearNotifications());
  assert.deepEqual(errors,[]);
  console.log('Notifications: light/dark/narrow, layout stability, bounded stack, focus retention and modal visibility passed.');
}finally{await browser.close();}
