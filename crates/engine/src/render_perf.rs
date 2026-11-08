//! Render hot-path bookkeeping (#214): persistent texture uploads and quantized world-mesh
//! keys, plus the counters devtools and the perf tests read.
//!
//! The WebGL calls themselves live in `gl`/`gl3d`; the decisions are kept pure here so they can
//! be unit-tested without a browser context.

/// Bookkeeping for one persistent texture. `note` is called on every upload, so a same-size
/// upload can reuse the storage with `texSubImage2D` instead of reallocating it with
/// `texImage2D` (the per-frame stall in #214).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct TextureUpload {
    /// Size of the storage currently allocated on the GPU (0×0 before the first upload).
    pub size: (u32, u32),
    pub uploads: u64,
    /// Uploads that reallocated the texture storage (first upload / size change).
    pub reallocations: u64,
}

impl TextureUpload {
    /// Note one `width × height` upload. Returns `true` when the texture storage must be
    /// (re)allocated because the size changed; `false` means a sub-image upload is enough.
    pub fn note(&mut self, width: u32, height: u32) -> bool {
        self.uploads += 1;
        if self.size != (width, height) {
            self.size = (width, height);
            self.reallocations += 1;
            true
        } else {
            false
        }
    }

    /// Note an upload that replaced the storage unconditionally (`texImage2D`); used by paths
    /// that always reallocate, like a full pixel-array refresh.
    pub fn note_realloc(&mut self, width: u32, height: u32) {
        self.uploads += 1;
        self.reallocations += 1;
        self.size = (width, height);
    }
}

/// Map pixels per civ mesh pan cell (#214). A 1-pixel camera pan stays inside a cell, so the
/// full `TerrainMesh::build` and its vertex-buffer upload are skipped; crossing a cell rebuilds
/// the mesh for the new view. Small enough that the geometric lag stays at tile granularity.
/// Mirrored by `CIV_MESH_PAN_STRIDE` in `src/render/render-perf.js`; tests pin both.
#[allow(dead_code)]
pub const CIV_MESH_PAN_STRIDE: f32 = 8.0;

/// Quantized pan cell for one axis. Non-finite values (a bad camera) map to 0 so they cannot
/// poison the key, and strides below one pixel degrade to the default instead of dividing by 0.
/// The runtime key is built in JS (`civMeshKey`); this is the same rule, kept here for tests.
#[allow(dead_code)]
pub fn pan_bucket(coord: f32, stride: f32) -> i64 {
    let stride = if stride.is_finite() && stride >= 1.0 { stride } else { CIV_MESH_PAN_STRIDE };
    let coord = if coord.is_finite() { coord } else { 0.0 };
    (coord / stride).floor() as i64
}

/// The world-mesh cache: remembers the last key the engine built a mesh for and counts hits
/// (skips) and misses (builds). A key bundles everything that changes the mesh — the world and,
/// for the civ sandbox, the quantized camera cell.
#[derive(Debug, Default)]
pub struct MeshCache {
    key: Option<String>,
    pub builds: u64,
    pub skips: u64,
}

impl MeshCache {
    /// Returns `true` when `key` differs from the cached one, so the caller should rebuild the
    /// mesh. Same key = keep the existing mesh and count a skip.
    pub fn needs_build(&mut self, key: &str) -> bool {
        if self.key.as_deref() == Some(key) {
            self.skips += 1;
            false
        } else {
            self.key = Some(key.to_string());
            self.builds += 1;
            true
        }
    }

