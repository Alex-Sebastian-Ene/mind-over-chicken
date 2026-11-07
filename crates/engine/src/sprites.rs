// Hand-placed pixel sprites rendered to a 320x180 RGBA buffer or 256x256 sprite atlas.

pub const W: usize = 640;
pub const H: usize = 360;
pub const ATLAS_SIZE: usize = 256;
pub const CELL_SIZE: usize = 32;

pub const SPRITE_VILLAGER: u32 = 0;
pub const SPRITE_MAMMOTH: u32 = 1;
pub const SPRITE_GOAT: u32 = 2;
pub const SPRITE_CHICKEN: u32 = 3;
pub const SPRITE_FIRE: u32 = 4;
pub const SPRITE_HUT: u32 = 5;
pub const SPRITE_HOUSE: u32 = 6;
pub const SPRITE_TOWER: u32 = 7;
pub const SPRITE_FACTORY: u32 = 8;
pub const SPRITE_EXCHANGE: u32 = 9;
pub const SPRITE_HANGAR: u32 = 10;
pub const SPRITE_PAD: u32 = 11;
pub const SPRITE_ROCKET: u32 = 12;
pub const SPRITE_RETICLE: u32 = 13;
pub const SPRITE_TREE: u32 = 14;
pub const SPRITE_SHADOW: u32 = 15;

#[derive(Clone, Copy, Debug)]
pub struct AtlasEntry {
    pub u0: f32,
    pub v0: f32,
    pub u1: f32,
    pub v1: f32,
    pub world_w: f32,
    pub world_h: f32,
}

#[inline]
pub fn put_pixel_buf(buf: &mut [u8], stride: usize, max_h: usize, x: i32, y: i32, r: u8, g: u8, b: u8, a: u8) {
    if x < 0 || y < 0 || x >= stride as i32 || y >= max_h as i32 {
        return;
    }
    let idx = ((y as usize) * stride + (x as usize)) * 4;
    if a == 255 {
        buf[idx] = r;
        buf[idx + 1] = g;
        buf[idx + 2] = b;
        buf[idx + 3] = 255;
    } else if a > 0 {
        let alpha = a as u32;
        let inv = 255 - alpha;
        buf[idx] = ((r as u32 * alpha + buf[idx] as u32 * inv) / 255) as u8;
        buf[idx + 1] = ((g as u32 * alpha + buf[idx + 1] as u32 * inv) / 255) as u8;
        buf[idx + 2] = ((b as u32 * alpha + buf[idx + 2] as u32 * inv) / 255) as u8;
        buf[idx + 3] = 255;
    }
}

#[inline]
pub fn fill_rect_buf(buf: &mut [u8], stride: usize, max_h: usize, x: i32, y: i32, w: i32, h: i32, r: u8, g: u8, b: u8, a: u8) {
    for cy in y..(y + h) {
        for cx in x..(x + w) {
            put_pixel_buf(buf, stride, max_h, cx, cy, r, g, b, a);
        }
    }
}

#[inline]
pub fn put_pixel(buf: &mut [u8], x: i32, y: i32, r: u8, g: u8, b: u8, a: u8) {
    put_pixel_buf(buf, W, H, x, y, r, g, b, a);
}

#[inline]
pub fn fill_rect(buf: &mut [u8], x: i32, y: i32, w: i32, h: i32, r: u8, g: u8, b: u8, a: u8) {
    fill_rect_buf(buf, W, H, x, y, w, h, r, g, b, a);
}

// Structures
pub fn draw_hut(buf: &mut [u8], x: i32, y: i32) {
    fill_rect(buf, x - 2, y - 2, 5, 2, 0x7a, 0x55, 0x30, 255); // walls
    fill_rect(buf, x - 2, y - 3, 5, 1, 0xc9, 0xa3, 0x5a, 255); // straw
    fill_rect(buf, x - 1, y - 4, 3, 1, 0xd8, 0xb5, 0x6a, 255);
    put_pixel(buf, x, y - 5, 0xe2, 0xc2, 0x7a, 255);
    put_pixel(buf, x, y - 1, 0x2a, 0x1c, 0x10, 255);          // door
}

pub fn draw_house(buf: &mut [u8], x: i32, y: i32, roof: (u8, u8, u8)) {
    fill_rect(buf, x - 2, y - 3, 5, 3, 0xe3, 0xd3, 0xb4, 255);
    fill_rect(buf, x - 3, y - 4, 7, 1, roof.0, roof.1, roof.2, 255);
    fill_rect(buf, x - 2, y - 5, 5, 1, roof.0, roof.1, roof.2, 255);
    fill_rect(buf, x - 1, y - 6, 3, 1, roof.0, roof.1, roof.2, 255);
    fill_rect(buf, x, y - 2, 1, 2, 0x4b, 0x35, 0x24, 255);
    put_pixel(buf, x - 2, y - 2, 0xff, 0xd3, 0x6e, 255);
}

