//! WebGL2 backend: batched colored lines/triangles + textured image/text quads.

use std::num::NonZeroUsize;

use lru::LruCache;

use super::image_cache::{ImageCache, ImageCacheHandle};
use crate::buffer::Color;
use crate::renderer::Renderer;
use js_sys::Float32Array;
use wasm_bindgen::prelude::*;
use wasm_bindgen::JsCast;
use web_sys::{
    CanvasRenderingContext2d, HtmlCanvasElement, HtmlImageElement, WebGl2RenderingContext,
    WebGlBuffer, WebGlProgram, WebGlShader, WebGlTexture, WebGlUniformLocation,
    WebGlVertexArrayObject,
};

const CIRCLE_SEGMENTS: i32 = 64;
const FLOATS_PER_VERT: usize = 6;
const FLOATS_PER_TEX_VERT: usize = 4; // x, y, u, v
/// Match [`super::image_cache`] — visible rails + neighbors without thrashing.
const IMAGE_TEX_CAP: usize = 96;
/// Titles / metadata strings churn as focus moves; keep a larger text budget.
const TEXT_TEX_CAP: usize = 192;

const VERT_SRC: &str = r#"#version 300 es
precision highp float;
layout(location = 0) in vec2 a_pos;
layout(location = 1) in vec4 a_color;
out vec4 v_color;
void main() {
    gl_Position = vec4(a_pos, 0.0, 1.0);
    v_color = a_color;
}
"#;

const FRAG_SRC: &str = r#"#version 300 es
precision highp float;
in vec4 v_color;
out vec4 frag_color;
void main() {
    frag_color = v_color;
}
"#;

const TEX_VERT_SRC: &str = r#"#version 300 es
precision highp float;
layout(location = 0) in vec2 a_pos;
layout(location = 1) in vec2 a_uv;
out vec2 v_uv;
void main() {
    gl_Position = vec4(a_pos, 0.0, 1.0);
    v_uv = a_uv;
}
"#;

const TEX_FRAG_SRC: &str = r#"#version 300 es
precision highp float;
uniform sampler2D u_tex;
in vec2 v_uv;
out vec4 frag_color;
void main() {
    frag_color = texture(u_tex, v_uv);
}
"#;

/// Draws via WebGL2 into a dedicated canvas (`antialias: false`, transparent).
pub struct WebGl2Renderer {
    gl: WebGl2RenderingContext,
    program: WebGlProgram,
    vao: WebGlVertexArrayObject,
    vbo: WebGlBuffer,
    tex_program: WebGlProgram,
    tex_vao: WebGlVertexArrayObject,
    tex_vbo: WebGlBuffer,
    tex_uniform: WebGlUniformLocation,
    textures: LruCache<String, WebGlTexture>,
    /// Cached rasterized system-font text → (texture, width, height).
    text_textures: LruCache<String, (WebGlTexture, i32, i32)>,
    images: ImageCacheHandle,
    width: f32,
    height: f32,
    line_verts: Vec<f32>,
    tri_verts: Vec<f32>,
}

