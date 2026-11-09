//! Which OCR model generation a pipeline loads, and the per-generation constants the pipeline cannot
//! share. PP-OCRv4 is the shipped default. PP-OCRv6 small is the trial target: its recogniser has an
//! 18,708-entry Traditional+English dictionary and its detector ships different DB thresholds.
//!
//! The version is deliberately separate from the loader so the existing `OcrPipeline::load` keeps its
//! v4 behaviour, and the v6 path can be enabled for a corpus run before it becomes a config choice.

/// The detector constants from the model's `inference.yml` `PostProcess` block.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct DetectorProfile {
    /// `thresh`: the probability-map threshold.
    pub threshold: f32,
    /// `box_thresh`: the mean score a candidate box must reach.
    pub box_threshold: f64,
    /// `unclip_ratio`: the DB box inflation ratio.
    pub unclip_ratio: f64,
}

impl DetectorProfile {
    /// PP-OCRv4's constants: `thresh` 0.3, `box_thresh` 0.6, `unclip_ratio` 1.5.
    pub const V4: Self = Self {
        threshold: 0.3,
        box_threshold: 0.6,
        unclip_ratio: 1.5,
    };

    /// PP-OCRv6 small's `inference.yml`: `thresh` 0.2, `box_thresh` 0.45, `unclip_ratio` 1.4.
    pub const V6_SMALL: Self = Self {
        threshold: 0.2,
        box_threshold: 0.45,
        unclip_ratio: 1.4,
    };
}

/// One OCR model generation.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum OcrModelVersion {
    /// The shipped PP-OCRv4 ch models.
    V4,
    /// The PP-OCRv6 small models; the trial target for Traditional Chinese + English.
    V6Small,
}

impl OcrModelVersion {
    /// The stable spelling used by `ocr_eval --model-version` and, later, config.
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::V4 => "v4",
            Self::V6Small => "v6-small",
        }
    }

    /// Parse the CLI spelling; `None` for an unknown version.
    #[must_use]
    pub fn parse(value: &str) -> Option<Self> {
        match value {
            "v4" => Some(Self::V4),
            "v6-small" => Some(Self::V6Small),
            _ => None,
        }
    }

    /// The detector file name this version requires.
    #[must_use]
    pub const fn det_file(self) -> &'static str {
        match self {
            Self::V4 => "ch_PP-OCRv4_det_infer.onnx",
            Self::V6Small => "PP-OCRv6_small_det_infer.onnx",
        }
    }

    /// The recogniser file name this version requires.
    #[must_use]
    pub const fn rec_file(self) -> &'static str {
        match self {
            Self::V4 => "ch_PP-OCRv4_rec_infer.onnx",
            Self::V6Small => "PP-OCRv6_small_rec_infer.onnx",
        }
    }

    /// The recogniser dictionary file, when the export does not embed it. PP-OCRv4 embeds its
    /// dictionary in ONNX metadata; the official v6 ONNX export does not, so it needs the sidecar.
    #[must_use]
    pub const fn dictionary_file(self) -> Option<&'static str> {
        match self {
            Self::V4 => None,
            Self::V6Small => Some("PP-OCRv6_small_rec_dict.txt"),
        }
    }

    /// The detector's DB constants.
    #[must_use]
    pub const fn detector(self) -> DetectorProfile {
        match self {
            Self::V4 => DetectorProfile::V4,
            Self::V6Small => DetectorProfile::V6_SMALL,
        }
    }

    /// The files a candidate model directory must contain.
    #[must_use]
    pub const fn required_models(self) -> &'static [&'static str] {
        match self {
            Self::V4 => &["ch_PP-OCRv4_det_infer.onnx", "ch_PP-OCRv4_rec_infer.onnx"],
            Self::V6Small => &[
                "PP-OCRv6_small_det_infer.onnx",
                "PP-OCRv6_small_rec_infer.onnx",
                "PP-OCRv6_small_rec_dict.txt",
            ],
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn v4_keeps_the_shipped_file_names_and_embedded_dictionary() {
        assert_eq!(
            OcrModelVersion::V4.required_models(),
            ["ch_PP-OCRv4_det_infer.onnx", "ch_PP-OCRv4_rec_infer.onnx"]
        );
        assert_eq!(OcrModelVersion::V4.dictionary_file(), None);
        assert_eq!(
            OcrModelVersion::V4.detector(),
            DetectorProfile {
                threshold: 0.3,
                box_threshold: 0.6,
                unclip_ratio: 1.5,
            }
        );
    }

    #[test]
    fn v6_small_requires_its_sidecar_dictionary_and_v6_thresholds() {
        assert_eq!(
            OcrModelVersion::V6Small.required_models(),
            [
                "PP-OCRv6_small_det_infer.onnx",
                "PP-OCRv6_small_rec_infer.onnx",
                "PP-OCRv6_small_rec_dict.txt"
            ]
        );
        assert_eq!(
            OcrModelVersion::V6Small.dictionary_file(),
            Some("PP-OCRv6_small_rec_dict.txt")
        );
        assert_eq!(
            OcrModelVersion::V6Small.detector(),
            DetectorProfile {
                threshold: 0.2,
                box_threshold: 0.45,
                unclip_ratio: 1.4,
            }
        );
    }

    #[test]
    fn version_spellings_round_trip() {
        for version in [OcrModelVersion::V4, OcrModelVersion::V6Small] {
            assert_eq!(OcrModelVersion::parse(version.as_str()), Some(version));
        }
        assert_eq!(OcrModelVersion::parse("v5"), None);
    }
}