pub fn draw_tower(buf: &mut [u8], x: i32, y: i32, h: i32) {
    fill_rect(buf, x - 2, y - h, 5, h, 0x6c, 0x7a, 0x92, 255);
    fill_rect(buf, x - 2, y - h, 1, h, 0x8a, 0x97, 0xad, 255);
    let mut i = 2;
    while i < h - 1 {
        let (r1, g1, b1) = if (i * 7 + x) % 3 != 0 { (0xff, 0xe3, 0x9a) } else { (0x3d, 0x45, 0x58) };
        let (r2, g2, b2) = if (i * 5 + x) % 4 != 0 { (0xff, 0xe3, 0x9a) } else { (0x3d, 0x45, 0x58) };
        put_pixel(buf, x - 1, y - i - 1, r1, g1, b1, 255);
        put_pixel(buf, x + 1, y - i - 1, r2, g2, b2, 255);
        i += 2;
    }
    fill_rect(buf, x, y - h - 2, 1, 2, 0xc9, 0xce, 0xd8, 255);
    put_pixel(buf, x, y - h - 3, 0xff, 0x5a, 0x5a, 255);
}

pub fn draw_factory(buf: &mut [u8], x: i32, y: i32) {
    fill_rect(buf, x - 7, y - 6, 14, 6, 0x8e, 0x3f, 0x2c, 255);
    fill_rect(buf, x - 7, y - 7, 14, 1, 0x6b, 0x2f, 0x22, 255);
    for i in [-5, -2, 1, 4] {
        fill_rect(buf, x + i, y - 4, 2, 1, 0xff, 0xcf, 0x5c, 255);
    }
    fill_rect(buf, x - 1, y - 2, 2, 2, 0x3a, 0x21, 0x16, 255);
    fill_rect(buf, x - 5, y - 13, 2, 7, 0x5d, 0x5a, 0x5a, 255);
    fill_rect(buf, x + 3, y - 11, 2, 5, 0x5d, 0x5a, 0x5a, 255);
    fill_rect(buf, x - 5, y - 13, 2, 1, 0x7b, 0x77, 0x77, 255);
    fill_rect(buf, x + 3, y - 11, 2, 1, 0x7b, 0x77, 0x77, 255);
}

pub fn draw_exchange(buf: &mut [u8], x: i32, y: i32) {
    fill_rect(buf, x - 6, y - 1, 13, 1, 0xcf, 0xc8, 0xb8, 255);
    fill_rect(buf, x - 5, y - 6, 11, 5, 0xef, 0xe9, 0xda, 255);
    for i in [-4, -2, 0, 2, 4] {
        fill_rect(buf, x + i, y - 6, 1, 5, 0xbd, 0xb5, 0xa2, 255);
    }
    fill_rect(buf, x - 6, y - 7, 13, 1, 0xd8, 0xd0, 0xbd, 255);
    fill_rect(buf, x - 4, y - 8, 9, 1, 0xd8, 0xd0, 0xbd, 255);
    fill_rect(buf, x - 2, y - 9, 5, 1, 0xd8, 0xd0, 0xbd, 255);
    put_pixel(buf, x, y - 8, 0xf2, 0xc1, 0x4e, 255); // golden goat
}

pub fn draw_hangar(buf: &mut [u8], x: i32, y: i32) {
    fill_rect(buf, x - 7, y - 5, 15, 5, 0x8a, 0x93, 0xa3, 255);
    fill_rect(buf, x - 6, y - 6, 13, 1, 0x9e, 0xa7, 0xb6, 255);
    fill_rect(buf, x - 4, y - 7, 9, 1, 0x9e, 0xa7, 0xb6, 255);
    fill_rect(buf, x - 3, y - 4, 7, 4, 0x3b, 0x42, 0x52, 255);
    fill_rect(buf, x - 7, y - 5, 1, 5, 0xb5, 0xbd, 0xcb, 255);
}

pub fn draw_pad(buf: &mut [u8], x: i32, y: i32) {
    fill_rect(buf, x - 6, y - 1, 13, 2, 0x5f, 0x64, 0x70, 255);
    fill_rect(buf, x - 5, y - 2, 11, 1, 0x76, 0x7c, 0x88, 255);
    put_pixel(buf, x - 3, y, 0xf2, 0xc1, 0x4e, 255);
    put_pixel(buf, x + 3, y, 0xf2, 0xc1, 0x4e, 255);
}