    /// Forget the cached key, so the next `needs_build` always rebuilds. Used when a mesh is
    /// replaced outside the keyed path (a new world is generated, the 3D renderer is created).
    pub fn reset(&mut self) {
        self.key = None;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn texture_upload_reallocates_once_per_size_not_every_frame() {
        // #214: 600 frames of the same 320×180 canvas = 600 uploads but only 1 reallocation.
        let mut texture = TextureUpload::default();
        assert!(texture.note(320, 180), "the first upload allocates the storage");
        for _ in 1..600 {
            assert!(!texture.note(320, 180), "same size reuses the storage");
        }
        assert_eq!((texture.uploads, texture.reallocations), (600, 1));

        // A resize reallocates once, then the new size is reused again.
        assert!(texture.note(640, 360));
        assert!(!texture.note(640, 360));
        assert_eq!((texture.uploads, texture.reallocations), (602, 2));
        assert_eq!(texture.size, (640, 360));
    }

    #[test]
    fn texture_upload_note_realloc_counts_forced_tex_image_uploads() {
        let mut texture = TextureUpload::default();
        texture.note_realloc(320, 180);
        texture.note(320, 180);
        assert_eq!((texture.uploads, texture.reallocations), (2, 1));
    }

    #[test]
    fn pan_bucket_is_stable_inside_a_chunk_and_changes_on_the_boundary() {
        let stride = CIV_MESH_PAN_STRIDE;
        assert_eq!(pan_bucket(0.0, stride), pan_bucket(stride - 0.01, stride));
        assert_ne!(pan_bucket(stride - 0.01, stride), pan_bucket(stride, stride));
        assert_eq!(pan_bucket(3.5, stride), 0);
        assert_eq!(pan_bucket(8.0, stride), 1);
        assert_eq!(pan_bucket(-0.001, stride), -1, "negative cameras still quantize");
        assert_eq!(pan_bucket(f32::NAN, stride), 0, "a bad camera cannot poison the key");
        assert_eq!(pan_bucket(f32::INFINITY, stride), 0);
        assert_eq!(pan_bucket(5.0, 0.0), pan_bucket(5.0, CIV_MESH_PAN_STRIDE));
    }

    #[test]
    fn mesh_cache_skips_inside_a_chunk_and_rebuilds_on_chunk_change() {
        let mut cache = MeshCache::default();
        assert!(cache.needs_build("civ_1_0_0"), "the first key builds");
        for _ in 0..59 {
            assert!(!cache.needs_build("civ_1_0_0"), "1-pixel pans in the cell skip");
        }
        assert_eq!((cache.builds, cache.skips), (1, 59));

        assert!(cache.needs_build("civ_1_1_0"), "crossing the cell rebuilds");
        assert_eq!((cache.builds, cache.skips), (2, 59));
        assert!(!cache.needs_build("civ_1_1_0"));

        // A new world always invalidates, even if the camera cell repeats.
        cache.reset();
        assert!(cache.needs_build("civ_1_1_0"));
    }

    #[test]
    fn a_128_pixel_pan_builds_17_times_instead_of_129() {
        // Wall-clock free simulation of a 1-pixel-per-frame pan across one 128-px span: the
        // quantized key (8-px cells) builds 17 times, the old rounded key every frame.
        let mut cache = MeshCache::default();
        let mut naive_keys = std::collections::HashSet::new();
        for i in 0..=128i32 {
            let x = i as f32;
            let key = format!("civ_1_{}_{}", pan_bucket(x, CIV_MESH_PAN_STRIDE), pan_bucket(0.0, CIV_MESH_PAN_STRIDE));
            cache.needs_build(&key);
            naive_keys.insert(i); // Math.round(camX) changed every pixel
        }
        assert_eq!(cache.builds, 17);
        assert_eq!(naive_keys.len(), 129);
        assert_eq!(cache.skips, 129 - 17);
    }

    /// Not part of the normal suite: `cargo test -p mind-over-chicken-engine --release
    /// -- --ignored --nocapture mesh_build_cost` prints how expensive the skipped work was.
    #[test]
    #[ignore = "timing benchmark; run with --release --ignored --nocapture"]
    fn mesh_build_cost_320x180() {
        let heights = vec![0.47f32; 320 * 180];
        let start = std::time::Instant::now();
        let rounds = 100;
        for _ in 0..rounds {
            let mesh = crate::mesh3d::TerrainMesh::build(&heights, 320, 180, 320.0, 180.0, 24.0, 0.33);
            std::hint::black_box(&mesh);
        }
        let per = start.elapsed().as_secs_f64() * 1000.0 / rounds as f64;
        println!("TerrainMesh::build 320x180: {per:.3} ms/build ({rounds} builds)");
    }
}
