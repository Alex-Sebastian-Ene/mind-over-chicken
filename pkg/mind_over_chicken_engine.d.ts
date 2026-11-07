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
    project_to_screen(wx: number, wz: number, cw: number, ch: number): Float32Array;
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

export class PointerGesture {
    free(): void;
    [Symbol.dispose](): void;
    /**
     * A main-button or touch press. `sx, sy` screen pixels; `wx, wy` world units.
     */
    down(id: number, sx: number, sy: number, wx: number, wy: number): void;
    is_panning(): boolean;
    /**
     * Pointer moved. Returns true when this move pans the map; read the delta with
     * `pan_dx()` / `pan_dy()` (world units since the previous move). Returns false while the
     * press is still a possible tap, or for another pointer.
     */
    moved(id: number, sx: number, sy: number, wx: number, wy: number): boolean;
    /**
     * `threshold_px`: how far (screen pixels) a press must move before it counts as a drag.
     */
    constructor(threshold_px: number);
    /**
     * Is this pointer the press being tracked?
     */
    owns(id: number): boolean;
    pan_dx(): number;
    pan_dy(): number;
    /**
     * Press ended. Returns true when it was a tap (blink). A cancelled pointer never taps.
     */
    up(id: number, cancelled: boolean): boolean;
}

export class RustBlinkClassifier {
    free(): void;
    [Symbol.dispose](): void;
    accuracy(): number;
    /**
     * Arms a flash-cue capture window (e.g., 1.2s) right when the visual cue flashes on screen.
     */
    arm_flash_window(seconds: number): void;
    baseline_samples(): number;
    blink_samples(): number;
    ingest_frame(tp9: Float32Array | number[], af7: Float32Array | number[], af8: Float32Array | number[], tp10: Float32Array | number[], motion: number, dt: number): boolean;
    is_capturing(): boolean;
    is_trained(): boolean;
    last_af7_ptp(): number;
    last_af8_ptp(): number;
    last_corr(): number;
    last_eff_amp(): number;
    last_probability(): number;
    last_tp10_ptp(): number;
    last_tp9_ptp(): number;
    learned_af7_uv(): number;
    learned_af8_uv(): number;
    learned_threshold_uv(): number;
    constructor();
    /**
     * Records the latest EEG window as a non-blink negative sample (used during Hold Still
     * and Move Head Around Without Blinking calibration stages to cancel noise & hum).
     */
    record_baseline_button(): boolean;
    /**
     * Searches the last ~2.5s of EEG windows for the strongest blink peak if the user blinked
     * before clicking, or arms a 2.5s capture window if they click first and then blink.
     */
    record_blink_button(): boolean;
    reset_model(): void;
    total_detected(): number;
}

export class RustImuTracker {
    free(): void;
    [Symbol.dispose](): void;
    /**
     * Records a supervised sample pairing the user's current 3D relative orientation
     * `(rel_yaw, rel_pitch, rel_roll)` with the target white dot screen position `(dot_x, dot_y)` in `0..1`.
     */
    add_calib_point(dot_x: number, dot_y: number): void;
    aim_x(): number;
    aim_y(): number;
    begin_motion_calibration(): void;
    calib_progress(): number;
    calib_r2_score(): number;
    calib_sample_count(): number;
    cancel_motion_calibration(): void;
    /**
     * Hybrid Smooth-Pursuit + Boundary-Fixation + Center-Checkpoint Solver:
     * 1. Extracts Center checkpoints (`|tx| < 0.08 && |ty| < 0.08`) to remove any gyro yaw drift
     *    accumulated across calibration and guarantee zero-intercept symmetry on reverse.
     * 2. Searches optimal human smooth-pursuit reaction lag `L in 0..=2` on moving segments while
     *    weighting stationary boundary-hold anchors (`3.0x`) for exact screen edge locking.
     * 3. Solves zero-prior 3D sensor-to-head Ridge Regression (`[yaw, pitch, roll] -> (u_x, u_y)`)
     *    with cross-axis Gram-Schmidt decoupling so vertical nods never leak horizontally.
     * 4. Computes independent 4-boundary reach envelopes (`scale_left, scale_right, scale_up, scale_down`)
     *    so 89% of the user's calibrated edge posture reaches 100% of all 4 screen boundaries & corners.
     */
    finish_motion_calibration(): boolean;
    has_motion_model(): boolean;
    is_tared(): boolean;
    learned_pitch_deg_for_edge(): number;
    learned_pitch_sign(): number;
    learned_yaw_deg_for_edge(): number;
    learned_yaw_sign(): number;
    constructor();
    raw_pitch_deg(): number;
    raw_roll_deg(): number;
    raw_yaw_deg(): number;
    rel_pitch_deg(): number;
    rel_roll_deg(): number;
    rel_yaw_deg(): number;
    reset_motion_model(): void;
    set_config(sensitivity: number, invert_yaw: boolean, invert_pitch: boolean, deadzone_deg: number): void;
    tare(): void;
    tilt_x(): number;
    tilt_y(): number;
    update(ax: number, ay: number, az: number, gx: number, gy: number, gz: number, bridge_yaw: number, has_bridge_yaw: boolean, dt: number): void;
}

