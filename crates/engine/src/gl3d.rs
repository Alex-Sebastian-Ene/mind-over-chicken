use crate::camera3d::OrbitCamera;
use crate::math3d::{Frustum, Vec3};
use crate::mesh3d::{MeshChunk, TerrainMesh};
use web_sys::{
    WebGlBuffer, WebGlFramebuffer, WebGlProgram, WebGlRenderbuffer, WebGlRenderingContext,
    WebGlShader, WebGlTexture, WebGlUniformLocation,
};

pub struct Light3D {
    pub pos: Vec3,
    pub radius: f32,
    pub intensity: f32,
}

#[repr(C)]
#[derive(Clone, Copy, Debug)]
pub struct BillboardVertex {
    pub center: [f32; 3],
    pub offset: [f32; 3],
    pub uv: [f32; 2],
}

pub struct WebGl3DRenderer {
    pub gl: WebGlRenderingContext,
    pub terrain_program: WebGlProgram,
    pub water_program: WebGlProgram,
    pub billboard_program: WebGlProgram,

    // Terrain buffers
    pub terrain_vbo: WebGlBuffer,
    pub terrain_ibo: WebGlBuffer,
    pub terrain_index_count: i32,
    /// Independently drawn and culled pieces of the terrain (u16 indices each, #129).
    pub terrain_chunks: Vec<MeshChunk>,
    /// The world the mesh spans: the period of the wrap copies (a torus, #111).
    pub world_w: f32,
    pub world_h: f32,
    pub water_min: [f32; 3],
    pub water_max: [f32; 3],
    /// Size of the terrain texture now on the GPU, so per-frame uploads can reuse it.
    pub terrain_tex_size: (u32, u32),

    // Water buffers
    pub water_vbo: WebGlBuffer,
    pub water_ibo: WebGlBuffer,
    pub water_index_count: i32,

    // Billboard buffer
    pub billboard_vbo: WebGlBuffer,

    // Low-res offscreen render target (320x180) for retro pixel-art aesthetic
    pub fbo: WebGlFramebuffer,
    pub fbo_texture: WebGlTexture,
    pub _fbo_depth_rb: WebGlRenderbuffer,

    // Textures
    pub terrain_texture: WebGlTexture,
    pub atlas_texture: WebGlTexture,

    // Terrain uniforms
    pub u_terrain_vp: WebGlUniformLocation,
    pub u_terrain_sun_dir: WebGlUniformLocation,
    pub u_terrain_ambient: Option<WebGlUniformLocation>,
    pub u_terrain_night: WebGlUniformLocation,
    pub u_terrain_time: Option<WebGlUniformLocation>,
    pub u_terrain_cam_pos: Option<WebGlUniformLocation>,
    pub u_terrain_light_count: WebGlUniformLocation,
    pub u_terrain_light_pos: WebGlUniformLocation,
    pub u_terrain_light_rad: WebGlUniformLocation,
    pub u_terrain_light_int: WebGlUniformLocation,

    // Water uniforms
    pub u_water_vp: WebGlUniformLocation,
    pub u_water_sun_dir: WebGlUniformLocation,
    pub u_water_night: WebGlUniformLocation,
    pub u_water_time: WebGlUniformLocation,
    pub u_water_cam_pos: Option<WebGlUniformLocation>,
    pub u_water_light_count: WebGlUniformLocation,
    pub u_water_light_pos: WebGlUniformLocation,
    pub u_water_light_rad: WebGlUniformLocation,
    pub u_water_light_int: WebGlUniformLocation,

    // Billboard uniforms
    pub u_billboard_vp: WebGlUniformLocation,
    pub u_billboard_cam_right: WebGlUniformLocation,
    pub u_billboard_cam_up: WebGlUniformLocation,
    pub u_billboard_ground_forward: WebGlUniformLocation,
    pub u_billboard_night: WebGlUniformLocation,
    pub u_billboard_sun_dir: Option<WebGlUniformLocation>,
    pub u_billboard_light_count: Option<WebGlUniformLocation>,
    pub u_billboard_light_pos: Option<WebGlUniformLocation>,
    pub u_billboard_light_rad: Option<WebGlUniformLocation>,
    pub u_billboard_light_int: Option<WebGlUniformLocation>,
    pub u_terrain_offset_x: WebGlUniformLocation,
    pub u_terrain_offset_z: WebGlUniformLocation,
    pub u_water_offset_x: WebGlUniformLocation,
    pub u_water_offset_z: WebGlUniformLocation,
    pub u_billboard_offset_x: WebGlUniformLocation,
    pub u_billboard_offset_z: WebGlUniformLocation,

    /// Clear colour around the map (rgb 0..1): the world's deep sea, so the view's edges read
    /// as open ocean rather than a void. Set from JS with `Engine::set_backdrop`.
    pub backdrop: [f32; 3],
}

const TERRAIN_VS: &str = r#"
attribute vec3 a_position;
attribute vec2 a_texCoord;
attribute vec3 a_normal;

uniform mat4 u_viewProjection;
uniform float u_offsetX; // east-west copy of the map (the planet wraps)
uniform float u_offsetZ; // north-south copy for torus world (#111)

varying vec2 v_texCoord;
varying vec3 v_normal;
varying vec3 v_worldPos;

void main() {
    vec3 wpos = a_position + vec3(u_offsetX, 0.0, u_offsetZ);
    v_worldPos = wpos;
    v_normal = a_normal;
    v_texCoord = a_texCoord;
    gl_Position = u_viewProjection * vec4(wpos, 1.0);
}
"#;

const TERRAIN_FS: &str = r#"
precision mediump float;

varying vec2 v_texCoord;
varying vec3 v_normal;
varying vec3 v_worldPos;

uniform sampler2D u_texture;
uniform vec3 u_sunDir;
uniform vec3 u_camPos;
uniform float u_ambient;
uniform float u_night;
uniform int u_lightCount;
uniform vec3 u_lightPos[16];
uniform float u_lightRad[16];
uniform float u_lightInt[16];

