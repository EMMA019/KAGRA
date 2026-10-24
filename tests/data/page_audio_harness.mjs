// Runs the <kagra-audio> and <kagra-goals> blocks of scripts/preview_site/play.html
// under node with a fake AudioContext. Prints one JSON object of results.
import fs from "node:fs";
import { createRequire } from "node:module";

const html = fs.readFileSync(process.argv[2], "utf8");
const block = (name) => html.match(new RegExp(`// <${name}>([\\s\\S]*?)// </${name}>`))[1];
const require = createRequire(import.meta.url);
const I18n = require("../../scripts/preview_site/i18n.js");
// The goal block calls the page's `t`; build the page's blocks once per language.
const load = (lang) => new Function(
  "t",
  `${block("kagra-audio")}\n${block("kagra-goals")}\nreturn { createAudio, goalText };`,
)(I18n.create(lang).t);
const { createAudio, goalText } = load("ja");
const en = load("en");

class FakeParam {
  constructor() { this.value = 1; this.targets = []; }
  setTargetAtTime(v, t, k) { this.targets.push([v, t, k]); this.value = v; }
}
const makeCtx = (log) => class FakeCtx {
  constructor() {
    this.state = "suspended";
    this.currentTime = 0;
    this.destination = { name: "dest" };
    log.contexts.push(this);
  }
  createGain() { const g = { gain: new FakeParam(), to: null, connect(n) { this.to = n; } }; log.gains.push(g); return g; }
  createBuffer(c, n, r) { return { silent: true, c, n, r }; }
  createBufferSource() {
    const src = {
      buffer: null, loop: false, playbackRate: { value: 1 }, started: 0, stopped: 0, onended: null, to: null,
      connect(n) { this.to = n; },
      start() { this.started += 1; },
      stop() { this.stopped += 1; if (this.onended) this.onended(); },
    };
    log.sources.push(src);
    return src;
  }
  decodeAudioData(ab, ok, bad) {
    const bytes = new Uint8Array(ab);
    if (bytes[0] === 0xff) { bad(new Error("not audio")); return; }
    ok({ name: String.fromCharCode(...bytes) });
  }
  resume() { this.state = "running"; log.resumes += 1; return Promise.resolve(); }
  suspend() { this.state = "suspended"; log.suspends += 1; return Promise.resolve(); }
};

const cfg = () => ({
  enabled: true, master: 0.8, bgm_volume: 0.5, sfx_volume: 0.5,
  bgm: { file: "a/bgm.wav", volume: 0.5, fade_secs: 1 },
  sfx: {
    coin: { file: "a/coin.mp3", volume: 0.6, rate: 1.5 },
    hit: { file: "a/hit.ogg", volume: 1, rate: 1 },
    lost: { file: "a/missing.wav", volume: 1, rate: 1 },
    quiet: { file: "", volume: 1, rate: 1 },
  },
});
const settle = () => new Promise((r) => setTimeout(r, 20));
const out = {};
const fresh = (over = {}, spec = cfg()) => {
  const log = { contexts: [], gains: [], sources: [], resumes: 0, suspends: 0, fetched: [], warns: [] };
  const a = createAudio(spec, {
    AudioContextCtor: over.noCtor ? undefined : makeCtx(log),
    muted: !!over.muted,
    warn: (...x) => log.warns.push(x.map(String).join(" ")),
    fetchBytes: async (f) => {
      log.fetched.push(f);
      if (f.endsWith("missing.wav")) throw new Error("404");
      const b = new TextEncoder().encode(f);
      return b.buffer.slice(b.byteOffset, b.byteOffset + b.byteLength);
    },
  });
  return { a, log };
};
const cues = (oneshots, name = "", gain = 0) => JSON.stringify({ oneshots, bgm_name: name, bgm_gain: gain });

