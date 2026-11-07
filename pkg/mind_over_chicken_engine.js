/* @ts-self-types="./mind_over_chicken_engine.d.ts" */

export class Engine {
    __destroy_into_raw() {
        const ptr = this.__wbg_ptr;
        this.__wbg_ptr = 0;
        EngineFinalization.unregister(this);
        return ptr;
    }
    free() {
        const ptr = this.__destroy_into_raw();
        wasm.__wbg_engine_free(ptr, 0);
    }
    /**
     * @param {number} x
     * @param {number} y
     * @param {number} z
     * @param {number} radius
     * @param {number} intensity
     */
    add_3d_light(x, y, z, radius, intensity) {
        wasm.engine_add_3d_light(this.__wbg_ptr, x, y, z, radius, intensity);
    }
    /**
     * @param {number} sprite_id
     * @param {number} x
     * @param {number} z
     * @param {number} facing
     * @param {number} frame
     * @param {number} variant
     */
    add_billboard(sprite_id, x, z, facing, frame, variant) {
        wasm.engine_add_billboard(this.__wbg_ptr, sprite_id, x, z, facing, frame, variant);
    }
    /**
     * @param {number} x
     * @param {number} y
     * @param {number} z
     * @param {number} w
     * @param {number} h
     * @param {number} u0
     * @param {number} v0
     * @param {number} u1
     * @param {number} v1
     */
    add_custom_billboard(x, y, z, w, h, u0, v0, u1, v1) {
        wasm.engine_add_custom_billboard(this.__wbg_ptr, x, y, z, w, h, u0, v0, u1, v1);
    }
    /**
     * @param {number} x
     * @param {number} y
     * @param {number} radius
     * @param {number} intensity
     */
    add_light(x, y, radius, intensity) {
        wasm.engine_add_light(this.__wbg_ptr, x, y, radius, intensity);
    }
    /**
     * @param {string} key
     * @param {string} text
     * @param {string} mood
     * @param {number} priority
     * @param {number} cooldown
     * @returns {boolean}
     */
    advisor_say(key, text, mood, priority, cooldown) {
        const val4 = +priority;
        const val5 = +cooldown;
        let ptr0, len0, ptr1, len1, ptr2, len2;
        try {
            ptr0 = passStringToWasm0(key, wasm.__wbindgen_malloc, wasm.__wbindgen_realloc);
            len0 = WASM_VECTOR_LEN;
            ptr1 = passStringToWasm0(text, wasm.__wbindgen_malloc, wasm.__wbindgen_realloc);
            len1 = WASM_VECTOR_LEN;
            ptr2 = passStringToWasm0(mood, wasm.__wbindgen_malloc, wasm.__wbindgen_realloc);
            len2 = WASM_VECTOR_LEN;
        } catch (e) {
            if (ptr2) { wasm.__wbindgen_free(ptr2, len2 * 1, 1); }
            if (ptr1) { wasm.__wbindgen_free(ptr1, len1 * 1, 1); }
            if (ptr0) { wasm.__wbindgen_free(ptr0, len0 * 1, 1); }
            throw e;
        }
        const ret = wasm.engine_advisor_say(this.__wbg_ptr, ptr0, len0, ptr1, len1, ptr2, len2, val4, val5);
        return ret !== 0;
    }
    /**
     * @param {number} now
     * @param {number} bpm
     * @param {number} intensity
     */
    audio_heartbeat(now, bpm, intensity) {
        wasm.engine_audio_heartbeat(this.__wbg_ptr, now, bpm, intensity);
    }
    /**
     * @param {string} sound
     */
    audio_play(sound) {
        const ptr0 = passStringToWasm0(sound, wasm.__wbindgen_malloc, wasm.__wbindgen_realloc);
        const len0 = WASM_VECTOR_LEN;
        wasm.engine_audio_play(this.__wbg_ptr, ptr0, len0);
    }
    /**
     * @returns {boolean}
     */
    audio_toggle() {
        const ret = wasm.engine_audio_toggle(this.__wbg_ptr);
        return ret !== 0;
    }
    audio_unlock() {
        wasm.engine_audio_unlock(this.__wbg_ptr);
    }
    begin_frame() {
        wasm.engine_begin_frame(this.__wbg_ptr);
    }
    bind_dom_shell() {
        const ret = wasm.engine_bind_dom_shell(this.__wbg_ptr);
        if (ret[1]) {
            throw takeFromExternrefTable0(ret[0]);
        }
    }
    clear_3d_billboards() {
        wasm.engine_clear_3d_billboards(this.__wbg_ptr);
    }
    /**
     * @param {number} x
     * @param {number} y
     * @param {number} facing
     * @param {number} frame
     * @param {boolean} halo
     */
    draw_chicken(x, y, facing, frame, halo) {
        wasm.engine_draw_chicken(this.__wbg_ptr, x, y, facing, frame, halo);
    }
    /**
     * @param {number} x
     * @param {number} y
     * @param {number} heat
     * @param {number} t
     */
    draw_fire(x, y, heat, t) {
        wasm.engine_draw_fire(this.__wbg_ptr, x, y, heat, t);
    }
    /**
     * @param {number} x
     * @param {number} y
     * @param {number} facing
     */
    draw_goat(x, y, facing) {
        wasm.engine_draw_goat(this.__wbg_ptr, x, y, facing);
    }
    /**
     * @param {number} tx
     * @param {number} ty
     * @param {number} seed
     * @param {number} alpha
     */
    draw_lightning(tx, ty, seed, alpha) {
        wasm.engine_draw_lightning(this.__wbg_ptr, tx, ty, seed, alpha);
    }
    /**
     * @param {number} x
     * @param {number} y
     * @param {number} facing
     * @param {number} frame
     * @param {boolean} dead
     */
    draw_mammoth(x, y, facing, frame, dead) {
        wasm.engine_draw_mammoth(this.__wbg_ptr, x, y, facing, frame, dead);
    }
    /**
     * @param {number} x
     * @param {number} y
     * @param {number} frame
     * @param {number} pose
     * @param {number} shirt_r
     * @param {number} shirt_g
     * @param {number} shirt_b
     */
    draw_person(x, y, frame, pose, shirt_r, shirt_g, shirt_b) {
        wasm.engine_draw_person(this.__wbg_ptr, x, y, frame, pose, shirt_r, shirt_g, shirt_b);
    }
    /**
     * @param {number} x
     * @param {number} y
     * @param {number} r
     * @param {number} g
     * @param {number} b
     * @param {number} a
     */
    draw_pixel(x, y, r, g, b, a) {
        wasm.engine_draw_pixel(this.__wbg_ptr, x, y, r, g, b, a);
    }
    /**
     * @param {number} x
     * @param {number} y
     * @param {number} w
     * @param {number} h
     * @param {number} r
     * @param {number} g
     * @param {number} b
     * @param {number} a
     */
    draw_rect(x, y, w, h, r, g, b, a) {
        wasm.engine_draw_rect(this.__wbg_ptr, x, y, w, h, r, g, b, a);
    }
    /**
     * @param {number} x
     * @param {number} y
     * @param {number} t
     */
    draw_reticle(x, y, t) {
        wasm.engine_draw_reticle(this.__wbg_ptr, x, y, t);
    }
    /**
     * @param {number} x
     * @param {number} y
     * @param {number} progress
     * @param {number} t
     * @param {number} flame
     */
    draw_rocket(x, y, progress, t, flame) {
        wasm.engine_draw_rocket(this.__wbg_ptr, x, y, progress, t, flame);
    }
    /**
     * @param {number} x
     * @param {number} y
     * @param {string} text
     * @param {number} r
     * @param {number} g
     * @param {number} b
     * @param {number} a
     */
    draw_text(x, y, text, r, g, b, a) {
        const val2 = +x;
        const val3 = +y;
        const val4 = +r;
        const val5 = +g;
        const val6 = +b;
        const val7 = +a;
        const ptr0 = passStringToWasm0(text, wasm.__wbindgen_malloc, wasm.__wbindgen_realloc);
        const len0 = WASM_VECTOR_LEN;
        wasm.engine_draw_text(this.__wbg_ptr, val2, val3, ptr0, len0, val4, val5, val6, val7);
    }
    /**
     * @param {number} cx
     * @param {number} y
     * @param {string} text
     * @param {number} text_r
     * @param {number} text_g
     * @param {number} text_b
     * @param {number} bg_r
     * @param {number} bg_g
     * @param {number} bg_b
     * @param {number} bg_a
     */
    draw_text_badge(cx, y, text, text_r, text_g, text_b, bg_r, bg_g, bg_b, bg_a) {
        const val2 = +cx;
        const val3 = +y;
        const val4 = +text_r;
        const val5 = +text_g;
        const val6 = +text_b;
        const val7 = +bg_r;
        const val8 = +bg_g;
        const val9 = +bg_b;
        const val10 = +bg_a;
        const ptr0 = passStringToWasm0(text, wasm.__wbindgen_malloc, wasm.__wbindgen_realloc);
        const len0 = WASM_VECTOR_LEN;
        wasm.engine_draw_text_badge(this.__wbg_ptr, val2, val3, ptr0, len0, val4, val5, val6, val7, val8, val9, val10);
    }
    /**
     * @param {number} cx
     * @param {number} y
     * @param {string} text
     * @param {number} r
     * @param {number} g
     * @param {number} b
     * @param {number} a
     */
    draw_text_centered(cx, y, text, r, g, b, a) {
        const val2 = +cx;
        const val3 = +y;
        const val4 = +r;
        const val5 = +g;
        const val6 = +b;
        const val7 = +a;
        const ptr0 = passStringToWasm0(text, wasm.__wbindgen_malloc, wasm.__wbindgen_realloc);
        const len0 = WASM_VECTOR_LEN;
        wasm.engine_draw_text_centered(this.__wbg_ptr, val2, val3, ptr0, len0, val4, val5, val6, val7);
    }
    /**
     * @param {number} night
     * @param {number} flash
     * @param {number} fade
     * @param {number} shake_x
     * @param {number} shake_y
     * @param {number} canvas_w
     * @param {number} canvas_h
     */
    end_frame(night, flash, fade, shake_x, shake_y, canvas_w, canvas_h) {
        wasm.engine_end_frame(this.__wbg_ptr, night, flash, fade, shake_x, shake_y, canvas_w, canvas_h);
    }
    /**
     * @param {number} aim_x
     * @param {number} aim_y
     */
    game_on_blink(aim_x, aim_y) {
        wasm.engine_game_on_blink(this.__wbg_ptr, aim_x, aim_y);
    }
    /**
     * @param {string} spell
     * @param {number} aim_x
     * @param {number} aim_y
     */
    game_on_cast(spell, aim_x, aim_y) {
        const val2 = +aim_x;
        const val3 = +aim_y;
        const ptr0 = passStringToWasm0(spell, wasm.__wbindgen_malloc, wasm.__wbindgen_realloc);
        const len0 = WASM_VECTOR_LEN;
        wasm.engine_game_on_cast(this.__wbg_ptr, ptr0, len0, val2, val3);
    }
    game_restart_era() {
        wasm.engine_game_restart_era(this.__wbg_ptr);
    }
    /**
     * @param {number} index
     */
    game_start_era(index) {
        wasm.engine_game_start_era(this.__wbg_ptr, index);
    }
    /**
     * @param {number} dt
     * @param {number} calm
     * @param {number} focus
     * @param {number} panic
     * @param {number} hr
     * @param {number} rest_hr
     * @param {number} hr_delta
     * @param {number} blinks
     */
    game_tick(dt, calm, focus, panic, hr, rest_hr, hr_delta, blinks) {
        wasm.engine_game_tick(this.__wbg_ptr, dt, calm, focus, panic, hr, rest_hr, hr_delta, blinks);
    }
    /**
     * Standalone helper to generate a civ planet RGBA texture (#109).
     * @param {number} seed
     * @param {number} width
     * @param {number} height
     * @returns {Uint8Array}
     */
    static generate_civ_terrain(seed, width, height) {
        const ret = wasm.engine_generate_civ_terrain(seed, width, height);
        var v1 = getArrayU8FromWasm0(ret[0], ret[1]).slice();
        wasm.__wbindgen_free(ret[0], ret[1] * 1, 1);
        return v1;
    }
    /**
     * Generate realistic procedural terrain using moc-terrain (#109).
     * Updates the engine's internal heightmap, 3D relief mesh, and base pixel texture.
     * @param {number} seed
     */
    generate_terrain(seed) {
        const ret = wasm.engine_generate_terrain(this.__wbg_ptr, seed);
        if (ret[1]) {
            throw takeFromExternrefTable0(ret[0]);
        }
    }
    /**
     * @returns {number}
     */
    get_camera_distance() {
        const ret = wasm.engine_get_camera_distance(this.__wbg_ptr);
        return ret;
    }
    /**
     * @returns {number}
     */
    get_camera_pitch() {
        const ret = wasm.engine_get_camera_pitch(this.__wbg_ptr);
        return ret;
    }
    /**
     * @returns {number}
     */
    get_camera_target_x() {
        const ret = wasm.engine_get_camera_target_x(this.__wbg_ptr);
        return ret;
    }
    /**
     * @returns {number}
     */
    get_camera_target_z() {
        const ret = wasm.engine_get_camera_target_z(this.__wbg_ptr);
        return ret;
    }
    /**
     * @returns {number}
     */
    get_camera_yaw() {
        const ret = wasm.engine_get_camera_yaw(this.__wbg_ptr);
        return ret;
    }
    /**
     * @returns {number}
     */
    get_game_births_stat() {
        const ret = wasm.engine_get_game_births_stat(this.__wbg_ptr);
        return ret >>> 0;
    }
    /**
     * @returns {number}
     */
    get_game_deaths_stat() {
        const ret = wasm.engine_get_game_deaths_stat(this.__wbg_ptr);
        return ret >>> 0;
    }
    /**
     * @returns {string}
     */
    get_game_era_id() {
        let deferred1_0;
        let deferred1_1;
        try {
            const ret = wasm.engine_get_game_era_id(this.__wbg_ptr);
            deferred1_0 = ret[0];
            deferred1_1 = ret[1];
            return getStringFromWasm0(ret[0], ret[1]);
        } finally {
            wasm.__wbindgen_free(deferred1_0, deferred1_1, 1);
        }
    }
    /**
     * @returns {number}
     */
    get_game_era_index() {
        const ret = wasm.engine_get_game_era_index(this.__wbg_ptr);
        return ret >>> 0;
    }
    /**
     * @returns {string}
     */
    get_game_era_label() {
        let deferred1_0;
        let deferred1_1;
        try {
            const ret = wasm.engine_get_game_era_label(this.__wbg_ptr);
            deferred1_0 = ret[0];
            deferred1_1 = ret[1];
            return getStringFromWasm0(ret[0], ret[1]);
        } finally {
            wasm.__wbindgen_free(deferred1_0, deferred1_1, 1);
        }
    }
    /**
     * @returns {number}
     */
    get_game_lightning_stat() {
        const ret = wasm.engine_get_game_lightning_stat(this.__wbg_ptr);
        return ret >>> 0;
    }
    /**
     * @returns {number}
     */
    get_game_recessions_stat() {
        const ret = wasm.engine_get_game_recessions_stat(this.__wbg_ptr);
        return ret >>> 0;
    }
    /**
     * @returns {number}
     */
    get_game_restarts_stat() {
        const ret = wasm.engine_get_game_restarts_stat(this.__wbg_ptr);
        return ret >>> 0;
    }
    /**
     * @returns {number}
     */
    get_game_runtime() {
        const ret = wasm.engine_get_game_runtime(this.__wbg_ptr);
        return ret;
    }
    /**
     * @returns {number}
     */
    get_game_smitten_stat() {
        const ret = wasm.engine_get_game_smitten_stat(this.__wbg_ptr);
        return ret >>> 0;
    }
    /**
     * @returns {number}
     */
    get_site_cave_mouth_x() {
        const ret = wasm.engine_get_site_cave_mouth_x(this.__wbg_ptr);
        return ret;
    }
    /**
     * @returns {number}
     */
    get_site_cave_mouth_y() {
        const ret = wasm.engine_get_site_cave_mouth_y(this.__wbg_ptr);
        return ret;
    }
    /**
     * @returns {number}
     */
    get_site_cave_x() {
        const ret = wasm.engine_get_site_cave_x(this.__wbg_ptr);
        return ret;
    }
    /**
     * @returns {number}
     */
    get_site_cave_y() {
        const ret = wasm.engine_get_site_cave_y(this.__wbg_ptr);
        return ret;
    }
    /**
     * @returns {number}
     */
    get_site_exchange_x() {
        const ret = wasm.engine_get_site_exchange_x(this.__wbg_ptr);
        return ret;
    }
    /**
     * @returns {number}
     */
    get_site_exchange_y() {
        const ret = wasm.engine_get_site_exchange_y(this.__wbg_ptr);
        return ret;
    }
    /**
     * @returns {number}
     */
    get_site_factory_x() {
        const ret = wasm.engine_get_site_factory_x(this.__wbg_ptr);
        return ret;
    }
    /**
     * @returns {number}
     */
    get_site_factory_y() {
        const ret = wasm.engine_get_site_factory_y(this.__wbg_ptr);
        return ret;
    }
    /**
     * @returns {number}
     */
    get_site_hangar_x() {
        const ret = wasm.engine_get_site_hangar_x(this.__wbg_ptr);
        return ret;
    }
    /**
     * @returns {number}
     */
    get_site_hangar_y() {
        const ret = wasm.engine_get_site_hangar_y(this.__wbg_ptr);
        return ret;
    }
    /**
     * @returns {number}
     */
    get_site_pad_x() {
        const ret = wasm.engine_get_site_pad_x(this.__wbg_ptr);
        return ret;
    }
    /**
     * @returns {number}
     */
    get_site_pad_y() {
        const ret = wasm.engine_get_site_pad_y(this.__wbg_ptr);
        return ret;
    }
    /**
     * @returns {number}
     */
    get_site_village_x() {
        const ret = wasm.engine_get_site_village_x(this.__wbg_ptr);
        return ret;
    }
    /**
     * @returns {number}
     */
    get_site_village_y() {
        const ret = wasm.engine_get_site_village_y(this.__wbg_ptr);
        return ret;
    }
    /**
     * @param {number} x
     * @param {number} y
     * @returns {number}
     */
    get_terrain_type(x, y) {
        const ret = wasm.engine_get_terrain_type(this.__wbg_ptr, x, y);
        return ret;
    }
    init_3d() {
        const ret = wasm.engine_init_3d(this.__wbg_ptr);
        if (ret[1]) {
            throw takeFromExternrefTable0(ret[0]);
        }
    }
    /**
     * @param {number} seed
     */
    init_world(seed) {
        wasm.engine_init_world(this.__wbg_ptr, seed);
    }
    /**
     * @returns {boolean}
     */
    is_3d_mode() {
        const ret = wasm.engine_is_3d_mode(this.__wbg_ptr);
        return ret !== 0;
    }
    /**
     * @returns {string | undefined}
     */
    is_game_failed() {
        const ret = wasm.engine_is_game_failed(this.__wbg_ptr);
        let v1;
        if (ret[0] !== 0) {
            v1 = getStringFromWasm0(ret[0], ret[1]);
            wasm.__wbindgen_free(ret[0], ret[1] * 1, 1);
        }
        return v1;
    }
    /**
     * @returns {boolean}
     */
    is_game_finished() {
        const ret = wasm.engine_is_game_finished(this.__wbg_ptr);
        return ret !== 0;
    }
    /**
     * @param {number} x
     * @param {number} y
     * @returns {boolean}
     */
    is_walkable(x, y) {
        const ret = wasm.engine_is_walkable(this.__wbg_ptr, x, y);
        return ret !== 0;
    }
    /**
     * @param {string} canvas_id
     */
    constructor(canvas_id) {
        const ptr0 = passStringToWasm0(canvas_id, wasm.__wbindgen_malloc, wasm.__wbindgen_realloc);
        const len0 = WASM_VECTOR_LEN;
        const ret = wasm.engine_new(ptr0, len0);
        if (ret[2]) {
            throw takeFromExternrefTable0(ret[1]);
        }
        this.__wbg_ptr = ret[0];
        EngineFinalization.register(this, this.__wbg_ptr, this);
        return this;
    }
    /**
     * @param {number} delta_yaw
     * @param {number} delta_pitch
     */
    orbit_camera(delta_yaw, delta_pitch) {
        wasm.engine_orbit_camera(this.__wbg_ptr, delta_yaw, delta_pitch);
    }
    /**
     * @param {number} dx
     * @param {number} dz
     */
    pan_camera(dx, dz) {
        wasm.engine_pan_camera(this.__wbg_ptr, dx, dz);
    }
    /**
     * @param {number} wx
     * @param {number} wz
     * @param {number} cw
     * @param {number} ch
     * @returns {Float32Array}
     */
    project_to_screen(wx, wz, cw, ch) {
        const ret = wasm.engine_project_to_screen(this.__wbg_ptr, wx, wz, cw, ch);
        return ret;
    }
    /**
     * @param {number} dt
     * @param {number} sun_x
     * @param {number} sun_y
     * @param {number} sun_z
     * @param {number} night
     * @param {number} flash
     * @param {number} fade
     * @param {number} shake_x
     * @param {number} shake_y
     * @param {number} canvas_w
     * @param {number} canvas_h
     */
    render_3d(dt, sun_x, sun_y, sun_z, night, flash, fade, shake_x, shake_y, canvas_w, canvas_h) {
        wasm.engine_render_3d(this.__wbg_ptr, dt, sun_x, sun_y, sun_z, night, flash, fade, shake_x, shake_y, canvas_w, canvas_h);
    }
    /**
     * @param {number} night
     * @param {number} flash
     * @param {number} fade
     * @param {number} shake_x
     * @param {number} shake_y
     * @param {number} canvas_w
     * @param {number} canvas_h
     */
    render_uploaded_canvas(night, flash, fade, shake_x, shake_y, canvas_w, canvas_h) {
        wasm.engine_render_uploaded_canvas(this.__wbg_ptr, night, flash, fade, shake_x, shake_y, canvas_w, canvas_h);
    }
    /**
     * @param {number} sx
     * @param {number} sy
     * @param {number} cw
     * @param {number} ch
     * @returns {Float32Array}
     */
    screen_to_ground(sx, sy, cw, ch) {
        const ret = wasm.engine_screen_to_ground(this.__wbg_ptr, sx, sy, cw, ch);
        return ret;
    }
    /**
     * @param {boolean} enabled
     */
    set_3d_mode(enabled) {
        wasm.engine_set_3d_mode(this.__wbg_ptr, enabled);
    }
    /**
     * The colour around the map in 3D (the world's deep sea, 0..255), so the view's edges read
     * as open ocean. JS passes its palette's deep-water colour.
     * @param {number} r
     * @param {number} g
     * @param {number} b
     */
    set_backdrop(r, g, b) {
        wasm.engine_set_backdrop(this.__wbg_ptr, r, g, b);
    }
    /**
     * @param {number} tx
     * @param {number} tz
     */
    set_camera_target(tx, tz) {
        wasm.engine_set_camera_target(this.__wbg_ptr, tx, tz);
    }
    /**
     * @param {number} era
     */
    set_era(era) {
        wasm.engine_set_era(this.__wbg_ptr, era);
    }
    /**
     * Use the JS world's heights (row-major W×H, 0..1, sea level 0.33) for the 3D mesh, the
     * raycast and where billboards stand.
     * @param {Float32Array | number[]} heights
     */
    set_heightmap(heights) {
        const ptr0 = passArrayF32ToWasm0(heights, wasm.__wbindgen_malloc);
        const len0 = WASM_VECTOR_LEN;
        const ret = wasm.engine_set_heightmap(this.__wbg_ptr, ptr0, len0);
        if (ret[1]) {
            throw takeFromExternrefTable0(ret[0]);
        }
    }
    /**
     * @param {HTMLCanvasElement} canvas
     */
    upload_canvas(canvas) {
        const ret = wasm.engine_upload_canvas(this.__wbg_ptr, canvas);
        if (ret[1]) {
            throw takeFromExternrefTable0(ret[0]);
        }
    }
    /**
     * @param {number} factor
     */
    zoom_camera(factor) {
        wasm.engine_zoom_camera(this.__wbg_ptr, factor);
    }
}
if (Symbol.dispose) Engine.prototype[Symbol.dispose] = Engine.prototype.free;