// Entities
pub fn draw_person(
    buf: &mut [u8],
    x: i32,
    y: i32,
    frame: u32,
    pose: u8, // 0: walk, 1: pray, 2: panic, 3: dead
    shirt: (u8, u8, u8),
    skin: (u8, u8, u8),
    hair: (u8, u8, u8),
) {
    if pose == 3 {
        // Fallen smitten body on the ground with charred clothing
        fill_rect(buf, x - 3, y - 1, 7, 2, 0x1e, 0x14, 0x0d, 255);
        fill_rect(buf, x - 2, y - 1, 4, 1, shirt.0, shirt.1, shirt.2, 255);
        fill_rect(buf, x + 2, y - 1, 2, 1, skin.0, skin.1, skin.2, 255);
        put_pixel(buf, x + 3, y - 2, hair.0, hair.1, hair.2, 255);
        put_pixel(buf, x - 3, y - 2, 0x33, 0x33, 0x33, 255);
        return;
    }
    let bob = if pose == 1 { (frame % 2) as i32 } else { 0 };
    let sy = y - bob;
    fill_rect(buf, x - 2, y, 5, 1, 0, 0, 0, 64); // shadow

    // Legs with boots & walk cycle
    let leg_col = (0x2d, 0x21, 0x18);
    if pose == 0 && (frame % 2 == 1) {
        put_pixel(buf, x - 1, sy - 1, leg_col.0, leg_col.1, leg_col.2, 255);
        put_pixel(buf, x + 1, sy - 1, leg_col.0, leg_col.1, leg_col.2, 255);
        put_pixel(buf, x - 1, sy - 2, 0x4a, 0x34, 0x24, 255);
        put_pixel(buf, x + 1, sy - 2, 0x4a, 0x34, 0x24, 255);
    } else {
        put_pixel(buf, x, sy - 1, leg_col.0, leg_col.1, leg_col.2, 255);
        fill_rect(buf, x - 1, sy - 2, 3, 1, 0x4a, 0x34, 0x24, 255);
    }

    // Torso (3px wide with collar & belt for depth)
    fill_rect(buf, x - 1, sy - 4, 3, 2, shirt.0, shirt.1, shirt.2, 255);
    fill_rect(buf, x - 1, sy - 2, 3, 1, 0x2e, 0x1d, 0x13, 255); // belt

    // Arms with skin hands
    if pose == 1 || pose == 2 {
        let flick = if pose == 2 && frame % 2 == 1 { -1 } else { 0 };
        put_pixel(buf, x - 2, sy - 5 + flick, shirt.0, shirt.1, shirt.2, 255);
        put_pixel(buf, x + 2, sy - 5 - flick, shirt.0, shirt.1, shirt.2, 255);
        put_pixel(buf, x - 2, sy - 6 + flick, skin.0, skin.1, skin.2, 255);
        put_pixel(buf, x + 2, sy - 6 - flick, skin.0, skin.1, skin.2, 255);
    } else {
        fill_rect(buf, x - 2, sy - 4, 1, 2, shirt.0, shirt.1, shirt.2, 255);
        fill_rect(buf, x + 2, sy - 4, 1, 2, shirt.0, shirt.1, shirt.2, 255);
        put_pixel(buf, x - 2, sy - 2, skin.0, skin.1, skin.2, 255);
        put_pixel(buf, x + 2, sy - 2, skin.0, skin.1, skin.2, 255);
    }

    // Head, face, eye, and hair crown
    fill_rect(buf, x - 1, sy - 5, 3, 1, skin.0, skin.1, skin.2, 255); // face
    put_pixel(buf, x + 1, sy - 5, 0x1e, 0x10, 0x08, 255); // eye
    fill_rect(buf, x - 1, sy - 6, 3, 1, hair.0, hair.1, hair.2, 255); // hair crown
    put_pixel(buf, x, sy - 7, hair.0, hair.1, hair.2, 255); // hair tuft
}

pub fn draw_mammoth(buf: &mut [u8], x: i32, y: i32, facing: i32, frame: u32, dead: bool) {
    let fur = if dead { (0x5a, 0x40, 0x30) } else { (0x7b, 0x55, 0x36) };
    let dark = (0x5e, 0x3f, 0x27);
    if dead {
        fill_rect(buf, x - 3, y - 2, 7, 2, fur.0, fur.1, fur.2, 255);
        fill_rect(buf, x - 3, y - 3, 7, 1, 0x8f, 0x6a, 0x48, 255);
        return;
    }
    fill_rect(buf, x - 3, y, 7, 1, 0, 0, 0, 64);
    fill_rect(buf, x - 3, y - 5, 6, 3, fur.0, fur.1, fur.2, 255);
    fill_rect(buf, x - 2, y - 6, 4, 1, fur.0, fur.1, fur.2, 255);
    let hx = if facing > 0 { x + 2 } else { x - 4 };
    fill_rect(buf, hx, y - 6, 2, 3, dark.0, dark.1, dark.2, 255);
    let tx = if facing > 0 { x + 4 } else { x - 5 };
    put_pixel(buf, tx, y - 4, 0xf4, 0xec, 0xd8, 255); // tusk
    let trk = if facing > 0 { x + 3 } else { x - 4 };
    put_pixel(buf, trk, y - 3, dark.0, dark.1, dark.2, 255); // trunk
    let leg = (frame % 2) as i32;
    fill_rect(buf, x - 3 + leg, y - 2, 1, 2, dark.0, dark.1, dark.2, 255);
    fill_rect(buf, x + 1 - leg, y - 2, 1, 2, dark.0, dark.1, dark.2, 255);
}

pub fn draw_goat(buf: &mut [u8], x: i32, y: i32, facing: i32) {
    fill_rect(buf, x - 1, y - 2, 3, 1, 0xef, 0xe8, 0xdc, 255);
    let hx = if facing > 0 { x + 2 } else { x - 2 };
    put_pixel(buf, hx, y - 3, 0xcf, 0xc6, 0xb8, 255);
    put_pixel(buf, x - 1, y - 1, 0x7a, 0x6a, 0x58, 255);
    put_pixel(buf, x + 1, y - 1, 0x7a, 0x6a, 0x58, 255);
}