{ // nothing happens before a touch; fetching starts at once
  const { a, log } = fresh();
  a.pump(cues(["coin"], "bgm", 1));
  await settle();
  out.before_unlock = { contexts: log.contexts.length, sources: log.sources.length, fetched: [...log.fetched].sort() };
}
{ // unlock builds the graph, resumes and primes; cues then play the right file at the right level
  const { a, log } = fresh();
  a.unlock();
  await settle();
  out.after_unlock = {
    contexts: log.contexts.length, state: a.state, resumes: log.resumes, loaded: a.loaded, expected: a.expected,
    primed: log.sources.filter((s) => s.buffer && s.buffer.silent).length,
    warns: log.warns,
  };
  log.sources.length = 0;
  a.pump(cues(["coin", "hit", "nope", "quiet", "lost"]));
  const shots = log.sources.map((s) => ({
    file: s.buffer.name, rate: s.playbackRate.value, gain: s.to.gain.value, started: s.started,
    toSfx: s.to.to === log.gains[2],
  }));
  out.shots = shots;
  out.group_gains = { master: log.gains[0].gain.value, bgm: log.gains[1].gain.value, sfx: log.gains[2].gain.value };
  log.sources.length = 0;
  a.pump(cues(["coin"]));
  out.same_instant_dedup = log.sources.length;
  log.contexts[0].currentTime = 1;
  a.pump(cues(["coin"]));
  out.later_again = log.sources.length;
  // unlock twice does not build a second context
  a.unlock();
  out.contexts_after_second_unlock = log.contexts.length;
}
{ // BGM: starts once, loops, level = bgm_volume * bgm.volume * mixer gain, stops on empty name
  const { a, log } = fresh();
  a.unlock();
  await settle();
  log.sources.length = 0;
  a.pump(cues([], "bgm", 0.5));
  a.pump(cues([], "bgm", 1));
  const loops = log.sources.filter((s) => s.loop);
  out.bgm = {
    sources: loops.length, buffer: loops[0].buffer.name, started: loops[0].started,
    target: log.gains[1].gain.targets.map((t) => t[0]),
  };
  a.pump(cues([], "", 0));
  out.bgm_stopped = loops[0].stopped;
  a.pump(cues([], "bgm", 1));
  out.bgm_restart = log.sources.filter((s) => s.loop).length;
}
{ // mute silences the master and the cues, but still drains
  const { a, log } = fresh({ muted: true });
  a.unlock();
  await settle();
  out.muted_master = log.gains[0].gain.value;
  log.sources.length = 0;
  a.pump(cues(["coin"]));
  out.muted_shots = log.sources.length;
  a.setMuted(false);
  out.unmuted_master = log.gains[0].gain.value;
  log.contexts[0].currentTime = 2;
  a.pump(cues(["coin"]));
  out.unmuted_shots = log.sources.length;
}
{ // disabled spec, and no AudioContext at all
  const off = cfg(); off.enabled = false;
  const { a, log } = fresh({}, off);
  a.unlock();
  out.disabled = { usable: a.usable, contexts: log.contexts.length, fetched: log.fetched.length };
  const none = fresh({ noCtor: true });
  none.a.unlock();
  none.a.pump(cues(["coin"], "bgm", 1));
  out.no_ctor = { usable: none.a.usable };
}
{ // tab hidden / back
  const { a, log } = fresh();
  a.unlock();
  a.hidden(true);
  const s1 = a.state;
  a.hidden(false);
  out.visibility = [s1, a.state];
}
out.goal_text = [
  goalText({ kind: "coins", label: "COINS", target: 40 }),
  goalText({ kind: "gates", label: "GATES", target: 5 }),
  goalText({ kind: "perch", label: "PERCH", target: 1 }),
  goalText({ kind: "coins", label: "TUNA", target: 3 }),
  goalText({ kind: "passes", label: "PASSED", target: 30 }),
];
out.goal_text_en = [
  en.goalText({ kind: "coins", label: "COINS", target: 40 }),
  en.goalText({ kind: "gates", label: "GATES", target: 5 }),
  en.goalText({ kind: "perch", label: "PERCH", target: 1 }),
  en.goalText({ kind: "passes", label: "PASSED", target: 30 }),
];
console.log(JSON.stringify(out));