void main() {
    vec4 albedo = texture2D(u_texture, v_texCoord);
    vec3 norm = normalize(v_normal);
    vec3 sun = normalize(u_sunDir);

    // Dynamic sun elevation determines warm golden twilight vs noon sun
    float sunElevation = clamp(sun.y * 1.5, 0.0, 1.0);
    vec3 sunColor = mix(vec3(1.0, 0.58, 0.28), vec3(1.0, 0.96, 0.88), sunElevation);

    // Directional diffuse lighting with quantized half-Lambert wrap
    float nDotL = max(0.0, dot(norm, sun));
    float directLight = floor(nDotL * 5.0 + 0.5) / 5.0;
    float sunVisible = max(0.0, sun.y) * (1.0 - u_night);

    // Ambient occlusion based on surface slope / upward normal
    float ao = clamp(norm.y * 0.70 + 0.30, 0.40, 1.0);

    // Day sky ambient vs moonlit night
    vec3 dayAmbient = vec3(0.18, 0.26, 0.38) * ao;
    vec3 nightAmbient = vec3(0.04, 0.07, 0.18) * ao;
    vec3 ambientCol = mix(dayAmbient, nightAmbient, u_night);

    // Moon directional highlight at night
    vec3 moonDir = normalize(vec3(-sun.x, 0.75, -sun.z));
    float moonLight = max(0.0, dot(norm, moonDir)) * u_night * 0.35;
    vec3 moonCol = vec3(0.15, 0.22, 0.45) * moonLight;

    vec3 lightAccum = ambientCol + sunColor * (directLight * sunVisible) + moonCol;

    // Dynamic 3D point lights (fires, torches, lightning)
    for (int i = 0; i < 16; i++) {
        if (i >= u_lightCount) break;
        float d = distance(v_worldPos, u_lightPos[i]);
        if (d < u_lightRad[i]) {
            float atten = clamp(1.0 - d / u_lightRad[i], 0.0, 1.0);
            atten = atten * atten * u_lightInt[i];
            lightAccum += vec3(1.0, 0.68, 0.28) * atten;
        }
    }

    // Atmospheric horizon haze
    float camDist = distance(v_worldPos, u_camPos);
    float haze = clamp((camDist - 240.0) / 420.0, 0.0, 0.50);
    vec3 hazeCol = mix(vec3(0.09, 0.17, 0.36), vec3(0.03, 0.05, 0.12), u_night);

    vec3 finalColor = mix(albedo.rgb * lightAccum, hazeCol, haze);
    gl_FragColor = vec4(clamp(finalColor, 0.0, 1.0), albedo.a);
}
"#;

const WATER_VS: &str = r#"
attribute vec3 a_position;
attribute vec2 a_texCoord;
attribute vec3 a_normal;

uniform mat4 u_viewProjection;
uniform mediump float u_time; // must match WATER_FS precision or the program fails to link
uniform float u_offsetX;
uniform float u_offsetZ;

varying vec2 v_texCoord;
varying vec3 v_worldPos;

void main() {
    vec3 pos = a_position;
    // Two-octave animated 3D wave ripples
    float w1 = sin(pos.x * 0.14 + u_time * 2.4) * cos(pos.z * 0.14 + u_time * 1.9) * 0.36;
    float w2 = sin(pos.x * 0.28 - u_time * 3.1) * cos(pos.z * 0.24 + u_time * 2.7) * 0.14;
    pos.y += w1 + w2;

    vec3 wpos = pos + vec3(u_offsetX, 0.0, u_offsetZ);
    v_worldPos = wpos;
    v_texCoord = a_texCoord;
    gl_Position = u_viewProjection * vec4(wpos, 1.0);
}
"#;

const WATER_FS: &str = r#"
precision mediump float;

varying vec2 v_texCoord;
varying vec3 v_worldPos;

uniform vec3 u_sunDir;
uniform vec3 u_camPos;
uniform float u_night;
uniform float u_time;
uniform int u_lightCount;
uniform vec3 u_lightPos[16];
uniform float u_lightRad[16];
uniform float u_lightInt[16];

void main() {
    // Rich ocean base color: turquoise coast to deep oceanic navy
    vec3 dayWaterDeep = vec3(0.05, 0.14, 0.32);
    vec3 dayWaterShallow = vec3(0.12, 0.42, 0.58);
    vec3 nightWater = vec3(0.02, 0.05, 0.14);

    // Depth / latitude variation
    float depthGrad = clamp(abs(v_worldPos.z - 90.0) / 90.0, 0.0, 1.0);
    vec3 baseWater = mix(dayWaterShallow, dayWaterDeep, depthGrad * 0.8 + 0.2);
    vec3 col = mix(baseWater, nightWater, u_night);

    // Analytical wave normal from wave derivatives
    float dw_dx = 0.14 * cos(v_worldPos.x * 0.14 + u_time * 2.4) * cos(v_worldPos.z * 0.14 + u_time * 1.9) * 0.36
                + 0.28 * cos(v_worldPos.x * 0.28 - u_time * 3.1) * cos(v_worldPos.z * 0.24 + u_time * 2.7) * 0.14;
    float dw_dz = -0.14 * sin(v_worldPos.x * 0.14 + u_time * 2.4) * sin(v_worldPos.z * 0.14 + u_time * 1.9) * 0.36
                - 0.24 * sin(v_worldPos.x * 0.28 - u_time * 3.1) * sin(v_worldPos.z * 0.24 + u_time * 2.7) * 0.14;
    vec3 norm = normalize(vec3(-dw_dx, 1.0, -dw_dz));

    // Specular sunlight and moonlight glint (Fresnel / Blinn-Phong)
    vec3 viewDir = normalize(u_camPos - v_worldPos);
    vec3 sunDir = normalize(u_sunDir);
    vec3 halfDir = normalize(sunDir + viewDir);
    float nDotH = max(0.0, dot(norm, halfDir));
    float spec = pow(nDotH, 28.0);
    float glint = floor(spec * 4.0 + 0.5) / 4.0;
    float sunVis = max(0.0, sunDir.y) * (1.0 - u_night);
    vec3 glintColor = mix(vec3(1.0, 0.85, 0.55), vec3(0.5, 0.7, 1.0), u_night);
    col += glintColor * (glint * (sunVis + u_night * 0.3));

    // (No blinking foam cells: they read as random arrays of white pixels on the sea. Alex.)

    // Dynamic point lights reflection on water
    for (int i = 0; i < 16; i++) {
        if (i >= u_lightCount) break;
        float d = distance(v_worldPos, u_lightPos[i]);
        if (d < u_lightRad[i]) {
            float atten = clamp(1.0 - d / u_lightRad[i], 0.0, 1.0);
            atten = atten * atten * u_lightInt[i];
            col += vec3(1.0, 0.68, 0.28) * atten * 1.2;
        }
    }

    // Atmospheric horizon haze for water
    float camDist = distance(v_worldPos, u_camPos);
    float haze = clamp((camDist - 240.0) / 420.0, 0.0, 0.55);
    vec3 hazeCol = mix(vec3(0.09, 0.17, 0.36), vec3(0.03, 0.05, 0.12), u_night);
    col = mix(col, hazeCol, haze);

    gl_FragColor = vec4(clamp(col, 0.0, 1.0), 0.72);
}
"#;

