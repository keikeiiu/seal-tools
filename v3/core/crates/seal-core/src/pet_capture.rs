//! Pet increment 2: the icon-capture primitive behind `pet.capture_icon`.
//!
//! The shell hides itself, puts the bag page up and asks for one cell. The core only CAPTURES: it
//! never clicks, moves the cursor, opens the board or writes a log. The crop geometry is the same
//! derivation the run uses for its bag-cell checks (`BagGrid::centres` + the rounded pitch box,
//! `pet.rs`'s reload branch), so preflight-like previews and live reloads cannot disagree about
//! which pixels make up cell N.

use base64::Engine as _;
use seal_domain::geometry::Rect;
use seal_domain::tools::shop::BagGrid;
use serde_json::{Value, json};

use crate::dispatch::frame_rgba;

/// One parsed request: `kind` is either `queue` or `food`; `page` and `cell` are 1-based (the
/// numbers the Pet tab shows, 1..64 for cells).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct IconArguments {
    pub kind: IconKind,
    pub page: usize,
    pub cell: usize,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum IconKind {
    Queue,
    Food,
}

impl IconKind {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Queue => "queue",
            Self::Food => "food",
        }
    }
}

/// A failure that is not a capture failure. Capture itself maps through `win_error_response`;
/// this covers bad arguments and the two prerequisites that must be fixed in config.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum IconError {
    BadRequest {
        message: String,
        field: &'static str,
    },
    Internal(String),
}

/// Parse and validate `{"kind","page","cell"}`. Bad values are `bad_request` naming the field.
pub fn arguments(value: &Value) -> Result<IconArguments, (String, &'static str)> {
    let kind = match value.get("kind").and_then(Value::as_str) {
        Some("queue") => IconKind::Queue,
        Some("food") => IconKind::Food,
        Some(other) => {
            return Err((
                format!("pet.capture_icon's \"kind\" must be \"queue\" or \"food\", not {other:?}"),
                "kind",
            ));
        }
        None => {
            return Err((
                "pet.capture_icon needs a \"kind\" string: \"queue\" or \"food\"".to_owned(),
                "kind",
            ));
        }
    };
    let page = match value.get("page").and_then(Value::as_u64) {
        Some(page) if page >= 1 => page as usize,
        _ => {
            return Err((
                "pet.capture_icon's \"page\" must be a 1-based integer".to_owned(),
                "page",
            ));
        }
    };
    let cell = match value.get("cell").and_then(Value::as_u64) {
        Some(cell) if (1..=BagGrid::SLOT_COUNT as u64).contains(&cell) => cell as usize,
        Some(_) => {
            return Err((
                format!(
                    "pet.capture_icon's \"cell\" must be between 1 and {}",
                    BagGrid::SLOT_COUNT
                ),
                "cell",
            ));
        }
        None => {
            return Err((
                "pet.capture_icon's \"cell\" must be a 1-based integer".to_owned(),
                "cell",
            ));
        }
    };
    Ok(IconArguments { kind, page, cell })
}

/// Validate the prerequisites and derive the client-relative PHYSICAL crop rectangle. This is the
/// only place the page/cell → pixels rule lives; `pet.rs`'s bag-cell check uses the same
/// `BagGrid::centres` + rounded pitch box.
pub fn geometry(config: &Value, arguments: &IconArguments) -> Result<Rect, IconError> {
    let grid = config
        .pointer("/pet/bag_grid")
        .and_then(Value::as_array)
        .map(|values| {
            values
                .iter()
                .filter_map(Value::as_i64)
                .filter_map(|value| i32::try_from(value).ok())
                .collect::<Vec<_>>()
        })
        .filter(|grid| BagGrid::is_valid_rect(Some(grid)))
        .ok_or_else(|| IconError::BadRequest {
            message: "The boarding bag grid isn't calibrated — save the Boarding bag grid on the Calibrate tab."
                .to_owned(),
            field: "bag_grid",
        })?;
    let tabs = config
        .pointer("/pet/page_tabs")
        .and_then(Value::as_array)
        .map(|tabs| {
            tabs.iter()
                .map(|tab| tab.as_array().map(|values| values.len()).unwrap_or(0))
                .collect::<Vec<_>>()
        })
        .unwrap_or_default();
    let page_index = arguments.page - 1;
    if page_index >= tabs.len() || tabs[page_index] != 2 {
        return Err(IconError::BadRequest {
            message: format!(
                "Bag page {} has no 2-point tab mark — save the Bag page tab point for page {} on the Calibrate tab.",
                arguments.page, arguments.page
            ),
            field: "page",
        });
    }
    if arguments.cell > BagGrid::SLOT_COUNT {
        return Err(IconError::BadRequest {
            message: format!(
                "pet.capture_icon's \"cell\" must be between 1 and {}",
                BagGrid::SLOT_COUNT
            ),
            field: "cell",
        });
    }
    BagGrid::cell_box(Some(&grid), arguments.cell - 1).ok_or_else(|| IconError::BadRequest {
        message:
            "The boarding bag grid works out to an empty cell — re-drag it on the Calibrate tab."
                .to_owned(),
        field: "bag_grid",
    })
}