export class RustBlinkClassifier {
    __destroy_into_raw() {
        const ptr = this.__wbg_ptr;
        this.__wbg_ptr = 0;
        RustBlinkClassifierFinalization.unregister(this);
        return ptr;
    }
    free() {
        const ptr = this.__destroy_into_raw();
        wasm.__wbg_rustblinkclassifier_free(ptr, 0);
    }
    /**
     * @returns {number}
     */
    accuracy() {
        const ret = wasm.rustblinkclassifier_accuracy(this.__wbg_ptr);
        return ret;
    }
    /**
     * Arms a flash-cue capture window (e.g., 1.2s) right when the visual cue flashes on screen.
     * @param {number} seconds
     */
    arm_flash_window(seconds) {
        wasm.rustblinkclassifier_arm_flash_window(this.__wbg_ptr, seconds);
    }
    /**
     * @returns {number}
     */
    baseline_samples() {
        const ret = wasm.rustblinkclassifier_baseline_samples(this.__wbg_ptr);
        return ret >>> 0;
    }
    /**
     * @returns {number}
     */
    blink_samples() {
        const ret = wasm.rustblinkclassifier_blink_samples(this.__wbg_ptr);
        return ret >>> 0;
    }
    /**
     * @param {Float32Array | number[]} tp9
     * @param {Float32Array | number[]} af7
     * @param {Float32Array | number[]} af8
     * @param {Float32Array | number[]} tp10
     * @param {number} motion
     * @param {number} dt
     * @returns {boolean}
     */
    ingest_frame(tp9, af7, af8, tp10, motion, dt) {
        const val5 = +motion;
        const val6 = +dt;
        let ptr0, len0, ptr1, len1, ptr2, len2, ptr3, len3;
        try {
            ptr0 = passArrayF32ToWasm0(tp9, wasm.__wbindgen_malloc);
            len0 = WASM_VECTOR_LEN;
            ptr1 = passArrayF32ToWasm0(af7, wasm.__wbindgen_malloc);
            len1 = WASM_VECTOR_LEN;
            ptr2 = passArrayF32ToWasm0(af8, wasm.__wbindgen_malloc);
            len2 = WASM_VECTOR_LEN;
            ptr3 = passArrayF32ToWasm0(tp10, wasm.__wbindgen_malloc);
            len3 = WASM_VECTOR_LEN;
        } catch (e) {
            if (ptr3) { wasm.__wbindgen_free(ptr3, len3 * 4, 4); }
            if (ptr2) { wasm.__wbindgen_free(ptr2, len2 * 4, 4); }
            if (ptr1) { wasm.__wbindgen_free(ptr1, len1 * 4, 4); }
            if (ptr0) { wasm.__wbindgen_free(ptr0, len0 * 4, 4); }
            throw e;
        }
        const ret = wasm.rustblinkclassifier_ingest_frame(this.__wbg_ptr, ptr0, len0, ptr1, len1, ptr2, len2, ptr3, len3, val5, val6);
        return ret !== 0;
    }
    /**
     * @returns {boolean}
     */
    is_capturing() {
        const ret = wasm.rustblinkclassifier_is_capturing(this.__wbg_ptr);
        return ret !== 0;
    }
    /**
     * @returns {boolean}
     */
    is_trained() {
        const ret = wasm.rustblinkclassifier_is_trained(this.__wbg_ptr);
        return ret !== 0;
    }
    /**
     * @returns {number}
     */
    last_af7_ptp() {
        const ret = wasm.rustblinkclassifier_last_af7_ptp(this.__wbg_ptr);
        return ret;
    }
    /**
     * @returns {number}
     */
    last_af8_ptp() {
        const ret = wasm.rustblinkclassifier_last_af8_ptp(this.__wbg_ptr);
        return ret;
    }
    /**
     * @returns {number}
     */
    last_corr() {
        const ret = wasm.rustblinkclassifier_last_corr(this.__wbg_ptr);
        return ret;
    }
    /**
     * @returns {number}
     */
    last_eff_amp() {
        const ret = wasm.rustblinkclassifier_last_eff_amp(this.__wbg_ptr);
        return ret;
    }
    /**
     * @returns {number}
     */
    last_probability() {
        const ret = wasm.rustblinkclassifier_last_probability(this.__wbg_ptr);
        return ret;
    }
    /**
     * @returns {number}
     */
    last_tp10_ptp() {
        const ret = wasm.rustblinkclassifier_last_tp10_ptp(this.__wbg_ptr);
        return ret;
    }
    /**
     * @returns {number}
     */
    last_tp9_ptp() {
        const ret = wasm.rustblinkclassifier_last_tp9_ptp(this.__wbg_ptr);
        return ret;
    }
    /**
     * @returns {number}
     */
    learned_af7_uv() {
        const ret = wasm.rustblinkclassifier_learned_af7_uv(this.__wbg_ptr);
        return ret;
    }
    /**
     * @returns {number}
     */
    learned_af8_uv() {
        const ret = wasm.rustblinkclassifier_learned_af8_uv(this.__wbg_ptr);
        return ret;
    }
    /**
     * @returns {number}
     */
    learned_threshold_uv() {
        const ret = wasm.rustblinkclassifier_learned_threshold_uv(this.__wbg_ptr);
        return ret;
    }
    constructor() {
        const ret = wasm.rustblinkclassifier_new();
        this.__wbg_ptr = ret;
        RustBlinkClassifierFinalization.register(this, this.__wbg_ptr, this);
        return this;
    }
    /**
     * Records the latest EEG window as a non-blink negative sample (used during Hold Still
     * and Move Head Around Without Blinking calibration stages to cancel noise & hum).
     * @returns {boolean}
     */
    record_baseline_button() {
        const ret = wasm.rustblinkclassifier_record_baseline_button(this.__wbg_ptr);
        return ret !== 0;
    }
    /**
     * Searches the last ~2.5s of EEG windows for the strongest blink peak if the user blinked
     * before clicking, or arms a 2.5s capture window if they click first and then blink.
     * @returns {boolean}
     */
    record_blink_button() {
        const ret = wasm.rustblinkclassifier_record_blink_button(this.__wbg_ptr);
        return ret !== 0;
    }
    reset_model() {
        wasm.rustblinkclassifier_reset_model(this.__wbg_ptr);
    }
    /**
     * @returns {number}
     */
    total_detected() {
        const ret = wasm.rustblinkclassifier_total_detected(this.__wbg_ptr);
        return ret >>> 0;
    }
}
if (Symbol.dispose) RustBlinkClassifier.prototype[Symbol.dispose] = RustBlinkClassifier.prototype.free;