impl WebGl2Renderer {
    pub fn new(
        gl: WebGl2RenderingContext,
        width: u32,
        height: u32,
        images: ImageCacheHandle,
    ) -> Self {
        let vert = compile_shader(&gl, WebGl2RenderingContext::VERTEX_SHADER, VERT_SRC)
            .unwrap_or_else(|e| wasm_bindgen::throw_str(&e));
        let frag = compile_shader(&gl, WebGl2RenderingContext::FRAGMENT_SHADER, FRAG_SRC)
            .unwrap_or_else(|e| wasm_bindgen::throw_str(&e));
        let program =
            link_program(&gl, &vert, &frag).unwrap_or_else(|e| wasm_bindgen::throw_str(&e));

        let vao = gl
            .create_vertex_array()
            .expect_throw("Failed to create VAO");
        let vbo = gl.create_buffer().expect_throw("Failed to create VBO");

        gl.bind_vertex_array(Some(&vao));
        gl.bind_buffer(WebGl2RenderingContext::ARRAY_BUFFER, Some(&vbo));
        let stride = (FLOATS_PER_VERT * 4) as i32;
        gl.enable_vertex_attrib_array(0);
        gl.vertex_attrib_pointer_with_i32(0, 2, WebGl2RenderingContext::FLOAT, false, stride, 0);
        gl.enable_vertex_attrib_array(1);
        gl.vertex_attrib_pointer_with_i32(1, 4, WebGl2RenderingContext::FLOAT, false, stride, 8);
        gl.bind_vertex_array(None);

        let tex_vert = compile_shader(&gl, WebGl2RenderingContext::VERTEX_SHADER, TEX_VERT_SRC)
            .unwrap_or_else(|e| wasm_bindgen::throw_str(&e));
        let tex_frag = compile_shader(&gl, WebGl2RenderingContext::FRAGMENT_SHADER, TEX_FRAG_SRC)
            .unwrap_or_else(|e| wasm_bindgen::throw_str(&e));
        let tex_program =
            link_program(&gl, &tex_vert, &tex_frag).unwrap_or_else(|e| wasm_bindgen::throw_str(&e));
        let tex_uniform = gl
            .get_uniform_location(&tex_program, "u_tex")
            .expect_throw("u_tex uniform missing");

        let tex_vao = gl
            .create_vertex_array()
            .expect_throw("Failed to create texture VAO");
        let tex_vbo = gl
            .create_buffer()
            .expect_throw("Failed to create texture VBO");
        gl.bind_vertex_array(Some(&tex_vao));
        gl.bind_buffer(WebGl2RenderingContext::ARRAY_BUFFER, Some(&tex_vbo));
        let tex_stride = (FLOATS_PER_TEX_VERT * 4) as i32;
        gl.enable_vertex_attrib_array(0);
        gl.vertex_attrib_pointer_with_i32(0, 2, WebGl2RenderingContext::FLOAT, false, tex_stride, 0);
        gl.enable_vertex_attrib_array(1);
        gl.vertex_attrib_pointer_with_i32(1, 2, WebGl2RenderingContext::FLOAT, false, tex_stride, 8);
        gl.bind_vertex_array(None);
        gl.bind_buffer(WebGl2RenderingContext::ARRAY_BUFFER, None);

        gl.viewport(0, 0, width as i32, height as i32);
        gl.disable(WebGl2RenderingContext::DEPTH_TEST);

        Self {
            gl,
            program,
            vao,
            vbo,
            tex_program,
            tex_vao,
            tex_vbo,
            tex_uniform,
            textures: LruCache::new(NonZeroUsize::new(IMAGE_TEX_CAP).expect("cap > 0")),
            text_textures: LruCache::new(NonZeroUsize::new(TEXT_TEX_CAP).expect("cap > 0")),
            images,
            width: width as f32,
            height: height as f32,
            line_verts: Vec::new(),
            tri_verts: Vec::new(),
        }
    }

    /// Update the drawing resolution and GL viewport (kept for future DPR work).
    #[allow(dead_code)]
    pub fn resize(&mut self, width: u32, height: u32) {
        self.width = width as f32;
        self.height = height as f32;
        self.gl.viewport(0, 0, width as i32, height as i32);
    }

    fn to_ndc(&self, x: i32, y: i32) -> (f32, f32) {
        let nx = (x as f32 / self.width) * 2.0 - 1.0;
        let ny = 1.0 - (y as f32 / self.height) * 2.0;
        (nx, ny)
    }

    fn push_vert(buf: &mut Vec<f32>, x: f32, y: f32, color: Color) {
        buf.push(x);
        buf.push(y);
        buf.push(f32::from(color.r) / 255.0);
        buf.push(f32::from(color.g) / 255.0);
        buf.push(f32::from(color.b) / 255.0);
        buf.push(f32::from(color.a) / 255.0);
    }

    fn push_line(&mut self, x0: i32, y0: i32, x1: i32, y1: i32, color: Color) {
        let (ax, ay) = self.to_ndc(x0, y0);
        let (bx, by) = self.to_ndc(x1, y1);
        Self::push_vert(&mut self.line_verts, ax, ay, color);
        Self::push_vert(&mut self.line_verts, bx, by, color);
    }