const BILLBOARD_VS: &str = r#"
attribute vec3 a_center;
attribute vec3 a_offset;
attribute vec2 a_texCoord;

uniform mat4 u_viewProjection;
uniform vec3 u_camRight;
uniform vec3 u_camUp;
uniform vec3 u_groundForward;
uniform float u_offsetX;
uniform float u_offsetZ;

varying vec2 v_texCoord;
varying vec3 v_worldPos;

void main() {
    v_texCoord = a_texCoord;
    // Diorama cylindrical tilt: blend world-up with camera-up so sprites face the player comfortably
    vec3 spriteUp = normalize(mix(vec3(0.0, 1.0, 0.0), u_camUp, 0.68));

    // a_offset.x is along camRight
    // a_offset.y is along spriteUp (height)
    // a_offset.z is along groundForward (ground contact shadow)
    vec3 worldPos = a_center + vec3(u_offsetX, 0.0, u_offsetZ)
                  + u_camRight * (a_offset.x * 1.25)
                  + spriteUp * (a_offset.y * 1.25)
                  + u_groundForward * (a_offset.z * 1.25);

    v_worldPos = worldPos;
    gl_Position = u_viewProjection * vec4(worldPos, 1.0);
    // Depth bias: pull billboard slightly forward in clip space so it never clips into ground triangles
    gl_Position.z -= 0.003 * gl_Position.w;
}
"#;

const BILLBOARD_FS: &str = r#"
precision mediump float;

varying vec2 v_texCoord;
varying vec3 v_worldPos;

uniform sampler2D u_texture;
uniform vec3 u_sunDir;
uniform float u_night;
uniform int u_lightCount;
uniform vec3 u_lightPos[16];
uniform float u_lightRad[16];
uniform float u_lightInt[16];

void main() {
    vec4 c = texture2D(u_texture, v_texCoord);
    if (c.a < 0.02) discard;

    // Sprite day vs night lighting
    vec3 nightTint = vec3(0.20, 0.28, 0.50);
    float sunVis = max(0.0, normalize(u_sunDir).y) * (1.0 - u_night);
    vec3 ambient = mix(vec3(0.75 + 0.25 * sunVis), nightTint, u_night * 0.85);

    // Point lights illuminate sprites
    vec3 lightAdd = vec3(0.0);
    for (int i = 0; i < 16; i++) {
        if (i >= u_lightCount) break;
        float d = distance(v_worldPos, u_lightPos[i]);
        if (d < u_lightRad[i]) {
            float atten = clamp(1.0 - d / u_lightRad[i], 0.0, 1.0);
            atten = atten * atten * u_lightInt[i];
            lightAdd += vec3(1.0, 0.70, 0.30) * atten * 1.3;
        }
    }

    vec3 lit = clamp(c.rgb * ambient + lightAdd, 0.0, 1.0);
    gl_FragColor = vec4(lit, c.a);
}
"#;

