//! PSD (Adobe Photoshop) file format writer.
//!
//! Minimal-but-complete encoder: writes a header, an empty color-mode
//! and image-resources section, a layer & mask info block (each
//! layer is an RGBA image at a canvas-coord bounding box, blend mode
//! "normal", opacity 255, no mask, no extras), then a flat merged
//! composite as the fallback preview for non-Photoshop tools.
//!
//! No RLE / no ZIP compression — channel data is written raw. For
//! the puzzle export use case the simpler writer trades file size
//! for ~3× less code; Photoshop happily re-saves the file with RLE
//! enabled on first save if the artist cares.
//!
//! Reference: Adobe TN21080 "Adobe Photoshop File Formats
//! Specification". Big-endian everywhere; layer info length must be
//! rounded up to a multiple of 2; layer name is a Pascal string
//! whose entire field (length byte + chars) pads to a multiple of 4.

use anyhow::{Context, Result};
use image::RgbaImage;
use std::io::Write;

/// One PSD layer.
pub struct PsdLayer {
    /// Layer name shown in Photoshop's Layers panel. Truncated to
    /// 255 ASCII bytes (the Pascal-string limit).
    pub name: String,
    /// `(top, left, bottom, right)` in canvas-coord pixels.
    /// `bottom - top` must equal `image.height()`, `right - left`
    /// must equal `image.width()`. The encoder asserts this.
    pub rect: (u32, u32, u32, u32),
    pub image: RgbaImage,
}

/// Write a PSD file. `merged` is the visible flat preview baked into
/// the file for tools that don't read the layer block (also: the
/// thumbnail OSes use in file pickers). Typically the same image the
/// editor shows the artist; Photoshop overwrites it on first save.
pub fn write_psd<W: Write>(
    out: &mut W,
    canvas_width: u32,
    canvas_height: u32,
    layers: &[PsdLayer],
    merged: &RgbaImage,
) -> Result<()> {
    if canvas_width == 0 || canvas_height == 0 {
        anyhow::bail!("canvas dimensions must be non-zero");
    }
    if canvas_width > 30_000 || canvas_height > 30_000 {
        // PSD v1 limit; PSB extends further but isn't implemented here.
        anyhow::bail!(
            "canvas {}×{} exceeds PSD v1 30 000 px max — would need PSB",
            canvas_width,
            canvas_height
        );
    }
    if merged.width() != canvas_width || merged.height() != canvas_height {
        anyhow::bail!(
            "merged preview is {}×{} but canvas is {}×{}",
            merged.width(),
            merged.height(),
            canvas_width,
            canvas_height
        );
    }
    for (i, layer) in layers.iter().enumerate() {
        let (t, l, b, r) = layer.rect;
        if b <= t || r <= l {
            anyhow::bail!("layer {i} ({}) has degenerate rect {:?}", layer.name, layer.rect);
        }
        if r > canvas_width || b > canvas_height {
            anyhow::bail!(
                "layer {i} ({}) rect {:?} extends past canvas {}×{}",
                layer.name,
                layer.rect,
                canvas_width,
                canvas_height
            );
        }
        if layer.image.width() != r - l || layer.image.height() != b - t {
            anyhow::bail!(
                "layer {i} ({}) image is {}×{} but rect implies {}×{}",
                layer.name,
                layer.image.width(),
                layer.image.height(),
                r - l,
                b - t
            );
        }
    }

    write_header(out, canvas_width, canvas_height).context("writing PSD header")?;
    write_be_u32(out, 0).context("writing color-mode-data length")?; // empty
    write_be_u32(out, 0).context("writing image-resources length")?; // empty
    write_layer_and_mask_info(out, layers).context("writing layer info block")?;
    write_merged_image(out, merged).context("writing merged image data")?;
    Ok(())
}

// ── primitive writers ───────────────────────────────────────────────

fn write_be_u16<W: Write>(out: &mut W, v: u16) -> Result<()> {
    out.write_all(&v.to_be_bytes())?;
    Ok(())
}

fn write_be_u32<W: Write>(out: &mut W, v: u32) -> Result<()> {
    out.write_all(&v.to_be_bytes())?;
    Ok(())
}