    fn push_tri(&mut self, x0: i32, y0: i32, x1: i32, y1: i32, x2: i32, y2: i32, color: Color) {
        let (ax, ay) = self.to_ndc(x0, y0);
        let (bx, by) = self.to_ndc(x1, y1);
        let (cx, cy) = self.to_ndc(x2, y2);
        Self::push_vert(&mut self.tri_verts, ax, ay, color);
        Self::push_vert(&mut self.tri_verts, bx, by, color);
        Self::push_vert(&mut self.tri_verts, cx, cy, color);
    }

    fn flush_color_batches(&mut self) {
        self.flush_batch(&self.tri_verts, WebGl2RenderingContext::TRIANGLES);
        self.flush_batch(&self.line_verts, WebGl2RenderingContext::LINES);
        self.tri_verts.clear();
        self.line_verts.clear();
    }

    fn flush_batch(&self, verts: &[f32], mode: u32) {
        if verts.is_empty() {
            return;
        }
        let gl = &self.gl;
        gl.use_program(Some(&self.program));
        gl.bind_vertex_array(Some(&self.vao));
        gl.bind_buffer(WebGl2RenderingContext::ARRAY_BUFFER, Some(&self.vbo));

        let data = Float32Array::new_with_length(verts.len() as u32);
        data.copy_from(verts);
        gl.buffer_data_with_array_buffer_view(
            WebGl2RenderingContext::ARRAY_BUFFER,
            &data,
            WebGl2RenderingContext::DYNAMIC_DRAW,
        );

        let count = (verts.len() / FLOATS_PER_VERT) as i32;
        gl.draw_arrays(mode, 0, count);

        gl.bind_vertex_array(None);
        gl.bind_buffer(WebGl2RenderingContext::ARRAY_BUFFER, None);
    }

    fn texture_for(&mut self, url: &str, image: &HtmlImageElement) -> Option<WebGlTexture> {
        if let Some(texture) = self.textures.get(url) {
            return Some(texture.clone());
        }

        let gl = &self.gl;
        let texture = gl.create_texture()?;
        gl.bind_texture(WebGl2RenderingContext::TEXTURE_2D, Some(&texture));
        gl.pixel_storei(WebGl2RenderingContext::UNPACK_FLIP_Y_WEBGL, 1);
        gl.tex_parameteri(
            WebGl2RenderingContext::TEXTURE_2D,
            WebGl2RenderingContext::TEXTURE_WRAP_S,
            WebGl2RenderingContext::CLAMP_TO_EDGE as i32,
        );
        gl.tex_parameteri(
            WebGl2RenderingContext::TEXTURE_2D,
            WebGl2RenderingContext::TEXTURE_WRAP_T,
            WebGl2RenderingContext::CLAMP_TO_EDGE as i32,
        );
        gl.tex_parameteri(
            WebGl2RenderingContext::TEXTURE_2D,
            WebGl2RenderingContext::TEXTURE_MIN_FILTER,
            WebGl2RenderingContext::LINEAR as i32,
        );
        gl.tex_parameteri(
            WebGl2RenderingContext::TEXTURE_2D,
            WebGl2RenderingContext::TEXTURE_MAG_FILTER,
            WebGl2RenderingContext::LINEAR as i32,
        );

        // web-sys overload for HTMLImageElement
        if gl
            .tex_image_2d_with_u32_and_u32_and_html_image_element(
                WebGl2RenderingContext::TEXTURE_2D,
                0,
                WebGl2RenderingContext::RGBA as i32,
                WebGl2RenderingContext::RGBA,
                WebGl2RenderingContext::UNSIGNED_BYTE,
                image,
            )
            .is_err()
        {
            return None;
        }

        gl.bind_texture(WebGl2RenderingContext::TEXTURE_2D, None);
        if let Some(evicted) = self.textures.put(url.to_string(), texture.clone()) {
            self.gl.delete_texture(Some(&evicted));
        }
        Some(texture)
    }

    fn draw_textured_quad(&mut self, x: i32, y: i32, w: i32, h: i32, url: &str) {
        // Prefer an existing GL texture even if the HTML image was LRU-evicted.
        if let Some(texture) = self.textures.get(url).cloned() {
            self.draw_texture_quad(x, y, w, h, &texture);
            return;
        }

        let Some(image) = ImageCache::html_image(&self.images, url) else {
            return;
        };
        let Some(texture) = self.texture_for(url, &image) else {
            return;
        };
        self.draw_texture_quad(x, y, w, h, &texture);
    }