export class RustImuTracker {
    __destroy_into_raw() {
        const ptr = this.__wbg_ptr;
        this.__wbg_ptr = 0;
        RustImuTrackerFinalization.unregister(this);
        return ptr;
    }
    free() {
        const ptr = this.__destroy_into_raw();
        wasm.__wbg_rustimutracker_free(ptr, 0);
    }
    /**
     * Records a supervised sample pairing the user's current 3D relative orientation
     * `(rel_yaw, rel_pitch, rel_roll)` with the target white dot screen position `(dot_x, dot_y)` in `0..1`.
     * @param {number} dot_x
     * @param {number} dot_y
     */
    add_calib_point(dot_x, dot_y) {
        wasm.rustimutracker_add_calib_point(this.__wbg_ptr, dot_x, dot_y);
    }
    /**
     * @returns {number}
     */
    aim_x() {
        const ret = wasm.rustimutracker_aim_x(this.__wbg_ptr);
        return ret;
    }
    /**
     * @returns {number}
     */
    aim_y() {
        const ret = wasm.rustimutracker_aim_y(this.__wbg_ptr);
        return ret;
    }
    begin_motion_calibration() {
        wasm.rustimutracker_begin_motion_calibration(this.__wbg_ptr);
    }
    /**
     * @returns {number}
     */
    calib_progress() {
        const ret = wasm.rustimutracker_calib_progress(this.__wbg_ptr);
        return ret;
    }
    /**
     * @returns {number}
     */
    calib_r2_score() {
        const ret = wasm.rustimutracker_calib_r2_score(this.__wbg_ptr);
        return ret;
    }
    /**
     * @returns {number}
     */
    calib_sample_count() {
        const ret = wasm.rustimutracker_calib_sample_count(this.__wbg_ptr);
        return ret >>> 0;
    }
    cancel_motion_calibration() {
        wasm.rustimutracker_cancel_motion_calibration(this.__wbg_ptr);
    }
    /**
     * Fits a 2D Ridge Regression + Piecewise Directional Reach model in Rust mapping
     * `[1, rel_yaw, rel_pitch, rel_roll]` -> `(tilt_x, tilt_y)`.
     * Automatically discovers axis signs, high vertical pitch sensitivity, cross-axis coupling,
     * and separate Left/Right/Up/Down edge reach scales so every full screen edge is reachable.
     * @returns {boolean}
     */
    finish_motion_calibration() {
        const ret = wasm.rustimutracker_finish_motion_calibration(this.__wbg_ptr);
        return ret !== 0;
    }
    /**
     * @returns {boolean}
     */
    has_motion_model() {
        const ret = wasm.rustimutracker_has_motion_model(this.__wbg_ptr);
        return ret !== 0;
    }
    /**
     * @returns {boolean}
     */
    is_tared() {
        const ret = wasm.rustimutracker_is_tared(this.__wbg_ptr);
        return ret !== 0;
    }
    /**
     * @returns {number}
     */
    learned_pitch_deg_for_edge() {
        const ret = wasm.rustimutracker_learned_pitch_deg_for_edge(this.__wbg_ptr);
        return ret;
    }
    /**
     * @returns {number}
     */
    learned_pitch_sign() {
        const ret = wasm.rustimutracker_learned_pitch_sign(this.__wbg_ptr);
        return ret;
    }
    /**
     * @returns {number}
     */
    learned_yaw_deg_for_edge() {
        const ret = wasm.rustimutracker_learned_yaw_deg_for_edge(this.__wbg_ptr);
        return ret;
    }
    /**
     * @returns {number}
     */
    learned_yaw_sign() {
        const ret = wasm.rustimutracker_learned_yaw_sign(this.__wbg_ptr);
        return ret;
    }
    constructor() {
        const ret = wasm.rustimutracker_new();
        this.__wbg_ptr = ret;
        RustImuTrackerFinalization.register(this, this.__wbg_ptr, this);
        return this;
    }
    /**
     * @returns {number}
     */
    raw_pitch_deg() {
        const ret = wasm.rustimutracker_raw_pitch_deg(this.__wbg_ptr);
        return ret;
    }
    /**
     * @returns {number}
     */
    raw_roll_deg() {
        const ret = wasm.rustimutracker_raw_roll_deg(this.__wbg_ptr);
        return ret;
    }
    /**
     * @returns {number}
     */
    raw_yaw_deg() {
        const ret = wasm.rustimutracker_raw_yaw_deg(this.__wbg_ptr);
        return ret;
    }
    /**
     * @returns {number}
     */
    rel_pitch_deg() {
        const ret = wasm.rustimutracker_rel_pitch_deg(this.__wbg_ptr);
        return ret;
    }
    /**
     * @returns {number}
     */
    rel_roll_deg() {
        const ret = wasm.rustimutracker_rel_roll_deg(this.__wbg_ptr);
        return ret;
    }
    /**
     * @returns {number}
     */
    rel_yaw_deg() {
        const ret = wasm.rustimutracker_rel_yaw_deg(this.__wbg_ptr);
        return ret;
    }
    reset_motion_model() {
        wasm.rustimutracker_reset_motion_model(this.__wbg_ptr);
    }
    /**
     * @param {number} sensitivity
     * @param {boolean} invert_yaw
     * @param {boolean} invert_pitch
     * @param {number} deadzone_deg
     */
    set_config(sensitivity, invert_yaw, invert_pitch, deadzone_deg) {
        wasm.rustimutracker_set_config(this.__wbg_ptr, sensitivity, invert_yaw, invert_pitch, deadzone_deg);
    }
    tare() {
        wasm.rustimutracker_tare(this.__wbg_ptr);
    }
    /**
     * @returns {number}
     */
    tilt_x() {
        const ret = wasm.rustimutracker_tilt_x(this.__wbg_ptr);
        return ret;
    }
    /**
     * @returns {number}
     */
    tilt_y() {
        const ret = wasm.rustimutracker_tilt_y(this.__wbg_ptr);
        return ret;
    }
    /**
     * @param {number} ax
     * @param {number} ay
     * @param {number} az
     * @param {number} gx
     * @param {number} gy
     * @param {number} gz
     * @param {number} bridge_yaw
     * @param {boolean} has_bridge_yaw
     * @param {number} dt
     */
    update(ax, ay, az, gx, gy, gz, bridge_yaw, has_bridge_yaw, dt) {
        wasm.rustimutracker_update(this.__wbg_ptr, ax, ay, az, gx, gy, gz, bridge_yaw, has_bridge_yaw, dt);
    }
}
if (Symbol.dispose) RustImuTracker.prototype[Symbol.dispose] = RustImuTracker.prototype.free;
function __wbg_get_imports() {
    const import0 = {
        __proto__: null,
        __wbg_Q_6a9f63322b7250de: function(arg0) {
            const ret = arg0.Q;
            return ret;
        },
        __wbg___wbindgen_boolean_get_4f1baf9a00be3c74: function(arg0) {
            const v = arg0;
            const ret = typeof(v) === 'boolean' ? v : undefined;
            return isLikeNone(ret) ? 0xFFFFFF : ret ? 1 : 0;
        },
        __wbg___wbindgen_debug_string_f249a2a76caa9a55: function(arg0, arg1) {
            const ret = debugString(arg1);
            const ptr1 = passStringToWasm0(ret, wasm.__wbindgen_malloc, wasm.__wbindgen_realloc);
            const len1 = WASM_VECTOR_LEN;
            getDataViewMemory0().setInt32((arg0 >>> 0) + 4 * 1, len1, true);
            getDataViewMemory0().setInt32((arg0 >>> 0) + 4 * 0, ptr1, true);
        },
        __wbg___wbindgen_is_undefined_be5b34249ff1fc15: function(arg0) {
            const ret = arg0 === undefined;
            return ret;
        },
        __wbg___wbindgen_throw_4c88d8bf264fd984: function(arg0, arg1) {
            throw new Error(getStringFromWasm0(arg0, arg1));
        },
        __wbg_activeTexture_45b425a49df733ce: function(arg0, arg1) {
            arg0.activeTexture(arg1 >>> 0);
        },
        __wbg_add_cf82c8d153d93ae1: function() { return handleError(function (arg0, arg1, arg2) {
            arg0.add(getStringFromWasm0(arg1, arg2));
        }, arguments); },
        __wbg_appendChild_dbf7ad2f48413560: function() { return handleError(function (arg0, arg1) {
            const ret = arg0.appendChild(arg1);
            return ret;
        }, arguments); },
        __wbg_attachShader_817dcec9e79fd18f: function(arg0, arg1, arg2) {
            arg0.attachShader(arg1, arg2);
        },
        __wbg_bindBuffer_b3bb001fdbee089a: function(arg0, arg1, arg2) {
            arg0.bindBuffer(arg1 >>> 0, arg2);
        },
        __wbg_bindFramebuffer_b245cd93a150dd96: function(arg0, arg1, arg2) {
            arg0.bindFramebuffer(arg1 >>> 0, arg2);
        },
        __wbg_bindRenderbuffer_3458a69a6dc55057: function(arg0, arg1, arg2) {
            arg0.bindRenderbuffer(arg1 >>> 0, arg2);
        },
        __wbg_bindTexture_6f822943b2790466: function(arg0, arg1, arg2) {
            arg0.bindTexture(arg1 >>> 0, arg2);
        },
        __wbg_blendFunc_68b4cfb9bd98a13f: function(arg0, arg1, arg2) {
            arg0.blendFunc(arg1 >>> 0, arg2 >>> 0);
        },
        __wbg_bufferData_2fb8740d60478bf0: function(arg0, arg1, arg2, arg3) {
            arg0.bufferData(arg1 >>> 0, arg2, arg3 >>> 0);
        },
        __wbg_bufferData_aaab2a06f5c64e23: function(arg0, arg1, arg2, arg3, arg4) {
            arg0.bufferData(arg1 >>> 0, getArrayU8FromWasm0(arg2, arg3), arg4 >>> 0);
        },
        __wbg_children_5767547ca7e1c276: function(arg0) {
            const ret = arg0.children;
            return ret;
        },
        __wbg_classList_97e966c261669b85: function(arg0) {
            const ret = arg0.classList;
            return ret;
        },
        __wbg_clearColor_0a9b08857907c212: function(arg0, arg1, arg2, arg3, arg4) {
            arg0.clearColor(arg1, arg2, arg3, arg4);
        },
        __wbg_clear_3ffa1ed794f92a04: function(arg0, arg1) {
            arg0.clear(arg1 >>> 0);
        },
        __wbg_compileShader_54c3771819e6caba: function(arg0, arg1) {
            arg0.compileShader(arg1);
        },
        __wbg_connect_744eb2f8329488e7: function() { return handleError(function (arg0, arg1) {
            const ret = arg0.connect(arg1);
            return ret;
        }, arguments); },
        __wbg_copyToChannel_0544d629fa356aea: function() { return handleError(function (arg0, arg1, arg2, arg3) {
            arg0.copyToChannel(getArrayF32FromWasm0(arg1, arg2), arg3);
        }, arguments); },
        __wbg_createBiquadFilter_acc67cb68f15ac1a: function() { return handleError(function (arg0) {
            const ret = arg0.createBiquadFilter();
            return ret;
        }, arguments); },
        __wbg_createBufferSource_a3f2da3d654cf683: function() { return handleError(function (arg0) {
            const ret = arg0.createBufferSource();
            return ret;
        }, arguments); },
        __wbg_createBuffer_822310b4ce995425: function() { return handleError(function (arg0, arg1, arg2, arg3) {
            const ret = arg0.createBuffer(arg1 >>> 0, arg2 >>> 0, arg3);
            return ret;
        }, arguments); },
        __wbg_createBuffer_ca6ca40e9fb13678: function(arg0) {
            const ret = arg0.createBuffer();
            return isLikeNone(ret) ? 0 : addToExternrefTable0(ret);
        },
        __wbg_createElement_de4a31ed2fb252be: function() { return handleError(function (arg0, arg1, arg2) {
            const ret = arg0.createElement(getStringFromWasm0(arg1, arg2));
            return ret;
        }, arguments); },
        __wbg_createFramebuffer_f52399034cb1a200: function(arg0) {
            const ret = arg0.createFramebuffer();
            return isLikeNone(ret) ? 0 : addToExternrefTable0(ret);
        },
        __wbg_createGain_044bd14c7dcc1831: function() { return handleError(function (arg0) {
            const ret = arg0.createGain();
            return ret;
        }, arguments); },
        __wbg_createOscillator_74d5c931ba0641d6: function() { return handleError(function (arg0) {
            const ret = arg0.createOscillator();
            return ret;
        }, arguments); },
        __wbg_createProgram_de0881c688a8e43b: function(arg0) {
            const ret = arg0.createProgram();
            return isLikeNone(ret) ? 0 : addToExternrefTable0(ret);
        },
        __wbg_createRenderbuffer_19c2eec619a37a15: function(arg0) {
            const ret = arg0.createRenderbuffer();
            return isLikeNone(ret) ? 0 : addToExternrefTable0(ret);
        },
        __wbg_createShader_d2d90020273cef2b: function(arg0, arg1) {
            const ret = arg0.createShader(arg1 >>> 0);
            return isLikeNone(ret) ? 0 : addToExternrefTable0(ret);
        },
        __wbg_createTexture_eec9a1994aabeca7: function(arg0) {
            const ret = arg0.createTexture();
            return isLikeNone(ret) ? 0 : addToExternrefTable0(ret);
        },
        __wbg_currentTime_8eea38acd4374c55: function(arg0) {
            const ret = arg0.currentTime;
            return ret;
        },
        __wbg_depthFunc_584bfa0174e7628a: function(arg0, arg1) {
            arg0.depthFunc(arg1 >>> 0);
        },
        __wbg_depthMask_276856a569045897: function(arg0, arg1) {
            arg0.depthMask(arg1 !== 0);
        },
        __wbg_destination_9a4d8f898e5c0a0b: function(arg0) {
            const ret = arg0.destination;
            return ret;
        },
        __wbg_disableVertexAttribArray_5c66375eba552ee7: function(arg0, arg1) {
            arg0.disableVertexAttribArray(arg1 >>> 0);
        },
        __wbg_disable_2ead6c11cefd0322: function(arg0, arg1) {
            arg0.disable(arg1 >>> 0);
        },
        __wbg_document_3b5c3071b43e97ee: function(arg0) {
            const ret = arg0.document;
            return isLikeNone(ret) ? 0 : addToExternrefTable0(ret);
        },
        __wbg_drawArrays_fe5a51410bfabb5b: function(arg0, arg1, arg2, arg3) {
            arg0.drawArrays(arg1 >>> 0, arg2, arg3);
        },
        __wbg_drawElements_c70e0887282fa70e: function(arg0, arg1, arg2, arg3, arg4) {
            arg0.drawElements(arg1 >>> 0, arg2, arg3 >>> 0, arg4);
        },
        __wbg_enableVertexAttribArray_d485706ade6bebf8: function(arg0, arg1) {
            arg0.enableVertexAttribArray(arg1 >>> 0);
        },
        __wbg_enable_c3c00565c89d3175: function(arg0, arg1) {
            arg0.enable(arg1 >>> 0);
        },
        __wbg_error_757e9472f8410341: function(arg0, arg1) {
            let deferred0_0;
            let deferred0_1;
            try {
                deferred0_0 = arg0;
                deferred0_1 = arg1;
                console.error(getStringFromWasm0(arg0, arg1));
            } finally {
                wasm.__wbindgen_free(deferred0_0, deferred0_1, 1);
            }
        },
        __wbg_exponentialRampToValueAtTime_9bda499351acfed8: function() { return handleError(function (arg0, arg1, arg2) {
            const ret = arg0.exponentialRampToValueAtTime(arg1, arg2);
            return ret;
        }, arguments); },
        __wbg_framebufferRenderbuffer_1c89563506a7d0ac: function(arg0, arg1, arg2, arg3, arg4) {
            arg0.framebufferRenderbuffer(arg1 >>> 0, arg2 >>> 0, arg3 >>> 0, arg4);
        },
        __wbg_framebufferTexture2D_2c69ac0ce67faf7d: function(arg0, arg1, arg2, arg3, arg4, arg5) {
            arg0.framebufferTexture2D(arg1 >>> 0, arg2 >>> 0, arg3 >>> 0, arg4, arg5);
        },
        __wbg_frequency_246b3759f7868e01: function(arg0) {
            const ret = arg0.frequency;
            return ret;
        },
        __wbg_frequency_66f15407131f5ab1: function(arg0) {
            const ret = arg0.frequency;
            return ret;
        },
        __wbg_gain_4109a65ad4b19834: function(arg0) {
            const ret = arg0.gain;
            return ret;
        },
        __wbg_getAttribLocation_8e502cd753de95f7: function(arg0, arg1, arg2, arg3) {
            const ret = arg0.getAttribLocation(arg1, getStringFromWasm0(arg2, arg3));
            return ret;
        },
        __wbg_getContext_d93f904b714852ae: function() { return handleError(function (arg0, arg1, arg2) {
            const ret = arg0.getContext(getStringFromWasm0(arg1, arg2));
            return isLikeNone(ret) ? 0 : addToExternrefTable0(ret);
        }, arguments); },
        __wbg_getElementById_7ee213db584f37b0: function(arg0, arg1, arg2) {
            const ret = arg0.getElementById(getStringFromWasm0(arg1, arg2));
            return isLikeNone(ret) ? 0 : addToExternrefTable0(ret);
        },
        __wbg_getProgramInfoLog_92d61551e1ab39ea: function(arg0, arg1, arg2) {
            const ret = arg1.getProgramInfoLog(arg2);
            var ptr1 = isLikeNone(ret) ? 0 : passStringToWasm0(ret, wasm.__wbindgen_malloc, wasm.__wbindgen_realloc);
            var len1 = WASM_VECTOR_LEN;
            getDataViewMemory0().setInt32((arg0 >>> 0) + 4 * 1, len1, true);
            getDataViewMemory0().setInt32((arg0 >>> 0) + 4 * 0, ptr1, true);
        },
        __wbg_getProgramParameter_3edd526f9e5f6c10: function(arg0, arg1, arg2) {
            const ret = arg0.getProgramParameter(arg1, arg2 >>> 0);
            return ret;
        },
        __wbg_getShaderInfoLog_8758e8aea7960121: function(arg0, arg1, arg2) {
            const ret = arg1.getShaderInfoLog(arg2);
            var ptr1 = isLikeNone(ret) ? 0 : passStringToWasm0(ret, wasm.__wbindgen_malloc, wasm.__wbindgen_realloc);
            var len1 = WASM_VECTOR_LEN;
            getDataViewMemory0().setInt32((arg0 >>> 0) + 4 * 1, len1, true);
            getDataViewMemory0().setInt32((arg0 >>> 0) + 4 * 0, ptr1, true);
        },
        __wbg_getShaderParameter_a346f74d5ba41c4b: function(arg0, arg1, arg2) {
            const ret = arg0.getShaderParameter(arg1, arg2 >>> 0);
            return ret;
        },
        __wbg_getUniformLocation_e0d11c86376cdd26: function(arg0, arg1, arg2, arg3) {
            const ret = arg0.getUniformLocation(arg1, getStringFromWasm0(arg2, arg3));
            return isLikeNone(ret) ? 0 : addToExternrefTable0(ret);
        },
        __wbg_instanceof_AudioNode_9604113cc9887c0d: function(arg0) {
            let result;
            try {
                result = arg0 instanceof AudioNode;
            } catch (_) {
                result = false;
            }
            const ret = result;
            return ret;
        },
        __wbg_instanceof_HtmlCanvasElement_89f280683e987b9e: function(arg0) {
            let result;
            try {
                result = arg0 instanceof HTMLCanvasElement;
            } catch (_) {
                result = false;
            }
            const ret = result;
            return ret;
        },
        __wbg_instanceof_HtmlElement_ea32ea7121e0a496: function(arg0) {
            let result;
            try {
                result = arg0 instanceof HTMLElement;
            } catch (_) {
                result = false;
            }
            const ret = result;
            return ret;
        },
        __wbg_instanceof_WebGlRenderingContext_6e70e224f022d27a: function(arg0) {
            let result;
            try {
                result = arg0 instanceof WebGLRenderingContext;
            } catch (_) {
                result = false;
            }
            const ret = result;
            return ret;
        },
        __wbg_instanceof_Window_5cf453f5639aa1a2: function(arg0) {
            let result;
            try {
                result = arg0 instanceof Window;
            } catch (_) {
                result = false;
            }
            const ret = result;
            return ret;
        },
        __wbg_item_66d2ef77cbc53343: function(arg0, arg1) {
            const ret = arg0.item(arg1 >>> 0);
            return isLikeNone(ret) ? 0 : addToExternrefTable0(ret);
        },
        __wbg_length_9f82f041dce0ac51: function(arg0) {
            const ret = arg0.length;
            return ret;
        },
        __wbg_linkProgram_dee7cda98bacb12f: function(arg0, arg1) {
            arg0.linkProgram(arg1);
        },
        __wbg_new_2dba524b1d8f2489: function() { return handleError(function () {
            const ret = new lAudioContext();
            return ret;
        }, arguments); },
        __wbg_new_bb74001155cb8ca8: function() {
            const ret = new Error();
            return ret;
        },
        __wbg_new_with_length_5c0c9043ebd9ec32: function(arg0) {
            const ret = new Float32Array(arg0 >>> 0);
            return ret;
        },
        __wbg_ownerDocument_adefd47fd596fe80: function(arg0) {
            const ret = arg0.ownerDocument;
            return isLikeNone(ret) ? 0 : addToExternrefTable0(ret);
        },
        __wbg_querySelector_1555c239948f2bf0: function() { return handleError(function (arg0, arg1, arg2) {
            const ret = arg0.querySelector(getStringFromWasm0(arg1, arg2));
            return isLikeNone(ret) ? 0 : addToExternrefTable0(ret);
        }, arguments); },
        __wbg_querySelector_72201b7c62dc196b: function() { return handleError(function (arg0, arg1, arg2) {
            const ret = arg0.querySelector(getStringFromWasm0(arg1, arg2));
            return isLikeNone(ret) ? 0 : addToExternrefTable0(ret);
        }, arguments); },
        __wbg_random_df36b90cd78a7d54: function() {
            const ret = Math.random();
            return ret;
        },
        __wbg_remove_12e9b1deffec7d2f: function() { return handleError(function (arg0, arg1, arg2) {
            arg0.remove(getStringFromWasm0(arg1, arg2));
        }, arguments); },
        __wbg_renderbufferStorage_0d080ee86520d3bd: function(arg0, arg1, arg2, arg3, arg4) {
            arg0.renderbufferStorage(arg1 >>> 0, arg2 >>> 0, arg3, arg4);
        },
        __wbg_resume_eb9b8678e3c9d27b: function() { return handleError(function (arg0) {
            const ret = arg0.resume();
            return ret;
        }, arguments); },
        __wbg_sampleRate_239ec0f08f69ef0b: function(arg0) {
            const ret = arg0.sampleRate;
            return ret;
        },
        __wbg_setAttribute_f75317b2b5cabfb4: function() { return handleError(function (arg0, arg1, arg2, arg3, arg4) {
            arg0.setAttribute(getStringFromWasm0(arg1, arg2), getStringFromWasm0(arg3, arg4));
        }, arguments); },
        __wbg_setProperty_3fe74e09d53acb0b: function() { return handleError(function (arg0, arg1, arg2, arg3, arg4) {
            arg0.setProperty(getStringFromWasm0(arg1, arg2), getStringFromWasm0(arg3, arg4));
        }, arguments); },
        __wbg_setValueAtTime_9bac12b7018fa62b: function() { return handleError(function (arg0, arg1, arg2) {
            const ret = arg0.setValueAtTime(arg1, arg2);
            return ret;
        }, arguments); },
        __wbg_set_buffer_beed412be951c2c1: function(arg0, arg1) {
            arg0.buffer = arg1;
        },
        __wbg_set_className_fcd90345b3a090af: function(arg0, arg1, arg2) {
            arg0.className = getStringFromWasm0(arg1, arg2);
        },
        __wbg_set_index_839acc297b0f8e94: function(arg0, arg1, arg2) {
            arg0[arg1 >>> 0] = arg2;
        },
        __wbg_set_innerHTML_a9d3a267967b5117: function(arg0, arg1, arg2) {
            arg0.innerHTML = getStringFromWasm0(arg1, arg2);
        },
        __wbg_set_textContent_0177b2a6d9293c2c: function(arg0, arg1, arg2) {
            arg0.textContent = arg1 === 0 ? undefined : getStringFromWasm0(arg1, arg2);
        },
        __wbg_set_type_383d7e6eabecd9c8: function(arg0, arg1) {
            arg0.type = __wbindgen_enum_OscillatorType[arg1];
        },
        __wbg_set_type_c80ee1b6e1ea14b8: function(arg0, arg1) {
            arg0.type = __wbindgen_enum_BiquadFilterType[arg1];
        },
        __wbg_set_value_892851ccbfe211bd: function(arg0, arg1) {
            arg0.value = arg1;
        },
        __wbg_shaderSource_c4c06a3cbc2e7bf3: function(arg0, arg1, arg2, arg3) {
            arg0.shaderSource(arg1, getStringFromWasm0(arg2, arg3));
        },
        __wbg_stack_575c4e0918581b9c: function(arg0, arg1) {
            const ret = arg1.stack;
            const ptr1 = passStringToWasm0(ret, wasm.__wbindgen_malloc, wasm.__wbindgen_realloc);
            const len1 = WASM_VECTOR_LEN;
            getDataViewMemory0().setInt32((arg0 >>> 0) + 4 * 1, len1, true);
            getDataViewMemory0().setInt32((arg0 >>> 0) + 4 * 0, ptr1, true);
        },
        __wbg_start_df2eaeb65de4742a: function() { return handleError(function (arg0, arg1) {
            arg0.start(arg1);
        }, arguments); },
        __wbg_start_e85e03c281bebe6e: function() { return handleError(function (arg0, arg1) {
            arg0.start(arg1);
        }, arguments); },
        __wbg_static_accessor_GLOBAL_THIS_003ee833af9699fc: function() {
            const ret = typeof globalThis === 'undefined' ? undefined : globalThis;
            return isLikeNone(ret) ? 0 : addToExternrefTable0(ret);
        },
        __wbg_static_accessor_GLOBAL_e4293fb22cbe77da: function() {
            const ret = typeof global === 'undefined' ? undefined : global;
            return isLikeNone(ret) ? 0 : addToExternrefTable0(ret);
        },
        __wbg_static_accessor_SELF_53bb20457401453d: function() {
            const ret = typeof self === 'undefined' ? undefined : self;
            return isLikeNone(ret) ? 0 : addToExternrefTable0(ret);
        },
        __wbg_static_accessor_WINDOW_b5e0810bfecb7feb: function() {
            const ret = typeof window === 'undefined' ? undefined : window;
            return isLikeNone(ret) ? 0 : addToExternrefTable0(ret);
        },
        __wbg_stop_491c9e10bf6cfe54: function() { return handleError(function (arg0, arg1) {
            arg0.stop(arg1);
        }, arguments); },
        __wbg_stop_aefe8f3db1cbc06a: function() { return handleError(function (arg0, arg1) {
            arg0.stop(arg1);
        }, arguments); },
        __wbg_style_f499e599ac70277f: function(arg0) {
            const ret = arg0.style;
            return ret;
        },
        __wbg_texImage2D_2cfc4b5b6b17c8d3: function() { return handleError(function (arg0, arg1, arg2, arg3, arg4, arg5, arg6, arg7, arg8, arg9, arg10) {
            arg0.texImage2D(arg1 >>> 0, arg2, arg3, arg4, arg5, arg6, arg7 >>> 0, arg8 >>> 0, arg9 === 0 ? undefined : getArrayU8FromWasm0(arg9, arg10));
        }, arguments); },
        __wbg_texImage2D_68efde4651dd3a10: function() { return handleError(function (arg0, arg1, arg2, arg3, arg4, arg5, arg6) {
            arg0.texImage2D(arg1 >>> 0, arg2, arg3, arg4 >>> 0, arg5 >>> 0, arg6);
        }, arguments); },
        __wbg_texParameteri_0afc5d515132fed2: function(arg0, arg1, arg2, arg3) {
            arg0.texParameteri(arg1 >>> 0, arg2 >>> 0, arg3);
        },
        __wbg_uniform1f_0b36dc15cb2b4991: function(arg0, arg1, arg2) {
            arg0.uniform1f(arg1, arg2);
        },
        __wbg_uniform1fv_19f7e96820e32a42: function(arg0, arg1, arg2, arg3) {
            arg0.uniform1fv(arg1, getArrayF32FromWasm0(arg2, arg3));
        },
        __wbg_uniform1i_385ebc488d4d763f: function(arg0, arg1, arg2) {
            arg0.uniform1i(arg1, arg2);
        },
        __wbg_uniform2f_6bd82d6e7f81060b: function(arg0, arg1, arg2, arg3) {
            arg0.uniform2f(arg1, arg2, arg3);
        },
        __wbg_uniform3f_b9f8bdb406b70135: function(arg0, arg1, arg2, arg3, arg4) {
            arg0.uniform3f(arg1, arg2, arg3, arg4);
        },
        __wbg_uniform3fv_431207a9407b7d29: function(arg0, arg1, arg2, arg3) {
            arg0.uniform3fv(arg1, getArrayF32FromWasm0(arg2, arg3));
        },
        __wbg_uniform4fv_a1415c11925ac651: function(arg0, arg1, arg2, arg3) {
            arg0.uniform4fv(arg1, getArrayF32FromWasm0(arg2, arg3));
        },
        __wbg_uniformMatrix4fv_cd0ba72d1d780759: function(arg0, arg1, arg2, arg3, arg4) {
            arg0.uniformMatrix4fv(arg1, arg2 !== 0, getArrayF32FromWasm0(arg3, arg4));
        },
        __wbg_useProgram_f6341ea2eeea1e5a: function(arg0, arg1) {
            arg0.useProgram(arg1);
        },
        __wbg_vertexAttribPointer_9e0e4517b8d274bd: function(arg0, arg1, arg2, arg3, arg4, arg5, arg6) {
            arg0.vertexAttribPointer(arg1 >>> 0, arg2, arg3 >>> 0, arg4 !== 0, arg5, arg6);
        },
        __wbg_viewport_aaac6dd8cff1831a: function(arg0, arg1, arg2, arg3, arg4) {
            arg0.viewport(arg1, arg2, arg3, arg4);
        },
        __wbindgen_generic_0000000000000001: function(arg0, arg1) {
            // Cast intrinsic for `Ref(Slice(F32)) -> NamedExternref("Float32Array")`.
            const ret = getArrayF32FromWasm0(arg0, arg1);
            return ret;
        },
        __wbindgen_generic_0000000000000002: function(arg0, arg1) {
            // Cast intrinsic for `Ref(Slice(U16)) -> NamedExternref("Uint16Array")`.
            const ret = getArrayU16FromWasm0(arg0, arg1);
            return ret;
        },
        __wbindgen_generic_0000000000000003: function(arg0, arg1) {
            // Cast intrinsic for `Ref(String) -> Externref`.
            const ret = getStringFromWasm0(arg0, arg1);
            return ret;
        },
        __wbindgen_init_externref_table: function() {
            const table = wasm.__wbindgen_externrefs;
            table.set(0, undefined);
            table.set(1024, undefined);
            table.set(1025, null);
            table.set(1026, true);
            table.set(1027, false);
        },
    };
    return {
        __proto__: null,
        "./mind_over_chicken_engine_bg.js": import0,
    };
}