fn write_be_i16<W: Write>(out: &mut W, v: i16) -> Result<()> {
    out.write_all(&v.to_be_bytes())?;
    Ok(())
}

/// Pascal string: 1-byte length followed by ASCII bytes, then enough
/// zero bytes that the *whole* field (length byte + chars + pad) is a
/// multiple of 4. The PSD spec calls this "PascalString" with
/// "padded to a multiple of 4 bytes".
fn write_pascal_string_padded4<W: Write>(out: &mut W, s: &str) -> Result<()> {
    let raw = s.as_bytes();
    let len = raw.len().min(255);
    out.write_all(&[len as u8])?;
    out.write_all(&raw[..len])?;
    let total = 1 + len;
    let pad = (4 - (total % 4)) % 4;
    for _ in 0..pad {
        out.write_all(&[0])?;
    }
    Ok(())
}

// ── section writers ─────────────────────────────────────────────────

fn write_header<W: Write>(out: &mut W, w: u32, h: u32) -> Result<()> {
    out.write_all(b"8BPS")?;
    write_be_u16(out, 1)?; // version 1 = PSD (PSB would be 2)
    out.write_all(&[0u8; 6])?;
    write_be_u16(out, 4)?; // channels: RGBA
    write_be_u32(out, h)?;
    write_be_u32(out, w)?;
    write_be_u16(out, 8)?; // depth
    write_be_u16(out, 3)?; // color mode 3 = RGB
    Ok(())
}

fn write_layer_and_mask_info<W: Write>(out: &mut W, layers: &[PsdLayer]) -> Result<()> {
    // Build the layer info subsection in a scratch buffer so we know
    // its length up front (the encoded length must precede the data).
    let mut layer_info: Vec<u8> = Vec::new();
    write_be_u16(&mut layer_info, layers.len() as u16)?;

    // ── layer records ───────────────────────────────────────────────
    for layer in layers {
        let (top, left, bottom, right) = layer.rect;
        let lw = right - left;
        let lh = bottom - top;
        // Per-channel image data = 2-byte compression header + raw plane.
        let channel_bytes = 2 + (lw as usize) * (lh as usize);

        write_be_u32(&mut layer_info, top)?;
        write_be_u32(&mut layer_info, left)?;
        write_be_u32(&mut layer_info, bottom)?;
        write_be_u32(&mut layer_info, right)?;
        write_be_u16(&mut layer_info, 4)?; // channel count

        // Channel info: R, G, B, alpha (PSD uses -1 for the alpha mask).
        for &ch_id in &[0_i16, 1, 2, -1] {
            write_be_i16(&mut layer_info, ch_id)?;
            write_be_u32(&mut layer_info, channel_bytes as u32)?;
        }

        layer_info.extend_from_slice(b"8BIM"); // blend signature
        layer_info.extend_from_slice(b"norm"); // blend mode key
        layer_info.push(255); // opacity
        layer_info.push(0); // clipping (0 = base)
        layer_info.push(0); // flags (0 = visible, no special bits)
        layer_info.push(0); // filler

        // Extra block: layer mask length + blending ranges length +
        // layer name. Length-prefixed so older readers can skip.
        let mut extra: Vec<u8> = Vec::new();
        write_be_u32(&mut extra, 0)?; // layer mask data length (none)
        write_be_u32(&mut extra, 0)?; // blending ranges length (none)
        write_pascal_string_padded4(&mut extra, &layer.name)?;
        write_be_u32(&mut layer_info, extra.len() as u32)?;
        layer_info.extend_from_slice(&extra);
    }

    // ── channel image data (one block per layer, per channel) ───────
    for layer in layers {
        let raw = layer.image.as_raw(); // tightly packed R,G,B,A,R,G,B,A,...
        for channel_idx in 0..4_usize {
            write_be_u16(&mut layer_info, 0)?; // compression = raw
            // Pull this channel's plane out of the interleaved buffer.
            // For typical NY canvases at 300 DPI this is tens of MB
            // per channel; the allocation is one-shot.
            let plane: Vec<u8> = raw
                .iter()
                .skip(channel_idx)
                .step_by(4)
                .copied()
                .collect();
            layer_info.write_all(&plane)?;
        }
    }

    // Spec: "Length of the layers info section, rounded up to a
    // multiple of 2." Pad if odd, then encode the (now even) length.
    if layer_info.len() % 2 != 0 {
        layer_info.push(0);
    }

    // Section wrapper:
    //   u32 section length = 4 (layer-info-length field) + layer_info + 4 (global mask length)
    //   u32 layer info length
    //   ...layer info...
    //   u32 global mask info length = 0
    let section_len = 4 + layer_info.len() + 4;
    write_be_u32(out, section_len as u32)?;
    write_be_u32(out, layer_info.len() as u32)?;
    out.write_all(&layer_info)?;
    write_be_u32(out, 0)?; // global mask info length

    Ok(())
}

