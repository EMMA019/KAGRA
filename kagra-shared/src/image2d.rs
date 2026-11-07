//! Pictures for the 2D HUD: PNG art registered once, then drawn as textured rectangles
//! ([`crate::scene::Sprite2d`]) inside a [`crate::scene::Mesh2d`], in the same order as the
//! triangles around them. The GPU packs the pictures a frame uses into one RGBA atlas
//! (`render`, shader kind 3); [`crate::raster2d`] samples the same pixels on the CPU.
//!
//! A kit registers its art once (a `OnceLock` of [`PicId`]s is enough) and draws it with
//! `hud_art::Painter::image` / `nine_slice`. Pixels are straight alpha; fully transparent
//! texels take the colour of their neighbours so bilinear filtering leaves no dark fringe.

use std::sync::{Arc, Mutex};

/// A registered picture. Ids are dense and never reused for the life of the process.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct PicId(pub u32);

/// Decoded RGBA8 pixels, rows top to bottom, straight alpha.
#[derive(Debug)]
pub struct Picture {
    pub width: u32,
    pub height: u32,
    pub rgba: Vec<u8>,
}

/// Largest side kept. Bigger art is box-filtered down at registration (the atlas is
/// [`ATLAS_SIDE`] square).
pub const MAX_SIDE: u32 = 1024;
/// Side of the GPU atlas (RGBA8: 4 MB at 1024).
pub const ATLAS_SIDE: u32 = 1024;

static REGISTRY: Mutex<Vec<Arc<Picture>>> = Mutex::new(Vec::new());

/// Decode a PNG (any colour type, 8 or 16 bit, palette) and register it.
pub fn register_png(bytes: &[u8]) -> Result<PicId, String> {
    let mut decoder = png::Decoder::new(std::io::Cursor::new(bytes));
    decoder.set_transformations(png::Transformations::EXPAND | png::Transformations::STRIP_16);
    let mut reader = decoder.read_info().map_err(|e| e.to_string())?;
    let mut buf = vec![0u8; reader.output_buffer_size()];
    let info = reader.next_frame(&mut buf).map_err(|e| e.to_string())?;
    let src = &buf[..info.buffer_size()];
    let rgba: Vec<u8> = match info.color_type {
        png::ColorType::Rgba => src.to_vec(),
        png::ColorType::Rgb => src
            .as_chunks::<3>()
            .0
            .iter()
            .flat_map(|c| [c[0], c[1], c[2], 255])
            .collect(),
        png::ColorType::GrayscaleAlpha => src
            .as_chunks::<2>()
            .0
            .iter()
            .flat_map(|c| [c[0], c[0], c[0], c[1]])
            .collect(),
        png::ColorType::Grayscale => src.iter().flat_map(|&g| [g, g, g, 255]).collect(),
        png::ColorType::Indexed => return Err("palette PNG was not expanded".into()),
    };
    register_rgba(info.width, info.height, rgba)
}

/// Register raw RGBA8 pixels (straight alpha), `width * height * 4` bytes.
pub fn register_rgba(width: u32, height: u32, rgba: Vec<u8>) -> Result<PicId, String> {
    if width == 0 || height == 0 || rgba.len() != (width as usize) * (height as usize) * 4 {
        return Err(format!(
            "bad picture {width}x{height}, {} bytes",
            rgba.len()
        ));
    }
    let mut pic = Picture {
        width,
        height,
        rgba,
    };
    while pic.width > MAX_SIDE || pic.height > MAX_SIDE {
        pic = halve(&pic);
    }
    bleed(&mut pic);
    let mut reg = REGISTRY.lock().unwrap_or_else(|e| e.into_inner());
    reg.push(Arc::new(pic));
    Ok(PicId(reg.len() as u32 - 1))
}

/// Draw a list once on the CPU ([`crate::raster2d`], `w` x `h` pixels, clear background)
/// and keep it as a picture. Vector art that does not change (a crowd, a building, a
/// sheet of frames) then draws as two triangles a frame instead of thousands.
pub fn bake(list: &crate::scene::DrawList, w: u32, h: u32) -> Result<PicId, String> {
    let mut c = crate::raster2d::Canvas::new(w.max(1), h.max(1), [0, 0, 0, 0]);
    c.draw(list);
    register_rgba(c.w, c.h, c.rgba)
}

