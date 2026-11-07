use wasm_bindgen::JsCast;
use web_sys::{
    HtmlCanvasElement, WebGlBuffer, WebGlProgram, WebGlRenderingContext, WebGlShader, WebGlTexture,
    WebGlUniformLocation,
};

pub struct Light {
    pub x: f32,
    pub y: f32,
    pub radius: f32,
    pub intensity: f32,
}

pub struct WebGlRenderer {
    pub gl: WebGlRenderingContext,
    pub program: WebGlProgram,
    pub _vertex_buffer: WebGlBuffer,
    pub texture: WebGlTexture,
    pub u_night: WebGlUniformLocation,
    pub u_flash: WebGlUniformLocation,
    pub u_fade: WebGlUniformLocation,
    pub u_shake: WebGlUniformLocation,
    pub u_light_count: WebGlUniformLocation,
    pub u_lights: WebGlUniformLocation,
}

const VERTEX_SHADER_SRC: &str = r#"
attribute vec2 a_position;
attribute vec2 a_texCoord;

uniform vec2 u_shake;
varying vec2 v_texCoord;

void main() {
    v_texCoord = a_texCoord;
    gl_Position = vec4(a_position + u_shake, 0.0, 1.0);
}
"#;

const FRAGMENT_SHADER_SRC: &str = r#"
precision mediump float;

varying vec2 v_texCoord;

uniform sampler2D u_texture;
uniform float u_night;
uniform float u_flash;
uniform float u_fade;
uniform int u_light_count;
uniform vec4 u_lights[16];

void main() {
    vec4 texColor = texture2D(u_texture, v_texCoord);
    vec2 worldCoord = vec2(v_texCoord.x * 320.0, v_texCoord.y * 180.0);

    // Night ambient darkness
    vec3 nightAmbient = vec3(0.09, 0.13, 0.28);
    vec3 ambient = mix(vec3(1.0), nightAmbient, u_night);

    // Dynamic light accumulation
    vec3 lightGlow = vec3(0.0);
    for (int i = 0; i < 16; i++) {
        if (i >= u_light_count) {
            break;
        }
        vec4 l = u_lights[i];
        float d = distance(worldCoord, l.xy);
        if (d < l.z) {
            float falloff = (1.0 - (d / l.z)) * l.w;
            lightGlow += vec3(1.0, 0.65, 0.3) * falloff;
        }
    }

    // Light restores daylight in radius and adds warm fire tint
    vec3 finalRgb = texColor.rgb * (ambient + lightGlow * u_night) + (lightGlow * 0.30);
    finalRgb += vec3(u_flash * 0.9);
    finalRgb = mix(finalRgb, vec3(0.02, 0.02, 0.06), u_fade);

    gl_FragColor = vec4(clamp(finalRgb, 0.0, 1.0), texColor.a);
}
"#;

impl WebGlRenderer {
    pub fn new(canvas: &HtmlCanvasElement) -> Result<Self, String> {
        let gl = canvas
            .get_context("webgl")
            .map_err(|e| format!("{:?}", e))?
            .ok_or_else(|| "WebGL not supported".to_string())?
            .dyn_into::<WebGlRenderingContext>()
            .map_err(|_| "Failed to cast to WebGlRenderingContext")?;

        let vert_shader = compile_shader(&gl, WebGlRenderingContext::VERTEX_SHADER, VERTEX_SHADER_SRC)?;
        let frag_shader = compile_shader(&gl, WebGlRenderingContext::FRAGMENT_SHADER, FRAGMENT_SHADER_SRC)?;
        let program = link_program(&gl, &vert_shader, &frag_shader)?;
        gl.use_program(Some(&program));

        // Quad covering the screen with UV coordinates:
        // [-1.0, 1.0] -> [0.0, 0.0] (top-left)
        // [ 1.0,-1.0] -> [1.0, 1.0] (bottom-right)
        #[rustfmt::skip]
        let vertices: [f32; 24] = [
            -1.0, -1.0,  0.0, 1.0,
             1.0, -1.0,  1.0, 1.0,
            -1.0,  1.0,  0.0, 0.0,
            -1.0,  1.0,  0.0, 0.0,
             1.0, -1.0,  1.0, 1.0,
             1.0,  1.0,  1.0, 0.0,
        ];

        let vertex_buffer = gl.create_buffer().ok_or("Failed to create buffer")?;
        gl.bind_buffer(WebGlRenderingContext::ARRAY_BUFFER, Some(&vertex_buffer));
        unsafe {
            let vert_array = js_sys::Float32Array::view(&vertices);
            gl.buffer_data_with_array_buffer_view(
                WebGlRenderingContext::ARRAY_BUFFER,
                &vert_array,
                WebGlRenderingContext::STATIC_DRAW,
            );
        }

        let a_position = gl.get_attrib_location(&program, "a_position") as u32;
        let a_tex_coord = gl.get_attrib_location(&program, "a_texCoord") as u32;

        gl.enable_vertex_attrib_array(a_position);
        gl.vertex_attrib_pointer_with_i32(
            a_position,
            2,
            WebGlRenderingContext::FLOAT,
            false,
            4 * 4,
            0,
        );

        gl.enable_vertex_attrib_array(a_tex_coord);
        gl.vertex_attrib_pointer_with_i32(
            a_tex_coord,
            2,
            WebGlRenderingContext::FLOAT,
            false,
            4 * 4,
            2 * 4,
        );

        // Texture creation for 320x180 RGBA
        let texture = gl.create_texture().ok_or("Failed to create texture")?;
        gl.bind_texture(WebGlRenderingContext::TEXTURE_2D, Some(&texture));
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

        let u_night = gl.get_uniform_location(&program, "u_night").ok_or("u_night uniform missing")?;
        let u_flash = gl.get_uniform_location(&program, "u_flash").ok_or("u_flash uniform missing")?;
        let u_fade = gl.get_uniform_location(&program, "u_fade").ok_or("u_fade uniform missing")?;
        let u_shake = gl.get_uniform_location(&program, "u_shake").ok_or("u_shake uniform missing")?;
        let u_light_count = gl.get_uniform_location(&program, "u_light_count").ok_or("u_light_count missing")?;
        let u_lights = gl.get_uniform_location(&program, "u_lights").ok_or("u_lights missing")?;

        Ok(Self {
            gl,
            program,
            _vertex_buffer: vertex_buffer,
            texture,
            u_night,
            u_flash,
            u_fade,
            u_shake,
            u_light_count,
            u_lights,
        })
    }

