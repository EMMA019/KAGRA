/* tslint:disable */
/* eslint-disable */

export class WasmLockstep {
    free(): void;
    [Symbol.dispose](): void;
    ingest(json: string): void;
    constructor(slot: number, players: number);
    ready(tick: number): boolean;
    submitLocal(tick: number, lx: number, lz: number, jump: boolean, attack: boolean, dodge: boolean): string;
}

export class WasmSession {
    free(): void;
    [Symbol.dispose](): void;
    /**
     * canvas を描画先にする。WebGPU / WebGL2 のどちらかが使えれば成功する。
     *
     * アダプタ取得が非同期なので JS からは `await session.attachCanvas(canvas)`。
     * `createSurface` before this only stores the size. The canvas's own
     * width/height is what the swapchain is configured with, and
     * `attach_renderer` applies it again so a pre-attach resize is not lost.
     */
    attachCanvas(canvas: HTMLCanvasElement): Promise<void>;
    /**
     * Mixer cues for the page to play (wasm has no speakers of its own).
     */
    audioCuesJson(): string;
    /**
     * 音声レベル JSON（engine / wind / brake）。再生はブラウザ側。
     */
    audioJson(): string;
    clearMount(): void;
    createSurface(width: number, height: number): void;
    /**
     * ゲーム状態 JSON（phase / time / score / objective）。
     */
    gameJson(): string;
    /**
     * 描画先が接続済みか。
     */
    hasRenderer(): boolean;
    /**
     * World dump JSON. The scene waits on the title until a tap confirms.
     */
    loadGame(json: string): void;
    /**
     * ジャンル dump。`"genre": "neon_run"` は Lumen を起動する。
     */
    loadGameJson(json: string): void;
    /**
     * セーブ JSON を読み込んで状態を復元する。
     */
    loadJson(json: string): void;
    /**
     * Repo-relative file for [`crate::assets::resolve_asset`] (no filesystem on wasm).
     */
    mountFile(path: string, bytes: Uint8Array): void;
    constructor();
    pause(): void;
    playtestAction(action: string): void;
    playtestPacketJson(): string;
    /**
     * Gesture-only pointer. Does not start the game.
     */
    playtestPointer(id: number, x: number, y: number, phase: number, pressure: number): boolean;
    playtestStateJson(): string;
    pushKey(name: string, down: boolean): boolean;
    pushPointer(id: number, x: number, y: number, phase: number, pressure: number): void;
    /**
     * 現在のシーンを 1 枚描く。`requestFrame()` の後に呼ぶ。
     */
    render(): void;
    /**
     * この wasm が描画機能付きでビルドされているか。
     */
    renderSupported(): boolean;
    requestFrame(): number;
    resume(): void;
    /**
     * セーブ JSON（pretty）。シェルが localStorage 等へ書き出す。
     */
    saveJson(): string;
    setAssetRoot(root: string): void;
    /**
     * 連続値のドライバ入力。`steer` は -1..1、`throttle` と `brake` は 0..1。
     */
    setDrive(steer: number, throttle: number, brake: number): void;
    /**
     * `"auto"` (default), `"webgpu"`, or `"gl"` / `"webgl2"`. Call before
     * `attachCanvas`. `?backend=gl` on the page forces WebGL2; a blank
     * WebGPU frame does the same with a fresh canvas.
     */
    setGraphicsBackend(name: string): void;
    setJump(held: boolean): void;
    setPad(x: number, y: number): void;
    /**
     * Packaged play. Sticks are -1..1. Lumen uses `lx`/`lz` to steer and `jump` to boost.
     */
    setPlayInput(lx: number, lz: number, jump: boolean, attack: boolean, dodge: boolean): void;
    setPlaytestDevice(label: string, lang: string): void;
    setPlaytestEnabled(on: boolean): void;
    setPlaytestExternalChrome(on: boolean): void;
    setPlaytestNote(note: string): void;
    /**
     * 0 = 運転（3D）、1 = タッチデモ（2D）、2 = Crest Isle。
     */
    setScene(kind: number): void;
    /**
     * `muted` は真なら無音。
     */
    setSettings(master_volume: number, steer_sensitivity: number, muted: boolean): void;
    /**
     * Crest Isle の歩き。`lx`/`lz` は -1..1。
     */
    setWalk(lx: number, lz: number, jump: boolean): void;
    set_playtest_shot_path(path: string): void;
    /**
     * タイトル画面へ。
     */
    showTitle(): void;
    /**
     * 速度（km/h）。HUD をブラウザ側で出すとき用。
     */
    speedKmh(): number;
    /**
     * 配送ランを開始／再スタート。
     */
    startGame(): void;
    statsJson(): string;
}