    fn draw_texture_quad(&self, x: i32, y: i32, w: i32, h: i32, texture: &WebGlTexture) {
        let (x0, y0) = self.to_ndc(x, y);
        let (x1, y1) = self.to_ndc(x + w, y + h);
        // Two triangles; UVs with v=0 at bottom after UNPACK_FLIP_Y.
        let verts: [f32; 24] = [
            x0, y0, 0.0, 1.0, //
            x1, y0, 1.0, 1.0, //
            x0, y1, 0.0, 0.0, //
            x0, y1, 0.0, 0.0, //
            x1, y0, 1.0, 1.0, //
            x1, y1, 1.0, 0.0,
        ];

        let gl = &self.gl;

        gl.enable(WebGl2RenderingContext::BLEND);
        gl.blend_func(
            WebGl2RenderingContext::SRC_ALPHA,
            WebGl2RenderingContext::ONE_MINUS_SRC_ALPHA,
        );

        gl.use_program(Some(&self.tex_program));
        gl.active_texture(WebGl2RenderingContext::TEXTURE0);
        gl.bind_texture(WebGl2RenderingContext::TEXTURE_2D, Some(texture));
        gl.uniform1i(Some(&self.tex_uniform), 0);

        gl.bind_vertex_array(Some(&self.tex_vao));
        gl.bind_buffer(WebGl2RenderingContext::ARRAY_BUFFER, Some(&self.tex_vbo));
        let data = Float32Array::new_with_length(verts.len() as u32);
        data.copy_from(&verts);
        gl.buffer_data_with_array_buffer_view(
            WebGl2RenderingContext::ARRAY_BUFFER,
            &data,
            WebGl2RenderingContext::DYNAMIC_DRAW,
        );
        gl.draw_arrays(WebGl2RenderingContext::TRIANGLES, 0, 6);

        gl.bind_vertex_array(None);
        gl.bind_texture(WebGl2RenderingContext::TEXTURE_2D, None);
        gl.disable(WebGl2RenderingContext::BLEND);
    }

    fn text_cache_key(size: i32, color: Color, text: &str) -> String {
        format!(
            "{}px|{},{},{},{}|{}",
            size, color.r, color.g, color.b, color.a, text
        )
    }

    /// Rasterize `text` with a system sans-serif font onto an offscreen canvas.
    fn rasterize_text(size: i32, color: Color, text: &str) -> Option<(HtmlCanvasElement, i32, i32)> {
        let document = web_sys::window()?.document()?;
        let canvas = document
            .create_element("canvas")
            .ok()?
            .dyn_into::<HtmlCanvasElement>()
            .ok()?;
        let ctx = canvas
            .get_context("2d")
            .ok()??
            .dyn_into::<CanvasRenderingContext2d>()
            .ok()?;

        let font = format!("{}px sans-serif", size);
        ctx.set_font(&font);
        ctx.set_text_baseline("top");
        let metrics = ctx.measure_text(text).ok()?;
        let width = metrics.width().ceil().max(1.0) as u32;
        // Pad height slightly for descenders / glyph overflow.
        let height = ((size as f64) * 1.25).ceil().max(1.0) as u32;
        canvas.set_width(width);
        canvas.set_height(height);

        // Canvas resize clears state — reapply font/styles.
        ctx.set_font(&font);
        ctx.set_text_baseline("top");
        ctx.set_fill_style_str(&color.to_css_rgba());
        ctx.fill_text(text, 0.0, 0.0).ok()?;

        Some((canvas, width as i32, height as i32))
    }

