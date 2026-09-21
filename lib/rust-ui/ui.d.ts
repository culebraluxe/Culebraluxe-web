/* tslint:disable */
/* eslint-disable */

/**
 * The DOM event name the host listens for. Named here, next to the shell that emits it, because a typo in a string
 * that only the TypeScript side knows about is a bug that fails silently.
 */
export function effect_event_name(): string;

/**
 * Mount the program into `element_id`, opening `start` — a screen key such as `site-home` or `dashboard` — and return
 * the effects the host must run, as a JSON array.
 *
 * The starting screen comes from the host because the host owns the URL. An unknown key is refused rather than
 * quietly opening the first screen: a page that renders the wrong screen without saying so is a bug that gets
 * debugged twice.
 */
export function mount(element_id: string, start: string): string;

/**
 * The mount container id, so the host and the shell cannot disagree about it in silence.
 */
export function mount_id(): string;

/**
 * The typed bridge: the host fetched the rows from an application route, and this is how they land.
 *
 * The host owns the network on purpose. It holds the session; this module holds no credential, so a compromised view
 * layer cannot be talked into fetching somewhere else.
 */
export function rows_loaded(payload: string): void;

export type InitInput = RequestInfo | URL | Response | BufferSource | WebAssembly.Module;

export interface InitOutput {
    readonly memory: WebAssembly.Memory;
    readonly effect_event_name: () => [number, number];
    readonly mount: (a: number, b: number, c: number, d: number) => [number, number, number, number];
    readonly mount_id: () => [number, number];
    readonly rows_loaded: (a: number, b: number) => [number, number];
    readonly wasm_bindgen_2e2b1a5be97eea88___convert__closures_____invoke___web_sys_17113061404c37eb___features__gen_MouseEvent__MouseEvent______true_: (a: number, b: number, c: any) => void;
    readonly wasm_bindgen_2e2b1a5be97eea88___convert__closures_____invoke___web_sys_17113061404c37eb___features__gen_MouseEvent__MouseEvent______true__5: (a: number, b: number, c: any) => void;
    readonly __externref_table_alloc: () => number;
    readonly __wbindgen_externrefs: WebAssembly.Table;
    readonly __wbindgen_exn_store: (a: number) => void;
    readonly __wbindgen_free: (a: number, b: number, c: number) => void;
    readonly __wbindgen_malloc: (a: number, b: number) => number;
    readonly __wbindgen_realloc: (a: number, b: number, c: number, d: number) => number;
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
