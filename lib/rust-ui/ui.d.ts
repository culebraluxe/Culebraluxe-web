/* tslint:disable */
/* eslint-disable */

/**
 * The DOM event name the host listens for. Named here, next to the shell that emits it, because a typo in a string
 * that only the TypeScript side knows about is a bug that fails silently.
 */
export function effect_event_name(): string;

export function island_event_name(): string;

/**
 * Mount the program into `element_id`, opening `start` — a screen key such as `site-home` or `dashboard` — and return
 * the effects the host must run, as a JSON array.
 *
 * `generation` IS A `u32` ON PURPOSE, and it is the reason every screen using this path died at once. wasm-bindgen maps
 * `u64` to a JavaScript **BigInt**, and the host passes a plain number, so a `u64` parameter fails at the boundary with
 * "Invalid argument type in ToBigInt operation" — before `mount` runs, which is why nothing rendered and nothing said
 * why. A counter of mounts never needs more than four billion.
 *
 * The starting screen comes from the host because the host owns the URL. An unknown key is refused rather than
 * quietly opening the first screen: a page that renders the wrong screen without saying so is a bug that gets
 * debugged twice.
 */
export function mount(element_id: string, start: string, generation: number): string;

/**
 * The mount container id, so the host and the shell cannot disagree about it in silence.
 */
export function mount_id(): string;

/**
 * The same bridge for a page: the host fetched its blocks from an application route, and it names the screen and the
 * mount they were fetched for so the reducer can refuse an answer whose screen has moved on.
 */
export function page_loaded(screen: string, generation: number, payload: string): void;

/**
 * The typed bridge: the host fetched the rows from an application route, and this is how they land.
 *
 * `screen` AND `generation` ARE PART OF THE ANSWER. The host says which screen it fetched for and which mount asked;
 * the reducer refuses the payload if either has moved on (`update::owns`). Without them a response for one screen can
 * land while another is mounted — the browser shows the page it was told to and the model holds a different one.
 *
 * The host owns the network on purpose. It holds the session; this module holds no credential, so a compromised view
 * layer cannot be talked into fetching somewhere else.
 */
export function rows_loaded(screen: string, generation: number, payload: string): void;

/**
 * Mount the Yew application into `element_id`.
 *
 * THE ENTRY POINT A PAGE CALLS, and the only thing the browser needs from this crate now: the module boots, this
 * function takes the container, and from there the router owns the URL — so a deep link, a refresh and the back button
 * are the router's business rather than a `start` prop a page has to work out.
 */
export function yew_mount(element_id: string): void;

export type InitInput = RequestInfo | URL | Response | BufferSource | WebAssembly.Module;

export interface InitOutput {
    readonly memory: WebAssembly.Memory;
    readonly effect_event_name: () => [number, number];
    readonly island_event_name: () => [number, number];
    readonly mount: (a: number, b: number, c: number, d: number, e: number) => [number, number, number, number];
    readonly mount_id: () => [number, number];
    readonly page_loaded: (a: number, b: number, c: number, d: number, e: number) => [number, number];
    readonly rows_loaded: (a: number, b: number, c: number, d: number, e: number) => [number, number];
    readonly yew_mount: (a: number, b: number) => [number, number];
    readonly wasm_bindgen_4d678d08ce442f14___convert__closures_____invoke___wasm_bindgen_4d678d08ce442f14___JsValue__core_ed718c3d60ebd546___result__Result_____wasm_bindgen_4d678d08ce442f14___JsError___true_: (a: number, b: number, c: any) => [number, number];
    readonly wasm_bindgen_4d678d08ce442f14___convert__closures________invoke___web_sys_aa02f1f3172a77c1___features__gen_Event__Event______true_: (a: number, b: number, c: any) => void;
    readonly wasm_bindgen_4d678d08ce442f14___convert__closures________invoke___web_sys_aa02f1f3172a77c1___features__gen_Event__Event______true__1_: (a: number, b: number, c: any) => void;
    readonly wasm_bindgen_4d678d08ce442f14___convert__closures_____invoke___web_sys_aa02f1f3172a77c1___features__gen_CloseEvent__CloseEvent______true_: (a: number, b: number, c: any) => void;
    readonly wasm_bindgen_4d678d08ce442f14___convert__closures_____invoke___web_sys_aa02f1f3172a77c1___features__gen_MouseEvent__MouseEvent______true_: (a: number, b: number, c: any) => void;
    readonly wasm_bindgen_4d678d08ce442f14___convert__closures_____invoke_______true_: (a: number, b: number) => void;
    readonly __wbindgen_malloc: (a: number, b: number) => number;
    readonly __wbindgen_realloc: (a: number, b: number, c: number, d: number) => number;
    readonly __externref_table_alloc: () => number;
    readonly __wbindgen_externrefs: WebAssembly.Table;
    readonly __wbindgen_exn_store: (a: number) => void;
    readonly __wbindgen_free: (a: number, b: number, c: number) => void;
    readonly __externref_drop_slice: (a: number, b: number) => void;
    readonly __wbindgen_destroy_closure: (a: number, b: number) => void;
    readonly __externref_table_dealloc: (a: number) => void;
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