    fn text_texture_for(
        &mut self,
        size: i32,
        color: Color,
        text: &str,
    ) -> Option<(WebGlTexture, i32, i32)> {
        let key = Self::text_cache_key(size, color, text);
        if let Some((tex, w, h)) = self.text_textures.get(&key) {
            return Some((tex.clone(), *w, *h));
        }

        let (canvas, w, h) = Self::rasterize_text(size, color, text)?;
        let gl = &self.gl;
        let texture = gl.create_texture()?;
        gl.bind_texture(WebGl2RenderingContext::TEXTURE_2D, Some(&texture));
        gl.pixel_storei(WebGl2RenderingContext::UNPACK_FLIP_Y_WEBGL, 1);
        gl.tex_parameteri(
            WebGl2RenderingContext::TEXTURE_2D,
            WebGl2RenderingContext::TEXTURE_WRAP_S,
            WebGl2RenderingContext::CLAMP_TO_EDGE as i32,
        );
        gl.tex_parameteri(
            WebGl2RenderingContext::TEXTURE_2D,
            WebGl2RenderingContext::TEXTURE_WRAP_T,
            WebGl2RenderingContext::CLAMP_TO_EDGE as i32,
        );
        gl.tex_parameteri(
            WebGl2RenderingContext::TEXTURE_2D,
            WebGl2RenderingContext::TEXTURE_MIN_FILTER,
            WebGl2RenderingContext::LINEAR as i32,
        );
        gl.tex_parameteri(
            WebGl2RenderingContext::TEXTURE_2D,
            WebGl2RenderingContext::TEXTURE_MAG_FILTER,
            WebGl2RenderingContext::LINEAR as i32,
        );

        if gl
            .tex_image_2d_with_u32_and_u32_and_html_canvas_element(
                WebGl2RenderingContext::TEXTURE_2D,
                0,
                WebGl2RenderingContext::RGBA as i32,
                WebGl2RenderingContext::RGBA,
                WebGl2RenderingContext::UNSIGNED_BYTE,
                &canvas,
            )
            .is_err()
        {
            return None;
        }
        gl.bind_texture(WebGl2RenderingContext::TEXTURE_2D, None);

        if let Some((evicted, _, _)) = self.text_textures.put(key, (texture.clone(), w, h)) {
            self.gl.delete_texture(Some(&evicted));
        }
        Some((texture, w, h))
    }
}

impl Renderer for WebGl2Renderer {
    fn begin_frame(&mut self, clear: Color) {
        self.line_verts.clear();
        self.tri_verts.clear();
        self.gl.disable(WebGl2RenderingContext::SCISSOR_TEST);
        self.gl.clear_color(
            f32::from(clear.r) / 255.0,
            f32::from(clear.g) / 255.0,
            f32::from(clear.b) / 255.0,
            f32::from(clear.a) / 255.0,
        );
        self.gl.clear(WebGl2RenderingContext::COLOR_BUFFER_BIT);
    }

    fn end_frame(&mut self) {
        self.flush_color_batches();
        self.gl.disable(WebGl2RenderingContext::SCISSOR_TEST);
    }

    fn stroke_line(&mut self, x0: i32, y0: i32, x1: i32, y1: i32, color: Color) {
        self.push_line(x0, y0, x1, y1, color);
    }

    fn stroke_circle(&mut self, cx: i32, cy: i32, radius: i32, color: Color) {
        if radius < 0 {
            return;
        }
        if radius == 0 {
            self.push_line(cx, cy, cx, cy, color);
            return;
        }
        let r = radius as f64;
        let n = CIRCLE_SEGMENTS;
        for i in 0..n {
            let a0 = std::f64::consts::TAU * (i as f64) / (n as f64);
            let a1 = std::f64::consts::TAU * ((i + 1) as f64) / (n as f64);
            let x0 = cx + (r * a0.cos()).round() as i32;
            let y0 = cy + (r * a0.sin()).round() as i32;
            let x1 = cx + (r * a1.cos()).round() as i32;
            let y1 = cy + (r * a1.sin()).round() as i32;
            self.push_line(x0, y0, x1, y1, color);
        }
    }

