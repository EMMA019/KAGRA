// Runs the <kagra-audio> block of scripts/preview_site/play.html under node with a
// fake AudioContext and checks the synced BGM start used by HOP-BEAT.
import fs from "node:fs";

const html = fs.readFileSync(process.argv[2], "utf8");
const block = html.match(/\/\/ <kagra-audio>([\s\S]*?)\/\/ <\/kagra-audio>/)[1];
const { createAudio } = new Function(`${block}\nreturn { createAudio };`)();

const log = { sources: [], ctx: null };
class FakeCtx {
  constructor() { this.state = "suspended"; this.currentTime = 10; this.outputLatency = 0.04; this.destination = {}; log.ctx = this; }
  createGain() { return { gain: { value: 1, setTargetAtTime() {} }, connect() {} }; }
  createBuffer(c, n, r) { return { silent: true }; }
  createBufferSource() {
    const s = { loop: false, playbackRate: { value: 1 }, connect() {}, start(at, off) { this.at = at; this.off = off; }, stop() { this.stopped = true; } };
    log.sources.push(s);
    return s;
  }
  decodeAudioData(ab, ok) { ok({ name: "loop", duration: 60.952 }); }
  resume() { this.state = "running"; return Promise.resolve(); }
  suspend() { this.state = "suspended"; return Promise.resolve(); }
}
const a = createAudio(
  { enabled: true, master: 1, bgm_volume: 1, sfx_volume: 1, bgm: { file: "loop.mp3", volume: 1, fade_secs: 0 }, sfx: {} },
  { AudioContextCtor: FakeCtx, muted: false, warn() {}, fetchBytes: async () => new ArrayBuffer(4) },
);
const out = {};
let pos = 5;
a.setBgmPos(() => pos);
a.unlock();
await new Promise((r) => setTimeout(r, 20));
out.before_start = Number.isNaN(a.songClock());
log.ctx.state = "suspended";
a.pump({ oneshots: [], bgm_name: "loop", bgm_gain: 1 });
out.not_started_while_suspended = log.sources.filter((s) => s.loop).length;
log.ctx.state = "running";
a.pump({ oneshots: [], bgm_name: "loop", bgm_gain: 1 });
const src = log.sources.filter((s) => s.loop);
out.sources = src.length;
out.loop = src[0].loop;
out.start_offset = src[0].off;
out.start_at = src[0].at;
// the clock reads the music position that is being heard (latency taken off)
log.ctx.currentTime = 12;
out.clock = a.songClock();
// a second pump does not restart the loop
a.pump({ oneshots: [], bgm_name: "loop", bgm_gain: 1 });
out.sources_after = log.sources.filter((s) => s.loop).length;
// a start position past the loop length wraps inside the buffer
a.pump({ oneshots: [], bgm_name: "", bgm_gain: 0 });
out.stopped = src[0].stopped === true;
out.clock_when_stopped = Number.isNaN(a.songClock());
pos = 70;
a.pump({ oneshots: [], bgm_name: "loop", bgm_gain: 1 });
const again = log.sources.filter((s) => s.loop)[1];
out.wrapped_offset = again.off;
console.log(JSON.stringify(out));
