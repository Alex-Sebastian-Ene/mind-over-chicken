// Render hot-path helpers (#214).
//
// The civ sandbox derives a 3D terrain mesh from the 320×180 view around its camera. Before
// #214 the mesh key rounded `camX/camY`, so every 1-pixel pan rebuilt the whole `TerrainMesh`
// (a full vertex buffer re-upload). Quantizing the camera to a small pan cell keeps the mesh
// while the camera stays inside the cell and rebuilds only when it crosses into the next one.
//
// The engine mirrors this stride in `crates/engine/src/render_perf.rs` (`CIV_MESH_PAN_STRIDE`).

/** Map pixels per civ mesh pan cell. 8 px at the sandbox's 140 px/s pan is ~17 rebuilds/s
 *  instead of one per frame, and bounds the geometric lag to one small tile. */
export const CIV_MESH_PAN_STRIDE = 8;

/** Floor-divide a camera coordinate into its pan cell. Bad/non-finite values map to cell 0. */
export function panBucket(coord, stride = CIV_MESH_PAN_STRIDE) {
  const s = Number.isFinite(stride) && stride >= 1 ? stride : CIV_MESH_PAN_STRIDE;
  const c = Number.isFinite(coord) ? coord : 0;
  return Math.floor(c / s);
}

/** The civ sandbox's 3D mesh key at the given camera position. Pure, so tests can pin it. */
export function civMeshKey(seed, camX, camY, stride = CIV_MESH_PAN_STRIDE) {
  return `civ_${seed || 0}_${panBucket(camX, stride)}_${panBucket(camY, stride)}`;
}
