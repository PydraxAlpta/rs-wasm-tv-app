/**
 * Tiny TWGL-style helpers for the Rust WebGL2 backend.
 *
 * Shaders, programs, VAOs, and texture ownership stay in Rust. These exports
 * collapse multi-call WebGL / Canvas2D sequences into one wasm→JS import, and
 * upload vertex bytes straight from wasm linear memory (no intermediate
 * Float32Array alloc + copy).
 */

/** Upload `floatCount` floats from wasm memory into the bound ARRAY_BUFFER. */
function bufferDataFromWasm(
  gl: WebGL2RenderingContext,
  memory: WebAssembly.Memory,
  byteOffset: number,
  floatCount: number,
): void {
  // WebGL2: srcOffset/length are in elements of the view. Uint8 → byte units.
  const bytes = new Uint8Array(memory.buffer);
  gl.bufferData(
    gl.ARRAY_BUFFER,
    bytes,
    gl.DYNAMIC_DRAW,
    byteOffset,
    floatCount * 4,
  );
}

/** Clamp-to-edge + linear filtering used by all 2D uploads in this app. */
function configureTexture2D(gl: WebGL2RenderingContext): void {
  gl.pixelStorei(gl.UNPACK_FLIP_Y_WEBGL, 1);
  gl.texParameteri(gl.TEXTURE_2D, gl.TEXTURE_WRAP_S, gl.CLAMP_TO_EDGE);
  gl.texParameteri(gl.TEXTURE_2D, gl.TEXTURE_WRAP_T, gl.CLAMP_TO_EDGE);
  gl.texParameteri(gl.TEXTURE_2D, gl.TEXTURE_MIN_FILTER, gl.LINEAR);
  gl.texParameteri(gl.TEXTURE_2D, gl.TEXTURE_MAG_FILTER, gl.LINEAR);
}

/**
 * Flat-colour pipeline: bind program/VAO/VBO, upload verts from wasm, draw.
 */
export function flushColorBatch(
  gl: WebGL2RenderingContext,
  program: WebGLProgram,
  vao: WebGLVertexArrayObject,
  vbo: WebGLBuffer,
  memory: WebAssembly.Memory,
  byteOffset: number,
  floatCount: number,
  mode: number,
  floatsPerVert: number,
): void {
  if (floatCount === 0) return;

  gl.useProgram(program);
  gl.bindVertexArray(vao);
  gl.bindBuffer(gl.ARRAY_BUFFER, vbo);
  bufferDataFromWasm(gl, memory, byteOffset, floatCount);
  gl.drawArrays(mode, 0, floatCount / floatsPerVert);
  gl.bindVertexArray(null);
  gl.bindBuffer(gl.ARRAY_BUFFER, null);
}

/**
 * Single textured quad (banner / rasterized text): blend on, bind 2D texture
 * program, upload 6-vert quad from wasm, draw, tear down blend.
 */
export function drawTexturedQuad(
  gl: WebGL2RenderingContext,
  program: WebGLProgram,
  vao: WebGLVertexArrayObject,
  vbo: WebGLBuffer,
  texture: WebGLTexture,
  texUniform: WebGLUniformLocation,
  memory: WebAssembly.Memory,
  byteOffset: number,
  floatCount: number,
): void {
  gl.enable(gl.BLEND);
  gl.blendFunc(gl.SRC_ALPHA, gl.ONE_MINUS_SRC_ALPHA);

  gl.useProgram(program);
  gl.activeTexture(gl.TEXTURE0);
  gl.bindTexture(gl.TEXTURE_2D, texture);
  gl.uniform1i(texUniform, 0);

  gl.bindVertexArray(vao);
  gl.bindBuffer(gl.ARRAY_BUFFER, vbo);
  bufferDataFromWasm(gl, memory, byteOffset, floatCount);
  gl.drawArrays(gl.TRIANGLES, 0, 6);

  gl.bindVertexArray(null);
  gl.bindTexture(gl.TEXTURE_2D, null);
  gl.disable(gl.BLEND);
}

/**
 * Instanced card posters: blend on, bind TEXTURE_2D_ARRAY program, upload
 * instance attrs from wasm, drawArraysInstanced, tear down.
 */
export function drawArrayInstances(
  gl: WebGL2RenderingContext,
  program: WebGLProgram,
  vao: WebGLVertexArrayObject,
  instanceVbo: WebGLBuffer,
  arrayTexture: WebGLTexture,
  atlasUniform: WebGLUniformLocation,
  memory: WebAssembly.Memory,
  byteOffset: number,
  floatCount: number,
  instanceCount: number,
): void {
  if (instanceCount <= 0) return;

  gl.enable(gl.BLEND);
  gl.blendFunc(gl.SRC_ALPHA, gl.ONE_MINUS_SRC_ALPHA);

  gl.useProgram(program);
  gl.activeTexture(gl.TEXTURE0);
  gl.bindTexture(gl.TEXTURE_2D_ARRAY, arrayTexture);
  gl.uniform1i(atlasUniform, 0);

  gl.bindVertexArray(vao);
  gl.bindBuffer(gl.ARRAY_BUFFER, instanceVbo);
  bufferDataFromWasm(gl, memory, byteOffset, floatCount);
  gl.drawArraysInstanced(gl.TRIANGLES, 0, 6, instanceCount);

  gl.bindVertexArray(null);
  gl.bindTexture(gl.TEXTURE_2D_ARRAY, null);
  gl.disable(gl.BLEND);
}

/**
 * Instanced SDF round-rects (fill or stroke): blend on, upload instance attrs
 * from wasm, drawArraysInstanced, tear down. No texture.
 */