fn write_merged_image<W: Write>(out: &mut W, img: &RgbaImage) -> Result<()> {
    write_be_u16(out, 0)?; // compression = raw
    let raw = img.as_raw();
    for channel_idx in 0..4_usize {
        let plane: Vec<u8> = raw.iter().skip(channel_idx).step_by(4).copied().collect();
        out.write_all(&plane)?;
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use image::Rgba;

    #[test]
    fn header_signature_and_dimensions() {
        let img = RgbaImage::from_pixel(4, 3, Rgba([0, 0, 0, 0]));
        let mut buf = Vec::new();
        write_psd(&mut buf, 4, 3, &[], &img).unwrap();
        assert_eq!(&buf[0..4], b"8BPS");
        assert_eq!(&buf[4..6], &[0, 1]); // version 1
        assert_eq!(&buf[12..14], &[0, 4]); // 4 channels
        assert_eq!(&buf[14..18], &[0, 0, 0, 3]); // height
        assert_eq!(&buf[18..22], &[0, 0, 0, 4]); // width
        assert_eq!(&buf[22..24], &[0, 8]); // depth
        assert_eq!(&buf[24..26], &[0, 3]); // RGB color mode
    }

    #[test]
    fn empty_layer_block_when_no_layers_provided() {
        let img = RgbaImage::from_pixel(2, 2, Rgba([255, 255, 255, 255]));
        let mut buf = Vec::new();
        write_psd(&mut buf, 2, 2, &[], &img).unwrap();
        // After header (26) + color-mode-len (4) + image-resources-len (4)
        // we expect the layer&mask section length, then layer-info-length=2
        // (just the u16 zero layer count), then the count itself.
        let lm_section_len = u32::from_be_bytes([buf[34], buf[35], buf[36], buf[37]]);
        // section = 4 (layer info length field) + 2 (count, no records) + 4 (global mask len)
        assert_eq!(lm_section_len, 10);
    }

    #[test]
    fn one_layer_roundtrips_channel_data() {
        // Layer is a 2×1 with R=0x11, G=0x22, B=0x33, A=0x44 in pixel 0
        // and R=0x55, G=0x66, B=0x77, A=0x88 in pixel 1.
        let mut layer_img = RgbaImage::new(2, 1);
        layer_img.put_pixel(0, 0, Rgba([0x11, 0x22, 0x33, 0x44]));
        layer_img.put_pixel(1, 0, Rgba([0x55, 0x66, 0x77, 0x88]));

        let layer = PsdLayer {
            name: "test".to_string(),
            rect: (0, 0, 1, 2),
            image: layer_img,
        };
        let merged = RgbaImage::from_pixel(2, 1, Rgba([0, 0, 0, 0]));
        let mut buf = Vec::new();
        write_psd(&mut buf, 2, 1, &[layer], &merged).unwrap();

        // The R plane appears as 0x11 0x55, G as 0x22 0x66, etc. —
        // search for the unique RG pattern to confirm channels are
        // emitted as planes, not interleaved.
        let has_r_plane = buf.windows(2).any(|w| w == [0x11, 0x55]);
        let has_g_plane = buf.windows(2).any(|w| w == [0x22, 0x66]);
        let has_b_plane = buf.windows(2).any(|w| w == [0x33, 0x77]);
        let has_a_plane = buf.windows(2).any(|w| w == [0x44, 0x88]);
        assert!(has_r_plane && has_g_plane && has_b_plane && has_a_plane,
            "channels should be encoded as planes, not interleaved");
    }
}
