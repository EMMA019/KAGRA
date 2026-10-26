#!/usr/bin/env node
// Open every playable game of a staged browser preview in headless Chromium and fail when one
// does not start: a page error, the start or render error card, a loading card that never goes
// away, or a blank picture. CI's Rust and Python tests run without a browser, so a page that
// dies in a real browser used to pass every check. It cannot see a fault of one phone's GPU
// (the Pixel 10 black screen of PR #210 needed the phone); it does see a page that breaks
// everywhere.
//
//   python scripts/build_browser_preview.py --out dist/preview --require-pkg
//   node scripts/browser_smoke.mjs dist/preview            # every game in catalog.json
//   node scripts/browser_smoke.mjs dist/preview hop_beat   # just these
//
// WebGL2 on SwiftShader (no GPU on the runner): slow but deterministic. Needs `playwright`
// (npm) with its Chromium.
import { createServer } from "node:http";
import { readFile, stat } from "node:fs/promises";
import { extname, join, normalize, resolve } from "node:path";
import { createRequire } from "node:module";

// require() (not import) so a global or NODE_PATH install is found too.
const { chromium } = createRequire(import.meta.url)("playwright");

const root = resolve(process.argv[2] || "dist/preview");
const only = process.argv.slice(3);
// SwiftShader on a CI runner is slow: a heavy game took 115 s to its first picture with three
// at once, and 60 s timed one out (PR #213). Two at a time and a generous wait.
const WAIT_MS = Number(process.env.SMOKE_WAIT_MS || 150000);
const PARALLEL = Number(process.env.SMOKE_PARALLEL || 2);

const TYPES = {
  ".html": "text/html", ".js": "text/javascript", ".mjs": "text/javascript", ".json": "application/json",
  ".wasm": "application/wasm", ".png": "image/png", ".jpg": "image/jpeg", ".webp": "image/webp",
  ".glb": "model/gltf-binary", ".gltf": "model/gltf+json", ".wav": "audio/wav", ".ogg": "audio/ogg",
  ".mp3": "audio/mpeg", ".ttf": "font/ttf", ".woff2": "font/woff2", ".css": "text/css", ".bin": "application/octet-stream",
};

function serve(dir) {
  const server = createServer(async (req, res) => {
    const path = normalize(decodeURIComponent(new URL(req.url, "http://x").pathname)).replace(/^([/\\])+/, "");
    const file = join(dir, path || "index.html");
    if (!file.startsWith(dir)) { res.writeHead(403).end(); return; }
    try {
      const s = await stat(file);
      const body = await readFile(s.isDirectory() ? join(file, "index.html") : file);
      res.writeHead(200, { "content-type": TYPES[extname(file)] || "application/octet-stream" }).end(body);
    } catch {
      res.writeHead(404).end();
    }
  });
  return new Promise((ok) => server.listen(0, "127.0.0.1", () => ok(server)));
}

async function check(browser, base, slug) {
  const ctx = await browser.newContext({
    viewport: { width: 360, height: 640 }, deviceScaleFactor: 1, isMobile: true, hasTouch: true, locale: "en-US",
  });
  const page = await ctx.newPage();
  const problems = [];
  page.on("pageerror", (e) => problems.push(`page error: ${e.message}`));
  page.on("console", (m) => { if (/kagra panic/.test(m.text())) problems.push(m.text().slice(0, 300)); });
  const t0 = Date.now();
  try {
    await page.goto(`${base}/play.html?game=${encodeURIComponent(slug)}`, { waitUntil: "load" });
    // Started: the loading card is gone, or it turned into an error card.
    await page.waitForFunction(() => {
      const b = document.getElementById("boot");
      return b && (b.classList.contains("err") || getComputedStyle(b).display === "none");
    }, null, { timeout: WAIT_MS, polling: 500 });
    const boot = await page.evaluate(() => {
      const b = document.getElementById("boot");
      return b.classList.contains("err") ? b.textContent.slice(0, 300) : "";
    });
    if (boot) problems.push(`error card: ${boot}`);
    // A few frames, then the picture must not be one flat colour.
    await page.waitForTimeout(4000);
    const shot = await page.screenshot({ timeout: WAIT_MS, type: "png" });
    const spread = await page.evaluate(async (b64) => {
      const img = new Image();
      img.src = `data:image/png;base64,${b64}`;
      await img.decode();
      const c = document.createElement("canvas");
      c.width = 32; c.height = 32;
      const g = c.getContext("2d");
      g.drawImage(img, 0, 0, 32, 32);
      const d = g.getImageData(0, 0, 32, 32).data;
      let lo = 255, hi = 0;
      for (let i = 0; i < d.length; i += 4) {
        const v = (d[i] + d[i + 1] + d[i + 2]) / 3;
        lo = Math.min(lo, v); hi = Math.max(hi, v);
      }
      return hi - lo;
    }, shot.toString("base64"));
    if (spread < 8) problems.push(`blank picture (brightness spread ${spread.toFixed(1)})`);
  } catch (e) {
    problems.push(`did not start within ${WAIT_MS / 1000}s: ${String(e.message || e).split("\n")[0]}`);
  } finally {
    await ctx.close();
  }
  return { slug, ok: problems.length === 0, secs: (Date.now() - t0) / 1000, problems };
}

const catalog = JSON.parse(await readFile(join(root, "catalog.json"), "utf8"));
const slugs = (catalog.games || [])
  .filter((g) => g.world && (!only.length || only.includes(g.slug)))
  .map((g) => g.slug);
if (!slugs.length) {
  console.error(`no playable games in ${root}/catalog.json`);
  process.exit(2);
}
const server = await serve(root);
const base = `http://127.0.0.1:${server.address().port}`;
const browser = await chromium.launch({
  args: ["--disable-features=WebGPU", "--use-angle=swiftshader", "--enable-unsafe-swiftshader"],
});
const results = [];
const queue = slugs.slice();
await Promise.all(Array.from({ length: Math.min(PARALLEL, queue.length) }, async () => {
  while (queue.length) {
    const r = await check(browser, base, queue.shift());
    results.push(r);
    console.log(`${r.ok ? "ok  " : "FAIL"} ${r.slug} (${r.secs.toFixed(1)} s)${r.ok ? "" : "\n     " + r.problems.join("\n     ")}`);
  }
}));
await browser.close();
server.close();
const bad = results.filter((r) => !r.ok);
console.log(`${results.length - bad.length}/${results.length} games started`);
process.exit(bad.length ? 1 : 0);