export type InitInput = RequestInfo | URL | Response | BufferSource | WebAssembly.Module;

export interface InitOutput {
    readonly memory: WebAssembly.Memory;
    readonly __wbg_engine_free: (a: number, b: number) => void;
    readonly __wbg_pointergesture_free: (a: number, b: number) => void;
    readonly __wbg_rustblinkclassifier_free: (a: number, b: number) => void;
    readonly __wbg_rustimutracker_free: (a: number, b: number) => void;
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
    readonly engine_project_to_screen: (a: number, b: number, c: number, d: number, e: number) => any;
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
    readonly pointergesture_down: (a: number, b: number, c: number, d: number, e: number, f: number) => void;
    readonly pointergesture_is_panning: (a: number) => number;
    readonly pointergesture_moved: (a: number, b: number, c: number, d: number, e: number, f: number) => number;
    readonly pointergesture_new: (a: number) => number;
    readonly pointergesture_owns: (a: number, b: number) => number;
    readonly pointergesture_pan_dx: (a: number) => number;
    readonly pointergesture_pan_dy: (a: number) => number;
    readonly pointergesture_up: (a: number, b: number, c: number) => number;
    readonly rustblinkclassifier_accuracy: (a: number) => number;
    readonly rustblinkclassifier_arm_flash_window: (a: number, b: number) => void;
    readonly rustblinkclassifier_baseline_samples: (a: number) => number;
    readonly rustblinkclassifier_blink_samples: (a: number) => number;
    readonly rustblinkclassifier_ingest_frame: (a: number, b: number, c: number, d: number, e: number, f: number, g: number, h: number, i: number, j: number, k: number) => number;
    readonly rustblinkclassifier_is_capturing: (a: number) => number;
    readonly rustblinkclassifier_is_trained: (a: number) => number;
    readonly rustblinkclassifier_last_af7_ptp: (a: number) => number;
    readonly rustblinkclassifier_last_af8_ptp: (a: number) => number;
    readonly rustblinkclassifier_last_corr: (a: number) => number;
    readonly rustblinkclassifier_last_eff_amp: (a: number) => number;
    readonly rustblinkclassifier_last_probability: (a: number) => number;
    readonly rustblinkclassifier_last_tp10_ptp: (a: number) => number;
    readonly rustblinkclassifier_last_tp9_ptp: (a: number) => number;
    readonly rustblinkclassifier_learned_af7_uv: (a: number) => number;
    readonly rustblinkclassifier_learned_af8_uv: (a: number) => number;
    readonly rustblinkclassifier_learned_threshold_uv: (a: number) => number;
    readonly rustblinkclassifier_new: () => number;
    readonly rustblinkclassifier_record_baseline_button: (a: number) => number;
    readonly rustblinkclassifier_record_blink_button: (a: number) => number;
    readonly rustblinkclassifier_reset_model: (a: number) => void;
    readonly rustblinkclassifier_total_detected: (a: number) => number;
    readonly rustimutracker_add_calib_point: (a: number, b: number, c: number) => void;
    readonly rustimutracker_aim_x: (a: number) => number;
    readonly rustimutracker_aim_y: (a: number) => number;
    readonly rustimutracker_begin_motion_calibration: (a: number) => void;
    readonly rustimutracker_calib_progress: (a: number) => number;
    readonly rustimutracker_calib_r2_score: (a: number) => number;
    readonly rustimutracker_calib_sample_count: (a: number) => number;
    readonly rustimutracker_cancel_motion_calibration: (a: number) => void;
    readonly rustimutracker_finish_motion_calibration: (a: number) => number;
    readonly rustimutracker_has_motion_model: (a: number) => number;
    readonly rustimutracker_is_tared: (a: number) => number;
    readonly rustimutracker_learned_pitch_deg_for_edge: (a: number) => number;
    readonly rustimutracker_learned_pitch_sign: (a: number) => number;
    readonly rustimutracker_learned_yaw_deg_for_edge: (a: number) => number;
    readonly rustimutracker_learned_yaw_sign: (a: number) => number;
    readonly rustimutracker_new: () => number;
    readonly rustimutracker_raw_pitch_deg: (a: number) => number;
    readonly rustimutracker_raw_roll_deg: (a: number) => number;
    readonly rustimutracker_raw_yaw_deg: (a: number) => number;
    readonly rustimutracker_rel_pitch_deg: (a: number) => number;
    readonly rustimutracker_rel_roll_deg: (a: number) => number;
    readonly rustimutracker_rel_yaw_deg: (a: number) => number;
    readonly rustimutracker_reset_motion_model: (a: number) => void;
    readonly rustimutracker_set_config: (a: number, b: number, c: number, d: number, e: number) => void;
    readonly rustimutracker_tare: (a: number) => void;
    readonly rustimutracker_tilt_x: (a: number) => number;
    readonly rustimutracker_tilt_y: (a: number) => number;
    readonly rustimutracker_update: (a: number, b: number, c: number, d: number, e: number, f: number, g: number, h: number, i: number, j: number) => void;
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