impl WebGl3DRenderer {
    pub fn new(gl: WebGlRenderingContext) -> Result<Self, String> {
        let t_vert = compile_shader(&gl, WebGlRenderingContext::VERTEX_SHADER, TERRAIN_VS)?;
        let t_frag = compile_shader(&gl, WebGlRenderingContext::FRAGMENT_SHADER, TERRAIN_FS)?;
        let terrain_program = link_program(&gl, &t_vert, &t_frag)?;

        let w_vert = compile_shader(&gl, WebGlRenderingContext::VERTEX_SHADER, WATER_VS)?;
        let w_frag = compile_shader(&gl, WebGlRenderingContext::FRAGMENT_SHADER, WATER_FS)?;
        let water_program = link_program(&gl, &w_vert, &w_frag)?;

        let b_vert = compile_shader(&gl, WebGlRenderingContext::VERTEX_SHADER, BILLBOARD_VS)?;
        let b_frag = compile_shader(&gl, WebGlRenderingContext::FRAGMENT_SHADER, BILLBOARD_FS)?;
        let billboard_program = link_program(&gl, &b_vert, &b_frag)?;

        let terrain_vbo = gl.create_buffer().ok_or("Failed to create terrain vbo")?;
        let terrain_ibo = gl.create_buffer().ok_or("Failed to create terrain ibo")?;
        let water_vbo = gl.create_buffer().ok_or("Failed to create water vbo")?;
        let water_ibo = gl.create_buffer().ok_or("Failed to create water ibo")?;
        let billboard_vbo = gl.create_buffer().ok_or("Failed to create billboard vbo")?;

        // Offscreen Framebuffer (320x180) for low-res pixel art pass
        let fbo = gl.create_framebuffer().ok_or("Failed to create framebuffer")?;
        let fbo_texture = gl.create_texture().ok_or("Failed to create fbo texture")?;
        gl.bind_texture(WebGlRenderingContext::TEXTURE_2D, Some(&fbo_texture));
        gl.tex_image_2d_with_i32_and_i32_and_i32_and_format_and_type_and_opt_u8_array(
            WebGlRenderingContext::TEXTURE_2D,
            0,
            WebGlRenderingContext::RGBA as i32,
            320,
            180,
            0,
            WebGlRenderingContext::RGBA,
            WebGlRenderingContext::UNSIGNED_BYTE,
            None,
        )
        .map_err(|e| format!("{:?}", e))?;
        gl.tex_parameteri(
            WebGlRenderingContext::TEXTURE_2D,
            WebGlRenderingContext::TEXTURE_MIN_FILTER,
            WebGlRenderingContext::NEAREST as i32,
        );
        gl.tex_parameteri(
            WebGlRenderingContext::TEXTURE_2D,
            WebGlRenderingContext::TEXTURE_MAG_FILTER,
            WebGlRenderingContext::NEAREST as i32,
        );
        gl.tex_parameteri(
            WebGlRenderingContext::TEXTURE_2D,
            WebGlRenderingContext::TEXTURE_WRAP_S,
            WebGlRenderingContext::CLAMP_TO_EDGE as i32,
        );
        gl.tex_parameteri(
            WebGlRenderingContext::TEXTURE_2D,
            WebGlRenderingContext::TEXTURE_WRAP_T,
            WebGlRenderingContext::CLAMP_TO_EDGE as i32,
        );

        let fbo_depth_rb = gl
            .create_renderbuffer()
            .ok_or("Failed to create depth renderbuffer")?;
        gl.bind_renderbuffer(WebGlRenderingContext::RENDERBUFFER, Some(&fbo_depth_rb));
        gl.renderbuffer_storage(
            WebGlRenderingContext::RENDERBUFFER,
            WebGlRenderingContext::DEPTH_COMPONENT16,
            320,
            180,
        );

        gl.bind_framebuffer(WebGlRenderingContext::FRAMEBUFFER, Some(&fbo));
        gl.framebuffer_texture_2d(
            WebGlRenderingContext::FRAMEBUFFER,
            WebGlRenderingContext::COLOR_ATTACHMENT0,
            WebGlRenderingContext::TEXTURE_2D,
            Some(&fbo_texture),
            0,
        );
        gl.framebuffer_renderbuffer(
            WebGlRenderingContext::FRAMEBUFFER,
            WebGlRenderingContext::DEPTH_ATTACHMENT,
            WebGlRenderingContext::RENDERBUFFER,
            Some(&fbo_depth_rb),
        );
        gl.bind_framebuffer(WebGlRenderingContext::FRAMEBUFFER, None);

        // Terrain texture
        let terrain_texture = gl.create_texture().ok_or("Failed to create terrain texture")?;
        gl.bind_texture(WebGlRenderingContext::TEXTURE_2D, Some(&terrain_texture));
        gl.tex_parameteri(
            WebGlRenderingContext::TEXTURE_2D,
            WebGlRenderingContext::TEXTURE_MIN_FILTER,
            WebGlRenderingContext::NEAREST as i32,
        );
        gl.tex_parameteri(
            WebGlRenderingContext::TEXTURE_2D,
            WebGlRenderingContext::TEXTURE_MAG_FILTER,
            WebGlRenderingContext::NEAREST as i32,
        );
        gl.tex_parameteri(
            WebGlRenderingContext::TEXTURE_2D,
            WebGlRenderingContext::TEXTURE_WRAP_S,
            WebGlRenderingContext::CLAMP_TO_EDGE as i32,
        );
        gl.tex_parameteri(
            WebGlRenderingContext::TEXTURE_2D,
            WebGlRenderingContext::TEXTURE_WRAP_T,
            WebGlRenderingContext::CLAMP_TO_EDGE as i32,
        );

        // Atlas texture for billboards
        let atlas_texture = gl.create_texture().ok_or("Failed to create atlas texture")?;
        gl.bind_texture(WebGlRenderingContext::TEXTURE_2D, Some(&atlas_texture));
        gl.tex_parameteri(
            WebGlRenderingContext::TEXTURE_2D,
            WebGlRenderingContext::TEXTURE_MIN_FILTER,
            WebGlRenderingContext::NEAREST as i32,
        );
        gl.tex_parameteri(
            WebGlRenderingContext::TEXTURE_2D,
            WebGlRenderingContext::TEXTURE_MAG_FILTER,
            WebGlRenderingContext::NEAREST as i32,
        );
        gl.tex_parameteri(
            WebGlRenderingContext::TEXTURE_2D,
            WebGlRenderingContext::TEXTURE_WRAP_S,
            WebGlRenderingContext::CLAMP_TO_EDGE as i32,
        );
        gl.tex_parameteri(
            WebGlRenderingContext::TEXTURE_2D,
            WebGlRenderingContext::TEXTURE_WRAP_T,
            WebGlRenderingContext::CLAMP_TO_EDGE as i32,
        );

        // Terrain uniform locations
        let u_terrain_vp = gl
            .get_uniform_location(&terrain_program, "u_viewProjection")
            .ok_or("Missing u_viewProjection in terrain shader")?;
        let u_terrain_sun_dir = gl
            .get_uniform_location(&terrain_program, "u_sunDir")
            .ok_or("Missing u_sunDir in terrain shader")?;
        let u_terrain_ambient = gl
            // Optional: the redesigned terrain shader lights from sun/moon colour and AO, so the
            // compiler strips u_ambient. A required lookup made 3D silently fall back to 2D.
            .get_uniform_location(&terrain_program, "u_ambient");
        let u_terrain_night = gl
            .get_uniform_location(&terrain_program, "u_night")
            .ok_or("Missing u_night in terrain shader")?;
        // Optional uniforms
        let u_terrain_time = gl.get_uniform_location(&terrain_program, "u_time");
        let u_terrain_cam_pos = gl.get_uniform_location(&terrain_program, "u_camPos");
        let u_terrain_light_count = gl
            .get_uniform_location(&terrain_program, "u_lightCount")
            .ok_or("Missing u_lightCount in terrain shader")?;
        let u_terrain_light_pos = gl
            .get_uniform_location(&terrain_program, "u_lightPos")
            .ok_or("Missing u_lightPos in terrain shader")?;
        let u_terrain_light_rad = gl
            .get_uniform_location(&terrain_program, "u_lightRad")
            .ok_or("Missing u_lightRad in terrain shader")?;
        let u_terrain_light_int = gl
            .get_uniform_location(&terrain_program, "u_lightInt")
            .ok_or("Missing u_lightInt in terrain shader")?;

        // Water uniform locations
        let u_water_vp = gl
            .get_uniform_location(&water_program, "u_viewProjection")
            .ok_or("Missing u_viewProjection in water shader")?;
        let u_water_sun_dir = gl
            .get_uniform_location(&water_program, "u_sunDir")
            .ok_or("Missing u_sunDir in water shader")?;
        let u_water_night = gl
            .get_uniform_location(&water_program, "u_night")
            .ok_or("Missing u_night in water shader")?;
        let u_water_time = gl
            .get_uniform_location(&water_program, "u_time")
            .ok_or("Missing u_time in water shader")?;
        let u_water_cam_pos = gl.get_uniform_location(&water_program, "u_camPos");
        let u_water_light_count = gl
            .get_uniform_location(&water_program, "u_lightCount")
            .ok_or("Missing u_lightCount in water shader")?;
        let u_water_light_pos = gl
            .get_uniform_location(&water_program, "u_lightPos")
            .ok_or("Missing u_lightPos in water shader")?;
        let u_water_light_rad = gl
            .get_uniform_location(&water_program, "u_lightRad")
            .ok_or("Missing u_lightRad in water shader")?;
        let u_water_light_int = gl
            .get_uniform_location(&water_program, "u_lightInt")
            .ok_or("Missing u_lightInt in water shader")?;

        // Billboard uniform locations
        let u_billboard_vp = gl
            .get_uniform_location(&billboard_program, "u_viewProjection")
            .ok_or("Missing u_viewProjection in billboard shader")?;
        let u_billboard_cam_right = gl
            .get_uniform_location(&billboard_program, "u_camRight")
            .ok_or("Missing u_camRight in billboard shader")?;
        let u_billboard_cam_up = gl
            .get_uniform_location(&billboard_program, "u_camUp")
            .ok_or("Missing u_camUp in billboard shader")?;
        let u_billboard_ground_forward = gl
            .get_uniform_location(&billboard_program, "u_groundForward")
            .ok_or("Missing u_groundForward in billboard shader")?;
        let u_billboard_night = gl
            .get_uniform_location(&billboard_program, "u_night")
            .ok_or("Missing u_night in billboard shader")?;
        let u_billboard_sun_dir = gl.get_uniform_location(&billboard_program, "u_sunDir");
        let u_billboard_light_count = gl.get_uniform_location(&billboard_program, "u_lightCount");
        let u_billboard_light_pos = gl.get_uniform_location(&billboard_program, "u_lightPos");
        let u_billboard_light_rad = gl.get_uniform_location(&billboard_program, "u_lightRad");
        let u_billboard_light_int = gl.get_uniform_location(&billboard_program, "u_lightInt");
        let u_terrain_offset_x = gl
            .get_uniform_location(&terrain_program, "u_offsetX")
            .ok_or("Missing u_offsetX in terrain shader")?;
        let u_terrain_offset_z = gl
            .get_uniform_location(&terrain_program, "u_offsetZ")
            .ok_or("Missing u_offsetZ in terrain shader")?;
        let u_water_offset_x = gl
            .get_uniform_location(&water_program, "u_offsetX")
            .ok_or("Missing u_offsetX in water shader")?;
        let u_water_offset_z = gl
            .get_uniform_location(&water_program, "u_offsetZ")
            .ok_or("Missing u_offsetZ in water shader")?;
        let u_billboard_offset_x = gl
            .get_uniform_location(&billboard_program, "u_offsetX")
            .ok_or("Missing u_offsetX in billboard shader")?;
        let u_billboard_offset_z = gl
            .get_uniform_location(&billboard_program, "u_offsetZ")
            .ok_or("Missing u_offsetZ in billboard shader")?;

        Ok(Self {
            gl,
            terrain_program,
            water_program,
            billboard_program,
            terrain_vbo,
            terrain_ibo,
            terrain_index_count: 0,
            terrain_chunks: Vec::new(),
            world_w: crate::sprites::W as f32,
            world_h: crate::sprites::H as f32,
            water_min: [0.0; 3],
            water_max: [0.0; 3],
            terrain_tex_size: (0, 0),
            water_vbo,
            water_ibo,
            water_index_count: 0,
            billboard_vbo,
            fbo,
            fbo_texture,
            _fbo_depth_rb: fbo_depth_rb,
            terrain_texture,
            atlas_texture,
            u_terrain_vp,
            u_terrain_sun_dir,
            u_terrain_ambient,
            u_terrain_night,
            u_terrain_time,
            u_terrain_cam_pos,
            u_terrain_light_count,
            u_terrain_light_pos,
            u_terrain_light_rad,
            u_terrain_light_int,
            u_water_vp,
            u_water_sun_dir,
            u_water_night,
            u_water_time,
            u_water_cam_pos,
            u_water_light_count,
            u_water_light_pos,
            u_water_light_rad,
            u_water_light_int,
            u_billboard_vp,
            u_billboard_cam_right,
            u_billboard_cam_up,
            u_billboard_ground_forward,
            u_billboard_night,
            u_billboard_sun_dir,
            u_billboard_light_count,
            u_billboard_light_pos,
            u_billboard_light_rad,
            u_billboard_light_int,
            u_terrain_offset_x,
            u_terrain_offset_z,
            u_water_offset_x,
            u_water_offset_z,
            u_billboard_offset_x,
            u_billboard_offset_z,
            backdrop: [0.09, 0.17, 0.36],
        })
    }