const lAudioContext = (typeof AudioContext !== 'undefined' ? AudioContext : (typeof webkitAudioContext !== 'undefined' ? webkitAudioContext : undefined));
const __wbindgen_enum_BiquadFilterType = ["lowpass", "highpass", "bandpass", "lowshelf", "highshelf", "peaking", "notch", "allpass"];


const __wbindgen_enum_OscillatorType = ["sine", "square", "sawtooth", "triangle", "custom"];
const EngineFinalization = (typeof FinalizationRegistry === 'undefined')
    ? { register: () => {}, unregister: () => {} }
    : new FinalizationRegistry(ptr => wasm.__wbg_engine_free(ptr, 1));
const RustBlinkClassifierFinalization = (typeof FinalizationRegistry === 'undefined')
    ? { register: () => {}, unregister: () => {} }
    : new FinalizationRegistry(ptr => wasm.__wbg_rustblinkclassifier_free(ptr, 1));
const RustImuTrackerFinalization = (typeof FinalizationRegistry === 'undefined')
    ? { register: () => {}, unregister: () => {} }
    : new FinalizationRegistry(ptr => wasm.__wbg_rustimutracker_free(ptr, 1));

function addToExternrefTable0(obj) {
    const idx = wasm.__externref_table_alloc();
    wasm.__wbindgen_externrefs.set(idx, obj);
    return idx;
}

