//! The arithmetic this crate owns that needs no window: title matching, client-region→screen
//! mapping, and BGRA→PNG encoding. Everything here is testable on any machine; the Win32 calls are
//! `window.rs`'s and only appear in its `#[ignore]`d live tests.

use seal_domain::geometry::Rect;

use crate::error::WinError;

/// Does a window title match the configured title? Case-insensitive CONTAINS: the game's title bar
/// carries a version suffix, and v2 matched it the same way. An empty needle matches nothing (an
/// empty configured title is a config error, not "match every window").
#[must_use]
pub fn title_matches(candidate: &str, needle: &str) -> bool {
    if needle.trim().is_empty() {
        return false;
    }
    candidate.to_lowercase().contains(&needle.to_lowercase())
}

/// Map a client-relative physical rect to a screen-space physical rect, refusing anything that does
/// not fit inside the client area. A region with a non-positive size, or one whose origin/size runs
/// past the client, is a bug in the caller (or a stale display measurement) and must not become a
/// silent empty capture.
///
/// # Errors
///
/// Returns [`WinError::RegionOutsideClient`] when the region has a non-positive size, a negative
/// origin, or an edge past the client area.
pub fn region_to_screen(physical_client: Rect, region: Rect) -> Result<Rect, WinError> {
    if region.w <= 0 || region.h <= 0 || region.x < 0 || region.y < 0 {
        return Err(WinError::RegionOutsideClient);
    }
    if region.right() > physical_client.w || region.bottom() > physical_client.h {
        return Err(WinError::RegionOutsideClient);
    }
    Ok(Rect::new(
        physical_client.x + region.x,
        physical_client.y + region.y,
        region.w,
        region.h,
    ))
}

/// BGRA (bottom-up rows already flipped by the caller) → PNG bytes, 8-bit RGBA.
///
/// The bitmaps Win32 hands back are BGRA in memory; PNG is RGBA, so every pixel's first and third
/// bytes swap. Doing it here rather than in `window.rs` keeps the encoder testable without a screen.
///
/// # Errors
///
/// Returns [`WinError::EmptyCapture`] when either dimension is zero, when `bgra` is not exactly
/// `width * height * 4` bytes, or when the PNG encoder refuses the frame.
pub fn encode_png(width: u32, height: u32, bgra: &[u8]) -> Result<Vec<u8>, WinError> {
    let expected = width as usize * height as usize * 4;
    if width == 0 || height == 0 || bgra.len() != expected {
        return Err(WinError::EmptyCapture);
    }

    let mut rgba = Vec::with_capacity(bgra.len());
    let (pixels, _tail) = bgra.as_chunks::<4>();
    for pixel in pixels {
        rgba.extend_from_slice(&[pixel[2], pixel[1], pixel[0], pixel[3]]);
    }

    let mut out = Vec::new();
    {
        let mut encoder = png::Encoder::new(&mut out, width, height);
        encoder.set_color(png::ColorType::Rgba);
        encoder.set_depth(png::BitDepth::Eight);
        let mut writer = encoder.write_header().map_err(|_| WinError::EmptyCapture)?;
        writer
            .write_image_data(&rgba)
            .map_err(|_| WinError::EmptyCapture)?;
    }
    Ok(out)
}