export function drawRoundInstances(
  gl: WebGL2RenderingContext,
  program: WebGLProgram,
  vao: WebGLVertexArrayObject,
  instanceVbo: WebGLBuffer,
  memory: WebAssembly.Memory,
  byteOffset: number,
  floatCount: number,
  instanceCount: number,
): void {
  if (instanceCount <= 0) return;

  gl.enable(gl.BLEND);
  gl.blendFunc(gl.SRC_ALPHA, gl.ONE_MINUS_SRC_ALPHA);

  gl.useProgram(program);
  gl.bindVertexArray(vao);
  gl.bindBuffer(gl.ARRAY_BUFFER, instanceVbo);
  bufferDataFromWasm(gl, memory, byteOffset, floatCount);
  gl.drawArraysInstanced(gl.TRIANGLES, 0, 6, instanceCount);

  gl.bindVertexArray(null);
  gl.disable(gl.BLEND);
}

/** Clear colour buffer; scissor off. `r/g/b/a` are 0–1. */
export function beginFrame(
  gl: WebGL2RenderingContext,
  r: number,
  g: number,
  b: number,
  a: number,
): void {
  gl.disable(gl.SCISSOR_TEST);
  gl.clearColor(r, g, b, a);
  gl.clear(gl.COLOR_BUFFER_BIT);
}

/**
 * Enable scissor to the given GL-space rect, or disable when `enabled` is false.
 * `x/y/w/h` use WebGL's bottom-left origin (Rust converts from design space).
 */
export function setClip(
  gl: WebGL2RenderingContext,
  enabled: boolean,
  x: number,
  y: number,
  w: number,
  h: number,
): void {
  if (!enabled) {
    gl.disable(gl.SCISSOR_TEST);
    return;
  }
  gl.enable(gl.SCISSOR_TEST);
  gl.scissor(x, y, w, h);
}

/**
 * Create a TEXTURE_2D from an HTMLImageElement (clamp + linear + flip-Y).
 * Returns null if allocation or upload fails. Caller owns eviction.
 */
export function createTexture2DFromImage(
  gl: WebGL2RenderingContext,
  image: HTMLImageElement,
): WebGLTexture | null {
  const texture = gl.createTexture();
  if (!texture) return null;
  gl.bindTexture(gl.TEXTURE_2D, texture);
  configureTexture2D(gl);
  try {
    gl.texImage2D(gl.TEXTURE_2D, 0, gl.RGBA, gl.RGBA, gl.UNSIGNED_BYTE, image);
  } catch {
    gl.bindTexture(gl.TEXTURE_2D, null);
    gl.deleteTexture(texture);
    return null;
  }
  gl.bindTexture(gl.TEXTURE_2D, null);
  return texture;
}

/**
 * Create a TEXTURE_2D from a canvas (clamp + linear + flip-Y).
 * Returns null if allocation or upload fails. Caller owns eviction.
 */
export function createTexture2DFromCanvas(
  gl: WebGL2RenderingContext,
  canvas: HTMLCanvasElement,
): WebGLTexture | null {
  const texture = gl.createTexture();
  if (!texture) return null;
  gl.bindTexture(gl.TEXTURE_2D, texture);
  configureTexture2D(gl);
  try {
    gl.texImage2D(gl.TEXTURE_2D, 0, gl.RGBA, gl.RGBA, gl.UNSIGNED_BYTE, canvas);
  } catch {
    gl.bindTexture(gl.TEXTURE_2D, null);
    gl.deleteTexture(texture);
    return null;
  }
  gl.bindTexture(gl.TEXTURE_2D, null);
  return texture;
}

/**
 * Upload one layer of an existing TEXTURE_2D_ARRAY from an image or canvas.
 * Returns false if the upload throws.
 */
export function uploadArrayLayer(
  gl: WebGL2RenderingContext,
  arrayTexture: WebGLTexture,
  layer: number,
  width: number,
  height: number,
  source: TexImageSource,
): boolean {
  gl.bindTexture(gl.TEXTURE_2D_ARRAY, arrayTexture);
  gl.pixelStorei(gl.UNPACK_FLIP_Y_WEBGL, 1);
  try {
    gl.texSubImage3D(
      gl.TEXTURE_2D_ARRAY,
      0,
      0,
      0,
      layer,
      width,
      height,
      1,
      gl.RGBA,
      gl.UNSIGNED_BYTE,
      source,
    );
  } catch {
    gl.bindTexture(gl.TEXTURE_2D_ARRAY, null);
    return false;
  }
  gl.bindTexture(gl.TEXTURE_2D_ARRAY, null);
  return true;
}

export type RasterizedText = {
  canvas: HTMLCanvasElement;
  width: number;
  height: number;
};

/**
 * Rasterize `text` with a system sans-serif font onto an offscreen canvas.
 * `r/g/b/a` are 0–255 channel values. Returns null on failure.
 */
export function rasterizeText(
  size: number,
  r: number,
  g: number,
  b: number,
  a: number,
  text: string,
): RasterizedText | null {
  const canvas = document.createElement("canvas");
  const ctx = canvas.getContext("2d");
  if (!ctx) return null;

  const font = `${size}px sans-serif`;
  ctx.font = font;
  ctx.textBaseline = "top";
  const metrics = ctx.measureText(text);
  const width = Math.max(1, Math.ceil(metrics.width));
  // Pad height slightly for descenders / glyph overflow.
  const height = Math.max(1, Math.ceil(size * 1.25));
  canvas.width = width;
  canvas.height = height;

  // Canvas resize clears state — reapply font/styles.
  ctx.font = font;
  ctx.textBaseline = "top";
  ctx.fillStyle = `rgba(${r},${g},${b},${a / 255})`;
  ctx.fillText(text, 0, 0);

  return { canvas, width, height };
}