function debugString(val) {
    switch (typeof val) {
        // primitive types
        case 'number':
        case 'boolean':
        case 'undefined':
        return `${val}`;
        case 'string':
        return JSON.stringify(val);
        case 'bigint':
        return `${val}n`;
        case 'symbol':
        return val.description !== undefined
            ? `Symbol(${val.description})`
            : 'Symbol';
        case 'function':
        return typeof val.name === 'string' && val.name !== ''
            ? `Function(${val.name})`
            : 'Function';
        case 'object': {
            if (val === null) {
                return 'null';
            }
            // objects
            if (Array.isArray(val)) {
                return '[' + Array.from(val, debugString).join(', ') + ']';
            }
            // errors
            if (val instanceof Error) {
                return `${val.name}: ${val.message}\n${val.stack}`;
            }

            const toStringOutput = toString.call(val);
            // Test for built-in
            const className = /\[object ([^\]]+)\]/.exec(toStringOutput)?.[1];

            // Failed to match the standard '[object ClassName]'
            if (className === undefined) {
                return toStringOutput;
            }
            if (className === 'Object') {
                // we're a user defined class or Object
                // JSON.stringify avoids problems with cycles, and is generally much
                // easier than looping through ownProperties of `val`.
                try {
                    return 'Object(' + JSON.stringify(val) + ')';
                } catch (_) {
                    return 'Object';
                }
            }
            // TODO we could test for more things here, like `Set`s and `Map`s.
            return className;
        }
        // Per MDN: 'This list of values is exhaustive.'
        // https://developer.mozilla.org/en-US/docs/Web/JavaScript/Reference/Operators/typeof#description
    }
}