pub fn draw_chicken(buf: &mut [u8], x: i32, y: i32, facing: i32, frame: u32, halo: bool) {
    let hop = if frame % 4 == 0 { 1 } else { 0 };
    fill_rect(buf, x - 1, y - 2 - hop, 2, 2, 0xfb, 0xf6, 0xea, 255);
    let hx = if facing > 0 { x + 1 } else { x - 2 };
    put_pixel(buf, hx, y - 3 - hop, 0xfb, 0xf6, 0xea, 255);
    put_pixel(buf, hx, y - 4 - hop, 0xe5, 0x39, 0x35, 255); // comb
    let bx = if facing > 0 { x + 2 } else { x - 3 };
    put_pixel(buf, bx, y - 3 - hop, 0xf6, 0xa6, 0x23, 255); // beak
    if halo {
        fill_rect(buf, x - 1, y - 6 - hop, 3, 1, 0xff, 0xe0, 0x66, 255);
    }
}

pub fn draw_unlit_wood(buf: &mut [u8], x: i32, y: i32) {
    fill_rect(buf, x - 4, y, 9, 1, 0x50, 0x50, 0x50, 255); // hearth stones
    fill_rect(buf, x - 3, y + 1, 7, 1, 0x65, 0x65, 0x65, 255);
    fill_rect(buf, x - 3, y - 1, 7, 1, 0x44, 0x44, 0x44, 255);
    fill_rect(buf, x - 3, y, 7, 1, 0x5a, 0x3a, 0x22, 255); // bottom horizontal log
    put_pixel(buf, x - 3, y, 0x8d, 0x5b, 0x35, 255); // cut wood ends
    put_pixel(buf, x + 3, y, 0x8d, 0x5b, 0x35, 255);
    fill_rect(buf, x - 2, y - 1, 5, 1, 0x6e, 0x4a, 0x2c, 255); // diagonal crossed log
    fill_rect(buf, x - 1, y - 2, 3, 1, 0x7a, 0x52, 0x32, 255); // top crossed log
    put_pixel(buf, x, y - 3, 0x9c, 0x68, 0x3e, 255); // apex stick tip
    put_pixel(buf, x - 1, y - 1, 0x3e, 0x24, 0x13, 255); // notch shadows
    put_pixel(buf, x + 1, y - 1, 0x3e, 0x24, 0x13, 255);
}

pub fn draw_fire(buf: &mut [u8], x: i32, y: i32, heat: f32, t: f32) {
    if heat <= 0.0 {
        draw_unlit_wood(buf, x, y);
        return;
    }
    fill_rect(buf, x - 2, y - 1, 5, 1, 0x5a, 0x3a, 0x22, 255);
    fill_rect(buf, x - 1, y - 2, 3, 1, 0x6e, 0x4a, 0x2c, 255);
    put_pixel(buf, x - 3, y, 0x6b, 0x6b, 0x6b, 255);
    put_pixel(buf, x + 3, y, 0x6b, 0x6b, 0x6b, 255);
    fill_rect(buf, x - 2, y + 1, 5, 1, 0x6b, 0x6b, 0x6b, 255);

    let n = (heat * 6.0).ceil() as i32;
    for i in 0..n {
        let fx = x - 1 + ((i * 7 + (t * 12.0) as i32) % 3);
        let fh = (1.0 + heat * 4.0 * (0.5 + 0.5 * (t * 15.0 + i as f32 * 1.7).sin())) as i32;
        let color = if i % 3 == 0 {
            (0xff, 0xe0, 0x66)
        } else if i % 3 == 1 {
            (0xff, 0x9a, 0x2e)
        } else {
            (0xe8, 0x45, 0x2c)
        };
        fill_rect(buf, fx, y - 2 - fh, 1, fh.max(1), color.0, color.1, color.2, 255);
    }
    if heat > 0.75 {
        let py = y - 7 - (((t * 6.0) as i32) % 3);
        put_pixel(buf, x, py, 0xff, 0xd1, 0x66, 255);
    }
}

pub fn draw_rocket(buf: &mut [u8], x: i32, y: i32, progress: f32, t: f32, flame: f32) {
    let rocket_h = 22;
    let built = (rocket_h as f32 * progress) as i32;
    for i in 0..built {
        let row = y - 2 - i;
        if i < 3 {
            put_pixel(buf, x - 3, row, 0xc0, 0x39, 0x2b, 255);
            put_pixel(buf, x + 3, row, 0xc0, 0x39, 0x2b, 255);
        }
        if i < rocket_h - 4 {
            let col = if i % 6 == 5 { (0xc7, 0xcc, 0xd6) } else { (0xf4, 0xf6, 0xfa) };
            fill_rect(buf, x - 2, row, 5, 1, col.0, col.1, col.2, 255);
        } else if i < rocket_h - 2 {
            fill_rect(buf, x - 1, row, 3, 1, 0xe0, 0x4a, 0x3a, 255);
        } else {
            put_pixel(buf, x, row, 0xe0, 0x4a, 0x3a, 255);
        }
        if i == 12 {
            put_pixel(buf, x, row, 0x3e, 0x8e, 0xd0, 255);
        }
        if i == 8 {
            fill_rect(buf, x - 2, row, 5, 1, 0x1f, 0x2a, 0x44, 255);
        }
    }
    if progress < 1.0 && (t * 20.0).sin() > 0.0 {
        let sx = x - 3 + (((t * 9.0) as i32) % 7);
        put_pixel(buf, sx, y - 2 - built, 0xff, 0xe0, 0x66, 255);
    }
    if flame > 0.0 {
        let fl = (3.0 + flame * 6.0 + (t * 40.0).sin() * 2.0) as i32;
        for i in 0..fl {
            let w = (3 - i / 3).max(1);
            let col = if i < 2 {
                (0xff, 0xf4, 0xc2)
            } else if i < 5 {
                (0xff, 0xb0, 0x2e)
            } else {
                (0xe8, 0x45, 0x2c)
            };
            fill_rect(buf, x - w / 2, y - 1 + i, w, 1, col.0, col.1, col.2, 255);
        }
    }
}

