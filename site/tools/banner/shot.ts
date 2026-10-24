// Screenshots banner.html at an exact size through headless Edge's DevTools
// protocol. usage: bun shot.ts <width> <height> <out.png> [browser]
import { spawn } from "node:child_process";
import { mkdtempSync, writeFileSync } from "node:fs";
import { tmpdir } from "node:os";
import { join, resolve } from "node:path";
import { pathToFileURL } from "node:url";

const [w, h, out, browser = "C:\\Program Files (x86)\\Microsoft\\Edge\\Application\\msedge.exe"] = process.argv.slice(2);
const port = 9300 + Math.floor(Math.random() * 500);
const profile = mkdtempSync(join(tmpdir(), "luna-banner-"));
const proc = spawn(browser, [
  "--headless=new",
  `--remote-debugging-port=${port}`,
  `--user-data-dir=${profile}`,
  "--hide-scrollbars",
  "about:blank",
]);

const wait = (ms: number) => new Promise((r) => setTimeout(r, ms));
let target: { webSocketDebuggerUrl: string } | undefined;
for (let i = 0; i < 50 && !target; i++) {
  await wait(200);
  try {
    const list = await (await fetch(`http://127.0.0.1:${port}/json/list`)).json();
    target = list.find((t: { type: string }) => t.type === "page");
  } catch {}
}
if (!target) throw new Error("browser did not start");

const ws = new WebSocket(target.webSocketDebuggerUrl);
await new Promise((r) => ws.addEventListener("open", r, { once: true }));
let id = 0;
const pending = new Map<number, (v: any) => void>();
ws.addEventListener("message", (e) => {
  const msg = JSON.parse(String(e.data));
  pending.get(msg.id)?.(msg.result);
});
const send = (method: string, params = {}) =>
  new Promise<any>((r) => {
    pending.set(++id, r);
    ws.send(JSON.stringify({ id, method, params }));
  });

await send("Emulation.setDeviceMetricsOverride", { width: +w, height: +h, deviceScaleFactor: 1, mobile: false });
await send("Page.navigate", { url: pathToFileURL(resolve(import.meta.dir, "banner.html")).href });
await wait(1500);
await send("Runtime.evaluate", { expression: "document.fonts.ready.then(() => Promise.all([...document.images].map((i) => i.decode())))", awaitPromise: true });
const { data } = await send("Page.captureScreenshot", { format: "png" });
writeFileSync(out, Buffer.from(data, "base64"));
ws.close();
proc.kill();
console.log(out);