function getArrayF32FromWasm0(ptr, len) {
    ptr = ptr >>> 0;
    return getFloat32ArrayMemory0().subarray(ptr / 4, ptr / 4 + len);
}

function getArrayU16FromWasm0(ptr, len) {
    ptr = ptr >>> 0;
    return getUint16ArrayMemory0().subarray(ptr / 2, ptr / 2 + len);
}

function getArrayU8FromWasm0(ptr, len) {
    ptr = ptr >>> 0;
    return getUint8ArrayMemory0().subarray(ptr / 1, ptr / 1 + len);
}

let cachedDataViewMemory0 = null;
function getDataViewMemory0() {
    if (cachedDataViewMemory0 === null || cachedDataViewMemory0.buffer.detached === true || (cachedDataViewMemory0.buffer.detached === undefined && cachedDataViewMemory0.buffer !== wasm.memory.buffer)) {
        cachedDataViewMemory0 = new DataView(wasm.memory.buffer);
    }
    return cachedDataViewMemory0;
}

let cachedFloat32ArrayMemory0 = null;
function getFloat32ArrayMemory0() {
    if (cachedFloat32ArrayMemory0 === null || cachedFloat32ArrayMemory0.byteLength === 0) {
        cachedFloat32ArrayMemory0 = new Float32Array(wasm.memory.buffer);
    }
    return cachedFloat32ArrayMemory0;
}