/// PNG bytes → (width, height, RGBA8 pixels). The counterpart of [`encode_png`], used by the OCR
/// pipeline to turn a cached frame or an inline capture back into pixels.
///
/// The `png` crate already decodes all the colour types a Windows capture can produce; this only
/// normalises them to the one the pipeline accepts (RGBA8), so there is no second image dependency
/// in the core.
///
/// # Errors
///
/// Returns [`WinError::EmptyCapture`] when the bytes are not a decodable PNG, when the frame is
/// zero-sized, or when its colour type cannot be normalised to RGBA8.
pub fn decode_png(bytes: &[u8]) -> Result<(u32, u32, Vec<u8>), WinError> {
    let mut decoder = png::Decoder::new(bytes);
    decoder.set_transformations(png::Transformations::normalize_to_color8());
    let mut reader = decoder.read_info().map_err(|_| WinError::EmptyCapture)?;
    let mut buf = vec![0; reader.output_buffer_size()];
    let info = reader
        .next_frame(&mut buf)
        .map_err(|_| WinError::EmptyCapture)?;
    let (width, height) = (info.width, info.height);
    let source = &buf[..info.buffer_size()];
    let rgba = match info.color_type {
        png::ColorType::Rgba => source.to_vec(),
        png::ColorType::Rgb => {
            let (pixels, _) = source.as_chunks::<3>();
            let mut rgba = Vec::with_capacity(pixels.len() * 4);
            for pixel in pixels {
                rgba.extend_from_slice(&[pixel[0], pixel[1], pixel[2], 255]);
            }
            rgba
        }
        png::ColorType::Grayscale => source
            .iter()
            .flat_map(|value| [*value, *value, *value, 255])
            .collect(),
        png::ColorType::GrayscaleAlpha => {
            let (pixels, _) = source.as_chunks::<2>();
            let mut rgba = Vec::with_capacity(pixels.len() * 4);
            for pixel in pixels {
                rgba.extend_from_slice(&[pixel[0], pixel[0], pixel[0], pixel[1]]);
            }
            rgba
        }
        png::ColorType::Indexed => return Err(WinError::EmptyCapture),
    };
    if width == 0 || height == 0 || rgba.len() != width as usize * height as usize * 4 {
        return Err(WinError::EmptyCapture);
    }
    Ok((width, height, rgba))
}

/// Crop an RGBA image to `region`, which must fit inside it. Pure: the calibrator's crop is
/// arithmetic on pixels the capture already produced, so it is testable with a hand-built buffer.
///
/// # Errors
///
/// Returns [`WinError::EmptyCapture`] when the image has a zero dimension or `rgba` is shorter than
/// `width * height * 4` bytes, and [`WinError::RegionOutsideClient`] when the crop does not lie
/// inside the image.
pub fn crop_rgba(
    width: u32,
    height: u32,
    rgba: &[u8],
    region: Rect,
) -> Result<(u32, u32, Vec<u8>), WinError> {
    let width = width as usize;
    let height = height as usize;
    if width == 0 || height == 0 || rgba.len() < width * height * 4 {
        return Err(WinError::EmptyCapture);
    }
    // The region is signed because it is compared against measured offsets; every value used below
    // must be a non-negative index, so the conversions refuse a negative instead of wrapping it.
    let to_index = |value: i32| usize::try_from(value).map_err(|_| WinError::RegionOutsideClient);
    let (x, y) = (to_index(region.x)?, to_index(region.y)?);
    let (crop_w, crop_h) = (to_index(region.w)?, to_index(region.h)?);
    if crop_w == 0
        || crop_h == 0
        || x.checked_add(crop_w).is_none_or(|right| right > width)
        || y.checked_add(crop_h).is_none_or(|bottom| bottom > height)
    {
        return Err(WinError::RegionOutsideClient);
    }
    let mut out = Vec::with_capacity(crop_w * crop_h * 4);
    for row in 0..crop_h {
        let start = ((y + row) * width + x) * 4;
        out.extend_from_slice(&rgba[start..start + crop_w * 4]);
    }
    let to_u32 = |value: usize| u32::try_from(value).map_err(|_| WinError::RegionOutsideClient);
    Ok((to_u32(crop_w)?, to_u32(crop_h)?, out))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn titles_match_case_insensitively_and_only_by_containment() {
        assert!(title_matches("Seal Online TW v7.1", "seal online"));
        assert!(title_matches("seal online", "Seal Online"));
        assert!(!title_matches("Seal Online", "other game"));
        assert!(!title_matches("anything", ""));
        assert!(!title_matches("anything", "   "));
    }

    #[test]
    fn a_client_region_maps_to_the_screen_rect_that_contains_it() {
        let client = Rect::new(100, 50, 800, 600);
        let screen = region_to_screen(client, Rect::new(10, 20, 30, 40)).expect("fits");
        assert_eq!(screen, Rect::new(110, 70, 30, 40));

        let whole = region_to_screen(client, Rect::new(0, 0, 800, 600)).expect("fits exactly");
        assert_eq!(whole, Rect::new(100, 50, 800, 600));
    }

    #[test]
    fn regions_outside_the_client_are_refused_not_clipped() {
        let client = Rect::new(0, 0, 100, 100);
        for bad in [
            Rect::new(0, 0, 0, 10),
            Rect::new(0, 0, 10, 0),
            Rect::new(-1, 0, 10, 10),
            Rect::new(0, -1, 10, 10),
            Rect::new(90, 0, 20, 10),
            Rect::new(0, 90, 10, 20),
            Rect::new(101, 0, 10, 10),
        ] {
            assert_eq!(
                region_to_screen(client, bad),
                Err(WinError::RegionOutsideClient),
                "{bad:?} should be refused"
            );
        }
    }

    #[test]
    fn png_bytes_have_a_png_signature_and_the_stated_size() {
        let bgra = [
            0u8, 0, 255, 255, 0, 255, 0, 255, 255, 0, 0, 255, 255, 255, 255, 255,
        ];
        let png = encode_png(2, 2, &bgra).expect("encodes");
        assert_eq!(&png[..8], b"\x89PNG\r\n\x1a\n");

        let decoder = png::Decoder::new(png.as_slice());
        let mut reader = decoder.read_info().expect("valid png");
        let info = reader.info();
        assert_eq!((info.width, info.height), (2, 2));
        let mut buf = vec![0; reader.output_buffer_size()];
        let frame = reader.next_frame(&mut buf).expect("decodes");
        assert_eq!(
            &buf[..frame.buffer_size()],
            &[
                255u8, 0, 0, 255, 0, 255, 0, 255, 0, 0, 255, 255, 255, 255, 255, 255
            ]
        );
    }

    #[test]
    fn a_bad_buffer_is_refused() {
        assert_eq!(
            encode_png(2, 2, &[0, 0, 0, 255]),
            Err(WinError::EmptyCapture)
        );
        assert_eq!(encode_png(0, 2, &[]), Err(WinError::EmptyCapture));
    }

    #[test]
    fn encode_then_decode_returns_the_same_pixels() {
        let bgra = [
            0u8, 0, 255, 255, 0, 255, 0, 255, 255, 0, 0, 255, 255, 255, 255, 255,
        ];
        let png = encode_png(2, 2, &bgra).expect("encodes");
        let (width, height, rgba) = decode_png(&png).expect("decodes");
        assert_eq!((width, height), (2, 2));
        assert_eq!(
            rgba,
            vec![
                255u8, 0, 0, 255, 0, 255, 0, 255, 0, 0, 255, 255, 255, 255, 255, 255
            ]
        );
    }

    #[test]
    fn decode_refuses_bytes_that_are_not_a_png() {
        assert_eq!(decode_png(b"not a png"), Err(WinError::EmptyCapture));
        assert_eq!(decode_png(&[]), Err(WinError::EmptyCapture));
    }
}

