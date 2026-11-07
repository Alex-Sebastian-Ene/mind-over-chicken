/* tslint:disable */
/* eslint-disable */

export class Engine {
    free(): void;
    [Symbol.dispose](): void;
    add_3d_light(x: number, y: number, z: number, radius: number, intensity: number): void;
    add_billboard(sprite_id: number, x: number, z: number, facing: number, frame: number, variant: number): void;
    add_custom_billboard(x: number, y: number, z: number, w: number, h: number, u0: number, v0: number, u1: number, v1: number): void;
    add_light(x: number, y: number, radius: number, intensity: number): void;
    advisor_say(key: string, text: string, mood: string, priority: number, cooldown: number): boolean;
    audio_heartbeat(now: number, bpm: number, intensity: number): void;
    audio_play(sound: string): void;
    audio_toggle(): boolean;
    audio_unlock(): void;
    begin_frame(): void;
    bind_dom_shell(): void;
    clear_3d_billboards(): void;
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
    draw_text(x: number, y: number, text: string, r: number, g: number, b: number, a: number): void;
    draw_text_badge(cx: number, y: number, text: string, text_r: number, text_g: number, text_b: number, bg_r: number, bg_g: number, bg_b: number, bg_a: number): void;
    draw_text_centered(cx: number, y: number, text: string, r: number, g: number, b: number, a: number): void;
    end_frame(night: number, flash: number, fade: number, shake_x: number, shake_y: number, canvas_w: number, canvas_h: number): void;
    game_on_blink(aim_x: number, aim_y: number): void;
    game_on_cast(spell: string, aim_x: number, aim_y: number): void;
    game_restart_era(): void;
    game_start_era(index: number): void;
    game_tick(dt: number, calm: number, focus: number, panic: number, hr: number, rest_hr: number, hr_delta: number, blinks: number): void;
    get_camera_distance(): number;
    get_camera_pitch(): number;
    get_camera_target_x(): number;
    get_camera_target_z(): number;
    get_camera_yaw(): number;
    get_game_births_stat(): number;
    get_game_deaths_stat(): number;
    get_game_era_id(): string;
    get_game_era_index(): number;
    get_game_era_label(): string;
    get_game_lightning_stat(): number;
    get_game_recessions_stat(): number;
    get_game_restarts_stat(): number;
    get_game_runtime(): number;
    get_game_smitten_stat(): number;
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
    init_3d(): void;
    init_world(seed: number): void;
    is_3d_mode(): boolean;
    is_game_failed(): string | undefined;
    is_game_finished(): boolean;
    is_walkable(x: number, y: number): boolean;
    constructor(canvas_id: string);
    orbit_camera(delta_yaw: number, delta_pitch: number): void;
    pan_camera(dx: number, dz: number): void;
    render_3d(dt: number, sun_x: number, sun_y: number, sun_z: number, night: number, flash: number, fade: number, shake_x: number, shake_y: number, canvas_w: number, canvas_h: number): void;
    render_uploaded_canvas(night: number, flash: number, fade: number, shake_x: number, shake_y: number, canvas_w: number, canvas_h: number): void;
    screen_to_ground(sx: number, sy: number, cw: number, ch: number): Float32Array;
    set_3d_mode(enabled: boolean): void;
    /**
     * The colour around the map in 3D (the world's deep sea, 0..255), so the view's edges read
     * as open ocean. JS passes its palette's deep-water colour.
     */
    set_backdrop(r: number, g: number, b: number): void;
    set_camera_target(tx: number, tz: number): void;
    set_era(era: number): void;
    /**
     * Use the JS world's heights (row-major W×H, 0..1, sea level 0.33) for the 3D mesh, the
     * raycast and where billboards stand. The engine's own `Terrain` is still the old single
     * island, so without this the relief doesn't match the map the simulation runs on.
     * Call after `init_world`.
     */
    set_heightmap(heights: Float32Array | number[]): void;
    upload_canvas(canvas: HTMLCanvasElement): void;
    zoom_camera(factor: number): void;
}

export type InitInput = RequestInfo | URL | Response | BufferSource | WebAssembly.Module;