function getStringFromWasm0(ptr, len) {
    ptr = ptr >>> 0;
    return decodeText(getUint8ArrayMemory0().subarray(ptr, ptr + len));
}

let cachedUint16ArrayMemory0 = null;
function getUint16ArrayMemory0() {
    if (cachedUint16ArrayMemory0 === null || cachedUint16ArrayMemory0.byteLength === 0) {
        cachedUint16ArrayMemory0 = new Uint16Array(wasm.memory.buffer);
    }
    return cachedUint16ArrayMemory0;
}

let cachedUint8ArrayMemory0 = null;
function getUint8ArrayMemory0() {
    if (cachedUint8ArrayMemory0 === null || cachedUint8ArrayMemory0.byteLength === 0) {
        cachedUint8ArrayMemory0 = new Uint8Array(wasm.memory.buffer);
    }
    return cachedUint8ArrayMemory0;
}

function handleError(f, args) {
    try {
        return f.apply(this, args);
    } catch (e) {
        const idx = addToExternrefTable0(e);
        wasm.__wbindgen_exn_store(idx);
    }
}

function isLikeNone(x) {
    return x === undefined || x === null;
}

function passArrayF32ToWasm0(arg, malloc) {
    const ptr = malloc(arg.length * 4, 4) >>> 0;
    getFloat32ArrayMemory0().set(arg, ptr / 4);
    WASM_VECTOR_LEN = arg.length;
    return ptr;
}