#[cfg(test)]
mod crop_tests {
    use super::*;

    fn two_by_two() -> Vec<u8> {
        // Four RGBA pixels: red, green, blue, white.
        vec![
            255, 0, 0, 255, 0, 255, 0, 255, 0, 0, 255, 255, 255, 255, 255, 255,
        ]
    }

    #[test]
    fn a_crop_copies_exactly_the_requested_pixels() {
        let rgba = two_by_two();
        let (w, h, crop) = crop_rgba(2, 2, &rgba, Rect::new(1, 0, 1, 2)).expect("crops");
        assert_eq!((w, h), (1, 2));
        assert_eq!(crop, vec![0, 255, 0, 255, 255, 255, 255, 255]);
    }

    #[test]
    fn a_region_outside_the_image_is_refused() {
        let rgba = two_by_two();
        assert_eq!(
            crop_rgba(2, 2, &rgba, Rect::new(1, 1, 2, 2)),
            Err(WinError::RegionOutsideClient)
        );
        assert_eq!(
            crop_rgba(2, 2, &rgba, Rect::new(0, 0, 0, 1)),
            Err(WinError::RegionOutsideClient)
        );
    }

    #[test]
    fn a_short_buffer_is_refused_rather_than_read_past() {
        assert_eq!(
            crop_rgba(2, 2, &[0, 0, 0, 255], Rect::new(0, 0, 1, 1)),
            Err(WinError::EmptyCapture)
        );
    }
}