/// The pixels of a registered picture.
pub fn get(id: PicId) -> Option<Arc<Picture>> {
    let reg = REGISTRY.lock().unwrap_or_else(|e| e.into_inner());
    reg.get(id.0 as usize).cloned()
}

/// Width and height of a registered picture.
pub fn size(id: PicId) -> Option<(u32, u32)> {
    get(id).map(|p| (p.width, p.height))
}

impl Picture {
    pub fn pixel(&self, x: u32, y: u32) -> [u8; 4] {
        let i = ((y.min(self.height - 1) * self.width + x.min(self.width - 1)) * 4) as usize;
        [
            self.rgba[i],
            self.rgba[i + 1],
            self.rgba[i + 2],
            self.rgba[i + 3],
        ]
    }

    /// Bilinear sample at texel coordinates (`u`, `v` in pixels, 0.5 = first texel's centre),
    /// edges clamped, like the GPU atlas sampler on a padded slot.
    pub fn sample(&self, u: f32, v: f32) -> [f32; 4] {
        let (fx, fy) = (u - 0.5, v - 0.5);
        let (x0, y0) = (fx.floor(), fy.floor());
        let (tx, ty) = (fx - x0, fy - y0);
        let at = |x: f32, y: f32| -> [f32; 4] {
            let x = x.clamp(0.0, (self.width - 1) as f32) as u32;
            let y = y.clamp(0.0, (self.height - 1) as f32) as u32;
            self.pixel(x, y).map(|c| c as f32 / 255.0)
        };
        let (a, b) = (at(x0, y0), at(x0 + 1.0, y0));
        let (c, d) = (at(x0, y0 + 1.0), at(x0 + 1.0, y0 + 1.0));
        std::array::from_fn(|k| {
            let top = a[k] + (b[k] - a[k]) * tx;
            let bot = c[k] + (d[k] - c[k]) * tx;
            top + (bot - top) * ty
        })
    }
}

/// Half size, 2x2 box filter weighted by alpha.
fn halve(p: &Picture) -> Picture {
    let (w, h) = (p.width.div_ceil(2), p.height.div_ceil(2));
    let mut rgba = Vec::with_capacity((w * h * 4) as usize);
    for y in 0..h {
        for x in 0..w {
            let mut acc = [0.0f32; 4];
            for (dx, dy) in [(0, 0), (1, 0), (0, 1), (1, 1)] {
                let c = p.pixel(x * 2 + dx, y * 2 + dy);
                let a = c[3] as f32;
                for k in 0..3 {
                    acc[k] += c[k] as f32 * a;
                }
                acc[3] += a;
            }
            let a = acc[3].max(1.0);
            rgba.extend([
                (acc[0] / a).round() as u8,
                (acc[1] / a).round() as u8,
                (acc[2] / a).round() as u8,
                (acc[3] / 4.0).round() as u8,
            ]);
        }
    }
    Picture {
        width: w,
        height: h,
        rgba,
    }
}

