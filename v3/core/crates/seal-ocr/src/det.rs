//! The detector session stub and the named post-processing spike.
//!
//! Only the session and its shapes live here. The detector's post-processing is the OpenCV-shaped
//! work identified in `v3/docs/records/OCR-RESEARCH.md` §5 and is deliberately NOT ported yet; the
//! spike list below is the contract for that work.
//!
//! # The spike list (what the port must implement, and the pure-Rust question)
//!
//! The C# `DetPostprocess.cs` does, in order:
//!
//! 1. `Cv2.Threshold` the probability map at `thresh = 0.3`.
//! 2. `Cv2.Dilate` with a 2×2 rectangle kernel.
//! 3. `Cv2.FindContours` (`RetrievalModes.List`, `ApproxSimple`) to get candidate contours.
//! 4. `Cv2.MinAreaRect` + `Cv2.BoxPoints` per contour (min-area rectangle, ordered points).
//! 5. A box score: `Cv2.FillPoly` a mask over the box, `Cv2.Mean` the probability map under the mask,
//!    keep boxes scoring ≥ `box_thresh = 0.6`.
//! 6. `Cv2.ContourArea` + `Cv2.ArcLength` to compute the unclip distance, then Clipper's
//!    `InflatePaths` (round join) with `unclip_ratio = 1.5`.
//! 7. `Cv2.GetPerspectiveTransform` + `Cv2.WarpPerspective` (cubic, replicate border) to crop each
//!    box; rotate 90° CW when `h/w ≥ 1.5`.
//! 8. Sort by Y with a 10 px row threshold, then by X within a row.
//!
//! What a purely-Rust implementation needs: threshold/dilate are trivial on a `Vec<u8>`; contours and
//! min-area rectangles are not in `image`/`imageproc` (a connected-component pass plus a convex hull
//! and rotating calipers would port them); the masked mean is easy; `InflatePaths` maps to a polygon
//! offsetting crate (e.g. `i_overlay`/`geo`-style offsetting, or a small port of the clipper offset);
//! the perspective warp is a per-pixel inverse mapping. The alternative is the `opencv` crate, which
//! pulls a native OpenCV build — the v3 design deliberately does not have one. This weighting is the
//! spike's job to settle; nothing here should be read as "port it with OpenCV".

use std::path::Path;

use ort::environment::Environment;
use ort::session::Session;

use crate::model::{ModelIo, OcrError};

/// The detector session: enough to load the model and record its shapes.
pub struct DetModel {
    // Kept alive with the session; ORT's environment must outlive its sessions.
    _environment: Environment,
    session: Session,
    io: ModelIo,
}

impl DetModel {
    pub fn load(path: &Path, threads: usize) -> Result<Self, OcrError> {
        let (environment, session, io) = crate::model::open_session(path, threads, "seal-ocr-det")?;
        Ok(Self {
            _environment: environment,
            session,
            io,
        })
    }

    pub fn io(&self) -> &ModelIo {
        &self.io
    }

    /// The session is exposed for the post-processing spike; it is not run here.
    pub fn session(&mut self) -> &mut Session {
        &mut self.session
    }
}
