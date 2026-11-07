# moc-planet

The planetary layer for Mind over Chicken. The world is a **planet**: a 640×320 equirectangular map that wraps east–west, with poles at the top and bottom. The player sees it through a 320×180 pixel camera that pans around the globe. Zooming out shows the same map wrapped onto a sphere.

This crate has no dependencies and no rendering. It is just the maths the engine (`crates/engine`) needs to make the map planetary:

| Module | What it gives the engine |
|---|---|
| `rng` | `mulberry32`, with the same sequence as the JS build (seed 12345 → 0.979728267761, …) |
| `noise` | Value noise that tiles east–west, so generated terrain has no seam at the date line |
| `camera` | `Camera::pan` / `centre` / `to_screen` / `to_planet`, correct across the seam and clamped at the poles |
| `fb` | A 320×180 RGBA view buffer and `blit_planet`, which copies the visible window out of the planet texture even when it straddles the seam |
| `path` | `Field` (wrap-aware BFS distance map) and `FieldCache` (LRU on a snapped grid) for walkers on land and boats at sea |
| `lib` | `PW`, `PH`, `VW`, `VH`, `wrap_x`, `dx_wrap`, `dist_wrap`, `lon_lat` |

## Adopting it in the engine's terrain
1. Generate the height and terrain maps at `PW`×`PH`. Sample noise as `WrapNoise::fbm(x / PW, y / PH * 4.0, octaves)` so column 0 meets column `PW - 1` seamlessly.
2. Measure distances between continents and to sites with `dist_wrap`, not `hypot`.
3. Per frame:
   - copy the visible window with `fb.blit_planet(&terrain_rgba, &camera)`;
   - draw sprites at `camera.to_screen(x, y)`;
   - convert clicks with `camera.to_planet`.
4. Walkers use `FieldCache::get(tx, ty, |x, y| walkable(x, y))` and `Field::next_step`. Boats do the same with a water predicate.

## Tests
```bash
cargo test -p moc-planet
```
The tests cover:
- walking round the back of the planet when a pole-to-pole wall blocks the short way;
- the short way across the date line;
- unreachable lakes;
- seamless noise and blits across the seam;
- JS RNG parity.
