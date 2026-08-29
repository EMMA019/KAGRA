// src/text.rs
use std::collections::HashMap;
use ab_glyph::{Font, ScaleFont, PxScale, point};

#[derive(Clone, Copy)]
pub struct GlyphMetrics {
    pub tex_id:    u32,
    pub width:     u32,
    pub height:    u32,
    pub advance:   f32,
    pub bearing_x: f32,
    pub bearing_y: f32,
}

pub struct TextRenderer {
    fonts:        HashMap<u32, ab_glyph::FontVec>,
    next_font_id: u32,
    glyph_cache:  HashMap<(u32, char, u32), GlyphMetrics>,
    textures:     HashMap<u32, wgpu::BindGroup>,
    next_tex_id:  u32,
    texture_bgl:  wgpu::BindGroupLayout,
    sampler:      wgpu::Sampler,
}

impl TextRenderer {
    pub fn new(texture_bgl: wgpu::BindGroupLayout, sampler: wgpu::Sampler) -> Self {
        TextRenderer {
            fonts:        HashMap::new(),
            next_font_id: 1,
            glyph_cache:  HashMap::new(),
            textures:     HashMap::new(),
            next_tex_id:  10_000,
            texture_bgl,
            sampler,
        }
    }

    pub fn load_font(&mut self, path: &str) -> Result<u32, String> {
        let data = std::fs::read(path)
            .map_err(|e| format!("フォント読み込み失敗: {} ({})", path, e))?;
        let font = ab_glyph::FontVec::try_from_vec(data)
            .map_err(|e| format!("フォントパース失敗: {} ({:?})", path, e))?;
        let id = self.next_font_id;
        self.next_font_id += 1;
        self.fonts.insert(id, font);
        Ok(id)
    }

    pub fn layout_text(
        &mut self,
        device:   &wgpu::Device,
        queue:    &wgpu::Queue,
        font_id:  u32,
        text:     &str,
        size_px:  u32,
        origin_x: f32,
        origin_y: f32,
    ) -> Vec<(u32, f32, f32, f32, f32)> {
        if !self.fonts.contains_key(&font_id) { return vec![]; }
        let scale    = PxScale::from(size_px as f32);
        let ascent   = {
            let f = self.fonts[&font_id].as_scaled(scale);
            f.ascent()
        };
        let baseline_y = origin_y + ascent;
        let mut cursor_x = origin_x;
        let mut result   = Vec::new();

        for ch in text.chars() {
            if ch == ' '  { cursor_x += size_px as f32 * 0.28; continue; }
            if ch == '\t' { cursor_x += size_px as f32 * 1.12; continue; }
            if ch == '\n' { continue; }

            let key = (font_id, ch, size_px);
            if !self.glyph_cache.contains_key(&key) {
                self.rasterize(device, queue, font_id, ch, size_px);
            }
            if let Some(m) = self.glyph_cache.get(&key).copied() {
                if m.tex_id > 0 && m.width > 0 {
                    let x = cursor_x + m.bearing_x;
                    let y = baseline_y - m.bearing_y;
                    result.push((m.tex_id, x, y, m.width as f32, m.height as f32));
                }
                cursor_x += m.advance;
            }
        }
        result
    }

    pub fn measure_text(
        &mut self,
        device:  &wgpu::Device,
        queue:   &wgpu::Queue,
        font_id: u32,
        text:    &str,
        size_px: u32,
    ) -> (f32, f32) {
        let cmds = self.layout_text(device, queue, font_id, text, size_px, 0.0, 0.0);
        if cmds.is_empty() { return (0.0, size_px as f32); }
        let max_x = cmds.iter().map(|(_, x, _, w, _)| x + w).fold(0.0f32, f32::max);
        let max_y = cmds.iter().map(|(_, _, y, _, h)| y + h).fold(0.0f32, f32::max);
        (max_x, max_y)
    }

    pub fn get_bind_group(&self, tex_id: u32) -> Option<&wgpu::BindGroup> {
        self.textures.get(&tex_id)
    }

    fn rasterize(
        &mut self,
        device:  &wgpu::Device,
        queue:   &wgpu::Queue,
        font_id: u32,
        ch:      char,
        size_px: u32,
    ) {
        let key   = (font_id, ch, size_px);
        let scale = PxScale::from(size_px as f32);
        let font  = &self.fonts[&font_id];
        let sf    = font.as_scaled(scale);

        let glyph_id = sf.glyph_id(ch);
        let advance  = sf.h_advance(glyph_id);
        let glyph    = glyph_id.with_scale_and_position(scale, point(0.0, 0.0));

        let outlined = match font.outline_glyph(glyph) {
            Some(g) => g,
            None => {
                self.glyph_cache.insert(key, GlyphMetrics {
                    tex_id: 0, width: 0, height: 0, advance,
                    bearing_x: 0.0, bearing_y: 0.0,
                });
                return;
            }
        };

        let bounds = outlined.px_bounds();
        let w = (bounds.max.x - bounds.min.x).ceil() as u32;
        let h = (bounds.max.y - bounds.min.y).ceil() as u32;
        if w == 0 || h == 0 { return; }

        let mut pixels = vec![0u8; (w * h * 4) as usize];
        outlined.draw(|gx, gy, v| {
            let a = (v * 255.0) as u8;
            let idx = ((gy * w + gx) * 4) as usize;
            if idx + 3 < pixels.len() {
                pixels[idx]     = 255;
                pixels[idx + 1] = 255;
                pixels[idx + 2] = 255;
                pixels[idx + 3] = a;
            }
        });

        let tex = device.create_texture(&wgpu::TextureDescriptor {
            label: Some("Glyph"),
            size:  wgpu::Extent3d { width: w, height: h, depth_or_array_layers: 1 },
            mip_level_count: 1, sample_count: 1,
            dimension: wgpu::TextureDimension::D2,
            format:    wgpu::TextureFormat::Rgba8UnormSrgb,
            usage:     wgpu::TextureUsages::TEXTURE_BINDING | wgpu::TextureUsages::COPY_DST,
            view_formats: &[],
        });
        queue.write_texture(
            wgpu::ImageCopyTexture {
                texture: &tex, mip_level: 0,
                origin: wgpu::Origin3d::ZERO,
                aspect: wgpu::TextureAspect::All,
            },
            &pixels,
            wgpu::ImageDataLayout {
                offset: 0, bytes_per_row: Some(4 * w), rows_per_image: Some(h),
            },
            wgpu::Extent3d { width: w, height: h, depth_or_array_layers: 1 },
        );

        let view = tex.create_view(&wgpu::TextureViewDescriptor::default());
        let bg = device.create_bind_group(&wgpu::BindGroupDescriptor {
            label:   Some("Glyph BG"),
            layout:  &self.texture_bgl,
            entries: &[
                wgpu::BindGroupEntry {
                    binding: 0,
                    resource: wgpu::BindingResource::TextureView(&view),
                },
                wgpu::BindGroupEntry {
                    binding: 1,
                    resource: wgpu::BindingResource::Sampler(&self.sampler),
                },
            ],
        });

        let id = self.next_tex_id;
        self.next_tex_id += 1;
        self.textures.insert(id, bg);
        self.glyph_cache.insert(key, GlyphMetrics {
            tex_id: id, width: w, height: h, advance,
            bearing_x: bounds.min.x,
            bearing_y: -bounds.min.y,
        });
    }
}