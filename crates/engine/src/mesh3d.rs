use crate::math3d::Vec3;

// Vertex format: [x, y, z,  u, v,  nx, ny, nz] (8 floats per vertex)
pub const VERTEX_STRIDE: usize = 8;

/// Cells per terrain chunk side. A chunk's vertices, (CHUNK + 1)², must fit u16 indices
/// (WebGL 1 without extensions), and smaller chunks cull better (#129).
pub const CHUNK: usize = 128;

/// One independently drawable piece of the terrain: its vertices start at `first_vertex` in
/// `vertices` and its u16 indices (local to the chunk) at `first_index` in `indices`.
#[derive(Clone, Debug)]
pub struct MeshChunk {
    pub first_vertex: usize,
    pub first_index: usize,
    pub index_count: usize,
    pub min: [f32; 3],
    pub max: [f32; 3],
}

pub struct TerrainMesh {
    pub vertices: Vec<f32>,
    pub indices: Vec<u16>,
    pub chunks: Vec<MeshChunk>,
    pub water_vertices: Vec<f32>,
    pub water_indices: Vec<u16>,
    pub water_min: [f32; 3],
    pub water_max: [f32; 3],
    /// The world size the mesh spans (one torus period on each axis).
    pub world_w: f32,
    pub world_h: f32,
}