pub fn draw_tree_pine(buf: &mut [u8], x: i32, y: i32) {
    // Pine trunk
    fill_rect(buf, x - 1, y - 3, 2, 3, 0x4a, 0x33, 0x1f, 255);
    // Tier 1 (bottom boughs)
    fill_rect(buf, x - 4, y - 5, 9, 2, 0x1d, 0x48, 0x27, 255);
    fill_rect(buf, x - 3, y - 6, 7, 1, 0x27, 0x5a, 0x32, 255);
    // Tier 2 (mid boughs)
    fill_rect(buf, x - 3, y - 8, 7, 2, 0x22, 0x54, 0x2d, 255);
    fill_rect(buf, x - 2, y - 9, 5, 1, 0x2f, 0x6e, 0x3d, 255);
    // Tier 3 (top peak)
    fill_rect(buf, x - 2, y - 11, 5, 2, 0x35, 0x78, 0x42, 255);
    fill_rect(buf, x - 1, y - 13, 3, 2, 0x44, 0x8f, 0x51, 255);
    put_pixel(buf, x, y - 14, 0x58, 0xaa, 0x66, 255);
}

pub fn draw_tree_oak(buf: &mut [u8], x: i32, y: i32) {
    // Oak trunk
    fill_rect(buf, x - 1, y - 4, 3, 4, 0x52, 0x39, 0x22, 255);
    // Canopy underside
    fill_rect(buf, x - 5, y - 7, 11, 3, 0x1f, 0x4c, 0x22, 255);
    // Canopy body
    fill_rect(buf, x - 6, y - 11, 13, 4, 0x2d, 0x69, 0x2a, 255);
    // Rounded top canopy & highlights
    fill_rect(buf, x - 4, y - 13, 9, 2, 0x44, 0x8a, 0x39, 255);
    fill_rect(buf, x - 2, y - 14, 5, 1, 0x5a, 0xa5, 0x46, 255);
    put_pixel(buf, x - 3, y - 10, 0x5a, 0xa5, 0x46, 255);
    put_pixel(buf, x + 2, y - 11, 0x5a, 0xa5, 0x46, 255);
}

pub fn draw_shadow(buf: &mut [u8], cx: i32, cy: i32, rx: i32, ry: i32) {
    for dy in -ry..=ry {
        for dx in -rx..=rx {
            let nx = dx as f32 / rx as f32;
            let ny = dy as f32 / ry as f32;
            let d_sq = nx * nx + ny * ny;
            if d_sq <= 1.0 {
                let falloff = 1.0 - d_sq.sqrt();
                let alpha = (falloff * 140.0) as u8;
                put_pixel(buf, cx + dx, cy + dy, 0x06, 0x0a, 0x14, alpha);
            }
        }
    }
}

#[allow(dead_code)]
pub fn draw_gantry(buf: &mut [u8], x: i32, y: i32, h: i32) {
    for i in 0..h {
        let (r, g, b) = if i % 3 == 0 { (0xc9, 0xa2, 0x27) } else { (0x8c, 0x7a, 0x3c) };
        put_pixel(buf, x, y - i, r, g, b, 255);
    }
    let mut i = 0;
    while i < h {
        fill_rect(buf, x - 1, y - i, 3, 1, 0x8c, 0x7a, 0x3c, 255);
        i += 4;
    }
    put_pixel(buf, x - 1, y - h, 0xff, 0x5a, 0x5a, 255);
}

#[allow(dead_code)]
pub fn draw_boat(buf: &mut [u8], x: i32, y: i32, facing: i32, is_raft: bool, sail_color: Option<(u8, u8, u8)>) {
    let f = if facing >= 0 { 1 } else { -1 };
    if is_raft {
        fill_rect(buf, x - 2, y, 5, 1, 0x7a, 0x55, 0x30, 255);
        fill_rect(buf, x - 2, y - 1, 5, 1, 0x9a, 0x70, 0x40, 255);
        fill_rect(buf, x, y - 4, 1, 3, 0x6b, 0x4a, 0x2c, 255);
        fill_rect(buf, x + f, y - 4, 1, 2, 0xe8, 0xdc, 0xc0, 255);
    } else {
        fill_rect(buf, x - 3, y, 7, 1, 0x5a, 0x3a, 0x22, 255);
        fill_rect(buf, x - 2, y + 1, 5, 1, 0x4a, 0x2f, 0x1b, 255);
        let prow_x = x - 3 + if f > 0 { 6 } else { 0 };
        put_pixel(buf, prow_x, y - 1, 0x5a, 0x3a, 0x22, 255);
        fill_rect(buf, x, y - 6, 1, 6, 0x4a, 0x3a, 0x2a, 255);
        let (sr, sg, sb) = sail_color.unwrap_or((0xf2, 0xea, 0xd8));
        let sail_x = x - if f > 0 { 2 } else { 0 };
        fill_rect(buf, sail_x, y - 5, 2, 3, sr, sg, sb, 255);
    }
}