/// Crop one cell out of a captured client frame and return the protocol result. Kept separate from
/// the Win32 capture so the byte-level crop is testable on any machine.
pub fn icon_from_frame(
    arguments: &IconArguments,
    rect: Rect,
    frame: &Value,
) -> Result<Value, IconError> {
    let (width, height, rgba) = frame_rgba(frame).map_err(IconError::Internal)?;
    let (crop_w, crop_h, cropped) = seal_win::pure::crop_rgba(
        width as u32,
        height as u32,
        &rgba,
        rect,
    )
    .map_err(|error| IconError::BadRequest {
        message: format!(
            "the bag cell {rect:?} is outside the captured game frame ({width}x{height}): {error}"
        ),
        field: "bag_grid",
    })?;
    // `crop_rgba` works in the PNG decoder's RGBA order; `encode_png` takes the Win32 BGRA order.
    let (pixels, _) = cropped.as_chunks::<4>();
    let mut bgra = Vec::with_capacity(cropped.len());
    for pixel in pixels {
        bgra.extend_from_slice(&[pixel[2], pixel[1], pixel[0], pixel[3]]);
    }
    let png = seal_win::pure::encode_png(crop_w, crop_h, &bgra).map_err(|error| {
        IconError::Internal(format!(
            "the captured cell could not be encoded as PNG: {error}"
        ))
    })?;
    Ok(json!({
        "kind": arguments.kind.as_str(),
        "page": arguments.page,
        "cell": arguments.cell,
        "rect": [rect.x, rect.y, rect.w, rect.h],
        "png_base64": base64::engine::general_purpose::STANDARD.encode(png),
    }))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn config(grid: &[i32], tabs: &[&[i32]]) -> Value {
        json!({
            "pet": {
                "bag_grid": grid,
                "page_tabs": tabs,
            }
        })
    }

    fn frame(width: u32, height: u32) -> Value {
        // A deterministic RGBA image: each pixel encodes x in R, y in G, and a fixed B/A.
        let mut rgba = Vec::with_capacity((width * height * 4) as usize);
        for y in 0..height {
            for x in 0..width {
                rgba.extend_from_slice(&[x as u8, y as u8, 0x33, 0xff]);
            }
        }
        let (pixels, _) = rgba.as_chunks::<4>();
        let mut bgra = Vec::with_capacity(rgba.len());
        for pixel in pixels {
            bgra.extend_from_slice(&[pixel[2], pixel[1], pixel[0], pixel[3]]);
        }
        let png = seal_win::pure::encode_png(width, height, &bgra).expect("a synthetic frame");
        json!({
            "format": "png", "width": width, "height": height,
            "origin": {"x": 0, "y": 0},
            "data": base64::engine::general_purpose::STANDARD.encode(png),
        })
    }

    #[test]
    fn arguments_are_one_based_and_name_the_bad_field() {
        assert_eq!(
            arguments(&json!({"kind": "queue", "page": 1, "cell": 34})),
            Ok(IconArguments {
                kind: IconKind::Queue,
                page: 1,
                cell: 34
            })
        );
        assert_eq!(
            arguments(&json!({"kind": "food", "page": 2, "cell": 1}))
                .unwrap()
                .kind,
            IconKind::Food
        );
        assert_eq!(
            arguments(&json!({"kind": "pet", "page": 1, "cell": 1}))
                .unwrap_err()
                .1,
            "kind"
        );
        assert_eq!(
            arguments(&json!({"kind": "queue", "page": 0, "cell": 1}))
                .unwrap_err()
                .1,
            "page"
        );
        assert_eq!(
            arguments(&json!({"kind": "queue", "page": 1, "cell": 65}))
                .unwrap_err()
                .1,
            "cell"
        );
        assert_eq!(
            arguments(&json!({"kind": "queue", "page": 1}))
                .unwrap_err()
                .1,
            "cell"
        );
    }

    #[test]
    fn the_crop_rect_is_the_run_s_bag_grid_pitch_box() {
        // 8x8 bag at 80x80: pitch 10, cell 34 is zero-based 33 = row 4, col 1. Its centre is
        // (15, 45), so the pitch box is 10x10 at (10, 40).
        let answer = geometry(
            &config(&[0, 0, 80, 80], &[&[100, 100]]),
            &IconArguments {
                kind: IconKind::Queue,
                page: 1,
                cell: 34,
            },
        )
        .expect("geometry");
        assert_eq!(answer, Rect::new(10, 40, 10, 10));

        let answer = geometry(
            &config(&[20, 30, 80, 80], &[&[0, 0], &[1, 1]]),
            &IconArguments {
                kind: IconKind::Food,
                page: 2,
                cell: 1,
            },
        )
        .expect("geometry");
        assert_eq!(answer, Rect::new(20, 30, 10, 10));
    }

    #[test]
    fn missing_grid_or_page_tab_names_the_prerequisite() {
        let arguments = IconArguments {
            kind: IconKind::Queue,
            page: 1,
            cell: 1,
        };
        assert!(matches!(
            geometry(&config(&[], &[&[1, 1]]), &arguments),
            Err(IconError::BadRequest {
                field: "bag_grid",
                ..
            })
        ));
        assert!(matches!(
            geometry(&config(&[0, 0, 80, 80], &[&[1]]), &arguments),
            Err(IconError::BadRequest { field: "page", .. })
        ));
    }

    #[test]
    fn the_returned_png_is_a_byte_identical_crop_of_the_frame() {
        let frame = frame(80, 80);
        let arguments = IconArguments {
            kind: IconKind::Food,
            page: 1,
            cell: 34,
        };
        let rect = geometry(&config(&[0, 0, 80, 80], &[&[0, 0]]), &arguments).expect("geometry");
        let answer = icon_from_frame(&arguments, rect, &frame).expect("the crop");
        assert_eq!(answer["kind"], "food");
        assert_eq!(answer["page"], 1);
        assert_eq!(answer["cell"], 34);
        assert_eq!(answer["rect"], json!([10, 40, 10, 10]));

        let png = base64::engine::general_purpose::STANDARD
            .decode(answer["png_base64"].as_str().unwrap())
            .unwrap();
        let (width, height, rgba) = seal_win::pure::decode_png(&png).unwrap();
        assert_eq!((width, height), (10, 10));
        // Cell (10..20, 40..50) of the deterministic frame: R=x, G=y, B=0x33, A=0xff.
        for y in 0..10u32 {
            for x in 0..10u32 {
                let at = ((y * 10 + x) * 4) as usize;
                assert_eq!(
                    &rgba[at..at + 4],
                    &[(10 + x) as u8, (40 + y) as u8, 0x33, 0xff]
                );
            }
        }
    }

    #[test]
    fn a_cell_outside_the_frame_is_a_bad_grid_not_a_panic() {
        let arguments = IconArguments {
            kind: IconKind::Queue,
            page: 1,
            cell: 64,
        };
        let rect = geometry(&config(&[0, 0, 80, 80], &[&[0, 0]]), &arguments).expect("geometry");
        let answer = icon_from_frame(&arguments, rect, &frame(20, 20));
        assert!(matches!(
            answer,
            Err(IconError::BadRequest {
                field: "bag_grid",
                ..
            })
        ));
    }

    #[test]
    fn a_page_beyond_the_marks_is_a_clean_bad_request_naming_page() {
        let arguments = IconArguments {
            kind: IconKind::Queue,
            page: 3,
            cell: 1,
        };
        let answer = geometry(&config(&[0, 0, 80, 80], &[&[100, 100]]), &arguments);
        assert!(matches!(
            answer,
            Err(IconError::BadRequest { field: "page", .. })
        ));
    }

    #[test]
    fn the_shared_cell_box_is_the_run_pitch_box() {
        let grid = [0, 0, 80, 80];
        let cell = BagGrid::cell_box(Some(&grid), 33).expect("the shared box");
        assert_eq!(cell, Rect::new(10, 40, 10, 10));
        let centre = BagGrid::centres(Some(&grid))[33];
        let (pitch_x, pitch_y) = BagGrid::pitch(&grid).expect("a valid pitch");
        assert_eq!(
            cell,
            Rect::new(
                centre.x - pitch_x / 2,
                centre.y - pitch_y / 2,
                pitch_x,
                pitch_y
            )
        );
    }
}