    pub fn upload_mesh(&mut self, mesh: &TerrainMesh) {
        let gl = &self.gl;

        // Terrain vertices
        gl.bind_buffer(WebGlRenderingContext::ARRAY_BUFFER, Some(&self.terrain_vbo));
        unsafe {
            let view = js_sys::Float32Array::view(&mesh.vertices);
            gl.buffer_data_with_array_buffer_view(
                WebGlRenderingContext::ARRAY_BUFFER,
                &view,
                WebGlRenderingContext::STATIC_DRAW,
            );
        }

        // Terrain indices
        gl.bind_buffer(WebGlRenderingContext::ELEMENT_ARRAY_BUFFER, Some(&self.terrain_ibo));
        unsafe {
            let view = js_sys::Uint16Array::view(&mesh.indices);
            gl.buffer_data_with_array_buffer_view(
                WebGlRenderingContext::ELEMENT_ARRAY_BUFFER,
                &view,
                WebGlRenderingContext::STATIC_DRAW,
            );
        }
        self.terrain_index_count = mesh.indices.len() as i32;
        self.terrain_chunks = mesh.chunks.clone();
        self.world_w = mesh.world_w;
        self.world_h = mesh.world_h;
        self.water_min = mesh.water_min;
        self.water_max = mesh.water_max;

        // Water vertices
        gl.bind_buffer(WebGlRenderingContext::ARRAY_BUFFER, Some(&self.water_vbo));
        unsafe {
            let view = js_sys::Float32Array::view(&mesh.water_vertices);
            gl.buffer_data_with_array_buffer_view(
                WebGlRenderingContext::ARRAY_BUFFER,
                &view,
                WebGlRenderingContext::STATIC_DRAW,
            );
        }

        // Water indices
        gl.bind_buffer(WebGlRenderingContext::ELEMENT_ARRAY_BUFFER, Some(&self.water_ibo));
        unsafe {
            let view = js_sys::Uint16Array::view(&mesh.water_indices);
            gl.buffer_data_with_array_buffer_view(
                WebGlRenderingContext::ELEMENT_ARRAY_BUFFER,
                &view,
                WebGlRenderingContext::STATIC_DRAW,
            );
        }
        self.water_index_count = mesh.water_indices.len() as i32;
    }

