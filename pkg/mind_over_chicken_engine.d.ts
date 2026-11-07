/* tslint:disable */
/* eslint-disable */

export class Engine {
    free(): void;
    [Symbol.dispose](): void;
    add_light(x: number, y: number, radius: number, intensity: number): void;
    begin_frame(): void;
    draw_chicken(x: number, y: number, facing: number, frame: number, halo: boolean): void;
    draw_fire(x: number, y: number, heat: number, t: number): void;
    draw_goat(x: number, y: number, facing: number): void;
    draw_lightning(tx: number, ty: number, seed: number, alpha: number): void;
    draw_mammoth(x: number, y: number, facing: number, frame: number, dead: boolean): void;
    draw_person(x: number, y: number, frame: number, pose: number, shirt_r: number, shirt_g: number, shirt_b: number): void;
    draw_pixel(x: number, y: number, r: number, g: number, b: number, a: number): void;
    draw_rect(x: number, y: number, w: number, h: number, r: number, g: number, b: number, a: number): void;
    draw_reticle(x: number, y: number, t: number): void;
    draw_rocket(x: number, y: number, progress: number, t: number, flame: number): void;
    end_frame(night: number, flash: number, fade: number, shake_x: number, shake_y: number, canvas_w: number, canvas_h: number): void;
    get_site_cave_mouth_x(): number;
    get_site_cave_mouth_y(): number;
    get_site_cave_x(): number;
    get_site_cave_y(): number;
    get_site_exchange_x(): number;
    get_site_exchange_y(): number;
    get_site_factory_x(): number;
    get_site_factory_y(): number;
    get_site_hangar_x(): number;
    get_site_hangar_y(): number;
    get_site_pad_x(): number;
    get_site_pad_y(): number;
    get_site_village_x(): number;
    get_site_village_y(): number;
    get_terrain_type(x: number, y: number): number;
    init_world(seed: number): void;
    is_walkable(x: number, y: number): boolean;
    constructor(canvas_id: string);
    render_uploaded_canvas(night: number, flash: number, fade: number, shake_x: number, shake_y: number, canvas_w: number, canvas_h: number): void;
    set_era(era: number): void;
    upload_canvas(canvas: HTMLCanvasElement): void;
}

export type InitInput = RequestInfo | URL | Response | BufferSource | WebAssembly.Module;

export interface InitOutput {
    readonly memory: WebAssembly.Memory;
    readonly __wbg_engine_free: (a: number, b: number) => void;
    readonly engine_add_light: (a: number, b: number, c: number, d: number, e: number) => void;
    readonly engine_begin_frame: (a: number) => void;
    readonly engine_draw_chicken: (a: number, b: number, c: number, d: number, e: number, f: number) => void;
    readonly engine_draw_fire: (a: number, b: number, c: number, d: number, e: number) => void;
    readonly engine_draw_goat: (a: number, b: number, c: number, d: number) => void;
    readonly engine_draw_lightning: (a: number, b: number, c: number, d: number, e: number) => void;
    readonly engine_draw_mammoth: (a: number, b: number, c: number, d: number, e: number, f: number) => void;
    readonly engine_draw_person: (a: number, b: number, c: number, d: number, e: number, f: number, g: number, h: number) => void;
    readonly engine_draw_pixel: (a: number, b: number, c: number, d: number, e: number, f: number, g: number) => void;
    readonly engine_draw_rect: (a: number, b: number, c: number, d: number, e: number, f: number, g: number, h: number, i: number) => void;
    readonly engine_draw_reticle: (a: number, b: number, c: number, d: number) => void;
    readonly engine_draw_rocket: (a: number, b: number, c: number, d: number, e: number, f: number) => void;
    readonly engine_end_frame: (a: number, b: number, c: number, d: number, e: number, f: number, g: number, h: number) => void;
    readonly engine_get_site_cave_mouth_x: (a: number) => number;
    readonly engine_get_site_cave_mouth_y: (a: number) => number;
    readonly engine_get_site_cave_x: (a: number) => number;
    readonly engine_get_site_cave_y: (a: number) => number;
    readonly engine_get_site_exchange_x: (a: number) => number;
    readonly engine_get_site_exchange_y: (a: number) => number;
    readonly engine_get_site_factory_x: (a: number) => number;
    readonly engine_get_site_factory_y: (a: number) => number;
    readonly engine_get_site_hangar_x: (a: number) => number;
    readonly engine_get_site_hangar_y: (a: number) => number;
    readonly engine_get_site_pad_x: (a: number) => number;
    readonly engine_get_site_pad_y: (a: number) => number;
    readonly engine_get_site_village_x: (a: number) => number;
    readonly engine_get_site_village_y: (a: number) => number;
    readonly engine_get_terrain_type: (a: number, b: number, c: number) => number;
    readonly engine_init_world: (a: number, b: number) => void;
    readonly engine_is_walkable: (a: number, b: number, c: number) => number;
    readonly engine_new: (a: number, b: number) => [number, number, number];
    readonly engine_render_uploaded_canvas: (a: number, b: number, c: number, d: number, e: number, f: number, g: number, h: number) => void;
    readonly engine_set_era: (a: number, b: number) => void;
    readonly engine_upload_canvas: (a: number, b: any) => [number, number];
    readonly __wbindgen_malloc: (a: number, b: number) => number;
    readonly __wbindgen_realloc: (a: number, b: number, c: number, d: number) => number;
    readonly __wbindgen_free: (a: number, b: number, c: number) => void;
    readonly __externref_table_alloc: () => number;
    readonly __wbindgen_externrefs: WebAssembly.Table;
    readonly __wbindgen_exn_store: (a: number) => void;
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
