//! Thin wasm-bindgen imports for the `tv-webgl` JS helpers.
//!
//! Each helper collapses a multi-call WebGL / Canvas2D sequence into one
//! wasm→JS import. Draw helpers also upload vertex data from wasm linear
//! memory (no intermediate `Float32Array` alloc + copy). Shaders / VAOs /
//! programs stay owned by Rust.

use js_sys::WebAssembly;
use wasm_bindgen::prelude::*;
use wasm_bindgen::JsCast;
use web_sys::{
    HtmlCanvasElement, HtmlImageElement, WebGl2RenderingContext, WebGlBuffer, WebGlProgram,
    WebGlTexture, WebGlUniformLocation, WebGlVertexArrayObject,
};

#[wasm_bindgen(module = "tv-webgl")]
extern "C" {
    #[wasm_bindgen(js_name = flushColorBatch)]
    pub fn flush_color_batch(
        gl: &WebGl2RenderingContext,
        program: &WebGlProgram,
        vao: &WebGlVertexArrayObject,
        vbo: &WebGlBuffer,
        memory: &WebAssembly::Memory,
        byte_offset: u32,
        float_count: u32,
        mode: u32,
        floats_per_vert: u32,
    );

    #[wasm_bindgen(js_name = drawTexturedQuad)]
    pub fn draw_textured_quad(
        gl: &WebGl2RenderingContext,
        program: &WebGlProgram,
        vao: &WebGlVertexArrayObject,
        vbo: &WebGlBuffer,
        texture: &WebGlTexture,
        tex_uniform: &WebGlUniformLocation,
        memory: &WebAssembly::Memory,
        byte_offset: u32,
        float_count: u32,
    );

    #[wasm_bindgen(js_name = drawArrayInstances)]
    pub fn draw_array_instances(
        gl: &WebGl2RenderingContext,
        program: &WebGlProgram,
        vao: &WebGlVertexArrayObject,
        instance_vbo: &WebGlBuffer,
        array_texture: &WebGlTexture,
        atlas_uniform: &WebGlUniformLocation,
        memory: &WebAssembly::Memory,
        byte_offset: u32,
        float_count: u32,
        instance_count: i32,
    );

    #[wasm_bindgen(js_name = drawRoundInstances)]
    pub fn draw_round_instances(
        gl: &WebGl2RenderingContext,
        program: &WebGlProgram,
        vao: &WebGlVertexArrayObject,
        instance_vbo: &WebGlBuffer,
        memory: &WebAssembly::Memory,
        byte_offset: u32,
        float_count: u32,
        instance_count: i32,
    );

    #[wasm_bindgen(js_name = beginFrame)]
    pub fn begin_frame(gl: &WebGl2RenderingContext, r: f32, g: f32, b: f32, a: f32);

    #[wasm_bindgen(js_name = setClip)]
    pub fn set_clip(
        gl: &WebGl2RenderingContext,
        enabled: bool,
        x: i32,
        y: i32,
        w: i32,
        h: i32,
    );

    #[wasm_bindgen(js_name = createTexture2DFromImage)]
    pub fn create_texture_2d_from_image(
        gl: &WebGl2RenderingContext,
        image: &HtmlImageElement,
    ) -> Option<WebGlTexture>;

    #[wasm_bindgen(js_name = createTexture2DFromCanvas)]
    pub fn create_texture_2d_from_canvas(
        gl: &WebGl2RenderingContext,
        canvas: &HtmlCanvasElement,
    ) -> Option<WebGlTexture>;

    #[wasm_bindgen(js_name = uploadArrayLayer)]
    pub fn upload_array_layer(
        gl: &WebGl2RenderingContext,
        array_texture: &WebGlTexture,
        layer: u32,
        width: i32,
        height: i32,
        source: &JsValue,
    ) -> bool;

    #[wasm_bindgen(js_name = rasterizeText)]
    pub fn rasterize_text(
        size: i32,
        r: u8,
        g: u8,
        b: u8,
        a: u8,
        text: &str,
    ) -> Option<RasterizedText>;
}

/// Return value of [`rasterize_text`].
#[wasm_bindgen]
extern "C" {
    pub type RasterizedText;

    #[wasm_bindgen(method, getter)]
    pub fn canvas(this: &RasterizedText) -> HtmlCanvasElement;

    #[wasm_bindgen(method, getter)]
    pub fn width(this: &RasterizedText) -> i32;

    #[wasm_bindgen(method, getter)]
    pub fn height(this: &RasterizedText) -> i32;
}

/// Wasm linear memory as a typed `WebAssembly.Memory` handle for the helpers.
#[inline]
pub fn memory() -> WebAssembly::Memory {
    wasm_bindgen::memory().unchecked_into::<WebAssembly::Memory>()
}
