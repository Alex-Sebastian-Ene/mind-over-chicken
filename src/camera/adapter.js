// Thin MediaPipe Camera Adapter (Wave 3 of #108).
//
// Google's hand and face models are a JS/wasm library. This adapter runs the models
// and passes raw landmark coordinates and face blendshapes directly to the Rust engine (RustApp).
// All gestures, filters, debounce, god powers, and emotion analysis run purely in Rust.

import { initCamera } from './camera.js';

export function initCameraAdapter(app) {
  return initCamera({ app });
}