export interface InitOutput {
    readonly memory: WebAssembly.Memory;
    readonly __wbg_engine_free: (a: number, b: number) => void;
    readonly engine_add_3d_light: (a: number, b: number, c: number, d: number, e: number, f: number) => void;
    readonly engine_add_billboard: (a: number, b: number, c: number, d: number, e: number, f: number, g: number) => void;
    readonly engine_add_custom_billboard: (a: number, b: number, c: number, d: number, e: number, f: number, g: number, h: number, i: number, j: number) => void;
    readonly engine_add_light: (a: number, b: number, c: number, d: number, e: number) => void;
    readonly engine_advisor_say: (a: number, b: number, c: number, d: number, e: number, f: number, g: number, h: number, i: number) => number;
    readonly engine_audio_heartbeat: (a: number, b: number, c: number, d: number) => void;
    readonly engine_audio_play: (a: number, b: number, c: number) => void;
    readonly engine_audio_toggle: (a: number) => number;
    readonly engine_audio_unlock: (a: number) => void;
    readonly engine_begin_frame: (a: number) => void;
    readonly engine_bind_dom_shell: (a: number) => [number, number];
    readonly engine_clear_3d_billboards: (a: number) => void;
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
    readonly engine_draw_text: (a: number, b: number, c: number, d: number, e: number, f: number, g: number, h: number, i: number) => void;
    readonly engine_draw_text_badge: (a: number, b: number, c: number, d: number, e: number, f: number, g: number, h: number, i: number, j: number, k: number, l: number) => void;
    readonly engine_draw_text_centered: (a: number, b: number, c: number, d: number, e: number, f: number, g: number, h: number, i: number) => void;
    readonly engine_end_frame: (a: number, b: number, c: number, d: number, e: number, f: number, g: number, h: number) => void;
    readonly engine_game_on_blink: (a: number, b: number, c: number) => void;
    readonly engine_game_on_cast: (a: number, b: number, c: number, d: number, e: number) => void;
    readonly engine_game_restart_era: (a: number) => void;
    readonly engine_game_start_era: (a: number, b: number) => void;
    readonly engine_game_tick: (a: number, b: number, c: number, d: number, e: number, f: number, g: number, h: number, i: number) => void;
    readonly engine_get_camera_distance: (a: number) => number;
    readonly engine_get_camera_pitch: (a: number) => number;
    readonly engine_get_camera_target_x: (a: number) => number;
    readonly engine_get_camera_target_z: (a: number) => number;
    readonly engine_get_camera_yaw: (a: number) => number;
    readonly engine_get_game_births_stat: (a: number) => number;
    readonly engine_get_game_deaths_stat: (a: number) => number;
    readonly engine_get_game_era_id: (a: number) => [number, number];
    readonly engine_get_game_era_index: (a: number) => number;
    readonly engine_get_game_era_label: (a: number) => [number, number];
    readonly engine_get_game_lightning_stat: (a: number) => number;
    readonly engine_get_game_recessions_stat: (a: number) => number;
    readonly engine_get_game_restarts_stat: (a: number) => number;
    readonly engine_get_game_runtime: (a: number) => number;
    readonly engine_get_game_smitten_stat: (a: number) => number;
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
    readonly engine_init_3d: (a: number) => [number, number];
    readonly engine_init_world: (a: number, b: number) => void;
    readonly engine_is_3d_mode: (a: number) => number;
    readonly engine_is_game_failed: (a: number) => [number, number];
    readonly engine_is_game_finished: (a: number) => number;
    readonly engine_is_walkable: (a: number, b: number, c: number) => number;
    readonly engine_new: (a: number, b: number) => [number, number, number];
    readonly engine_orbit_camera: (a: number, b: number, c: number) => void;
    readonly engine_pan_camera: (a: number, b: number, c: number) => void;
    readonly engine_render_3d: (a: number, b: number, c: number, d: number, e: number, f: number, g: number, h: number, i: number, j: number, k: number, l: number) => void;
    readonly engine_render_uploaded_canvas: (a: number, b: number, c: number, d: number, e: number, f: number, g: number, h: number) => void;
    readonly engine_screen_to_ground: (a: number, b: number, c: number, d: number, e: number) => any;
    readonly engine_set_3d_mode: (a: number, b: number) => void;
    readonly engine_set_backdrop: (a: number, b: number, c: number, d: number) => void;
    readonly engine_set_camera_target: (a: number, b: number, c: number) => void;
    readonly engine_set_era: (a: number, b: number) => void;
    readonly engine_set_heightmap: (a: number, b: number, c: number) => [number, number];
    readonly engine_upload_canvas: (a: number, b: any) => [number, number];
    readonly engine_zoom_camera: (a: number, b: number) => void;
    readonly __wbindgen_malloc: (a: number, b: number) => number;
    readonly __wbindgen_realloc: (a: number, b: number, c: number, d: number) => number;
    readonly __wbindgen_free: (a: number, b: number, c: number) => void;
    readonly __wbindgen_exn_store: (a: number) => void;
    readonly __externref_table_alloc: () => number;
    readonly __wbindgen_externrefs: WebAssembly.Table;
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