    pub fn upload_pixels(&self, pixels: &[u8], width: i32, height: i32) {
        self.gl.bind_texture(WebGlRenderingContext::TEXTURE_2D, Some(&self.texture));
        self.gl
            .tex_image_2d_with_i32_and_i32_and_i32_and_format_and_type_and_opt_u8_array(
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

    pub fn upload_canvas(&self, canvas: &HtmlCanvasElement) -> Result<(), wasm_bindgen::JsValue> {
        self.gl.bind_texture(WebGlRenderingContext::TEXTURE_2D, Some(&self.texture));
        self.gl.tex_image_2d_with_u32_and_u32_and_canvas(
            WebGlRenderingContext::TEXTURE_2D,
            0,
            WebGlRenderingContext::RGBA as i32,
            WebGlRenderingContext::RGBA,
            WebGlRenderingContext::UNSIGNED_BYTE,
            canvas,
        )
    }

    pub fn render_scene(
        &self,
        night: f32,
        flash: f32,
        fade: f32,
        shake_x: f32,
        shake_y: f32,
        lights: &[Light],
        canvas_w: i32,
        canvas_h: i32,
    ) {
        self.gl.viewport(0, 0, canvas_w, canvas_h);
        self.gl.clear_color(0.02, 0.02, 0.06, 1.0);
        self.gl.clear(WebGlRenderingContext::COLOR_BUFFER_BIT);

        self.gl.use_program(Some(&self.program));
        self.gl.uniform1f(Some(&self.u_night), night);
        self.gl.uniform1f(Some(&self.u_flash), flash);
        self.gl.uniform1f(Some(&self.u_fade), fade);
        self.gl.uniform2f(Some(&self.u_shake), shake_x, shake_y);

        let count = lights.len().min(16);
        self.gl.uniform1i(Some(&self.u_light_count), count as i32);

        let mut light_data = [0.0f32; 16 * 4];
        for (i, l) in lights.iter().take(count).enumerate() {
            light_data[i * 4] = l.x;
            light_data[i * 4 + 1] = l.y;
            light_data[i * 4 + 2] = l.radius;
            light_data[i * 4 + 3] = l.intensity;
        }
        self.gl.uniform4fv_with_f32_array(Some(&self.u_lights), &light_data);

        self.gl.bind_texture(WebGlRenderingContext::TEXTURE_2D, Some(&self.texture));
        self.gl.draw_arrays(WebGlRenderingContext::TRIANGLES, 0, 6);
    }
}

fn compile_shader(gl: &WebGlRenderingContext, shader_type: u32, src: &str) -> Result<WebGlShader, String> {
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
        Err(gl.get_shader_info_log(&shader).unwrap_or_else(|| "Unknown compile error".into()))
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
        Err(gl.get_program_info_log(&program).unwrap_or_else(|| "Unknown link error".into()))
    }
}