#[allow(dead_code)]
pub fn draw_reticle(buf: &mut [u8], x: i32, y: i32, t: f32, color: (u8, u8, u8)) {
    let r = 3 + (((t * 6.0).sin() + 1.0) * 0.5).round() as i32;
    fill_rect(buf, x - r - 1, y, 2, 1, color.0, color.1, color.2, 255);
    fill_rect(buf, x + r, y, 2, 1, color.0, color.1, color.2, 255);
    fill_rect(buf, x, y - r - 1, 1, 2, color.0, color.1, color.2, 255);
    fill_rect(buf, x, y + r, 1, 2, color.0, color.1, color.2, 255);
    put_pixel(buf, x, y, color.0, color.1, color.2, 255);
}

#[derive(Clone, Copy, Debug)]
pub struct BoltPoint {
    pub x: f32,
    pub y: f32,
}

#[allow(dead_code)]
pub fn make_bolt<R: FnMut() -> f32>(tx: f32, ty: f32, mut rand_fn: R) -> Vec<BoltPoint> {
    let mut pts = Vec::with_capacity(11);
    let start_x = tx + (rand_fn() - 0.5) * 30.0;
    pts.push(BoltPoint { x: start_x, y: -4.0 });
    let steps = 9;
    for i in 1..steps {
        let t = i as f32 / steps as f32;
        let x = start_x + (tx - start_x) * t + (rand_fn() - 0.5) * 9.0;
        let y = ty * t;
        pts.push(BoltPoint { x, y });
    }
    pts.push(BoltPoint { x: tx, y: ty });
    pts
}

#[allow(dead_code)]
pub fn draw_bolt(buf: &mut [u8], pts: &[BoltPoint], alpha: f32) {
    if pts.len() < 2 || alpha <= 0.01 {
        return;
    }
    for i in 0..(pts.len() - 1) {
        let a = pts[i];
        let b = pts[i + 1];
        let n = ((b.x - a.x).abs().max((b.y - a.y).abs()) as i32).max(1);
        for s in 0..=n {
            let t = s as f32 / n as f32;
            let x = (a.x + (b.x - a.x) * t).round() as i32;
            let y = (a.y + (b.y - a.y) * t).round() as i32;
            put_pixel(buf, x - 1, y, 160, 200, 255, (alpha * 0.6 * 255.0) as u8);
            put_pixel(buf, x + 1, y, 160, 200, 255, (alpha * 0.6 * 255.0) as u8);
            put_pixel(buf, x, y, 255, 255, 255, (alpha * 255.0) as u8);
        }
    }
}

pub fn get_sprite_atlas_entry(sprite_id: u32, facing: i32, frame: u32, variant: u32) -> AtlasEntry {
    let (col, row, world_w, world_h) = match sprite_id {
        SPRITE_VILLAGER => {
            let pose = variant; // 0: walk, 1: pray, 2: panic, 3: dead
            let f = (frame % 2) as usize;
            let c = match pose {
                0 => f,
                1 => 2 + f,
                2 => 4 + f,
                _ => 6,
            };
            (c, 0, 8.0, 10.0)
        }
        SPRITE_CHICKEN => {
            let halo = variant == 1;
            if halo {
                (0, 1, 6.0, 6.0)
            } else {
                (7, 0, 6.0, 6.0)
            }
        }
        SPRITE_MAMMOTH => {
            let dead = variant == 1;
            let f = (frame % 2) as usize;
            if dead {
                (5, 1, 16.0, 14.0)
            } else if facing > 0 {
                (1 + f, 1, 16.0, 14.0)
            } else {
                (3 + f, 1, 16.0, 14.0)
            }
        }
        SPRITE_GOAT => {
            if facing > 0 {
                (6, 1, 8.0, 6.0)
            } else {
                (7, 1, 8.0, 6.0)
            }
        }
        SPRITE_FIRE => {
            if variant == 1 {
                (0, 4, 10.0, 8.0) // unlit firewood pile
            } else {
                let f = (frame % 3) as usize;
                (f, 2, 10.0, 10.0)
            }
        }
        SPRITE_RETICLE => (3, 2, 8.0, 8.0),
        SPRITE_HUT => (4, 2, 14.0, 14.0),
        SPRITE_HOUSE => (5, 2, 16.0, 16.0),
        SPRITE_TOWER => (6, 2, 14.0, 28.0),
        SPRITE_FACTORY => (7, 2, 24.0, 18.0),
        SPRITE_EXCHANGE => (0, 3, 20.0, 16.0),
        SPRITE_HANGAR => (1, 3, 20.0, 14.0),
        SPRITE_PAD => (2, 3, 18.0, 8.0),
        SPRITE_ROCKET => (3, 3, 16.0, 30.0),
        SPRITE_TREE => {
            let col = if variant == 1 { 5 } else { 4 };
            (col, 3, 14.0, 18.0)
        }
        SPRITE_SHADOW => match variant {
            0 => (6, 3, 8.0, 5.0),
            1 => (7, 3, 16.0, 8.0),
            _ => (4, 4, 24.0, 12.0),
        },
        _ => (0, 0, 8.0, 8.0),
    };

    let u0 = (col * CELL_SIZE) as f32 / ATLAS_SIZE as f32;
    let v0 = (row * CELL_SIZE) as f32 / ATLAS_SIZE as f32;
    let u1 = ((col + 1) * CELL_SIZE) as f32 / ATLAS_SIZE as f32;
    let v1 = ((row + 1) * CELL_SIZE) as f32 / ATLAS_SIZE as f32;

    AtlasEntry {
        u0,
        v0,
        u1,
        v1,
        world_w,
        world_h,
    }
}