impl TerrainMesh {
    /// A 1:1 mesh over a `grid_w × grid_h` heightmap (one vertex per map pixel, so coastlines,
    /// cliffs and ridges match the simulation grid), spanning `world_w × world_h` units and
    /// seamless on both axes (the map is a torus, #111). Split into chunks so any map size fits
    /// u16 indices and the renderer can skip the chunks out of sight (#129).
    pub fn build(
        heights: &[f32],
        grid_w: usize,
        grid_h: usize,
        world_w: f32,
        world_h: f32,
        height_scale: f32,
        water_level: f32,
    ) -> Self {
        let cols = grid_w.max(1);
        let rows = grid_h.max(1);
        let dx_world = world_w / cols as f32;
        let dz_world = world_h / rows as f32;

        // Height of map pixel (gx, gy), wrapping both ways so the last row/column of vertices
        // matches the first (seamless torus boundary, #111).
        let sample_h = |gx: isize, gy: isize| -> f32 {
            let px = gx.rem_euclid(cols as isize) as usize;
            let py = gy.rem_euclid(rows as isize) as usize;
            heights.get(py * grid_w + px).copied().unwrap_or(water_level)
        };

        let chunks_x = (cols + CHUNK - 1) / CHUNK;
        let chunks_z = (rows + CHUNK - 1) / CHUNK;
        let mut vertices = Vec::with_capacity((cols + chunks_x) * (rows + chunks_z) * VERTEX_STRIDE);
        let mut indices = Vec::with_capacity(cols * rows * 6);
        let mut chunks = Vec::with_capacity(chunks_x * chunks_z);

        for cz in 0..chunks_z {
            for cx in 0..chunks_x {
                let c0 = cx * CHUNK;
                let r0 = cz * CHUNK;
                let cw = CHUNK.min(cols - c0);
                let ch = CHUNK.min(rows - r0);
                let first_vertex = vertices.len() / VERTEX_STRIDE;
                let first_index = indices.len();
                let mut y_min = f32::MAX;
                let mut y_max = f32::MIN;

                for r in r0..=r0 + ch {
                    for c in c0..=c0 + cw {
                        let (ci, ri) = (c as isize, r as isize);
                        let h = sample_h(ci, ri);
                        let y = h * height_scale;
                        y_min = y_min.min(y);
                        y_max = y_max.max(y);

                        // Normal from the four neighbours, wrapping across both seams.
                        let h_l = sample_h(ci - 1, ri);
                        let h_r = sample_h(ci + 1, ri);
                        let h_u = sample_h(ci, ri - 1);
                        let h_d = sample_h(ci, ri + 1);
                        let normal = Vec3::new(
                            (h_l - h_r) * height_scale,
                            2.0 * dx_world.min(dz_world),
                            (h_u - h_d) * height_scale,
                        )
                        .normalize();

                        vertices.extend_from_slice(&[
                            c as f32 * dx_world, y, r as f32 * dz_world,
                            c as f32 / cols as f32, r as f32 / rows as f32,
                            normal.x, normal.y, normal.z,
                        ]);
                    }
                }

                let stride = cw + 1;
                for r in 0..ch {
                    for c in 0..cw {
                        let top_left = (r * stride + c) as u16;
                        let top_right = top_left + 1;
                        let bot_left = ((r + 1) * stride + c) as u16;
                        let bot_right = bot_left + 1;
                        indices.extend_from_slice(&[top_left, bot_left, top_right, top_right, bot_left, bot_right]);
                    }
                }

                chunks.push(MeshChunk {
                    first_vertex,
                    first_index,
                    index_count: indices.len() - first_index,
                    min: [c0 as f32 * dx_world, y_min, r0 as f32 * dz_world],
                    max: [(c0 + cw) as f32 * dx_world, y_max, (r0 + ch) as f32 * dz_world],
                });
            }
        }

        // Animated water: one coarse grid over exactly one period of the world (it tiles with
        // its wrap copies), about 4 units a cell, kept within u16 indices.
        let wy = water_level * height_scale;
        let mut step = 4.0f32;
        while ((world_w / step).ceil() + 1.0) * ((world_h / step).ceil() + 1.0) > 65_000.0 {
            step *= 1.5;
        }
        let w_cols = ((world_w / step).ceil() as usize).max(1);
        let w_rows = ((world_h / step).ceil() as usize).max(1);
        let w_dx = world_w / w_cols as f32;
        let w_dz = world_h / w_rows as f32;
        let mut water_vertices = Vec::with_capacity((w_cols + 1) * (w_rows + 1) * VERTEX_STRIDE);
        let mut water_indices = Vec::with_capacity(w_cols * w_rows * 6);
        for r in 0..=w_rows {
            for c in 0..=w_cols {
                water_vertices.extend_from_slice(&[
                    c as f32 * w_dx, wy, r as f32 * w_dz,
                    c as f32 / w_cols as f32, r as f32 / w_rows as f32,
                    0.0, 1.0, 0.0,
                ]);
            }
        }
        for r in 0..w_rows {
            for c in 0..w_cols {
                let top_left = (r * (w_cols + 1) + c) as u16;
                let top_right = top_left + 1;
                let bot_left = ((r + 1) * (w_cols + 1) + c) as u16;
                let bot_right = bot_left + 1;
                water_indices.extend_from_slice(&[top_left, bot_left, top_right, top_right, bot_left, bot_right]);
            }
        }

        Self {
            vertices,
            indices,
            chunks,
            water_vertices,
            water_indices,
            // Waves ripple about half a unit around the water plane.
            water_min: [0.0, wy - 1.0, 0.0],
            water_max: [world_w, wy + 1.0, world_h],
            world_w,
            world_h,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_terrain_mesh_build() {
        let heights = vec![0.5f32; 320 * 180];
        let mesh = TerrainMesh::build(&heights, 320, 180, 320.0, 180.0, 24.0, 0.33);
        assert!(!mesh.vertices.is_empty());
        assert!(!mesh.indices.is_empty());
        assert!(!mesh.water_vertices.is_empty());
        assert!(!mesh.water_indices.is_empty());
    }

    #[test]
    fn a_640_by_360_world_fits_u16_chunks_and_covers_every_cell() {
        let (w, h) = (640usize, 360usize);
        let heights: Vec<f32> = (0..w * h).map(|i| (i % 97) as f32 / 97.0).collect();
        let mesh = TerrainMesh::build(&heights, w, h, w as f32, h as f32, 24.0, 0.33);
        assert_eq!(mesh.chunks.len(), 5 * 3);
        let mut cells = 0;
        for ch in &mesh.chunks {
            let verts = ((ch.max[0] - ch.min[0]) as usize + 1) * ((ch.max[2] - ch.min[2]) as usize + 1);
            assert!(verts <= 65_536, "chunk has {} vertices", verts);
            let idx = &mesh.indices[ch.first_index..ch.first_index + ch.index_count];
            assert!(idx.iter().all(|&i| (i as usize) < verts));
            cells += ch.index_count / 6;
        }
        assert_eq!(cells, w * h);
        // The mesh spans the whole world, and the water exactly one period of it.
        let last = mesh.chunks.last().unwrap();
        assert_eq!((last.max[0], last.max[2]), (640.0, 360.0));
        assert_eq!((mesh.water_max[0], mesh.water_max[2]), (640.0, 360.0));
        assert!(mesh.water_vertices.len() / VERTEX_STRIDE <= 65_536);
    }

    #[test]
    fn vertices_sample_their_own_pixel_and_wrap_at_the_seams() {
        let (w, h) = (300usize, 200usize);
        let mut heights = vec![0.4f32; w * h];
        heights[5 * w + 7] = 0.9; // pixel (7, 5)
        heights[0] = 0.7; // pixel (0, 0): also the far corner of the torus
        let mesh = TerrainMesh::build(&heights, w, h, w as f32, h as f32, 10.0, 0.33);
        let find = |x: f32, z: f32| {
            mesh.vertices
                .chunks(VERTEX_STRIDE)
                .filter(|v| v[0] == x && v[2] == z)
                .map(|v| v[1])
                .collect::<Vec<f32>>()
        };
        assert!(find(7.0, 5.0).iter().all(|&y| (y - 9.0).abs() < 1e-5));
        // The vertex at (300, 200) closes the torus onto pixel (0, 0).
        let corner = find(300.0, 200.0);
        assert!(!corner.is_empty() && corner.iter().all(|&y| (y - 7.0).abs() < 1e-5));
    }
}
