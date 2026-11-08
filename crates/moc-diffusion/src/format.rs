//! Reader for the `.mocd` weights container (writer: `rl/terrain_diffusion/mocd.py`).
//!
//! `b"MOCD"`, u32 version (1), u32 header length, JSON header, zero padding to 16 bytes,
//! then the data block; each tensor is f16 or f32, little-endian, at a 16-byte aligned
//! offset relative to the data block.

use crate::json::{self, Json};
use std::collections::BTreeMap;
use std::fmt;

#[derive(Clone, Debug, PartialEq)]
pub enum LoadError {
    Truncated,
    BadMagic,
    Version(u32),
    Header(String),
    Tensor(String),
    Arch(String),
}

impl fmt::Display for LoadError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            LoadError::Truncated => write!(f, "file is truncated"),
            LoadError::BadMagic => write!(f, "not a .mocd file"),
            LoadError::Version(v) => write!(f, "unsupported .mocd version {v}"),
            LoadError::Header(e) => write!(f, "bad header: {e}"),
            LoadError::Tensor(e) => write!(f, "bad tensor: {e}"),
            LoadError::Arch(e) => write!(f, "unsupported architecture: {e}"),
        }
    }
}

impl std::error::Error for LoadError {}

#[derive(Clone, Debug)]
pub struct Tensor {
    pub shape: Vec<usize>,
    pub data: Vec<f32>,
}

#[derive(Clone, Debug)]
pub struct Mocd {
    pub header: Json,
    pub tensors: BTreeMap<String, Tensor>,
}

impl Mocd {
    pub fn tensor(&self, name: &str) -> Result<&Tensor, LoadError> {
        self.tensors.get(name).ok_or_else(|| LoadError::Tensor(format!("missing '{name}'")))
    }
}

fn align16(n: usize) -> usize {
    (n + 15) & !15
}

fn u32_at(b: &[u8], at: usize) -> Result<u32, LoadError> {
    let s = b.get(at..at + 4).ok_or(LoadError::Truncated)?;
    Ok(u32::from_le_bytes([s[0], s[1], s[2], s[3]]))
}

/// IEEE 754 binary16 -> f32 (handles subnormals, inf and NaN).
pub fn f16_to_f32(h: u16) -> f32 {
    let sign = ((h as u32) & 0x8000) << 16;
    let exp = ((h >> 10) & 0x1f) as u32;
    let man = (h & 0x3ff) as u32;
    let bits = if exp == 0 {
        if man == 0 {
            sign
        } else {
            // Subnormal: renormalise.
            let mut e: i32 = -14;
            let mut m = man;
            while m & 0x400 == 0 {
                m <<= 1;
                e -= 1;
            }
            sign | (((e + 127) as u32) << 23) | ((m & 0x3ff) << 13)
        }
    } else if exp == 31 {
        sign | 0x7f80_0000 | (man << 13)
    } else {
        sign | ((exp + 127 - 15) << 23) | (man << 13)
    };
    f32::from_bits(bits)
}

pub fn parse(bytes: &[u8]) -> Result<Mocd, LoadError> {
    if bytes.len() < 12 {
        return Err(LoadError::Truncated);
    }
    if &bytes[0..4] != b"MOCD" {
        return Err(LoadError::BadMagic);
    }
    let ver = u32_at(bytes, 4)?;
    if ver != 1 {
        return Err(LoadError::Version(ver));
    }
    let hl = u32_at(bytes, 8)? as usize;
    let hb = bytes.get(12..12 + hl).ok_or(LoadError::Truncated)?;
    let hs = std::str::from_utf8(hb).map_err(|e| LoadError::Header(e.to_string()))?;
    let header = json::parse(hs).map_err(LoadError::Header)?;
    let start = align16(12 + hl);
    let table = header
        .get("tensors")
        .and_then(Json::as_arr)
        .ok_or_else(|| LoadError::Header("no tensor table".into()))?;
    let mut tensors = BTreeMap::new();
    for t in table {
        let name = t.get("name").and_then(Json::as_str).ok_or_else(|| LoadError::Header("tensor without name".into()))?;
        let shape: Vec<usize> = t
            .get("shape")
            .and_then(Json::as_arr)
            .ok_or_else(|| LoadError::Header(format!("{name}: no shape")))?
            .iter()
            .map(|d| d.as_usize().ok_or_else(|| LoadError::Header(format!("{name}: bad shape"))))
            .collect::<Result<_, _>>()?;
        let n: usize = shape.iter().product();
        let off = t.get("offset").and_then(Json::as_usize).ok_or_else(|| LoadError::Header(format!("{name}: no offset")))?;
        let dtype = t.get("dtype").and_then(Json::as_str).unwrap_or("f32");
        let width = match dtype {
            "f16" => 2,
            "f32" => 4,
            other => return Err(LoadError::Tensor(format!("{name}: dtype {other}"))),
        };
        let raw = bytes.get(start + off..start + off + n * width).ok_or(LoadError::Truncated)?;
        let data: Vec<f32> = if width == 2 {
            raw.chunks_exact(2).map(|c| f16_to_f32(u16::from_le_bytes([c[0], c[1]]))).collect()
        } else {
            raw.chunks_exact(4).map(|c| f32::from_le_bytes([c[0], c[1], c[2], c[3]])).collect()
        };
        if data.iter().any(|x| !x.is_finite()) {
            return Err(LoadError::Tensor(format!("{name}: non-finite values")));
        }
        tensors.insert(name.to_string(), Tensor { shape, data });
    }
    Ok(Mocd { header, tensors })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn f16_widening_matches_reference_values() {
        assert_eq!(f16_to_f32(0x3c00), 1.0);
        assert_eq!(f16_to_f32(0xc000), -2.0);
        assert_eq!(f16_to_f32(0x7bff), 65504.0);
        assert_eq!(f16_to_f32(0x0001), 5.960_464_5e-8); // smallest subnormal
        assert_eq!(f16_to_f32(0x3555), 0.333_251_95);
        assert!(f16_to_f32(0x7c00).is_infinite());
        assert_eq!(f16_to_f32(0x8000).to_bits(), (-0.0f32).to_bits());
    }

    #[test]
    fn rejects_garbage() {
        assert_eq!(parse(b"nope").unwrap_err(), LoadError::Truncated);
        assert_eq!(parse(b"NOPE00000000").unwrap_err(), LoadError::BadMagic);
        let mut b = b"MOCD".to_vec();
        b.extend_from_slice(&2u32.to_le_bytes());
        b.extend_from_slice(&0u32.to_le_bytes());
        assert_eq!(parse(&b).unwrap_err(), LoadError::Version(2));
    }
}
