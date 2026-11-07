// Hand-placed pixel sprites rendered to a 320x180 RGBA buffer.

pub const W: usize = 320;
pub const H: usize = 180;

#[inline]
pub fn put_pixel(buf: &mut [u8], x: i32, y: i32, r: u8, g: u8, b: u8, a: u8) {
    if x < 0 || y < 0 || x >= W as i32 || y >= H as i32 {
        return;
    }
    let idx = ((y as usize) * W + (x as usize)) * 4;
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
pub fn fill_rect(buf: &mut [u8], x: i32, y: i32, w: i32, h: i32, r: u8, g: u8, b: u8, a: u8) {
    for cy in y..(y + h) {
        for cx in x..(x + w) {
            put_pixel(buf, cx, cy, r, g, b, a);
        }
    }
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
        // dead
        fill_rect(buf, x - 2, y - 1, 3, 1, shirt.0, shirt.1, shirt.2, 255);
        put_pixel(buf, x + 1, y - 1, skin.0, skin.1, skin.2, 255);
        return;
    }
    let bob = if pose == 1 { (frame % 2) as i32 } else { 0 };
    let sy = y - bob;
    fill_rect(buf, x - 1, y, 3, 1, 0, 0, 0, 64); // shadow

    if pose == 0 && (frame % 2 == 1) {
        put_pixel(buf, x - 1, sy - 1, 0x3a, 0x2b, 0x20, 255);
        put_pixel(buf, x + 1, sy - 1, 0x3a, 0x2b, 0x20, 255);
    } else {
        put_pixel(buf, x, sy - 1, 0x3a, 0x2b, 0x20, 255);
    }

    fill_rect(buf, x, sy - 3, 1, 2, shirt.0, shirt.1, shirt.2, 255);
    if pose == 1 || pose == 2 {
        let flick = if pose == 2 && frame % 2 == 1 { -1 } else { 0 };
        put_pixel(buf, x - 1, sy - 4 + flick, skin.0, skin.1, skin.2, 255);
        put_pixel(buf, x + 1, sy - 4 - flick, skin.0, skin.1, skin.2, 255);
    } else {
        put_pixel(buf, x - 1, sy - 3, shirt.0, shirt.1, shirt.2, 255);
        put_pixel(buf, x + 1, sy - 3, shirt.0, shirt.1, shirt.2, 255);
    }

    put_pixel(buf, x, sy - 4, skin.0, skin.1, skin.2, 255);
    put_pixel(buf, x, sy - 5, hair.0, hair.1, hair.2, 255);
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

pub fn draw_fire(buf: &mut [u8], x: i32, y: i32, heat: f32, t: f32) {
    fill_rect(buf, x - 2, y - 1, 5, 1, 0x5a, 0x3a, 0x22, 255);
    fill_rect(buf, x - 1, y - 2, 3, 1, 0x6e, 0x4a, 0x2c, 255);
    put_pixel(buf, x - 3, y, 0x6b, 0x6b, 0x6b, 255);
    put_pixel(buf, x + 3, y, 0x6b, 0x6b, 0x6b, 255);
    fill_rect(buf, x - 2, y + 1, 5, 1, 0x6b, 0x6b, 0x6b, 255);

    if heat <= 0.0 {
        if (t * 3.0).sin() > 0.6 {
            put_pixel(buf, x, y - 3, 0x9a, 0x9a, 0x9a, 255);
        }
        return;
    }
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
