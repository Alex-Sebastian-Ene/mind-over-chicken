use crate::math3d::Vec3;

// Vertex format: [x, y, z,  u, v,  nx, ny, nz] (8 floats per vertex)
pub const VERTEX_STRIDE: usize = 8;

pub struct TerrainMesh {
    pub vertices: Vec<f32>,
    pub indices: Vec<u16>,
    pub water_vertices: Vec<f32>,
    pub water_indices: Vec<u16>,
}

impl TerrainMesh {
    pub fn build(
        heights: &[f32],
        grid_w: usize,
        grid_h: usize,
        world_w: f32,
        world_h: f32,
        height_scale: f32,
        water_level: f32,
    ) -> Self {
        let cols = 64;
        let rows = 36;
        let total_verts = (cols + 1) * (rows + 1);
        let mut vertices = Vec::with_capacity(total_verts * VERTEX_STRIDE);
        let mut indices = Vec::with_capacity(cols * rows * 6);

        let dx_world = world_w / cols as f32;
        let dz_world = world_h / rows as f32;

        let sample_h = |gx: usize, gy: usize| -> f32 {
            let px = (gx * (grid_w - 1)) / cols;
            let py = (gy * (grid_h - 1)) / rows;
            let idx = (py * grid_w + px).min(heights.len() - 1);
            heights[idx]
        };

        for r in 0..=rows {
            for c in 0..=cols {
                let x = c as f32 * dx_world;
                let z = r as f32 * dz_world;
                let h = sample_h(c, r);
                let y = h * height_scale;

                let u = c as f32 / cols as f32;
                let v = r as f32 / rows as f32;

                // Compute normal from neighbors
                let h_l = if c > 0 { sample_h(c - 1, r) } else { h };
                let h_r = if c < cols { sample_h(c + 1, r) } else { h };
                let h_u = if r > 0 { sample_h(c, r - 1) } else { h };
                let h_d = if r < rows { sample_h(c, r + 1) } else { h };

                let normal = Vec3::new(
                    (h_l - h_r) * height_scale,
                    2.0 * dx_world.min(dz_world),
                    (h_u - h_d) * height_scale,
                )
                .normalize();

                vertices.extend_from_slice(&[
                    x, y, z,
                    u, v,
                    normal.x, normal.y, normal.z,
                ]);
            }
        }

        for r in 0..rows {
            for c in 0..cols {
                let top_left = (r * (cols + 1) + c) as u16;
                let top_right = top_left + 1;
                let bot_left = ((r + 1) * (cols + 1) + c) as u16;
                let bot_right = bot_left + 1;

                indices.push(top_left);
                indices.push(bot_left);
                indices.push(top_right);

                indices.push(top_right);
                indices.push(bot_left);
                indices.push(bot_right);
            }
        }

        // Animated water grid (32 cols x 18 rows) at y = water_level * height_scale
        let wy = water_level * height_scale;
        let w_cols = 32;
        let w_rows = 18;
        let total_water_verts = (w_cols + 1) * (w_rows + 1);
        let mut water_vertices = Vec::with_capacity(total_water_verts * VERTEX_STRIDE);
        let mut water_indices = Vec::with_capacity(w_cols * w_rows * 6);

        let w_dx = world_w / w_cols as f32;
        let w_dz = world_h / w_rows as f32;

        for r in 0..=w_rows {
            for c in 0..=w_cols {
                let x = c as f32 * w_dx;
                let z = r as f32 * w_dz;
                let u = c as f32 / w_cols as f32;
                let v = r as f32 / w_rows as f32;
                water_vertices.extend_from_slice(&[
                    x, wy, z,
                    u, v,
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

                water_indices.push(top_left);
                water_indices.push(bot_left);
                water_indices.push(top_right);

                water_indices.push(top_right);
                water_indices.push(bot_left);
                water_indices.push(bot_right);
            }
        }

        Self {
            vertices,
            indices,
            water_vertices,
            water_indices,
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
}
