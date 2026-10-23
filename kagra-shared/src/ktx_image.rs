//! Read-only KTX2. Pre-encoded ETC2, ASTC, and BC color blocks only.
//!
//! Basis Universal (`VK_FORMAT_UNDEFINED`) and any supercompression scheme
//! are rejected. There is no transcoder in this crate.

use ktx2::{Format, Header, LevelIndex, Reader};

/// One mip-0 color image, ready for `write_texture` when the device has the
/// matching `TEXTURE_COMPRESSION_*` feature.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct KtxImage {
    /// Vulkan `VkFormat` value. `0` is never stored.
    pub vk_format: u32,
    pub width: u32,
    pub height: u32,
    /// Uncompressed block bytes for mip 0. No supercompression.
    pub level0: Vec<u8>,
}

pub fn is_ktx2(bytes: &[u8]) -> bool {
    bytes.starts_with(&ktx2::MAGIC)
}

/// Parse a 2D, non-array, non-cubemap KTX2 whose level 0 is a color block
/// the shared renderer can sample. Mips above 0 are ignored.
pub fn parse_ktx2(bytes: &[u8]) -> Result<KtxImage, String> {
    let reader = Reader::new(bytes).map_err(|e| format!("ktx2: {e:?}"))?;
    let header = reader.header();
    if header.supercompression_scheme.is_some() {
        return Err("ktx2 supercompression is not decoded".into());
    }
    let format = header.format.ok_or_else(|| {
        "ktx2 has no Vulkan format; Basis Universal is not transcoded".to_string()
    })?;
    if header.pixel_height == 0
        || header.face_count != 1
        || header.layer_count > 1
        || header.pixel_depth > 1
    {
        return Err("ktx2 arrays, cubemaps, and 3D images are not uploaded".into());
    }
    let (bw, bh, block_bytes) = block_info(format).ok_or_else(|| {
        format!(
            "ktx2 format {} is not a pre-encoded ETC2, ASTC, or BC color block",
            format.value()
        )
    })?;
    let width = header.pixel_width;
    let height = header.pixel_height;
    if !width.is_multiple_of(bw) || !height.is_multiple_of(bh) {
        return Err("ktx2 size is not a whole number of blocks".into());
    }
    let level0 = reader
        .levels()
        .next()
        .ok_or_else(|| "ktx2 has no level".to_string())?;
    let need = (width / bw) as usize * (height / bh) as usize * block_bytes as usize;
    if level0.data.len() < need || level0.uncompressed_byte_length != level0.data.len() as u64 {
        return Err("ktx2 level 0 is shorter than its block grid".into());
    }
    Ok(KtxImage {
        vk_format: format.value(),
        width,
        height,
        level0: level0.data[..need].to_vec(),
    })
}

/// Block width, height, and byte size for a color format we will upload.
fn block_info(format: Format) -> Option<(u32, u32, u32)> {
    use Format as F;
    let bc8 = (4, 4, 8);
    let bc16 = (4, 4, 16);
    Some(match format {
        F::BC1_RGB_UNORM_BLOCK
        | F::BC1_RGB_SRGB_BLOCK
        | F::BC1_RGBA_UNORM_BLOCK
        | F::BC1_RGBA_SRGB_BLOCK => bc8,
        F::BC2_UNORM_BLOCK
        | F::BC2_SRGB_BLOCK
        | F::BC3_UNORM_BLOCK
        | F::BC3_SRGB_BLOCK
        | F::BC7_UNORM_BLOCK
        | F::BC7_SRGB_BLOCK => bc16,
        F::ETC2_R8G8B8_UNORM_BLOCK
        | F::ETC2_R8G8B8_SRGB_BLOCK
        | F::ETC2_R8G8B8A1_UNORM_BLOCK
        | F::ETC2_R8G8B8A1_SRGB_BLOCK => bc8,
        F::ETC2_R8G8B8A8_UNORM_BLOCK | F::ETC2_R8G8B8A8_SRGB_BLOCK => bc16,
        F::ASTC_4x4_UNORM_BLOCK | F::ASTC_4x4_SRGB_BLOCK => (4, 4, 16),
        F::ASTC_5x4_UNORM_BLOCK | F::ASTC_5x4_SRGB_BLOCK => (5, 4, 16),
        F::ASTC_5x5_UNORM_BLOCK | F::ASTC_5x5_SRGB_BLOCK => (5, 5, 16),
        F::ASTC_6x5_UNORM_BLOCK | F::ASTC_6x5_SRGB_BLOCK => (6, 5, 16),
        F::ASTC_6x6_UNORM_BLOCK | F::ASTC_6x6_SRGB_BLOCK => (6, 6, 16),
        F::ASTC_8x5_UNORM_BLOCK | F::ASTC_8x5_SRGB_BLOCK => (8, 5, 16),
        F::ASTC_8x6_UNORM_BLOCK | F::ASTC_8x6_SRGB_BLOCK => (8, 6, 16),
        F::ASTC_8x8_UNORM_BLOCK | F::ASTC_8x8_SRGB_BLOCK => (8, 8, 16),
        F::ASTC_10x5_UNORM_BLOCK | F::ASTC_10x5_SRGB_BLOCK => (10, 5, 16),
        F::ASTC_10x6_UNORM_BLOCK | F::ASTC_10x6_SRGB_BLOCK => (10, 6, 16),
        F::ASTC_10x8_UNORM_BLOCK | F::ASTC_10x8_SRGB_BLOCK => (10, 8, 16),
        F::ASTC_10x10_UNORM_BLOCK | F::ASTC_10x10_SRGB_BLOCK => (10, 10, 16),
        F::ASTC_12x10_UNORM_BLOCK | F::ASTC_12x10_SRGB_BLOCK => (12, 10, 16),
        F::ASTC_12x12_UNORM_BLOCK | F::ASTC_12x12_SRGB_BLOCK => (12, 12, 16),
        _ => return None,
    })
}

