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
     * @param {number} radius
     * @param {number} intensity
     */
    add_light(x, y, radius, intensity) {
        wasm.engine_add_light(this.__wbg_ptr, x, y, radius, intensity);
    }
    begin_frame() {
        wasm.engine_begin_frame(this.__wbg_ptr);
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
    /**
     * @param {number} seed
     */
    init_world(seed) {
        wasm.engine_init_world(this.__wbg_ptr, seed);
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
     * Render the uploaded canvas with this frame's lights, then clear them so the next
     * frame starts fresh (lights are added per frame with `add_light`).
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
     * @param {number} era
     */
    set_era(era) {
        wasm.engine_set_era(this.__wbg_ptr, era);
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
    last_probability() {
        const ret = wasm.rustblinkclassifier_last_probability(this.__wbg_ptr);
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
     * @returns {boolean}
     */
    record_baseline_button() {
        const ret = wasm.rustblinkclassifier_record_baseline_button(this.__wbg_ptr);
        return ret !== 0;
    }
    /**
     * Starts or commits a blink calibration capture.
     * If a prominent blink spike already occurred in the last ~1.0s (user blinked then clicked),
     * it commits immediately; otherwise it stays armed for 1.5s so the user can blink right after clicking.
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
    /**
     * @returns {number}
     */
    calib_progress() {
        const ret = wasm.rustimutracker_calib_progress(this.__wbg_ptr);
        return ret;
    }
    /**
     * @returns {boolean}
     */
    is_tared() {
        const ret = wasm.rustimutracker_is_tared(this.__wbg_ptr);
        return ret !== 0;
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
        __wbg_attachShader_817dcec9e79fd18f: function(arg0, arg1, arg2) {
            arg0.attachShader(arg1, arg2);
        },
        __wbg_bindBuffer_b3bb001fdbee089a: function(arg0, arg1, arg2) {
            arg0.bindBuffer(arg1 >>> 0, arg2);
        },
        __wbg_bindTexture_6f822943b2790466: function(arg0, arg1, arg2) {
            arg0.bindTexture(arg1 >>> 0, arg2);
        },
        __wbg_bufferData_2fb8740d60478bf0: function(arg0, arg1, arg2, arg3) {
            arg0.bufferData(arg1 >>> 0, arg2, arg3 >>> 0);
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
        __wbg_createBuffer_ca6ca40e9fb13678: function(arg0) {
            const ret = arg0.createBuffer();
            return isLikeNone(ret) ? 0 : addToExternrefTable0(ret);
        },
        __wbg_createProgram_de0881c688a8e43b: function(arg0) {
            const ret = arg0.createProgram();
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
        __wbg_document_3b5c3071b43e97ee: function(arg0) {
            const ret = arg0.document;
            return isLikeNone(ret) ? 0 : addToExternrefTable0(ret);
        },
        __wbg_drawArrays_fe5a51410bfabb5b: function(arg0, arg1, arg2, arg3) {
            arg0.drawArrays(arg1 >>> 0, arg2, arg3);
        },
        __wbg_enableVertexAttribArray_d485706ade6bebf8: function(arg0, arg1) {
            arg0.enableVertexAttribArray(arg1 >>> 0);
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
        __wbg_linkProgram_dee7cda98bacb12f: function(arg0, arg1) {
            arg0.linkProgram(arg1);
        },
        __wbg_new_bb74001155cb8ca8: function() {
            const ret = new Error();
            return ret;
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
        __wbg_uniform1i_385ebc488d4d763f: function(arg0, arg1, arg2) {
            arg0.uniform1i(arg1, arg2);
        },
        __wbg_uniform2f_6bd82d6e7f81060b: function(arg0, arg1, arg2, arg3) {
            arg0.uniform2f(arg1, arg2, arg3);
        },
        __wbg_uniform4fv_a1415c11925ac651: function(arg0, arg1, arg2, arg3) {
            arg0.uniform4fv(arg1, getArrayF32FromWasm0(arg2, arg3));
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