    fn fill_circle(&mut self, cx: i32, cy: i32, radius: i32, color: Color) {
        if radius < 0 {
            return;
        }
        if radius == 0 {
            self.push_tri(cx, cy, cx, cy, cx, cy, color);
            return;
        }
        let r = radius as f64;
        let n = CIRCLE_SEGMENTS;
        for i in 0..n {
            let a0 = std::f64::consts::TAU * (i as f64) / (n as f64);
            let a1 = std::f64::consts::TAU * ((i + 1) as f64) / (n as f64);
            let x0 = cx + (r * a0.cos()).round() as i32;
            let y0 = cy + (r * a0.sin()).round() as i32;
            let x1 = cx + (r * a1.cos()).round() as i32;
            let y1 = cy + (r * a1.sin()).round() as i32;
            self.push_tri(cx, cy, x0, y0, x1, y1, color);
        }
    }

    fn fill_triangle(
        &mut self,
        x0: i32,
        y0: i32,
        x1: i32,
        y1: i32,
        x2: i32,
        y2: i32,
        color: Color,
    ) {
        self.push_tri(x0, y0, x1, y1, x2, y2, color);
    }

    fn draw_image(&mut self, x: i32, y: i32, width: i32, height: i32, url: &str) {
        if width <= 0 || height <= 0 {
            return;
        }
        // Preserve draw order relative to batched vector shapes.
        self.flush_color_batches();
        self.draw_textured_quad(x, y, width, height, url);
    }

    fn draw_image_cached(&mut self, x: i32, y: i32, width: i32, height: i32, url: &str) {
        if width <= 0 || height <= 0 {
            return;
        }
        // Skip decode/upload — only paint textures already on the GPU.
        let Some(texture) = self.textures.get(url).cloned() else {
            return;
        };
        self.flush_color_batches();
        self.draw_texture_quad(x, y, width, height, &texture);
    }

    fn prefetch_image(&mut self, url: &str) {
        ImageCache::request(&self.images, url);
    }

    fn draw_text(&mut self, x: i32, y: i32, size: i32, color: Color, text: &str) {
        if size <= 0 || text.is_empty() {
            return;
        }
        // Preserve draw order relative to batched vector shapes.
        self.flush_color_batches();
        let Some((texture, w, h)) = self.text_texture_for(size, color, text) else {
            return;
        };
        self.draw_texture_quad(x, y, w, h, &texture);
    }

    fn set_clip(&mut self, clip: Option<crate::geom::Rect>) {
        self.flush_color_batches();
        match clip {
            Some(rect) if !rect.is_empty() => {
                let x = rect.x.round() as i32;
                let y = rect.y.round() as i32;
                let w = rect.w.round().max(0.0) as i32;
                let h = rect.h.round().max(0.0) as i32;
                // WebGL scissor origin is bottom-left.
                let gl_y = self.height as i32 - y - h;
                self.gl.enable(WebGl2RenderingContext::SCISSOR_TEST);
                self.gl.scissor(x, gl_y, w, h);
            }
            _ => {
                self.gl.disable(WebGl2RenderingContext::SCISSOR_TEST);
            }
        }
    }
}

fn compile_shader(
    gl: &WebGl2RenderingContext,
    shader_type: u32,
    source: &str,
) -> Result<WebGlShader, String> {
    let shader = gl
        .create_shader(shader_type)
        .ok_or_else(|| String::from("Unable to create shader"))?;
    gl.shader_source(&shader, source);
    gl.compile_shader(&shader);

    if gl
        .get_shader_parameter(&shader, WebGl2RenderingContext::COMPILE_STATUS)
        .as_bool()
        .unwrap_or(false)
    {
        Ok(shader)
    } else {
        Err(gl
            .get_shader_info_log(&shader)
            .unwrap_or_else(|| String::from("Unknown shader compile error")))
    }
}

fn link_program(
    gl: &WebGl2RenderingContext,
    vert: &WebGlShader,
    frag: &WebGlShader,
) -> Result<WebGlProgram, String> {
    let program = gl
        .create_program()
        .ok_or_else(|| String::from("Unable to create program"))?;
    gl.attach_shader(&program, vert);
    gl.attach_shader(&program, frag);
    gl.link_program(&program);

    if gl
        .get_program_parameter(&program, WebGl2RenderingContext::LINK_STATUS)
        .as_bool()
        .unwrap_or(false)
    {
        Ok(program)
    } else {
        Err(gl
            .get_program_info_log(&program)
            .unwrap_or_else(|| String::from("Unknown program link error")))
    }
}