    /// The terrain's colour map from a canvas the size of the world (#129): the era's base layer
    /// plus whatever the scene paints on the ground this frame. Reuses the texture's storage
    /// while the size stays the same (the per-frame case).
    pub fn upload_terrain_canvas(&mut self, canvas: &web_sys::HtmlCanvasElement) -> Result<(), wasm_bindgen::JsValue> {
        let gl = &self.gl;
        let size = (canvas.width(), canvas.height());
        gl.bind_texture(WebGlRenderingContext::TEXTURE_2D, Some(&self.terrain_texture));
        if size == self.terrain_tex_size {
            gl.tex_sub_image_2d_with_u32_and_u32_and_canvas(
                WebGlRenderingContext::TEXTURE_2D, 0, 0, 0,
                WebGlRenderingContext::RGBA, WebGlRenderingContext::UNSIGNED_BYTE, canvas,
            )?;
        } else {
            gl.tex_image_2d_with_u32_and_u32_and_canvas(
                WebGlRenderingContext::TEXTURE_2D, 0, WebGlRenderingContext::RGBA as i32,
                WebGlRenderingContext::RGBA, WebGlRenderingContext::UNSIGNED_BYTE, canvas,
            )?;
            self.terrain_tex_size = size;
        }
        Ok(())
    }

    pub fn upload_terrain_texture(&mut self, pixels: &[u8], width: i32, height: i32) {
        self.terrain_tex_size = (width as u32, height as u32);
        let gl = &self.gl;
        gl.bind_texture(WebGlRenderingContext::TEXTURE_2D, Some(&self.terrain_texture));
        gl.tex_image_2d_with_i32_and_i32_and_i32_and_format_and_type_and_opt_u8_array(
            WebGlRenderingContext::TEXTURE_2D,
            0,
            WebGlRenderingContext::RGBA as i32,
            width,
            height,
            0,
            WebGlRenderingContext::RGBA,
            WebGlRenderingContext::UNSIGNED_BYTE,
            Some(pixels),
        )
        .unwrap();
    }

    pub fn upload_atlas_texture(&self, pixels: &[u8], width: i32, height: i32) {
        let gl = &self.gl;
        gl.bind_texture(WebGlRenderingContext::TEXTURE_2D, Some(&self.atlas_texture));
        gl.tex_image_2d_with_i32_and_i32_and_i32_and_format_and_type_and_opt_u8_array(
            WebGlRenderingContext::TEXTURE_2D,
            0,
            WebGlRenderingContext::RGBA as i32,
            width,
            height,
            0,
            WebGlRenderingContext::RGBA,
            WebGlRenderingContext::UNSIGNED_BYTE,
            Some(pixels),
        )
        .unwrap();
    }

