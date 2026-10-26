//! The HUD's glyph atlas: each (character, raster size, face) is rasterized once into one
//! R8 texture, and a line of text draws as one textured quad per glyph instead of a quad per
//! covered pixel. Shelf packing; when the texture is full it starts over (`clear`).

use std::collections::HashMap;

use crate::font::{glyph_coverage, Coverage};
use crate::scene::TextFace;

/// Side of the square atlas texture (R8: 4 MB).
pub(crate) const ATLAS_SIDE: u32 = 2048;
/// Empty texels between glyphs, so bilinear sampling does not bleed a neighbour in.
const PAD: u32 = 1;

/// Where a glyph sits in the atlas, and its offset from the pen (in raster pixels).
#[derive(Clone, Copy, Debug, PartialEq)]
pub(crate) struct Slot {
    pub uv: [f32; 4],
    pub w: u32,
    pub h: u32,
    pub dx: f32,
    pub dy: f32,
}

/// The atlas has no room left for this glyph: `clear` it and lay the frame out again.
#[derive(Debug, PartialEq, Eq)]
pub(crate) struct Full;

#[derive(Debug, Default)]
pub(crate) struct Packer {
    x: u32,
    y: u32,
    row_h: u32,
    /// `None`: the face has no outline for it (a space, a missing glyph); nothing to draw.
    map: HashMap<(char, u32, TextFace), Option<Slot>>,
}

impl Packer {
    pub(crate) fn clear(&mut self) {
        *self = Self::default();
    }

    /// The glyph's slot, rasterizing and placing it on first use. `upload` receives the new
    /// texels (x, y, w, h, bytes) to copy into the texture.
    pub(crate) fn get(
        &mut self,
        ch: char,
        px: u32,
        face: TextFace,
        upload: &mut impl FnMut(u32, u32, u32, u32, &[u8]),
    ) -> Result<Option<Slot>, Full> {
        if let Some(s) = self.map.get(&(ch, px, face)) {
            return Ok(*s);
        }
        let Some(cov) = glyph_coverage(ch, px, face) else {
            self.map.insert((ch, px, face), None);
            return Ok(None);
        };
        let slot = self.place(&cov, upload)?;
        self.map.insert((ch, px, face), Some(slot));
        Ok(Some(slot))
    }

    fn place(
        &mut self,
        cov: &Coverage,
        upload: &mut impl FnMut(u32, u32, u32, u32, &[u8]),
    ) -> Result<Slot, Full> {
        let (w, h) = (cov.w, cov.h);
        if w + PAD > ATLAS_SIDE || h + PAD > ATLAS_SIDE {
            return Err(Full);
        }
        if self.x + w + PAD > ATLAS_SIDE {
            self.x = 0;
            self.y += self.row_h;
            self.row_h = 0;
        }
        if self.y + h + PAD > ATLAS_SIDE {
            return Err(Full);
        }
        let (x, y) = (self.x, self.y);
        let bytes: Vec<u8> = cov
            .pixels
            .iter()
            .map(|a| (a.clamp(0.0, 1.0) * 255.0).round() as u8)
            .collect();
        upload(x, y, w, h, &bytes);
        self.x += w + PAD;
        self.row_h = self.row_h.max(h + PAD);
        let side = ATLAS_SIDE as f32;
        Ok(Slot {
            uv: [
                x as f32 / side,
                y as f32 / side,
                (x + w) as f32 / side,
                (y + h) as f32 / side,
            ],
            w,
            h,
            dx: cov.dx,
            dy: cov.dy,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_glyph_is_placed_once_and_spaces_take_no_room() {
        let mut p = Packer::default();
        let uploads = std::cell::Cell::new(0);
        let mut up = |_: u32, _: u32, w: u32, h: u32, b: &[u8]| {
            assert_eq!(b.len(), (w * h) as usize);
            uploads.set(uploads.get() + 1);
        };
        let a = p.get('A', 48, TextFace::Ui, &mut up).unwrap().unwrap();
        let again = p.get('A', 48, TextFace::Ui, &mut up).unwrap().unwrap();
        assert_eq!(a, again);
        assert!(p.get(' ', 48, TextFace::Ui, &mut up).unwrap().is_none());
        let b = p.get('弾', 48, TextFace::Ui, &mut up).unwrap().unwrap();
        assert_eq!(uploads.get(), 2, "A and 弾 rasterized once each");
        assert!(a.uv[2] <= b.uv[0] || a.uv[3] <= b.uv[1], "no overlap");
        let d = p.get('7', 48, TextFace::Display, &mut up).unwrap().unwrap();
        assert_ne!(d, p.get('7', 48, TextFace::Ui, &mut up).unwrap().unwrap());
    }

    #[test]
    fn a_full_atlas_says_so_and_starts_over() {
        let mut p = Packer::default();
        let mut up = |_: u32, _: u32, _: u32, _: u32, _: &[u8]| {};
        let mut full = false;
        'fill: for px in [160u32, 128, 96] {
            for c in 0x4E00u32..0x4E00 + 3000 {
                let ch = char::from_u32(c).unwrap();
                if p.get(ch, px, TextFace::Ui, &mut up) == Err(Full) {
                    full = true;
                    break 'fill;
                }
            }
        }
        assert!(full, "the atlas fills up");
        p.clear();
        assert!(p.get('A', 48, TextFace::Ui, &mut up).unwrap().is_some());
    }
}
