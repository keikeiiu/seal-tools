//! The arithmetic this crate owns that needs no window: title matching, client-region→screen
//! mapping, and BGRA→PNG encoding. Everything here is testable on any machine; the Win32 calls are
//! `window.rs`'s and only appear in its `#[ignore]`d live tests.

use seal_domain::geometry::Rect;

use crate::error::WinError;

/// Does a window title match the configured title? Case-insensitive CONTAINS: the game's title bar
/// carries a version suffix, and v2 matched it the same way. An empty needle matches nothing (an
/// empty configured title is a config error, not "match every window").
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
}