    pub fn render_3d(
        &mut self,
        camera: &OrbitCamera,
        time: f32,
        sun_dir: Vec3,
        night: f32,
        lights: &[Light3D],
        billboards: &[BillboardVertex],
    ) {
        let gl = &self.gl;

        // 1. Render to offscreen 320x180 framebuffer for pixel-art aesthetic
        gl.bind_framebuffer(WebGlRenderingContext::FRAMEBUFFER, Some(&self.fbo));
        gl.viewport(0, 0, 320, 180);

        // Around the map: the world's deep sea, darkened at night like the terrain.
        let dim = 1.0 - 0.65 * night;
        let [br, bg, bb] = self.backdrop;
        gl.clear_color(br * dim, bg * dim, bb * dim, 1.0);
        gl.clear(WebGlRenderingContext::COLOR_BUFFER_BIT | WebGlRenderingContext::DEPTH_BUFFER_BIT);

        let aspect = 320.0 / 180.0;
        let (_view, _proj, view_proj) = camera.compute_matrices(aspect);
        let cam_right = Vec3::new(camera.yaw.cos(), 0.0, -camera.yaw.sin());
        let ground_forward = Vec3::new(-camera.yaw.sin(), 0.0, -camera.yaw.cos());
        let cam_up = Vec3::new(
            -camera.yaw.sin() * camera.pitch.sin(),
            camera.pitch.cos(),
            -camera.yaw.cos() * camera.pitch.sin(),
        );

        gl.enable(WebGlRenderingContext::DEPTH_TEST);
        gl.depth_func(WebGlRenderingContext::LEQUAL);
        gl.depth_mask(true);

        let cam_pos = camera.eye_position();

        // 2. Render Terrain Mesh
        gl.use_program(Some(&self.terrain_program));
        gl.uniform_matrix4fv_with_f32_array(Some(&self.u_terrain_vp), false, &view_proj.m);
        gl.uniform3f(Some(&self.u_terrain_sun_dir), sun_dir.x, sun_dir.y, sun_dir.z);
        if let Some(loc) = self.u_terrain_ambient.as_ref() {
            gl.uniform1f(Some(loc), 0.38);
        }
        gl.uniform1f(Some(&self.u_terrain_night), night);
        if let Some(loc) = self.u_terrain_time.as_ref() {
            gl.uniform1f(Some(loc), time);
        }
        if let Some(loc) = self.u_terrain_cam_pos.as_ref() {
            gl.uniform3f(Some(loc), cam_pos.x, cam_pos.y, cam_pos.z);
        }

        let count = lights.len().min(16);
        gl.uniform1i(Some(&self.u_terrain_light_count), count as i32);

        let mut pos_arr = [0.0f32; 16 * 3];
        let mut rad_arr = [0.0f32; 16];
        let mut int_arr = [0.0f32; 16];
        for (i, l) in lights.iter().take(count).enumerate() {
            pos_arr[i * 3] = l.pos.x;
            pos_arr[i * 3 + 1] = l.pos.y;
            pos_arr[i * 3 + 2] = l.pos.z;
            rad_arr[i] = l.radius;
            int_arr[i] = l.intensity;
        }
        gl.uniform3fv_with_f32_array(Some(&self.u_terrain_light_pos), &pos_arr);
        gl.uniform1fv_with_f32_array(Some(&self.u_terrain_light_rad), &rad_arr);
        gl.uniform1fv_with_f32_array(Some(&self.u_terrain_light_int), &int_arr);

        gl.active_texture(WebGlRenderingContext::TEXTURE0);
        gl.bind_texture(WebGlRenderingContext::TEXTURE_2D, Some(&self.terrain_texture));

        gl.bind_buffer(WebGlRenderingContext::ARRAY_BUFFER, Some(&self.terrain_vbo));
        let stride = 8 * 4;
        gl.bind_buffer(WebGlRenderingContext::ELEMENT_ARRAY_BUFFER, Some(&self.terrain_ibo));

        // The world is a torus (#111) bigger than the view (#129): each chunk is drawn at every
        // wrap copy (one world period either way) that the camera can see, and nowhere else.
        let frustum = Frustum::from_view_proj(&view_proj);
        let (map_w, map_h) = (self.world_w, self.world_h);
        let mut offsets = Vec::with_capacity(9);
        for oz in [-map_h, 0.0, map_h] {
            for ox in [-map_w, 0.0, map_w] {
                offsets.push((ox, oz));
            }
        }
        let visible = |min: [f32; 3], max: [f32; 3], pad: f32, ox: f32, oz: f32| {
            frustum.intersects_aabb(
                [min[0] + ox - pad, min[1] - pad, min[2] + oz - pad],
                [max[0] + ox + pad, max[1] + pad, max[2] + oz + pad],
            )
        };
        let mut terrain_attrs = [None, None, None];
        for chunk in &self.terrain_chunks {
            let seen: Vec<(f32, f32)> = offsets.iter().copied().filter(|&(ox, oz)| visible(chunk.min, chunk.max, 0.5, ox, oz)).collect();
            if seen.is_empty() {
                continue;
            }
            // u16 indices are local to the chunk, so point the attributes at its first vertex.
            let base = (chunk.first_vertex * 8 * 4) as i32;
            terrain_attrs = [
                bind_attr(gl, &self.terrain_program, "a_position", 3, stride, base),
                bind_attr(gl, &self.terrain_program, "a_texCoord", 2, stride, base + 3 * 4),
                bind_attr(gl, &self.terrain_program, "a_normal", 3, stride, base + 5 * 4),
            ];
            for (ox, oz) in seen {
                gl.uniform1f(Some(&self.u_terrain_offset_x), ox);
                gl.uniform1f(Some(&self.u_terrain_offset_z), oz);
                gl.draw_elements_with_i32(
                    WebGlRenderingContext::TRIANGLES,
                    chunk.index_count as i32,
                    WebGlRenderingContext::UNSIGNED_SHORT,
                    (chunk.first_index * 2) as i32,
                );
            }
        }
        unbind_attrs(gl, &terrain_attrs);

        // 3. Render Animated Water Grid
        gl.use_program(Some(&self.water_program));
        gl.uniform_matrix4fv_with_f32_array(Some(&self.u_water_vp), false, &view_proj.m);
        gl.uniform3f(Some(&self.u_water_sun_dir), sun_dir.x, sun_dir.y, sun_dir.z);
        gl.uniform1f(Some(&self.u_water_night), night);
        gl.uniform1f(Some(&self.u_water_time), time);
        if let Some(loc) = self.u_water_cam_pos.as_ref() {
            gl.uniform3f(Some(loc), cam_pos.x, cam_pos.y, cam_pos.z);
        }
        gl.uniform1i(Some(&self.u_water_light_count), count as i32);
        gl.uniform3fv_with_f32_array(Some(&self.u_water_light_pos), &pos_arr);
        gl.uniform1fv_with_f32_array(Some(&self.u_water_light_rad), &rad_arr);
        gl.uniform1fv_with_f32_array(Some(&self.u_water_light_int), &int_arr);

        gl.enable(WebGlRenderingContext::BLEND);
        gl.blend_func(
            WebGlRenderingContext::SRC_ALPHA,
            WebGlRenderingContext::ONE_MINUS_SRC_ALPHA,
        );

        gl.bind_buffer(WebGlRenderingContext::ARRAY_BUFFER, Some(&self.water_vbo));
        let water_attrs = [
            bind_attr(gl, &self.water_program, "a_position", 3, stride, 0),
            bind_attr(gl, &self.water_program, "a_texCoord", 2, stride, 3 * 4),
            bind_attr(gl, &self.water_program, "a_normal", 3, stride, 5 * 4),
        ];

        gl.bind_buffer(WebGlRenderingContext::ELEMENT_ARRAY_BUFFER, Some(&self.water_ibo));
        // The glaze must not hide sprites standing in shallow water, so it doesn't write depth.
        gl.depth_mask(false);
        for &(ox, oz) in &offsets {
            if !visible(self.water_min, self.water_max, 1.0, ox, oz) {
                continue;
            }
            gl.uniform1f(Some(&self.u_water_offset_x), ox);
            gl.uniform1f(Some(&self.u_water_offset_z), oz);
            gl.draw_elements_with_i32(
                WebGlRenderingContext::TRIANGLES,
                self.water_index_count,
                WebGlRenderingContext::UNSIGNED_SHORT,
                0,
            );
        }
        gl.depth_mask(true);
        unbind_attrs(gl, &water_attrs);

        // 4. Render Billboard Sprites (camera-facing entities standing on the ground)
        if !billboards.is_empty() {
            gl.use_program(Some(&self.billboard_program));
            gl.uniform_matrix4fv_with_f32_array(Some(&self.u_billboard_vp), false, &view_proj.m);
            gl.uniform3f(
                Some(&self.u_billboard_cam_right),
                cam_right.x,
                cam_right.y,
                cam_right.z,
            );
            gl.uniform3f(
                Some(&self.u_billboard_cam_up),
                cam_up.x,
                cam_up.y,
                cam_up.z,
            );
            gl.uniform3f(
                Some(&self.u_billboard_ground_forward),
                ground_forward.x,
                ground_forward.y,
                ground_forward.z,
            );
            gl.uniform1f(Some(&self.u_billboard_night), night);
            if let Some(loc) = self.u_billboard_sun_dir.as_ref() {
                gl.uniform3f(Some(loc), sun_dir.x, sun_dir.y, sun_dir.z);
            }
            if let Some(loc) = self.u_billboard_light_count.as_ref() {
                gl.uniform1i(Some(loc), count as i32);
            }
            if let Some(loc) = self.u_billboard_light_pos.as_ref() {
                gl.uniform3fv_with_f32_array(Some(loc), &pos_arr);
            }
            if let Some(loc) = self.u_billboard_light_rad.as_ref() {
                gl.uniform1fv_with_f32_array(Some(loc), &rad_arr);
            }
            if let Some(loc) = self.u_billboard_light_int.as_ref() {
                gl.uniform1fv_with_f32_array(Some(loc), &int_arr);
            }

            gl.active_texture(WebGlRenderingContext::TEXTURE0);
            gl.bind_texture(WebGlRenderingContext::TEXTURE_2D, Some(&self.atlas_texture));

            gl.bind_buffer(WebGlRenderingContext::ARRAY_BUFFER, Some(&self.billboard_vbo));
            unsafe {
                let bytes = std::slice::from_raw_parts(
                    billboards.as_ptr() as *const u8,
                    billboards.len() * std::mem::size_of::<BillboardVertex>(),
                );
                gl.buffer_data_with_u8_array(
                    WebGlRenderingContext::ARRAY_BUFFER,
                    bytes,
                    WebGlRenderingContext::DYNAMIC_DRAW,
                );
            }

            let b_stride = std::mem::size_of::<BillboardVertex>() as i32;
            let billboard_attrs = [
                bind_attr(gl, &self.billboard_program, "a_center", 3, b_stride, 0),
                bind_attr(gl, &self.billboard_program, "a_offset", 3, b_stride, 3 * 4),
                bind_attr(gl, &self.billboard_program, "a_texCoord", 2, b_stride, 6 * 4),
            ];

            // Sprites stand up to ~40 units tall and spread a few units around their feet.
            for &(ox, oz) in &offsets {
                if !visible([0.0, 0.0, 0.0], [map_w, 48.0, map_h], 16.0, ox, oz) {
                    continue;
                }
                gl.uniform1f(Some(&self.u_billboard_offset_x), ox);
                gl.uniform1f(Some(&self.u_billboard_offset_z), oz);
                gl.draw_arrays(WebGlRenderingContext::TRIANGLES, 0, billboards.len() as i32);
            }
            unbind_attrs(gl, &billboard_attrs);
        }

        gl.disable(WebGlRenderingContext::BLEND);
        gl.disable(WebGlRenderingContext::DEPTH_TEST);
        gl.bind_framebuffer(WebGlRenderingContext::FRAMEBUFFER, None);
    }
}

