//! Model-gated smoke test over v2's ten raw full-screen captures: the pipeline must accept real
//! pixels, find text, and never panic. Skipped (not failed) when the models or the captures are not
//! on this machine, so the normal suite stays green.

use seal_domain::config::AttributesConfig;

#[test]
fn the_pipeline_reads_the_real_v2_captures() {
    let Ok(models) = std::env::var("SEAL_OCR_MODELS") else {
        eprintln!("skipping: SEAL_OCR_MODELS is not set");
        return;
    };
    let captures =
        std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../../../v2/logs/captures");
    if !captures.is_dir() {
        eprintln!(
            "skipping: no v2 capture directory at {}",
            captures.display()
        );
        return;
    }

    let mut pipeline = seal_ocr::pipeline::OcrPipeline::load(
        std::path::Path::new(&models),
        &AttributesConfig::default(),
        10,
    )
    .expect("the models load");

    let mut files = 0;
    let mut non_empty = 0;
    let mut total_items = 0;
    for entry in std::fs::read_dir(&captures).expect("the capture dir lists") {
        let path = entry.expect("a capture entry").path();
        let name = path
            .file_name()
            .and_then(|name| name.to_str())
            .unwrap_or("");
        let is_capture = name.starts_with("full_")
            && std::path::Path::new(name)
                .extension()
                .is_some_and(|extension| extension.eq_ignore_ascii_case("png"));
        if !is_capture {
            continue;
        }
        files += 1;
        let bytes = std::fs::read(&path).expect("the capture reads");
        let (width, height, rgba) =
            seal_win::pure::decode_png(&bytes).expect("the capture decodes");
        let scan = pipeline
            .scan_rgba(width as usize, height as usize, &rgba)
            .expect("the pipeline scans a real capture");
        total_items += scan.items.len();
        if !scan.items.is_empty() {
            non_empty += 1;
        }
    }

    eprintln!("read {files} captures, {non_empty} produced text, {total_items} items total");
    assert!(files > 0, "the capture directory holds full_*.png files");
    assert!(
        non_empty > 0,
        "at least one real capture must produce text, otherwise the pipeline is broken"
    );
}