pub fn create_sprite_atlas() -> Vec<u8> {
    let mut atlas = vec![0u8; ATLAS_SIZE * ATLAS_SIZE * 4];
    let mut temp = vec![0u8; W * H * 4];

    let cx = 160i32;
    let cy = 90i32;

    let copy_cell = |atlas_buf: &mut [u8], temp_buf: &[u8], col: usize, row: usize, top_offset: i32| {
        for y in 0..CELL_SIZE {
            for x in 0..CELL_SIZE {
                let sx = cx - 16 + x as i32;
                let sy = cy - top_offset + y as i32;
                if sx >= 0 && sx < W as i32 && sy >= 0 && sy < H as i32 {
                    let s_idx = ((sy as usize) * W + (sx as usize)) * 4;
                    let dx = col * CELL_SIZE + x;
                    let dy = row * CELL_SIZE + y;
                    let d_idx = (dy * ATLAS_SIZE + dx) * 4;
                    atlas_buf[d_idx..d_idx + 4].copy_from_slice(&temp_buf[s_idx..s_idx + 4]);
                }
            }
        }
    };

    // Row 0: Villagers & Chicken
    temp.fill(0);
    draw_person(&mut temp, cx, cy, 0, 0, (0x4a, 0x90, 0xe2), (0xe0, 0xb4, 0x8a), (0x3b, 0x2a, 0x1e));
    copy_cell(&mut atlas, &temp, 0, 0, 31);

    temp.fill(0);
    draw_person(&mut temp, cx, cy, 1, 0, (0x4a, 0x90, 0xe2), (0xe0, 0xb4, 0x8a), (0x3b, 0x2a, 0x1e));
    copy_cell(&mut atlas, &temp, 1, 0, 31);

    temp.fill(0);
    draw_person(&mut temp, cx, cy, 0, 1, (0xe0, 0x60, 0x50), (0xe0, 0xb4, 0x8a), (0x3b, 0x2a, 0x1e));
    copy_cell(&mut atlas, &temp, 2, 0, 31);

    temp.fill(0);
    draw_person(&mut temp, cx, cy, 1, 1, (0xe0, 0x60, 0x50), (0xe0, 0xb4, 0x8a), (0x3b, 0x2a, 0x1e));
    copy_cell(&mut atlas, &temp, 3, 0, 31);

    temp.fill(0);
    draw_person(&mut temp, cx, cy, 0, 2, (0x50, 0xb0, 0x50), (0xe0, 0xb4, 0x8a), (0x3b, 0x2a, 0x1e));
    copy_cell(&mut atlas, &temp, 4, 0, 31);

    temp.fill(0);
    draw_person(&mut temp, cx, cy, 1, 2, (0x50, 0xb0, 0x50), (0xe0, 0xb4, 0x8a), (0x3b, 0x2a, 0x1e));
    copy_cell(&mut atlas, &temp, 5, 0, 31);

    temp.fill(0);
    draw_person(&mut temp, cx, cy, 0, 3, (0x80, 0x80, 0x80), (0xe0, 0xb4, 0x8a), (0x3b, 0x2a, 0x1e));
    copy_cell(&mut atlas, &temp, 6, 0, 31);

    temp.fill(0);
    draw_chicken(&mut temp, cx, cy, 1, 0, false);
    copy_cell(&mut atlas, &temp, 7, 0, 31);

    // Row 1: Chicken halo, Mammoths, Goats
    temp.fill(0);
    draw_chicken(&mut temp, cx, cy, 1, 0, true);
    copy_cell(&mut atlas, &temp, 0, 1, 31);

    temp.fill(0);
    draw_mammoth(&mut temp, cx, cy, 1, 0, false);
    copy_cell(&mut atlas, &temp, 1, 1, 31);

    temp.fill(0);
    draw_mammoth(&mut temp, cx, cy, 1, 1, false);
    copy_cell(&mut atlas, &temp, 2, 1, 31);

    temp.fill(0);
    draw_mammoth(&mut temp, cx, cy, -1, 0, false);
    copy_cell(&mut atlas, &temp, 3, 1, 31);

    temp.fill(0);
    draw_mammoth(&mut temp, cx, cy, -1, 1, false);
    copy_cell(&mut atlas, &temp, 4, 1, 31);

    temp.fill(0);
    draw_mammoth(&mut temp, cx, cy, 1, 0, true);
    copy_cell(&mut atlas, &temp, 5, 1, 31);

    temp.fill(0);
    draw_goat(&mut temp, cx, cy, 1);
    copy_cell(&mut atlas, &temp, 6, 1, 31);

    temp.fill(0);
    draw_goat(&mut temp, cx, cy, -1);
    copy_cell(&mut atlas, &temp, 7, 1, 31);

    // Row 2: Fire, Reticle, Hut, House, Tower, Factory
    temp.fill(0);
    draw_fire(&mut temp, cx, cy, 1.0, 0.0);
    copy_cell(&mut atlas, &temp, 0, 2, 31);

    temp.fill(0);
    draw_fire(&mut temp, cx, cy, 1.0, 0.8);
    copy_cell(&mut atlas, &temp, 1, 2, 31);

    temp.fill(0);
    draw_fire(&mut temp, cx, cy, 1.0, 1.6);
    copy_cell(&mut atlas, &temp, 2, 2, 31);

    temp.fill(0);
    fill_rect(&mut temp, cx - 4, cy, 2, 1, 255, 224, 102, 255);
    fill_rect(&mut temp, cx + 3, cy, 2, 1, 255, 224, 102, 255);
    fill_rect(&mut temp, cx, cy - 4, 1, 2, 255, 224, 102, 255);
    fill_rect(&mut temp, cx, cy + 3, 1, 2, 255, 224, 102, 255);
    put_pixel(&mut temp, cx, cy, 255, 224, 102, 255);
    copy_cell(&mut atlas, &temp, 3, 2, 16);

    temp.fill(0);
    draw_hut(&mut temp, cx, cy);
    copy_cell(&mut atlas, &temp, 4, 2, 32);

    temp.fill(0);
    draw_house(&mut temp, cx, cy, (0xb0, 0x40, 0x30));
    copy_cell(&mut atlas, &temp, 5, 2, 32);

    temp.fill(0);
    draw_tower(&mut temp, cx, cy, 24);
    copy_cell(&mut atlas, &temp, 6, 2, 32);

    temp.fill(0);
    draw_factory(&mut temp, cx, cy);
    copy_cell(&mut atlas, &temp, 7, 2, 32);

    // Row 3: Exchange, Hangar, Pad, Rocket
    temp.fill(0);
    draw_exchange(&mut temp, cx, cy);
    copy_cell(&mut atlas, &temp, 0, 3, 32);

    temp.fill(0);
    draw_hangar(&mut temp, cx, cy);
    copy_cell(&mut atlas, &temp, 1, 3, 32);

    temp.fill(0);
    draw_pad(&mut temp, cx, cy);
    copy_cell(&mut atlas, &temp, 2, 3, 32);

    temp.fill(0);
    draw_rocket(&mut temp, cx, cy, 1.0, 0.0, 1.0);
    copy_cell(&mut atlas, &temp, 3, 3, 32);

    // Tree Pine & Oak in Row 3 (cols 4, 5)
    temp.fill(0);
    draw_tree_pine(&mut temp, cx, cy);
    copy_cell(&mut atlas, &temp, 4, 3, 32);

    temp.fill(0);
    draw_tree_oak(&mut temp, cx, cy);
    copy_cell(&mut atlas, &temp, 5, 3, 32);

    // Shadows in Row 3 (cols 6, 7) & Row 4 (col 4)
    temp.fill(0);
    draw_shadow(&mut temp, cx, cy, 6, 3);
    copy_cell(&mut atlas, &temp, 6, 3, 16);

    temp.fill(0);
    draw_shadow(&mut temp, cx, cy, 10, 5);
    copy_cell(&mut atlas, &temp, 7, 3, 16);

    temp.fill(0);
    draw_shadow(&mut temp, cx, cy, 14, 7);
    copy_cell(&mut atlas, &temp, 4, 4, 16);

    // Unlit Firewood pile in Row 4 (col 0)
    temp.fill(0);
    draw_unlit_wood(&mut temp, cx, cy);
    copy_cell(&mut atlas, &temp, 0, 4, 31);

    atlas
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_create_sprite_atlas() {
        let atlas = create_sprite_atlas();
        assert_eq!(atlas.len(), ATLAS_SIZE * ATLAS_SIZE * 4);
        let has_non_zero = atlas.iter().any(|&b| b > 0);
        assert!(has_non_zero);

        let entry = get_sprite_atlas_entry(SPRITE_VILLAGER, 1, 0, 0);
        assert!(entry.u1 > entry.u0);
        assert!(entry.v1 > entry.v0);

        let tree_entry = get_sprite_atlas_entry(SPRITE_TREE, 1, 0, 0);
        assert!(tree_entry.u1 > tree_entry.u0);
        assert!(tree_entry.world_h > 0.0);

        let shadow_entry = get_sprite_atlas_entry(SPRITE_SHADOW, 1, 0, 0);
        assert!(shadow_entry.u1 > shadow_entry.u0);
        assert!(shadow_entry.world_w > 0.0);

        // Test newly added sprites
        let mut buf = vec![0u8; W * H * 4];
        draw_gantry(&mut buf, 50, 50, 20);
        draw_boat(&mut buf, 20, 20, 1, true, None);
        draw_boat(&mut buf, 30, 30, -1, false, Some((200, 200, 200)));
        draw_reticle(&mut buf, 60, 60, 1.0, (255, 255, 0));
        let bolt = make_bolt(50.0, 50.0, || 0.5);
        draw_bolt(&mut buf, &bolt, 1.0);
        assert!(buf.iter().any(|&b| b > 0));
    }
}