/// One mip, no supercompression, empty DFD. Tests and fixtures build files
/// with this. The runtime does not encode.
pub fn write_uncompressed_ktx2(
    format: Format,
    width: u32,
    height: u32,
    level0: &[u8],
) -> Result<Vec<u8>, String> {
    let (bw, bh, block_bytes) =
        block_info(format).ok_or_else(|| "unsupported ktx2 format".to_string())?;
    if width == 0 || height == 0 || !width.is_multiple_of(bw) || !height.is_multiple_of(bh) {
        return Err("ktx2 size is not a whole number of blocks".into());
    }
    let need = (width / bw) as usize * (height / bh) as usize * block_bytes as usize;
    if level0.len() != need {
        return Err("ktx2 level length does not match the block grid".into());
    }
    let dfd_offset = (Header::LENGTH + LevelIndex::LENGTH) as u32;
    let level_offset = (dfd_offset + 4) as u64;
    let header = Header {
        format: Some(format),
        type_size: 1,
        pixel_width: width,
        pixel_height: height,
        pixel_depth: 0,
        layer_count: 0,
        face_count: 1,
        level_count: 1,
        supercompression_scheme: None,
        index: ktx2::Index {
            dfd_byte_offset: dfd_offset,
            dfd_byte_length: 4,
            kvd_byte_offset: 0,
            kvd_byte_length: 0,
            sgd_byte_offset: 0,
            sgd_byte_length: 0,
        },
    };
    let level = LevelIndex {
        byte_offset: level_offset,
        byte_length: level0.len() as u64,
        uncompressed_byte_length: level0.len() as u64,
    };
    let mut out = Vec::with_capacity(level_offset as usize + level0.len());
    out.extend_from_slice(&header.as_bytes());
    out.extend_from_slice(&level.as_bytes());
    out.extend_from_slice(&4u32.to_le_bytes());
    out.extend_from_slice(level0);
    Ok(out)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn bc1_red() -> Vec<u8> {
        // color0 = RGB565 red (0xF800 LE), color1 = black, indices select color0.
        write_uncompressed_ktx2(
            Format::BC1_RGBA_SRGB_BLOCK,
            4,
            4,
            &[0x00, 0xF8, 0x00, 0x00, 0, 0, 0, 0],
        )
        .unwrap()
    }

    #[test]
    fn bc1_srgb_red_roundtrips() {
        let bytes = bc1_red();
        let img = parse_ktx2(&bytes).unwrap();
        assert_eq!(img.vk_format, Format::BC1_RGBA_SRGB_BLOCK.value());
        assert_eq!((img.width, img.height), (4, 4));
        assert_eq!(img.level0.len(), 8);
    }

    #[test]
    fn etc2_and_astc_headers_parse() {
        let etc =
            write_uncompressed_ktx2(Format::ETC2_R8G8B8A8_SRGB_BLOCK, 4, 4, &[0x11; 16]).unwrap();
        let astc =
            write_uncompressed_ktx2(Format::ASTC_4x4_UNORM_BLOCK, 4, 4, &[0x22; 16]).unwrap();
        assert_eq!(
            parse_ktx2(&etc).unwrap().vk_format,
            Format::ETC2_R8G8B8A8_SRGB_BLOCK.value()
        );
        assert_eq!(
            parse_ktx2(&astc).unwrap().vk_format,
            Format::ASTC_4x4_UNORM_BLOCK.value()
        );
    }

    #[test]
    fn basis_and_supercompression_are_rejected() {
        let mut bytes = bc1_red();
        bytes[12..16].copy_from_slice(&0u32.to_le_bytes());
        assert!(parse_ktx2(&bytes).unwrap_err().contains("Basis"));
        let mut zstd = bc1_red();
        zstd[44..48].copy_from_slice(&2u32.to_le_bytes());
        assert!(parse_ktx2(&zstd).unwrap_err().contains("supercompression"));
        let bc4 = write_uncompressed_ktx2(Format::BC4_UNORM_BLOCK, 4, 4, &[0; 8]);
        assert!(bc4.is_err(), "BC4 is not a color block");
    }
}
