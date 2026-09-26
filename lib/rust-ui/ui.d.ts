/* tslint:disable */
/* eslint-disable */

/**
 * Start the application in the container the PAGE owns, handed over by reference.
 *
 * WHY THIS EXISTS, AND WHY IT IS THE ONE THE PAGES CALL. `#rust-ui` is an id, and an id is not an identity: during a
 * client-side navigation Next renders the new route while the old one is still in the document, so two containers can
 * carry that id at once and a lookup answers with the FIRST one in document order — the page the user is leaving. The
 * module would then mount the new screen into a container that was about to be removed, and the container the user
 * could actually see stayed empty. Handing over the node React owns removes the guess: there is no lookup left to get
 * wrong, and the page may hold the only container that exists.
 */
export function start_in(root: HTMLElement): void;

export type InitInput = RequestInfo | URL | Response | BufferSource | WebAssembly.Module;

export interface InitOutput {
    readonly memory: WebAssembly.Memory;
    readonly start_in: (a: any) => void;
    readonly wasm_bindgen_4d678d08ce442f14___convert__closures_____invoke___wasm_bindgen_4d678d08ce442f14___JsValue__core_ed718c3d60ebd546___result__Result_____wasm_bindgen_4d678d08ce442f14___JsError___true_: (a: number, b: number, c: any) => [number, number];
    readonly wasm_bindgen_4d678d08ce442f14___convert__closures________invoke___web_sys_13a8c5d0ec4e83e8___features__gen_Event__Event______true_: (a: number, b: number, c: any) => void;
    readonly wasm_bindgen_4d678d08ce442f14___convert__closures________invoke___web_sys_13a8c5d0ec4e83e8___features__gen_Event__Event______true__1_: (a: number, b: number, c: any) => void;
    readonly wasm_bindgen_4d678d08ce442f14___convert__closures_____invoke___web_sys_13a8c5d0ec4e83e8___features__gen_CloseEvent__CloseEvent______true_: (a: number, b: number, c: any) => void;
    readonly wasm_bindgen_4d678d08ce442f14___convert__closures_____invoke___web_sys_13a8c5d0ec4e83e8___features__gen_MouseEvent__MouseEvent______true_: (a: number, b: number, c: any) => void;
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