/// Give fully transparent texels the average colour of their visible neighbours (a few
/// rings out, then the picture's mean), keeping alpha 0. Filtering then blends towards the
/// art's own colour instead of black.
fn bleed(p: &mut Picture) {
    let (w, h) = (p.width as i32, p.height as i32);
    let mut known: Vec<bool> = p.rgba.as_chunks::<4>().0.iter().map(|c| c[3] > 0).collect();
    if known.iter().all(|&k| k) {
        return;
    }
    for _ in 0..4 {
        let mut next = known.clone();
        let mut changed = false;
        for y in 0..h {
            for x in 0..w {
                let i = (y * w + x) as usize;
                if known[i] {
                    continue;
                }
                let mut acc = [0u32; 3];
                let mut n = 0;
                for (dx, dy) in [
                    (-1, 0),
                    (1, 0),
                    (0, -1),
                    (0, 1),
                    (-1, -1),
                    (1, 1),
                    (-1, 1),
                    (1, -1),
                ] {
                    let (nx, ny) = (x + dx, y + dy);
                    if nx < 0 || ny < 0 || nx >= w || ny >= h {
                        continue;
                    }
                    let j = (ny * w + nx) as usize;
                    if known[j] {
                        for (a, c) in acc.iter_mut().zip(&p.rgba[j * 4..j * 4 + 3]) {
                            *a += *c as u32;
                        }
                        n += 1;
                    }
                }
                if n == 0 {
                    continue;
                }
                for (c, a) in p.rgba[i * 4..i * 4 + 3].iter_mut().zip(acc) {
                    *c = a.checked_div(n).unwrap_or(0) as u8;
                }
                next[i] = true;
                changed = true;
            }
        }
        known = next;
        if !changed {
            break;
        }
    }
    let (mut acc, mut n) = ([0u64; 3], 0u64);
    for (c, k) in p.rgba.as_chunks::<4>().0.iter().zip(&known) {
        if *k {
            for i in 0..3 {
                acc[i] += c[i] as u64;
            }
            n += 1;
        }
    }
    let mean = acc.map(|a| a.checked_div(n).unwrap_or(0) as u8);
    for (c, k) in p.rgba.as_chunks_mut::<4>().0.iter_mut().zip(&known) {
        if !*k {
            c[..3].copy_from_slice(&mean);
        }
    }
}

/// Where a picture sits in a packed atlas: the uv rectangle of its pixels (inside a
/// one-texel border that repeats its edge).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Slot {
    pub uv: [f32; 4],
}

/// The atlas has no room for this picture: clear it and lay the frame out again.
#[derive(Debug, PartialEq, Eq)]
pub struct Full;

/// Shelf packing of whole pictures into an [`ATLAS_SIDE`] square, each with a one-texel
/// border copied from its edge (so a stretched picture's rim stays crisp under bilinear
/// filtering). Shared by the GPU renderer; tests use it directly.
#[derive(Debug, Default)]
pub struct Packer {
    x: u32,
    y: u32,
    row_h: u32,
    map: std::collections::HashMap<PicId, Slot>,
}

impl Packer {
    pub fn clear(&mut self) {
        *self = Self::default();
    }

    pub fn is_empty(&self) -> bool {
        self.map.is_empty()
    }