/// Point a vertex attribute at the bound ARRAY_BUFFER. Returns None, and touches nothing, when
/// the shader compiler stripped the attribute (location -1); `-1 as u32` would otherwise be
/// GL_INVALID_VALUE on every frame.
fn bind_attr(
    gl: &WebGlRenderingContext,
    program: &WebGlProgram,
    name: &str,
    size: i32,
    stride: i32,
    offset: i32,
) -> Option<u32> {
    let loc = gl.get_attrib_location(program, name);
    if loc < 0 {
        return None;
    }
    let loc = loc as u32;
    gl.enable_vertex_attrib_array(loc);
    gl.vertex_attrib_pointer_with_i32(loc, size, WebGlRenderingContext::FLOAT, false, stride, offset);
    Some(loc)
}

/// Leave no attribute arrays enabled for the next pass (the 2D blit shares this context).
fn unbind_attrs(gl: &WebGlRenderingContext, attrs: &[Option<u32>]) {
    for loc in attrs.iter().flatten() {
        gl.disable_vertex_attrib_array(*loc);
    }
}

fn compile_shader(
    gl: &WebGlRenderingContext,
    shader_type: u32,
    src: &str,
) -> Result<WebGlShader, String> {
    let shader = gl.create_shader(shader_type).ok_or("Cannot create shader")?;
    gl.shader_source(&shader, src);
    gl.compile_shader(&shader);

    if gl
        .get_shader_parameter(&shader, WebGlRenderingContext::COMPILE_STATUS)
        .as_bool()
        .unwrap_or(false)
    {
        Ok(shader)
    } else {
        Err(gl
            .get_shader_info_log(&shader)
            .unwrap_or_else(|| "Unknown compile error".into()))
    }
}

fn link_program(
    gl: &WebGlRenderingContext,
    vert: &WebGlShader,
    frag: &WebGlShader,
) -> Result<WebGlProgram, String> {
    let program = gl.create_program().ok_or("Cannot create program")?;
    gl.attach_shader(&program, vert);
    gl.attach_shader(&program, frag);
    gl.link_program(&program);

    if gl
        .get_program_parameter(&program, WebGlRenderingContext::LINK_STATUS)
        .as_bool()
        .unwrap_or(false)
    {
        Ok(program)
    } else {
        Err(gl
            .get_program_info_log(&program)
            .unwrap_or_else(|| "Unknown link error".into()))
    }
}
