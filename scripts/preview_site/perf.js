/* perf.js - the on-phone measurement tool (docs/phone_measure.md).
 *
 * Loaded by play.html only for ?dev=1 and never into a portal zip (the build copies it
 * to the preview site only). It makes no network request, keeps nothing in storage and
 * reads nothing personal: frame times, screen / GPU / browser facts, the JS heap.
 *
 * It is genre-neutral. It wraps `session.render` and `session.requestFrame` to time them
 * and runs its own requestAnimationFrame loop, so the game loop has no hook in it. The
 * A/B picture switches (`setSwitch` / `switchesJson` on the wasm session) are used when
 * the loaded game has them; otherwise the recorder takes one plain window.
 *
 * Plain script (no module) so the same file runs in the page and under node (tests).
 */
(function (root, factory) {
  const api = factory(root);
  if (typeof module === "object" && module.exports) module.exports = api;
  if (root) root.KagraPerf = api;
})(typeof self !== "undefined" ? self : globalThis, function (root) {
  "use strict";

  // ---------------------------------------------------------------- numbers --
  const r1 = (v) => (Number.isFinite(v) ? Math.round(v * 10) / 10 : null);
  const r2 = (v) => (Number.isFinite(v) ? Math.round(v * 100) / 100 : null);

  /** q in 0..1 on an ascending array, linear between neighbours. NaN for none. */
  function percentile(sorted, q) {
    const n = sorted.length;
    if (!n) return NaN;
    if (n === 1) return sorted[0];
    const pos = Math.min(1, Math.max(0, q)) * (n - 1);
    const lo = Math.floor(pos);
    const hi = Math.min(n - 1, lo + 1);
    return sorted[lo] + (sorted[hi] - sorted[lo]) * (pos - lo);
  }
  const asc = (a) => Float64Array.from(a).sort();
  const median = (a) => percentile(asc(a), 0.5);
  const mean = (a) => { let s = 0; for (let i = 0; i < a.length; i += 1) s += a[i]; return a.length ? s / a.length : NaN; };

  const REFRESH_LADDER = [48, 60, 72, 90, 120, 144, 165];

  /** The display refresh from rAF gaps: the fast end of the gaps (p10), snapped to a
   *  common rate when close. Returns { hz, intervalMs } or null with too few frames. */
  function estimateRefresh(dts) {
    const ok = [];
    for (let i = 0; i < dts.length; i += 1) if (dts[i] > 3 && dts[i] < 100) ok.push(dts[i]);
    if (ok.length < 30) return null;
    const p10 = percentile(asc(ok), 0.1);
    const raw = 1000 / p10;
    let hz = Math.round(raw);
    for (const cand of REFRESH_LADDER) if (Math.abs(raw - cand) / cand < 0.07) hz = cand;
    return { hz, intervalMs: 1000 / hz };
  }

  /** Frame-time summary for rAF gaps (ms). `intervalMs` is the display interval: a gap
   *  over 2 intervals is a jank frame, over 1.5 counts missed vsyncs, over 50 ms a hitch. */
  function summarize(dts, intervalMs, extra) {
    const I = intervalMs > 0 ? intervalMs : 1000 / 60;
    const n = dts.length;
    const out = { n, intervalMs: r2(I) };
    if (!n) return out;
    let sum = 0, max = 0, jank = 0, hitch = 0, missed = 0;
    for (let i = 0; i < n; i += 1) {
      const d = dts[i];
      sum += d;
      if (d > max) max = d;
      if (d > 2 * I) jank += 1;
      if (d > 50) hitch += 1;
      if (d > 1.5 * I) missed += Math.round(d / I) - 1;
    }
    const s = asc(dts);
    out.fps = r1((1000 * n) / sum);
    out.p50 = r2(percentile(s, 0.5));
    out.p95 = r2(percentile(s, 0.95));
    out.p99 = r2(percentile(s, 0.99));
    out.max = r1(max);
    out.jank = jank;
    out.hitches = hitch;
    out.dropPct = r2((100 * missed) / (missed + n));
    out.grade = grade(out, I);
    if (extra) Object.assign(out, extra);
    return out;
  }

  /** The plan's thresholds against the display interval T: good needs p50 <= 1.05T,
   *  p95 <= 1.3T, p99 <= 2T and under 1% dropped; ok needs p50 <= 1.5T and p95 <= 2T. */
  function grade(s, T) {
    if (!(s.p50 > 0)) return "n/a";
    if (s.p50 <= 1.05 * T && s.p95 <= 1.3 * T && s.p99 <= 2 * T && s.dropPct < 1) return "good";
    if (s.p50 <= 1.5 * T && s.p95 <= 2 * T) return "ok";
    return "poor";
  }

  /** Thermal guess from 10 s buckets of { dt, cpu } medians: the last 3 against the first 3.
   *  Frame times only drift once frames miss vsync, so the CPU ms per frame counts too. */
  function thermalGuess(buckets) {
    if (!buckets || buckets.length < 6) {
      return { label: "n/a (needs 60 s+; 10 min is better)", drift: null };
    }
    const med = (arr, key) => median(arr.map((b) => b[key]).filter((v) => v > 0));
    const f = buckets.slice(0, 3);
    const l = buckets.slice(-3);
    const drift = (key) => {
      const a = med(f, key);
      const b = med(l, key);
      return a > 0 && b > 0 ? b / a - 1 : 0;
    };
    const d = Math.max(drift("dt"), drift("cpu"));
    const lo = Math.min(drift("dt"), drift("cpu"));
    const pct = `${d >= 0 ? "+" : ""}${Math.round(d * 100)}%`;
    let label;
    if (d >= 0.25) label = `slowing ${pct}: likely throttling`;
    else if (d >= 0.1) label = `slowing ${pct}: maybe warming`;
    else if (lo <= -0.1) label = `faster ${Math.round(lo * 100)}%: warm-up or load changed`;
    else label = `stable (${pct})`;
    return { label, drift: r2(d) };
  }

  // ------------------------------------------------------------- the plan ----
  /** The recording steps for the switches the game has: every on/off mix, fewest on first
   *  (all-off, post, atmo, both for the two HOP-BEAT switches). No switch: one window. */
  function planSteps(switches, opts) {
    const o = opts || {};
    const stepMs = o.stepMs || 15000;
    const names = ["fx_post", "fx_atmo"].filter((n) => switches && n in switches);
    const steps = [];
    if (!names.length) {
      steps.push({ name: "as-is", sw: {}, ms: stepMs });
    } else {
      const masks = [];
      for (let m = 0; m < 1 << names.length; m += 1) masks.push(m);
      const pop = (m) => names.reduce((c, _, i) => c + ((m >> i) & 1), 0);
      masks.sort((a, b) => pop(a) - pop(b) || a - b);
      for (const m of masks) {
        const sw = {};
        names.forEach((n, i) => { sw[n] = Boolean((m >> i) & 1); });
        const on = names.filter((_, i) => (m >> i) & 1).map((n) => n.replace(/^fx_/, ""));
        let name = on.join("+");
        if (m === 0) name = "all-off";
        else if (m === (1 << names.length) - 1 && names.length > 1) name = "both";
        steps.push({ name, sw, ms: stepMs });
      }
    }
    if (o.skipStep) {
      const sw = {};
      names.forEach((n) => { sw[n] = true; });
      steps.push({ name: names.length ? "all-on+skip" : "as-is+skip", sw, ms: stepMs, skip: true });
    }
    return steps;
  }

  // ---------------------------------------------------------- device facts ---
  /** Browser / screen facts. Nothing that identifies the person: no URL, no storage. */
  function deviceInfo(env) {
    const nav = (env && env.navigator) || {};
    const scr = (env && env.screen) || {};
    const win = (env && (env.window || env)) || {};
    const uad = nav.userAgentData;
    return {
      ua: String(nav.userAgent || ""),
      platform: String((uad && uad.platform) || nav.platform || ""),
      mobile: uad ? Boolean(uad.mobile) : null,
      cores: nav.hardwareConcurrency || null,
      memGB: nav.deviceMemory || null,
      touch: nav.maxTouchPoints || 0,
      dpr: r2(win.devicePixelRatio || 1),
      screen: [scr.width || 0, scr.height || 0],
      viewport: [win.innerWidth || 0, win.innerHeight || 0],
    };
  }

  async function withTimeout(promise, ms) {
    let timer;
    try {
      return await Promise.race([
        promise,
        new Promise((_, rej) => { timer = setTimeout(() => rej(new Error("timeout")), ms); }),
      ]);
    } finally {
      clearTimeout(timer);
    }
  }

  /** GPU facts, best effort: WebGPU adapter info and the WebGL2 renderer string (a
   *  throwaway canvas). Model names come from the browser and the GPU, not the user. */
  async function gatherAdapter(env) {
    const nav = (env && env.navigator) || {};
    const doc = env && env.document;
    const out = {};
    try {
      if (nav.gpu && nav.gpu.requestAdapter) {
        const a = await withTimeout(nav.gpu.requestAdapter(), 1500);
        if (a) {
          const info = a.info || (a.requestAdapterInfo ? await withTimeout(a.requestAdapterInfo(), 1000) : null) || {};
          out.webgpu = {
            vendor: info.vendor || "", architecture: info.architecture || "",
            device: info.device || "", description: info.description || "",
            fallback: Boolean(a.isFallbackAdapter),
          };
        } else {
          out.webgpu = null;
        }
      }
    } catch (e) { out.webgpuError = String((e && e.message) || e); }
    try {
      if (doc) {
        const gl = doc.createElement("canvas").getContext("webgl2");
        if (gl) {
          const ext = gl.getExtension("WEBGL_debug_renderer_info");
          out.webgl2 = {
            vendor: String(ext ? gl.getParameter(ext.UNMASKED_VENDOR_WEBGL) : gl.getParameter(gl.VENDOR)),
            renderer: String(ext ? gl.getParameter(ext.UNMASKED_RENDERER_WEBGL) : gl.getParameter(gl.RENDERER)),
            maxTexture: gl.getParameter(gl.MAX_TEXTURE_SIZE),
          };
          const lose = gl.getExtension("WEBGL_lose_context");
          if (lose) lose.loseContext();
        } else {
          out.webgl2 = null;
        }
      }
    } catch (e) { out.webgl2Error = String((e && e.message) || e); }
    try {
      const uad = nav.userAgentData;
      if (uad && uad.getHighEntropyValues) {
        const h = await withTimeout(uad.getHighEntropyValues(["model", "platformVersion", "architecture"]), 1000);
        out.uaHigh = { model: h.model || "", platformVersion: h.platformVersion || "", architecture: h.architecture || "" };
      }
    } catch (_) { /* the browser said no */ }
    return out;
  }

  const adapterLine = (ad) => {
    if (!ad) return "adapter ...";
    const g = ad.webgpu;
    if (g) return `gpu ${[g.vendor, g.architecture, g.device, g.description].filter(Boolean).join(" ") || "(no info)"}`;
    if (ad.webgl2) return `gl ${ad.webgl2.renderer}`;
    return "adapter n/a";
  };

  // ------------------------------------------------------------- the report --
  /** The JSON the user pastes into chat. */
  function buildReport(ctx) {
    return {
      tool: "kagra-perf",
      v: 1,
      when: ctx.when || null,
      page: ctx.page || {},
      device: ctx.device || {},
      adapter: ctx.adapter || null,
      display: { hz: ctx.refresh ? ctx.refresh.hz : null },
      canvas: ctx.canvas || null,
      scale: ctx.scale || null,
      quality: ctx.quality || null,
      thermal: ctx.thermal || null,
      overlayMs: ctx.overlayMs == null ? null : r2(ctx.overlayMs),
      longTasks: ctx.longTasks == null ? null : ctx.longTasks,
      steps: ctx.steps || [],
    };
  }

  /** The result table, monospace, one row per step. */
  function formatTable(steps) {
    const pad = (v, w) => String(v == null ? "-" : v).padStart(w);
    const rows = [
      `${"step".padEnd(11)}${pad("fps", 6)}${pad("p50", 6)}${pad("p95", 6)}${pad("p99", 6)}${pad("jnk", 4)}${pad("drp%", 5)}${pad("cpu", 5)}${pad("rnd/s", 6)} grade`,
    ];
    for (const s of steps) {
      rows.push(
        `${String(s.name).padEnd(11).slice(0, 11)}${pad(s.fps, 6)}${pad(s.p50, 6)}${pad(s.p95, 6)}${pad(s.p99, 6)}${pad(s.jank, 4)}${pad(s.dropPct, 5)}${pad(s.cpuMs, 5)}${pad(s.rendersPerSec, 6)} ${s.grade || ""}`,
      );
    }
    return rows.join("\n");
  }

  // ----------------------------------------------------------- the tool ------
  const PARAM_WHITELIST = ["game", "dev", "backend", "quality", "fx", "fx_post", "fx_atmo", "adapt", "render_skip", "audio", "debug"];

  /** Start the overlay on a page. `ctx`:
   *    session            the wasm session (render / requestFrame are wrapped)
   *    params             URLSearchParams of the page
   *    getCanvas()        the live canvas
   *    getBackend()       "gl" | "webgpu" | ...
   *    getScale()         { cap, step } of the render-scale ladder
   *    getQuality()       the picture quality name
   *    getRenderSkip() / setRenderSkip(on)   the render_skip switch (optional)
   *    build              the build id (optional)
   *  Returns the control object (also on window.kagraPerf). */
  function start(ctx) {
    const win = root;
    const doc = win && win.document;
    if (!doc) throw new Error("kagra-perf needs a page");
    if (win.kagraPerf) return win.kagraPerf;
    const now = () => win.performance.now();
    const session = ctx.session;

    // ---- timing wrappers: JS-side cost of the two calls the game makes each frame.
    let pendSim = 0, pendRender = 0, pendRenderCalls = 0, simCalls = 0;
    const wrap = (name, onDone) => {
      const orig = session[name];
      if (typeof orig !== "function") return;
      session[name] = function wrapped() {
        const t0 = now();
        try { return orig.apply(this, arguments); } finally { onDone(now() - t0); }
      };
    };
    wrap("render", (ms) => { pendRender += ms; pendRenderCalls += 1; });
    wrap("requestFrame", (ms) => { pendSim += ms; simCalls += 1; });

    // ---- rolling window (live numbers) and 10 s buckets (thermal trend)
    const CAP = 1024;
    const rT = new Float64Array(CAP), rDt = new Float64Array(CAP), rCpu = new Float64Array(CAP);
    const rRend = new Uint8Array(CAP), rSim = new Float64Array(CAP), rRnd = new Float64Array(CAP);
    let rN = 0, rHead = 0;
    let lastT = 0, pauses = 0;
    let totalFrames = 0, totalJank = 0;
    let refresh = null;
    const early = [];
    const buckets = [];
    let bStart = 0, bDt = [], bCpu = [];
    let overlayMs = 0, longTasks = 0;
    let rec = null; // { steps, i, t0, settleUntil, data, done }
    const results = [];
    let resultReport = null;
    let adapterInfo = null;

    const baseSw = () => {
      try { return JSON.parse(session.switchesJson ? session.switchesJson() : "{}"); } catch (_) { return {}; }
    };

    function onFrame(t) {
      win.requestAnimationFrame(onFrame);
      const sim = pendSim, rnd = pendRender, rc = pendRenderCalls;
      pendSim = 0; pendRender = 0; pendRenderCalls = 0;
      if (!lastT) { lastT = t; return; }
      const dt = t - lastT;
      lastT = t;
      if (dt <= 0) return;
      if (dt > 1000) { pauses += 1; bStart = 0; return; } // a hidden tab or a stop: not a frame
      totalFrames += 1;
      if (early.length < 240) {
        early.push(dt);
        // a first guess after 60 frames, firmed up at 240 (fast end of the gaps)
        if (early.length === 60 || early.length === 240) refresh = estimateRefresh(early) || refresh;
      }
      const I = refresh ? refresh.intervalMs : 1000 / 60;
      if (dt > 2 * I) totalJank += 1;
      const cpu = sim + rnd;
      rT[rHead] = t; rDt[rHead] = dt; rCpu[rHead] = cpu; rRend[rHead] = rc > 0 ? 1 : 0; rSim[rHead] = sim; rRnd[rHead] = rnd;
      rHead = (rHead + 1) % CAP;
      if (rN < CAP) rN += 1;
      // 10 s buckets
      if (!bStart) bStart = t;
      bDt.push(dt); bCpu.push(cpu);
      if (t - bStart >= 10000) {
        if (bDt.length > 100) buckets.push({ dt: median(bDt), cpu: mean(bCpu) });
        if (buckets.length > 360) buckets.shift();
        bStart = t; bDt = []; bCpu = [];
      }
      // the recorder
      if (rec && !rec.done && t >= rec.settleUntil && rec.active) {
        const d = rec.data;
        d.dts.push(dt); d.cpu.push(cpu); d.sim.push(sim); d.rnd.push(rnd); d.rend += rc > 0 ? 1 : 0;
      }
    }

    function live(windowMs) {
      const span = windowMs || 5000;
      const tNow = lastT;
      const dts = [];
      let cpu = 0, sim = 0, rnd = 0, rend = 0, k = 0;
      for (let i = 0; i < rN; i += 1) {
        const idx = (rHead - 1 - i + CAP * 2) % CAP;
        if (tNow - rT[idx] > span) break;
        dts.push(rDt[idx]); cpu += rCpu[idx]; sim += rSim[idx]; rnd += rRnd[idx]; rend += rRend[idx]; k += 1;
      }
      const I = refresh ? refresh.intervalMs : 1000 / 60;
      const s = summarize(dts, I);
      const secs = dts.reduce((a, b) => a + b, 0) / 1000;
      s.simMs = k ? r2(sim / k) : null;
      s.renderMs = k ? r2(rnd / k) : null;
      s.cpuMs = k ? r2(cpu / k) : null;
      s.rendersPerSec = secs > 0 ? r1(rend / secs) : null;
      return s;
    }

    const heapMB = () => {
      const m = win.performance && win.performance.memory;
      return m && m.usedJSHeapSize ? r1(m.usedJSHeapSize / 1048576) : null;
    };

    function canvasInfo() {
      const c = ctx.getCanvas ? ctx.getCanvas() : null;
      return c ? { w: c.width, h: c.height, cssW: c.clientWidth, cssH: c.clientHeight, dpr: r2(win.devicePixelRatio || 1) } : null;
    }
    function scaleInfo() { return ctx.getScale ? ctx.getScale() : null; }
    function switchesNow() {
      const sw = baseSw();
      const out = {};
      for (const k of Object.keys(sw)) out[k] = sw[k] ? "on" : "off";
      if (ctx.getRenderSkip) out.render_skip = ctx.getRenderSkip() ? "on" : "off";
      return out;
    }
    function pageInfo() {
      const params = {};
      if (ctx.params) for (const k of PARAM_WHITELIST) if (ctx.params.has(k)) params[k] = ctx.params.get(k);
      let status = "";
      try { status = session.pictureStatus ? String(session.pictureStatus()) : ""; } catch (_) { /* no renderer */ }
      return { game: ctx.params && ctx.params.get("game") || "", build: ctx.build || "", backend: ctx.getBackend ? ctx.getBackend() : "", params, picture: status };
    }

    // ---- overlay
    const root_ = doc.createElement("div");
    root_.id = "kagra-perf";
    root_.setAttribute("data-perf", "1");
    root_.style.cssText = "position:fixed;left:4px;top:calc(env(safe-area-inset-top,0px) + 4px);z-index:2147483000;" +
      "font:10px/1.3 ui-monospace,Menlo,Consolas,monospace;color:#d8ffe8;background:rgba(0,0,0,.66);" +
      "padding:4px 6px;border-radius:6px;max-width:96vw;box-sizing:border-box;pointer-events:none;" +
      "-webkit-user-select:none;user-select:none;touch-action:manipulation;white-space:pre";
    const text = doc.createElement("div");
    const table = doc.createElement("div");
    const bar = doc.createElement("div");
    bar.style.cssText = "display:flex;gap:6px;margin-top:4px;pointer-events:auto";
    const mkBtn = (label) => {
      const b = doc.createElement("button");
      b.type = "button";
      b.textContent = label;
      b.style.cssText = "font:inherit;font-size:11px;padding:6px 9px;border-radius:5px;border:1px solid #6c9;" +
        "background:#123;color:#d8ffe8;touch-action:manipulation;min-height:30px";
      return b;
    };
    const btnRec = mkBtn("Record 60 s");
    const btnCopy = mkBtn("Copy JSON");
    const btnMin = mkBtn("-");
    const btnPos = mkBtn("\u21C5");
    btnCopy.hidden = true;
    bar.append(btnRec, btnCopy, btnPos, btnMin);
    const area = doc.createElement("textarea");
    area.readOnly = true;
    area.hidden = true;
    area.rows = 6;
    area.style.cssText = "display:block;width:90vw;max-width:520px;margin-top:4px;font:10px ui-monospace,monospace;" +
      "pointer-events:auto;-webkit-user-select:text;user-select:text;background:#021;color:#d8ffe8;border:1px solid #6c9";
    root_.append(text, table, bar, area);
    for (const ev of ["pointerdown", "pointerup", "touchstart", "touchend", "mousedown", "mouseup", "click"]) {
      root_.addEventListener(ev, (e) => e.stopPropagation());
    }
    let minimized = false, atBottom = false;
    btnMin.onclick = () => { minimized = !minimized; btnMin.textContent = minimized ? "+" : "-"; paint(); };
    btnPos.onclick = () => {
      atBottom = !atBottom;
      root_.style.top = atBottom ? "auto" : "calc(env(safe-area-inset-top,0px) + 4px)";
      root_.style.bottom = atBottom ? "calc(env(safe-area-inset-bottom,0px) + 4px)" : "auto";
    };
    doc.body.appendChild(root_);

    function summaryLines() {
      const s = live(5000);
      const th = thermalGuess(buckets);
      const hz = refresh ? `${refresh.hz} Hz` : "...";
      const c = canvasInfo();
      const sc = scaleInfo();
      const sw = switchesNow();
      const heap = heapMB();
      const sws = Object.keys(sw).map((k) => `${k.replace(/^fx_/, "")}:${sw[k]}`).join(" ");
      const q = ctx.getQuality ? ctx.getQuality() : "";
      return [
        `${s.fps == null ? "--" : s.fps} fps  p50 ${s.p50 ?? "-"}  p95 ${s.p95 ?? "-"}  p99 ${s.p99 ?? "-"} ms`,
        `jank ${totalJank}/${totalFrames} (5s: ${s.jank ?? 0})  hitch5s ${s.hitches ?? 0}  disp ~${hz}`,
        `${ctx.getBackend ? ctx.getBackend() : "?"}  scale cap ${sc ? sc.cap : "?"}  ${c ? `${c.w}x${c.h} dpr ${c.dpr}` : ""}`,
        `sim ${s.simMs ?? "-"}  render ${s.renderMs ?? "-"} ms (js)  draws/s ${s.rendersPerSec ?? "-"}`,
        `heap ${heap == null ? "n/a" : `${heap} MB`}  ovh ${r2(overlayMs) ?? "-"} ms  thermal: ${th.label}`,
        adapterLine(adapterInfo),
        `tier ${q || "-"}  ${sws}`,
      ];
    }
    function paint() {
      const t0 = now();
      if (minimized) {
        const s = live(5000);
        text.textContent = `${s.fps ?? "--"} fps  p95 ${s.p95 ?? "-"} ms`;
      } else {
        const lines = summaryLines();
        if (rec && !rec.done) {
          const st = rec.steps[rec.i];
          const left = Math.max(0, Math.ceil((rec.t0 + st.ms - now()) / 1000));
          lines.unshift(`REC ${rec.i + 1}/${rec.steps.length} ${st.name}  ${left}s  (keep playing)`);
        }
        text.textContent = lines.join("\n");
      }
      table.textContent = !minimized && results.length ? formatTable(results) : "";
      overlayMs += (now() - t0 - overlayMs) * 0.1;
    }

    // ---- recorder
    function applySwitches(sw, skip) {
      for (const k of Object.keys(sw)) { try { session.setSwitch && session.setSwitch(k, sw[k]); } catch (_) { /* a game without it */ } }
      if (ctx.setRenderSkip) ctx.setRenderSkip(Boolean(skip));
    }
    function beginStep() {
      const st = rec.steps[rec.i];
      applySwitches(st.sw, st.skip);
      rec.t0 = now();
      rec.settleUntil = lastT + 1500; // the first 1.5 s after a switch (shader / pipeline rebuild) is dropped
      rec.active = true;
      rec.data = { dts: [], cpu: [], sim: [], rnd: [], rend: 0, heap0: heapMB() };
    }
    function endStep() {
      const st = rec.steps[rec.i];
      const d = rec.data;
      const I = refresh ? refresh.intervalMs : 1000 / 60;
      const secs = d.dts.reduce((a, b) => a + b, 0) / 1000;
      const heap1 = heapMB();
      const row = summarize(d.dts, I, {
        name: st.name,
        sw: st.sw,
        skip: Boolean(st.skip),
        // means over all frames (a skipped render counts as 0): the CPU a frame costs on average
        simMs: r2(mean(d.sim)),
        renderMs: r2(mean(d.rnd)),
        renderP95Ms: r2(percentile(asc(d.rnd), 0.95)),
        cpuMs: r2(mean(d.cpu)),
        rendersPerSec: secs > 0 ? r1(d.rend / secs) : null,
        heapDeltaMB: d.heap0 != null && heap1 != null ? r1(heap1 - d.heap0) : null,
      });
      if (d.dts.length < 60) row.note = "too few frames (tab hidden or screen off?)";
      results.push(row);
    }
    function finish() {
      rec.done = true;
      rec.active = false;
      applySwitches(rec.restore, rec.restoreSkip);
      btnRec.disabled = false;
      btnRec.textContent = "Record again";
      btnCopy.hidden = false;
      resultReport = report();
      paint();
    }
    function tick() {
      if (rec && !rec.done && rec.active && now() - rec.t0 >= rec.steps[rec.i].ms) {
        endStep();
        rec.i += 1;
        if (rec.i >= rec.steps.length) finish(); else beginStep();
      }
      paint();
    }
    function record(opts) {
      if (rec && !rec.done) return false;
      const sw = baseSw();
      const o = Object.assign({ stepMs: 15000 }, opts);
      const canSkip = Boolean(ctx.setRenderSkip) && refresh && refresh.hz >= 100 && Boolean(session.setSwitch);
      results.length = 0;
      resultReport = null;
      rec = {
        steps: planSteps(sw, { stepMs: o.stepMs, skipStep: canSkip && o.skipStep !== false }),
        i: 0, t0: 0, settleUntil: 0, done: false, active: false, data: null,
        restore: sw, restoreSkip: ctx.getRenderSkip ? ctx.getRenderSkip() : false,
      };
      btnRec.disabled = true;
      btnRec.textContent = "Recording...";
      btnCopy.hidden = true;
      area.hidden = true;
      beginStep();
      paint();
      return true;
    }
    function report() {
      return buildReport({
        when: new Date().toISOString().slice(0, 16) + "Z",
        page: pageInfo(),
        device: Object.assign(deviceInfo(win), adapterInfo && adapterInfo.uaHigh ? { uaHigh: adapterInfo.uaHigh } : {}),
        adapter: adapterInfo ? { webgpu: adapterInfo.webgpu, webgl2: adapterInfo.webgl2 } : null,
        refresh,
        canvas: canvasInfo(),
        scale: scaleInfo(),
        quality: ctx.getQuality ? ctx.getQuality() : null,
        thermal: thermalGuess(buckets),
        overlayMs,
        longTasks,
        steps: results.slice(),
      });
    }
    async function copyJson() {
      const json = JSON.stringify(resultReport || report());
      try {
        if (win.navigator.clipboard && win.navigator.clipboard.writeText) {
          await win.navigator.clipboard.writeText(json);
          btnCopy.textContent = "Copied";
          setTimeout(() => { btnCopy.textContent = "Copy JSON"; }, 1500);
          return "clipboard";
        }
      } catch (_) { /* fall through to the text area */ }
      area.hidden = false;
      area.value = json;
      area.focus();
      area.select();
      let ok = false;
      try { ok = doc.execCommand && doc.execCommand("copy"); } catch (_) { ok = false; }
      btnCopy.textContent = ok ? "Copied" : "Select + copy below";
      setTimeout(() => { btnCopy.textContent = "Copy JSON"; }, 2500);
      return ok ? "execCommand" : "textarea";
    }
    btnRec.onclick = () => record();
    btnCopy.onclick = () => { copyJson(); };

    try {
      if (win.PerformanceObserver) {
        const po = new win.PerformanceObserver((list) => { longTasks += list.getEntries().length; });
        po.observe({ entryTypes: ["longtask"] });
      }
    } catch (_) { /* not supported */ }
    gatherAdapter(win).then((a) => { adapterInfo = a; }).catch(() => { adapterInfo = {}; });

    win.requestAnimationFrame(onFrame);
    const timer = win.setInterval(tick, 250);
    const api = { record, report, copyJson, live, results, buckets, stop() { win.clearInterval(timer); root_.remove(); } };
    Object.defineProperty(api, "recording", { get: () => Boolean(rec && !rec.done) });
    Object.defineProperty(api, "refresh", { get: () => refresh });
    win.kagraPerf = api;
    paint();
    return api;
  }

  return {
    percentile, estimateRefresh, summarize, grade, thermalGuess, planSteps,
    deviceInfo, gatherAdapter, buildReport, formatTable, adapterLine, start,
  };
});