export function netDecode(json: string): string;

export function netDt(num: number, den: number): number;

export function netHello(room: string, name: string): string;

export function sharedVersion(): string;

export function wasm_start(): void;

export type InitInput = RequestInfo | URL | Response | BufferSource | WebAssembly.Module;

export interface InitOutput {
    readonly memory: WebAssembly.Memory;
    readonly kagra_shared_attach_android_surface: (a: number, b: number, c: number, d: number) => number;
    readonly kagra_shared_attach_ios_view: (a: number, b: number, c: number, d: number) => number;
    readonly kagra_shared_attach_offscreen: (a: number, b: number, c: number) => number;
    readonly kagra_shared_audio_json: (a: number, b: number, c: number) => number;
    readonly kagra_shared_create: () => number;
    readonly kagra_shared_create_surface: (a: number, b: number, c: number) => number;
    readonly kagra_shared_destroy: (a: number) => void;
    readonly kagra_shared_detach_surface: (a: number) => number;
    readonly kagra_shared_has_renderer: (a: number) => number;
    readonly kagra_shared_last_error: () => number;
    readonly kagra_shared_load_game: (a: number, b: number) => number;
    readonly kagra_shared_load_game_file: (a: number, b: number) => number;
    readonly kagra_shared_load_json: (a: number, b: number) => number;
    readonly kagra_shared_pause: (a: number) => number;
    readonly kagra_shared_playtest_action: (a: number, b: number) => number;
    readonly kagra_shared_playtest_blocks_input: (a: number) => number;
    readonly kagra_shared_playtest_packet_json: (a: number, b: number, c: number) => number;
    readonly kagra_shared_playtest_pointer: (a: number, b: number, c: number, d: number, e: number, f: number) => number;
    readonly kagra_shared_playtest_push_text: (a: number, b: number) => number;
    readonly kagra_shared_playtest_set_note: (a: number, b: number) => number;
    readonly kagra_shared_playtest_set_shot_path: (a: number, b: number) => number;
    readonly kagra_shared_playtest_state_json: (a: number, b: number, c: number) => number;
    readonly kagra_shared_push_key: (a: number, b: number, c: number) => number;
    readonly kagra_shared_push_pointer: (a: number, b: number, c: number, d: number, e: number, f: number) => number;
    readonly kagra_shared_render: (a: number) => number;
    readonly kagra_shared_request_frame: (a: number) => bigint;
    readonly kagra_shared_resolve_alias: (a: number, b: number, c: number, d: number) => number;
    readonly kagra_shared_resume: (a: number) => number;
    readonly kagra_shared_save_json: (a: number, b: number, c: number) => number;
    readonly kagra_shared_set_asset_root: (a: number, b: number) => number;
    readonly kagra_shared_set_drive: (a: number, b: number, c: number, d: number) => number;
    readonly kagra_shared_set_jump: (a: number, b: number) => number;
    readonly kagra_shared_set_pad: (a: number, b: number, c: number) => number;
    readonly kagra_shared_set_play_input: (a: number, b: number, c: number, d: number, e: number, f: number) => number;
    readonly kagra_shared_set_playtest_device: (a: number, b: number, c: number) => number;
    readonly kagra_shared_set_playtest_enabled: (a: number, b: number) => number;
    readonly kagra_shared_set_playtest_external_chrome: (a: number, b: number) => number;
    readonly kagra_shared_set_save_root: (a: number) => number;
    readonly kagra_shared_set_scene: (a: number, b: number) => number;
    readonly kagra_shared_set_settings: (a: number, b: number, c: number, d: number) => number;
    readonly kagra_shared_set_walk: (a: number, b: number, c: number, d: number) => number;
    readonly kagra_shared_stats_json: (a: number, b: number, c: number) => number;
    readonly kagra_shared_take_audio_cues: (a: number, b: number) => number;
    readonly kagra_shared_version: () => number;
    readonly kagra_net_hello: (a: number, b: number, c: number, d: number) => number;
    readonly kagra_net_ingest: (a: number, b: number) => number;
    readonly kagra_net_input_axis: (a: number, b: number, c: number, d: number, e: number, f: number) => number;
    readonly kagra_net_lockstep_free: (a: number) => void;
    readonly kagra_net_lockstep_new: (a: number, b: number) => number;
    readonly kagra_net_ready: (a: number, b: number) => number;
    readonly kagra_net_submit_local: (a: number, b: number, c: number, d: number, e: number, f: number, g: number) => number;
    readonly __wbg_wasmsession_free: (a: number, b: number) => void;
    readonly sharedVersion: () => [number, number];
    readonly wasm_start: () => void;
    readonly wasmsession_attachCanvas: (a: number, b: any) => any;
    readonly wasmsession_audioCuesJson: (a: number) => [number, number];
    readonly wasmsession_audioJson: (a: number) => [number, number];
    readonly wasmsession_clearMount: (a: number) => void;
    readonly wasmsession_createSurface: (a: number, b: number, c: number) => void;
    readonly wasmsession_gameJson: (a: number) => [number, number];
    readonly wasmsession_hasRenderer: (a: number) => number;
    readonly wasmsession_loadGame: (a: number, b: number, c: number) => [number, number];
    readonly wasmsession_loadJson: (a: number, b: number, c: number) => [number, number];
    readonly wasmsession_mountFile: (a: number, b: number, c: number, d: number, e: number) => void;
    readonly wasmsession_new: () => number;
    readonly wasmsession_pause: (a: number) => void;
    readonly wasmsession_playtestAction: (a: number, b: number, c: number) => void;
    readonly wasmsession_playtestPacketJson: (a: number) => [number, number];
    readonly wasmsession_playtestPointer: (a: number, b: number, c: number, d: number, e: number, f: number) => number;
    readonly wasmsession_playtestStateJson: (a: number) => [number, number];
    readonly wasmsession_pushKey: (a: number, b: number, c: number, d: number) => number;
    readonly wasmsession_pushPointer: (a: number, b: number, c: number, d: number, e: number, f: number) => void;
    readonly wasmsession_render: (a: number) => [number, number];
    readonly wasmsession_renderSupported: (a: number) => number;
    readonly wasmsession_requestFrame: (a: number) => number;
    readonly wasmsession_resume: (a: number) => void;
    readonly wasmsession_saveJson: (a: number) => [number, number, number, number];
    readonly wasmsession_setAssetRoot: (a: number, b: number, c: number) => void;
    readonly wasmsession_setDrive: (a: number, b: number, c: number, d: number) => void;
    readonly wasmsession_setGraphicsBackend: (a: number, b: number, c: number) => void;
    readonly wasmsession_setJump: (a: number, b: number) => void;
    readonly wasmsession_setPad: (a: number, b: number, c: number) => void;
    readonly wasmsession_setPlayInput: (a: number, b: number, c: number, d: number, e: number, f: number) => void;
    readonly wasmsession_setPlaytestDevice: (a: number, b: number, c: number, d: number, e: number) => void;
    readonly wasmsession_setPlaytestEnabled: (a: number, b: number) => void;
    readonly wasmsession_setPlaytestExternalChrome: (a: number, b: number) => void;
    readonly wasmsession_setPlaytestNote: (a: number, b: number, c: number) => void;
    readonly wasmsession_setScene: (a: number, b: number) => void;
    readonly wasmsession_setSettings: (a: number, b: number, c: number, d: number) => void;
    readonly wasmsession_setWalk: (a: number, b: number, c: number, d: number) => void;
    readonly wasmsession_set_playtest_shot_path: (a: number, b: number, c: number) => void;
    readonly wasmsession_showTitle: (a: number) => void;
    readonly wasmsession_speedKmh: (a: number) => number;
    readonly wasmsession_startGame: (a: number) => void;
    readonly wasmsession_statsJson: (a: number) => [number, number];
    readonly wasmsession_loadGameJson: (a: number, b: number, c: number) => [number, number];
    readonly __wbg_wasmlockstep_free: (a: number, b: number) => void;
    readonly netDecode: (a: number, b: number) => [number, number, number, number];
    readonly netDt: (a: number, b: number) => number;
    readonly netHello: (a: number, b: number, c: number, d: number) => [number, number, number, number];
    readonly wasmlockstep_ingest: (a: number, b: number, c: number) => [number, number];
    readonly wasmlockstep_new: (a: number, b: number) => [number, number, number];
    readonly wasmlockstep_ready: (a: number, b: number) => number;
    readonly wasmlockstep_submitLocal: (a: number, b: number, c: number, d: number, e: number, f: number, g: number) => [number, number, number, number];
    readonly wasm_bindgen_42bf64e63cc180fa___convert__closures_____invoke___wasm_bindgen_42bf64e63cc180fa___JsValue__core_37ee0ce0c4cd57d5___result__Result_____wasm_bindgen_42bf64e63cc180fa___JsError___true_: (a: number, b: number, c: any) => [number, number];
    readonly wasm_bindgen_42bf64e63cc180fa___convert__closures_____invoke___wasm_bindgen_42bf64e63cc180fa___sys__JsOption_wgpu_51a115929c565234___backend__webgpu__webgpu_sys__gen_GpuError__GpuError___core_37ee0ce0c4cd57d5___result__Result_____wasm_bindgen_42bf64e63cc180fa___JsError___true_: (a: number, b: number, c: any) => [number, number];
    readonly wasm_bindgen_42bf64e63cc180fa___convert__closures_____invoke___wasm_bindgen_42bf64e63cc180fa___sys__JsOption_wgpu_51a115929c565234___backend__webgpu__webgpu_sys__gen_GpuError__GpuError___core_37ee0ce0c4cd57d5___result__Result_____wasm_bindgen_42bf64e63cc180fa___JsError___true__3: (a: number, b: number, c: any) => [number, number];
    readonly wasm_bindgen_42bf64e63cc180fa___convert__closures_____invoke___wasm_bindgen_42bf64e63cc180fa___sys__JsOption_wgpu_51a115929c565234___backend__webgpu__webgpu_sys__gen_GpuError__GpuError___core_37ee0ce0c4cd57d5___result__Result_____wasm_bindgen_42bf64e63cc180fa___JsError___true__4: (a: number, b: number, c: any) => [number, number];
    readonly wasm_bindgen_42bf64e63cc180fa___convert__closures_____invoke___js_sys_c984a0d025684b7b___Function_fn_wasm_bindgen_42bf64e63cc180fa___JsValue_____wasm_bindgen_42bf64e63cc180fa___sys__Undefined___js_sys_c984a0d025684b7b___Function_fn_wasm_bindgen_42bf64e63cc180fa___JsValue_____wasm_bindgen_42bf64e63cc180fa___sys__Undefined_______true_: (a: number, b: number, c: any, d: any) => void;
    readonly wasm_bindgen_42bf64e63cc180fa___convert__closures_____invoke___wgpu_51a115929c565234___backend__webgpu__webgpu_sys__gen_GpuDeviceLostInfo__GpuDeviceLostInfo______true_: (a: number, b: number, c: any) => void;
    readonly __wbindgen_malloc: (a: number, b: number) => number;
    readonly __wbindgen_realloc: (a: number, b: number, c: number, d: number) => number;
    readonly __wbindgen_exn_store: (a: number) => void;
    readonly __externref_table_alloc: () => number;
    readonly __wbindgen_externrefs: WebAssembly.Table;
    readonly __wbindgen_destroy_closure: (a: number, b: number) => void;
    readonly __externref_table_dealloc: (a: number) => void;
    readonly __wbindgen_free: (a: number, b: number, c: number) => void;
    readonly __wbindgen_start: () => void;
}

export type SyncInitInput = BufferSource | WebAssembly.Module;

/**
 * Instantiates the given `module`, which can either be bytes or
 * a precompiled `WebAssembly.Module`.
 *
 * @param {{ module: SyncInitInput }} module - Passing `SyncInitInput` directly is deprecated.
 *
 * @returns {InitOutput}
 */
export function initSync(module: { module: SyncInitInput } | SyncInitInput): InitOutput;

/**
 * If `module_or_path` is {RequestInfo} or {URL}, makes a request and
 * for everything else, calls `WebAssembly.instantiate` directly.
 *
 * @param {{ module_or_path: InitInput | Promise<InitInput> }} module_or_path - Passing `InitInput` directly is deprecated.
 *
 * @returns {Promise<InitOutput>}
 */
export default function __wbg_init (module_or_path?: { module_or_path: InitInput | Promise<InitInput> } | InitInput | Promise<InitInput>): Promise<InitOutput>;
