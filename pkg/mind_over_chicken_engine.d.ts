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
    /**
     * One tick of the Rust eras. `stress` is the JS brain's `brain.stress`
     * (EEG agitation or face anger, 0..1, about 0.15 at rest).
     */
    game_tick(dt: number, calm: number, focus: number, stress: number, blinks: number): void;
    /**
     * Standalone helper to generate a civ planet RGBA texture (#109).
     */
    static generate_civ_terrain(seed: number, width: number, height: number): Uint8Array;
    /**
     * Generate a home world with moc-terrain (#109) and make it the engine's: heights, tiles,
     * colours, sites and the 3D relief. `width`/`height` 0 (or left out) mean the engine's
     * 320×180. Any size drives the 3D mesh, raycast and billboards (#129); the Rust 2D port's
     * own tiles stay 320×180.
     */
    generate_terrain(seed: number, width: number, height: number): void;
    get_camera_distance(): number;
    get_camera_pitch(): number;
    get_camera_target_x(): number;
    get_camera_target_z(): number;
    get_camera_yaw(): number;
    get_continent_count(): number;
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
    get_site_fire_x(): number;
    get_site_fire_y(): number;
    get_site_hangar_x(): number;
    get_site_hangar_y(): number;
    get_site_pad_x(): number;
    get_site_pad_y(): number;
    get_site_village_x(): number;
    get_site_village_y(): number;
    get_terrain_field(tx: number, ty: number): Int16Array;
    get_terrain_sea_field(tx: number, ty: number): Int16Array;
    get_terrain_type(x: number, y: number): number;
    get_tree_count(): number;
    get_tree_variant(idx: number): number;
    get_tree_x(idx: number): number;
    get_tree_y(idx: number): number;
    init_3d(): void;
    init_world(seed: number): void;
    is_3d_mode(): boolean;
    is_game_failed(): string | undefined;
    is_game_finished(): boolean;
    is_terrain_reachable(x: number, y: number): boolean;
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
     * Use the JS world's heights (row-major `width × height`, 0..1, sea level 0.33) for the 3D
     * mesh, the raycast and where billboards stand. The world may be bigger than the view
     * (#129: 640×360 under a 320×180 screen); width 0 means the classic 320×180.
     */
    set_heightmap(heights: Float32Array | number[], width: number, height: number): void;
    set_terrain_biome(biome_name: string): void;
    /**
     * The 2D frame: the low-res view canvas (320×180) the 2D renderer scales up and lights.
     */
    upload_canvas(canvas: HTMLCanvasElement): void;
    /**
     * The 3D terrain's colour map: a canvas the size of the world (#129), not the screen. The
     * mesh's texture coordinates span the whole world, so this is what drapes over the relief.
     */
    upload_terrain_canvas(canvas: HTMLCanvasElement): void;
    world_h(): number;
    /**
     * The world's size in map pixels (= 3D units), as last set by `set_heightmap` (#129).
     */
    world_w(): number;
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

/**
 * Standalone WebAssembly MLP Policy AI God (#25, #108)
 */
export class RustAiPolicy {
    free(): void;
    [Symbol.dispose](): void;
    act(obs: Float32Array | number[], greedy: boolean, sample_val: number): number;
    constructor();
    probs(obs: Float32Array | number[]): Float32Array;
    self_check(): number;
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

/**
 * Standalone WebAssembly Brain signal processor and personal model (#20, #108)
 */
export class RustBrain {
    free(): void;
    [Symbol.dispose](): void;
    accuracy(): number;
    anger(): number;
    blinks(): number;
    calm(): number;
    focus(): number;
    ingest(delta: number, theta: number, alpha: number, beta: number, gamma: number, blink: boolean, hr: number, tilt_x: number, tilt_y: number, motion: number, quality: number): boolean;
    constructor();
    retrain(): boolean;
    set_cue(cue_id: number): void;
    stress(): number;
    update(dt: number): boolean;
}

/**
 * Standalone WebAssembly Calibration traits (#54, #108)
 */
export class RustCalibration {
    private constructor();
    free(): void;
    [Symbol.dispose](): void;
    static random_traits(seed: number): Float32Array;
    static traits_from_sensors(calm: number, focus: number, stress: number, blink_rate: number, joy: number, anger: number): Float32Array;
}

/**
 * Standalone WebAssembly Cast Controller for camera god powers (#33, #108)
 */
export class RustCastController {
    free(): void;
    [Symbol.dispose](): void;
    cooldown_frac(spell_id: string, now: number): number;
    cooldown_left(spell_id: string, now: number): number;
    constructor();
    ready(spell_id: string, now: number): boolean;
    reset(): void;
    spell_for(gesture: string): string | undefined;
    trigger(spell_id: string, now: number): boolean;
    update(gesture: string, score: number, now: number): Array<any>;
}

/**
 * Standalone WebAssembly Face Emotion Tracker (#33, #108)
 */
export class RustEmotionTracker {
    free(): void;
    [Symbol.dispose](): void;
    constructor(calibrate_seconds: number);
    reset(): void;
    update_direct(has_face: boolean, mouth_smile_left: number, mouth_smile_right: number, brow_down_left: number, brow_down_right: number, mouth_press_left: number, mouth_press_right: number, jaw_open: number, brow_inner_up: number, eye_wide_left: number, eye_wide_right: number, mouth_frown_left: number, mouth_frown_right: number, dt: number): Array<any>;
}

/**
 * Standalone WebAssembly Finger Controller for smoothing, deadzone and pinching (#33, #67, #108)
 */
export class RustFingerController {
    free(): void;
    [Symbol.dispose](): void;
    constructor();
    reset(): void;
    update(landmarks: Float64Array | number[] | null | undefined, gesture: string, score: number, now: number, aspect: number): Array<any>;
}

/**
 * Standalone WebAssembly Galaxy & Multi-World Simulator (#24, #108)
 */
export class RustGalaxy {
    free(): void;
    [Symbol.dispose](): void;
    get_attended(): number;
    is_mind_done(): boolean;
    constructor(seed: number);
    on_blink(): void;
    planet_biome(idx: number): string;
    planet_channel(idx: number): string;
    planet_count(): number;
    planet_era(idx: number): number;
    planet_is_alive(idx: number): boolean;
    planet_is_forming(idx: number): boolean;
    planet_market(idx: number): number;
    planet_name(idx: number): string;
    planet_pop(idx: number): number;
    planet_pos_x(idx: number): number;
    planet_pos_y(idx: number): number;
    planet_progress(idx: number): number;
    set_attended(planet_idx: number): void;
    step(dt: number, calm: number, focus: number, stress: number, blinks_last_10s: number, blinks_total: number): void;
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

/**
 * Standalone WebAssembly Simulated Headset Source (#20, #108)
 */
export class RustSimSource {
    free(): void;
    [Symbol.dispose](): void;
    blink(): void;
    constructor(seed: number);
    set_input(calm: boolean, focus: boolean, panic: boolean): void;
    step(dt: number): Float32Array;
}

/**
 * A generated world for JS (#109, from #112): the JS `World` is a thin wrapper that fills its
 * arrays from this. `new(seed)` is the 320×180 home world; `home(seed, w, h)` any size;
 * `planet(seed, biome)` a galaxy planet's 640×360 surface with its 320×180 capital (the AI
 * tribe's world) cut from it at (0, 90). Accessors without `surface_` describe the world (the
 * capital for a planet); `surface_*` describe the whole surface.
 */
export class RustWorldGen {
    free(): void;
    [Symbol.dispose](): void;
    biomes(): Uint8Array;
    continents_json(): string;
    has_surface(): boolean;
    height(): number;
    /**
     * Heights 0..1, sea level 0.33 (like the old maps, so the 3D water level is unchanged).
     */
    heights(): Float32Array;
    /**
     * A home world of any size (0 × 0: the game's 320×180) for a planet biome (temperate,
     * desert, ice, jungle, alien).
     */
    static home(seed: number, width: number, height: number, biome: string): RustWorldGen;
    lakes(): Uint8Array;
    /**
     * Generation time, ms.
     */
    ms(): number;
    /**
     * The 320×180 home world.
     */
    constructor(seed: number);
    /**
     * Campaign landmass per tile (index into `continents_json`), 255 for none.
     */
    owner(): Uint8Array;
    /**
     * A galaxy planet: its 640×360 surface, and the 320×180 capital at (0, 90) as the world.
     */
    static planet(seed: number, biome: string): RustWorldGen;
    /**
     * The base colour layer, RGBA.
     */
    rgba(): Uint8Array;
    rivers(): Uint8Array;
    sites_json(): string;
    stats_json(): string;
    surface_height(): number;
    surface_heights(): Float32Array;
    /**
     * The surface's landmasses and settlement spots (capital first, then nearest first), as
     * the galaxy's surface view expects them: {tile, continents, spots}.
     */
    surface_json(): string;
    surface_rgba(): Uint8Array;
    surface_stats_json(): string;
    surface_types(): Uint8Array;
    surface_width(): number;
    /**
     * Game tiles (the JS `T` codes).
     */
    types(): Uint8Array;
    width(): number;
}

/**
 * The 2D camera over the home world. `pan(dx, dy)` moves the map under the window like a drag
 * (the content follows the pointer); everything wraps both ways (a torus, #111).
 */
export class ViewCamera {
    free(): void;
    [Symbol.dispose](): void;
    /**
     * The rectangles that paint the window from a world-sized canvas, cut at the seams:
     * `[sx, sy, w, h, dx, dy]` per rectangle (1 to 4 of them, more only if the world is
     * smaller than the view). Draw each with `drawImage(world, sx, sy, w, h, dx, dy, w, h)`.
     */
    blits(): Float32Array;
    center_x(): number;
    center_y(): number;
    /**
     * Translations `[ox, oy]` (pairs) that take world coordinates to screen coordinates, one per
     * copy of the world that reaches the window (widened by `margin` pixels on every side).
     * Overlays drawn in world units repeat at each, so labels and lights wrap with the map.
     */
    copies(margin: number): Float32Array;
    /**
     * The shorter signed east-west way from b to a.
     */
    delta_x(a: number, b: number): number;
    /**
     * The shorter signed north-south way from b to a.
     */
    delta_y(a: number, b: number): number;
    /**
     * World x at the window's left edge, snapped to whole pixels so the 2D map stays crisp.
     * Every 2D conversion uses the snapped edge, so what is drawn and what is clicked agree.
     */
    left(): number;
    constructor(world_w: number, world_h: number, view_w: number, view_h: number);
    /**
     * Drag the map by (dx, dy) world pixels: the content moves with the pointer, so the window
     * moves the other way.
     */
    pan(dx: number, dy: number): void;
    set_center(x: number, y: number): void;
    /**
     * A new world size (another map was loaded). The centre stays put, wrapped into it.
     */
    set_world(world_w: number, world_h: number): void;
    /**
     * World x → screen x of its copy nearest the window's centre (may fall off screen).
     */
    to_screen_x(wx: number): number;
    /**
     * World y → screen y of its copy nearest the window's centre.
     */
    to_screen_y(wy: number): number;
    /**
     * Screen x (view pixels, 0..view_w) → world x.
     */
    to_world_x(sx: number): number;
    /**
     * Screen y (view pixels, 0..view_h) → world y.
     */
    to_world_y(sy: number): number;
    /**
     * World y at the window's top edge (snapped, like `left`).
     */
    top(): number;
    view_h(): number;
    view_w(): number;
    world_h(): number;
    world_w(): number;
}

/**
 * The era's land use (farms, roads, paving: tiles whose type differs from the base type)
 * painted over the base colour layer, in a planet biome's colours.
 */
export function paint_land_use(base: Uint8Array | number[], types: Uint8Array | number[], base_types: Uint8Array | number[], width: number, biome: string): Uint8Array;

/**
 * Walking distance (8-neighbour steps, no wrap) from every tile to (tx, ty) over walkable
 * tiles, or over water with `sea`; -1 where unreachable. The JS `World.field`/`seaField`.
 */
export function walk_field(types: Uint8Array | number[], width: number, height: number, tx: number, ty: number, sea: boolean): Int16Array;

export type InitInput = RequestInfo | URL | Response | BufferSource | WebAssembly.Module;

export interface InitOutput {
    readonly memory: WebAssembly.Memory;
    readonly __wbg_engine_free: (a: number, b: number) => void;
    readonly __wbg_pointergesture_free: (a: number, b: number) => void;
    readonly __wbg_rustaipolicy_free: (a: number, b: number) => void;
    readonly __wbg_rustblinkclassifier_free: (a: number, b: number) => void;
    readonly __wbg_rustbrain_free: (a: number, b: number) => void;
    readonly __wbg_rustcalibration_free: (a: number, b: number) => void;
    readonly __wbg_rustcastcontroller_free: (a: number, b: number) => void;
    readonly __wbg_rustemotiontracker_free: (a: number, b: number) => void;
    readonly __wbg_rustfingercontroller_free: (a: number, b: number) => void;
    readonly __wbg_rustgalaxy_free: (a: number, b: number) => void;
    readonly __wbg_rustimutracker_free: (a: number, b: number) => void;
    readonly __wbg_rustsimsource_free: (a: number, b: number) => void;
    readonly __wbg_rustworldgen_free: (a: number, b: number) => void;
    readonly __wbg_viewcamera_free: (a: number, b: number) => void;
    readonly engine_add_3d_light: (a: number, b: number, c: number, d: number, e: number, f: number) => void;
    readonly engine_add_billboard: (a: number, b: number, c: number, d: number, e: number, f: number, g: number) => void;
    readonly engine_add_custom_billboard: (a: number, b: number, c: number, d: number, e: number, f: number, g: number, h: number, i: number, j: number) => void;
    readonly engine_add_light: (a: number, b: number, c: number, d: number, e: number) => void;
    readonly engine_advisor_say: (a: number, b: number, c: number, d: number, e: number, f: number, g: number, h: number, i: number) => number;
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
    readonly engine_game_tick: (a: number, b: number, c: number, d: number, e: number, f: number) => void;
    readonly engine_generate_civ_terrain: (a: number, b: number, c: number) => [number, number];
    readonly engine_generate_terrain: (a: number, b: number, c: number, d: number) => [number, number];
    readonly engine_get_camera_distance: (a: number) => number;
    readonly engine_get_camera_pitch: (a: number) => number;
    readonly engine_get_camera_target_x: (a: number) => number;
    readonly engine_get_camera_target_z: (a: number) => number;
    readonly engine_get_camera_yaw: (a: number) => number;
    readonly engine_get_continent_count: (a: number) => number;
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
    readonly engine_get_site_fire_x: (a: number) => number;
    readonly engine_get_site_fire_y: (a: number) => number;
    readonly engine_get_site_hangar_x: (a: number) => number;
    readonly engine_get_site_hangar_y: (a: number) => number;
    readonly engine_get_site_pad_x: (a: number) => number;
    readonly engine_get_site_pad_y: (a: number) => number;
    readonly engine_get_site_village_x: (a: number) => number;
    readonly engine_get_site_village_y: (a: number) => number;
    readonly engine_get_terrain_field: (a: number, b: number, c: number) => [number, number];
    readonly engine_get_terrain_sea_field: (a: number, b: number, c: number) => [number, number];
    readonly engine_get_terrain_type: (a: number, b: number, c: number) => number;
    readonly engine_get_tree_count: (a: number) => number;
    readonly engine_get_tree_variant: (a: number, b: number) => number;
    readonly engine_get_tree_x: (a: number, b: number) => number;
    readonly engine_get_tree_y: (a: number, b: number) => number;
    readonly engine_init_3d: (a: number) => [number, number];
    readonly engine_init_world: (a: number, b: number) => void;
    readonly engine_is_3d_mode: (a: number) => number;
    readonly engine_is_game_failed: (a: number) => [number, number];
    readonly engine_is_game_finished: (a: number) => number;
    readonly engine_is_terrain_reachable: (a: number, b: number, c: number) => number;
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
    readonly engine_set_heightmap: (a: number, b: number, c: number, d: number, e: number) => [number, number];
    readonly engine_set_terrain_biome: (a: number, b: number, c: number) => void;
    readonly engine_upload_canvas: (a: number, b: any) => [number, number];
    readonly engine_upload_terrain_canvas: (a: number, b: any) => [number, number];
    readonly engine_world_h: (a: number) => number;
    readonly engine_world_w: (a: number) => number;
    readonly engine_zoom_camera: (a: number, b: number) => void;
    readonly paint_land_use: (a: number, b: number, c: number, d: number, e: number, f: number, g: number, h: number, i: number) => [number, number];
    readonly pointergesture_down: (a: number, b: number, c: number, d: number, e: number, f: number) => void;
    readonly pointergesture_is_panning: (a: number) => number;
    readonly pointergesture_moved: (a: number, b: number, c: number, d: number, e: number, f: number) => number;
    readonly pointergesture_new: (a: number) => number;
    readonly pointergesture_owns: (a: number, b: number) => number;
    readonly pointergesture_pan_dx: (a: number) => number;
    readonly pointergesture_pan_dy: (a: number) => number;
    readonly pointergesture_up: (a: number, b: number, c: number) => number;
    readonly rustaipolicy_act: (a: number, b: number, c: number, d: number, e: number) => number;
    readonly rustaipolicy_new: () => number;
    readonly rustaipolicy_probs: (a: number, b: number, c: number) => any;
    readonly rustaipolicy_self_check: (a: number) => number;
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
    readonly rustbrain_accuracy: (a: number) => number;
    readonly rustbrain_anger: (a: number) => number;
    readonly rustbrain_blinks: (a: number) => number;
    readonly rustbrain_calm: (a: number) => number;
    readonly rustbrain_focus: (a: number) => number;
    readonly rustbrain_ingest: (a: number, b: number, c: number, d: number, e: number, f: number, g: number, h: number, i: number, j: number, k: number, l: number) => number;
    readonly rustbrain_new: () => number;
    readonly rustbrain_retrain: (a: number) => number;
    readonly rustbrain_set_cue: (a: number, b: number) => void;
    readonly rustbrain_stress: (a: number) => number;
    readonly rustbrain_update: (a: number, b: number) => number;
    readonly rustcalibration_random_traits: (a: number) => any;
    readonly rustcalibration_traits_from_sensors: (a: number, b: number, c: number, d: number, e: number, f: number) => any;
    readonly rustcastcontroller_cooldown_frac: (a: number, b: number, c: number, d: number) => number;
    readonly rustcastcontroller_cooldown_left: (a: number, b: number, c: number, d: number) => number;
    readonly rustcastcontroller_new: () => number;
    readonly rustcastcontroller_ready: (a: number, b: number, c: number, d: number) => number;
    readonly rustcastcontroller_reset: (a: number) => void;
    readonly rustcastcontroller_spell_for: (a: number, b: number, c: number) => [number, number];
    readonly rustcastcontroller_trigger: (a: number, b: number, c: number, d: number) => number;
    readonly rustcastcontroller_update: (a: number, b: number, c: number, d: number, e: number) => any;
    readonly rustemotiontracker_new: (a: number) => number;
    readonly rustemotiontracker_reset: (a: number) => void;
    readonly rustemotiontracker_update_direct: (a: number, b: number, c: number, d: number, e: number, f: number, g: number, h: number, i: number, j: number, k: number, l: number, m: number, n: number, o: number) => any;
    readonly rustfingercontroller_new: () => number;
    readonly rustfingercontroller_reset: (a: number) => void;
    readonly rustfingercontroller_update: (a: number, b: number, c: number, d: number, e: number, f: number, g: number, h: number) => any;
    readonly rustgalaxy_get_attended: (a: number) => number;
    readonly rustgalaxy_is_mind_done: (a: number) => number;
    readonly rustgalaxy_new: (a: number) => number;
    readonly rustgalaxy_on_blink: (a: number) => void;
    readonly rustgalaxy_planet_biome: (a: number, b: number) => [number, number];
    readonly rustgalaxy_planet_channel: (a: number, b: number) => [number, number];
    readonly rustgalaxy_planet_count: (a: number) => number;
    readonly rustgalaxy_planet_era: (a: number, b: number) => number;
    readonly rustgalaxy_planet_is_alive: (a: number, b: number) => number;
    readonly rustgalaxy_planet_is_forming: (a: number, b: number) => number;
    readonly rustgalaxy_planet_market: (a: number, b: number) => number;
    readonly rustgalaxy_planet_name: (a: number, b: number) => [number, number];
    readonly rustgalaxy_planet_pop: (a: number, b: number) => number;
    readonly rustgalaxy_planet_pos_x: (a: number, b: number) => number;
    readonly rustgalaxy_planet_pos_y: (a: number, b: number) => number;
    readonly rustgalaxy_planet_progress: (a: number, b: number) => number;
    readonly rustgalaxy_set_attended: (a: number, b: number) => void;
    readonly rustgalaxy_step: (a: number, b: number, c: number, d: number, e: number, f: number, g: number) => void;
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
    readonly rustsimsource_blink: (a: number) => void;
    readonly rustsimsource_new: (a: number) => number;
    readonly rustsimsource_set_input: (a: number, b: number, c: number, d: number) => void;
    readonly rustsimsource_step: (a: number, b: number) => any;
    readonly rustworldgen_biomes: (a: number) => any;
    readonly rustworldgen_continents_json: (a: number) => [number, number];
    readonly rustworldgen_has_surface: (a: number) => number;
    readonly rustworldgen_height: (a: number) => number;
    readonly rustworldgen_heights: (a: number) => any;
    readonly rustworldgen_home: (a: number, b: number, c: number, d: number, e: number) => number;
    readonly rustworldgen_lakes: (a: number) => any;
    readonly rustworldgen_ms: (a: number) => number;
    readonly rustworldgen_new: (a: number) => number;
    readonly rustworldgen_owner: (a: number) => any;
    readonly rustworldgen_planet: (a: number, b: number, c: number) => number;
    readonly rustworldgen_rgba: (a: number) => any;
    readonly rustworldgen_rivers: (a: number) => any;
    readonly rustworldgen_sites_json: (a: number) => [number, number];
    readonly rustworldgen_stats_json: (a: number) => [number, number];
    readonly rustworldgen_surface_height: (a: number) => number;
    readonly rustworldgen_surface_heights: (a: number) => any;
    readonly rustworldgen_surface_json: (a: number) => [number, number];
    readonly rustworldgen_surface_rgba: (a: number) => any;
    readonly rustworldgen_surface_stats_json: (a: number) => [number, number];
    readonly rustworldgen_surface_types: (a: number) => any;
    readonly rustworldgen_surface_width: (a: number) => number;
    readonly rustworldgen_types: (a: number) => any;
    readonly rustworldgen_width: (a: number) => number;
    readonly viewcamera_blits: (a: number) => [number, number];
    readonly viewcamera_center_x: (a: number) => number;
    readonly viewcamera_center_y: (a: number) => number;
    readonly viewcamera_copies: (a: number, b: number) => [number, number];
    readonly viewcamera_delta_x: (a: number, b: number, c: number) => number;
    readonly viewcamera_delta_y: (a: number, b: number, c: number) => number;
    readonly viewcamera_left: (a: number) => number;
    readonly viewcamera_new: (a: number, b: number, c: number, d: number) => number;
    readonly viewcamera_pan: (a: number, b: number, c: number) => void;
    readonly viewcamera_set_center: (a: number, b: number, c: number) => void;
    readonly viewcamera_set_world: (a: number, b: number, c: number) => void;
    readonly viewcamera_to_screen_x: (a: number, b: number) => number;
    readonly viewcamera_to_screen_y: (a: number, b: number) => number;
    readonly viewcamera_to_world_x: (a: number, b: number) => number;
    readonly viewcamera_to_world_y: (a: number, b: number) => number;
    readonly viewcamera_top: (a: number) => number;
    readonly viewcamera_view_h: (a: number) => number;
    readonly viewcamera_view_w: (a: number) => number;
    readonly viewcamera_world_h: (a: number) => number;
    readonly viewcamera_world_w: (a: number) => number;
    readonly walk_field: (a: number, b: number, c: number, d: number, e: number, f: number, g: number) => [number, number];
    readonly moc_brain_free: (a: number) => void;
    readonly moc_brain_get_agitation: (a: number) => number;
    readonly moc_brain_get_calm: (a: number) => number;
    readonly moc_brain_get_focus: (a: number) => number;
    readonly moc_brain_get_hr_delta: (a: number) => number;
    readonly moc_brain_get_model_accuracy: (a: number) => number;
    readonly moc_brain_get_model_trust: (a: number, b: number) => number;
    readonly moc_brain_get_stress: (a: number) => number;
    readonly moc_brain_ingest: (a: number, b: number, c: number, d: number, e: number, f: number, g: number, h: number, i: number, j: number, k: number, l: number) => number;
    readonly moc_brain_new: () => number;
    readonly moc_brain_set_cue: (a: number, b: number) => void;
    readonly moc_brain_update: (a: number, b: number) => number;
    readonly moc_calibration_traits_from_sensors: (a: number, b: number, c: number, d: number, e: number, f: number, g: number) => void;
    readonly moc_calibration_traits_random: (a: number, b: number) => void;
    readonly moc_sim_blink: (a: number) => void;
    readonly moc_sim_free: (a: number) => void;
    readonly moc_sim_new: (a: number) => number;
    readonly moc_sim_set_input: (a: number, b: number, c: number, d: number) => void;
    readonly moc_ai_policy_act: (a: number, b: number, c: number, d: number) => number;
    readonly moc_ai_policy_free: (a: number) => void;
    readonly moc_ai_policy_new: () => number;
    readonly moc_ai_policy_probs: (a: number, b: number, c: number) => void;
    readonly moc_ai_policy_self_check: (a: number) => number;
    readonly moc_galaxy_blink: (a: number) => void;
    readonly moc_galaxy_free: (a: number) => void;
    readonly moc_galaxy_new: (a: number) => number;
    readonly moc_galaxy_planet_count: (a: number) => number;
    readonly moc_galaxy_planet_metrics: (a: number, b: number, c: number) => number;
    readonly moc_galaxy_set_attended: (a: number, b: number) => void;
    readonly moc_galaxy_step: (a: number, b: number, c: number, d: number, e: number, f: number, g: number) => void;
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