    /// The picture's slot, packing it on first use. `upload` receives the new texels
    /// (x, y, w, h, rgba bytes) to copy into the texture. `Ok(None)`: no such picture.
    pub fn get(
        &mut self,
        id: PicId,
        upload: &mut impl FnMut(u32, u32, u32, u32, &[u8]),
    ) -> Result<Option<Slot>, Full> {
        if let Some(s) = self.map.get(&id) {
            return Ok(Some(*s));
        }
        let Some(pic) = get(id) else {
            return Ok(None);
        };
        let (w, h) = (pic.width + 2, pic.height + 2);
        if w > ATLAS_SIDE || h > ATLAS_SIDE {
            return Err(Full);
        }
        if self.x + w > ATLAS_SIDE {
            self.x = 0;
            self.y += self.row_h;
            self.row_h = 0;
        }
        if self.y + h > ATLAS_SIDE {
            return Err(Full);
        }
        let (x, y) = (self.x, self.y);
        let mut bytes = Vec::with_capacity((w * h * 4) as usize);
        for py in 0..h {
            let sy = py.saturating_sub(1).min(pic.height - 1);
            for px in 0..w {
                let sx = px.saturating_sub(1).min(pic.width - 1);
                bytes.extend(pic.pixel(sx, sy));
            }
        }
        upload(x, y, w, h, &bytes);
        self.x += w;
        self.row_h = self.row_h.max(h);
        let side = ATLAS_SIDE as f32;
        let slot = Slot {
            uv: [
                (x + 1) as f32 / side,
                (y + 1) as f32 / side,
                (x + 1 + pic.width) as f32 / side,
                (y + 1 + pic.height) as f32 / side,
            ],
        };
        self.map.insert(id, slot);
        Ok(Some(slot))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn checker(w: u32, h: u32) -> Vec<u8> {
        (0..w * h)
            .flat_map(|i| {
                if (i % w + i / w).is_multiple_of(2) {
                    [255, 0, 0, 255]
                } else {
                    [0, 0, 0, 0]
                }
            })
            .collect()
    }

    #[test]
    fn transparent_texels_take_their_neighbours_colour() {
        let id = register_rgba(4, 4, checker(4, 4)).unwrap();
        let p = get(id).unwrap();
        // Every texel is red now; alpha still alternates.
        assert!(p.rgba.as_chunks::<4>().0.iter().all(|c| c[0] == 255));
        assert_eq!(p.pixel(1, 0)[3], 0);
        // Halfway between a visible and a hidden texel: red, half alpha (no dark fringe).
        let s = p.sample(1.0, 0.5);
        assert!(
            (s[0] - 1.0).abs() < 1e-3 && (s[3] - 0.5).abs() < 1e-3,
            "{s:?}"
        );
    }

    #[test]
    fn oversized_art_is_halved_and_bad_sizes_are_refused() {
        let id = register_rgba(
            MAX_SIDE * 2 + 2,
            2,
            vec![9; ((MAX_SIDE * 2 + 2) * 8) as usize],
        )
        .unwrap();
        assert_eq!(size(id), Some((MAX_SIDE / 2 + 1, 1)));
        assert!(register_rgba(2, 2, vec![0; 3]).is_err());
        assert!(register_png(b"not a png").is_err());
    }

    #[test]
    fn the_packer_places_each_picture_once_with_a_border() {
        let a = register_rgba(10, 6, checker(10, 6)).unwrap();
        let b = register_rgba(3, 3, vec![200; 36]).unwrap();
        let mut p = Packer::default();
        let mut uploads = Vec::new();
        let mut up = |x: u32, y: u32, w: u32, h: u32, bytes: &[u8]| {
            assert_eq!(bytes.len(), (w * h * 4) as usize);
            uploads.push((x, y, w, h));
        };
        let sa = p.get(a, &mut up).unwrap().unwrap();
        let again = p.get(a, &mut up).unwrap().unwrap();
        let sb = p.get(b, &mut up).unwrap().unwrap();
        assert_eq!(sa, again);
        assert!(p.get(PicId(u32::MAX), &mut up).unwrap().is_none());
        assert_eq!(uploads, vec![(0, 0, 12, 8), (12, 0, 5, 5)]);
        let side = ATLAS_SIDE as f32;
        assert_eq!(sa.uv, [1.0 / side, 1.0 / side, 11.0 / side, 7.0 / side]);
        assert!(sa.uv[2] < sb.uv[0]);
    }

    #[test]
    fn a_full_atlas_says_so() {
        let big = register_rgba(
            MAX_SIDE - 2,
            MAX_SIDE - 2,
            vec![1; ((MAX_SIDE - 2).pow(2) * 4) as usize],
        )
        .unwrap();
        let small = register_rgba(2, 2, vec![1; 16]).unwrap();
        let mut p = Packer::default();
        let mut up = |_: u32, _: u32, _: u32, _: u32, _: &[u8]| {};
        assert!(p.get(big, &mut up).unwrap().is_some());
        assert_eq!(p.get(small, &mut up), Err(Full));
        p.clear();
        assert!(p.is_empty());
        assert!(p.get(small, &mut up).unwrap().is_some());
    }

    #[test]
    fn baked_vector_art_becomes_a_picture() {
        use crate::scene::{DrawList, Quad};
        let mut list = DrawList::default();
        list.quads
            .push(Quad::new(2.0, 2.0, 4.0, 4.0, [10, 200, 30, 255]));
        let id = bake(&list, 8, 8).unwrap();
        let p = get(id).unwrap();
        assert_eq!(p.pixel(3, 3), [10, 200, 30, 255]);
        assert_eq!(p.pixel(0, 0)[3], 0, "clear outside");
        // The clear texels take the art's colour (no dark fringe when filtered).
        assert_eq!(&p.pixel(1, 3)[..3], &[10, 200, 30]);
    }

    #[test]
    fn the_games_png_art_decodes() {
        let id = register_png(include_bytes!("../../assets/diamond_days/ball.png")).unwrap();
        assert_eq!(size(id), Some((96, 96)));
    }
}