function passStringToWasm0(arg, malloc, realloc) {
    if (realloc === undefined) {
        const buf = cachedTextEncoder.encode(arg);
        const ptr = malloc(buf.length, 1) >>> 0;
        getUint8ArrayMemory0().subarray(ptr, ptr + buf.length).set(buf);
        WASM_VECTOR_LEN = buf.length;
        return ptr;
    }

    let len = arg.length;
    let ptr = malloc(len, 1) >>> 0;

    const mem = getUint8ArrayMemory0();

    let offset = 0;

    for (; offset < len; offset++) {
        const code = arg.charCodeAt(offset);
        if (code > 0x7F) break;
        mem[ptr + offset] = code;
    }
    if (offset !== len) {
        if (offset !== 0) {
            arg = arg.slice(offset);
        }
        ptr = realloc(ptr, len, len = offset + arg.length * 3, 1) >>> 0;
        const view = getUint8ArrayMemory0().subarray(ptr + offset, ptr + len);
        const ret = cachedTextEncoder.encodeInto(arg, view);

        offset += ret.written;
        ptr = realloc(ptr, len, offset, 1) >>> 0;
    }

    WASM_VECTOR_LEN = offset;
    return ptr;
}

function takeFromExternrefTable0(idx) {
    const value = wasm.__wbindgen_externrefs.get(idx);
    wasm.__externref_table_dealloc(idx);
    return value;
}

let cachedTextDecoder = new TextDecoder('utf-8', { ignoreBOM: true });
cachedTextDecoder.decode();
const MAX_SAFARI_DECODE_BYTES = 2146435072;
let numBytesDecoded = 0;
function decodeText(bytes) {
    numBytesDecoded += bytes.length;
    if (numBytesDecoded >= MAX_SAFARI_DECODE_BYTES) {
        cachedTextDecoder = new TextDecoder('utf-8', { ignoreBOM: true });
        cachedTextDecoder.decode();
        numBytesDecoded = bytes.length;
    }
    return cachedTextDecoder.decode(bytes);
}

const cachedTextEncoder = new TextEncoder();

if (!('encodeInto' in cachedTextEncoder)) {
    cachedTextEncoder.encodeInto = function (arg, view) {
        const buf = cachedTextEncoder.encode(arg);
        view.set(buf);
        return {
            read: arg.length,
            written: buf.length
        };
    };
}

let WASM_VECTOR_LEN = 0;

let wasmModule, wasmInstance, wasm;
function __wbg_finalize_init(instance, module) {
    wasmInstance = instance;
    wasm = instance.exports;
    wasmModule = module;
    cachedDataViewMemory0 = null;
    cachedFloat32ArrayMemory0 = null;
    cachedUint16ArrayMemory0 = null;
    cachedUint8ArrayMemory0 = null;
    wasm.__wbindgen_start();
    return wasm;
}

async function __wbg_load(module, imports) {
    if (typeof Response === 'function' && module instanceof Response) {
        if (!module.ok) {
            throw new Error(`failed to fetch Wasm: ${module.status} ${module.statusText} fetching '${module.url}'`);
        }

        if (typeof WebAssembly.instantiateStreaming === 'function') {
            try {
                return await WebAssembly.instantiateStreaming(module, imports);
            } catch (e) {
                const validResponse = expectedResponseType(module.type);

                if (validResponse && module.headers.get('Content-Type') !== 'application/wasm') {
                    console.warn("`WebAssembly.instantiateStreaming` failed because your server does not serve Wasm with `application/wasm` MIME type. Falling back to `WebAssembly.instantiate` which is slower. Original error:\n", e);

                } else { throw e; }
            }
        }

        const bytes = await module.arrayBuffer();
        return await WebAssembly.instantiate(bytes, imports);
    } else {
        const instance = await WebAssembly.instantiate(module, imports);

        if (instance instanceof WebAssembly.Instance) {
            return { instance, module };
        } else {
            return instance;
        }
    }

    function expectedResponseType(type) {
        switch (type) {
            case 'basic': case 'cors': case 'default': return true;
        }
        return false;
    }
}

function initSync(module) {
    if (wasm !== undefined) return wasm;


    if (module !== undefined) {
        if (Object.getPrototypeOf(module) === Object.prototype) {
            ({module} = module)
        } else {
            console.warn('using deprecated parameters for `initSync()`; pass a single object instead')
        }
    }

    const imports = __wbg_get_imports();
    if (!(module instanceof WebAssembly.Module)) {
        module = new WebAssembly.Module(module);
    }
    const instance = new WebAssembly.Instance(module, imports);
    return __wbg_finalize_init(instance, module);
}

let __wbg_init_promise;
async function __wbg_init(module_or_path) {
    if (wasm !== undefined) return wasm;
    if (__wbg_init_promise === undefined) {
        __wbg_init_promise = __wbg_init_once(module_or_path);
        __wbg_init_promise.catch(() => { __wbg_init_promise = undefined; });
    }
    return __wbg_init_promise;
}

async function __wbg_init_once(module_or_path) {

    if (module_or_path !== undefined) {
        if (Object.getPrototypeOf(module_or_path) === Object.prototype) {
            ({module_or_path} = module_or_path)
        } else {
            console.warn('using deprecated parameters for the initialization function; pass a single object instead')
        }
    }

    if (module_or_path === undefined) {
        module_or_path = new URL('mind_over_chicken_engine_bg.wasm', import.meta.url);
    }
    const imports = __wbg_get_imports();

    if (typeof module_or_path === 'string' || (typeof Request === 'function' && module_or_path instanceof Request) || (typeof URL === 'function' && module_or_path instanceof URL)) {
        module_or_path = fetch(module_or_path);
    }

    const { instance, module } = await __wbg_load(await module_or_path, imports);

    return __wbg_finalize_init(instance, module);
}

export { initSync, __wbg_init as default };
