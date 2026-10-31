//! The live-path switch through the PROTOCOL (increment 8 acceptance): a migrated install answers
//! `hello` from the portable tree and `config.apply` writes the owning files, not local.yaml.

mod common;

use common::TestDir;
use seal_config::file;
use seal_config::registry::{self, FileDef};
use seal_core::dispatch::{CoreState, handle};
use seal_proto::{Request, Response};
use serde_json::{Value, json};

fn def(path: &str) -> &'static FileDef {
    registry::FILES
        .iter()
        .find(|file| file.path == path)
        .expect("a registry entry")
}

fn new_layout() -> (TestDir, CoreState) {
    let dir = TestDir::new();
    std::fs::create_dir_all(dir.path().join("config")).unwrap();
    let root = dir.path().join("config");
    std::fs::write(root.join("defaults.yaml.example"), common::BASE_CONFIG).unwrap();
    std::fs::write(root.join("local.yaml.example"), "").unwrap();
    std::fs::write(root.join("attributes.yaml"), common::BASE_ATTRIBUTES).unwrap();
    file::write(
        &root,
        def("meta.yaml"),
        &json!({"layout_version": 1, "legacy_folded": true}),
    )
    .unwrap();
    file::write(
        &root,
        def("setup/common.yaml"),
        &json!({"window": {"title": "TW_LIVE"}, "hotkeys": {"start": 123}}),
    )
    .unwrap();
    file::write(
        &root,
        def("setup/buy_sell.yaml"),
        &json!({"buy_sell": {"presets": {"Potion": {"scroll": 0, "row": 2, "count": 2}}}}),
    )
    .unwrap();
    file::write(
        &root,
        def("machine/ui.yaml"),
        &json!({"ui": {"left": 10.0}}),
    )
    .unwrap();
    let mut state = CoreState::load(dir.path().to_path_buf()).expect("the portable tree starts");
    let hello = handle(
        &mut state,
        Request::new("1", "hello", json!({"protocol": 1, "client": "0.1.0"})),
    );
    assert!(hello.ok, "hello: {:?}", hello.error);
    (dir, state)
}

fn call(state: &mut CoreState, id: &str, cmd: &str, args: Value) -> Response {
    handle(state, Request::new(id, cmd, args))
}

#[test]
fn hello_serves_the_portable_tree_and_apply_writes_the_owning_files() {
    let (dir, mut state) = new_layout();
    let hello = call(
        &mut state,
        "2",
        "hello",
        json!({"protocol": 1, "client": "0.1.0"}),
    );
    assert!(hello.ok, "{:?}", hello.error);
    let config = hello.result.unwrap()["config"].clone();
    assert_eq!(config["window"]["title"], "TW_LIVE");
    assert_eq!(config["hotkeys"]["start"], 123);
    assert_eq!(config["buy_sell"]["presets"]["Potion"]["count"], 2);

    let rev = state.rev;
    let response = call(
        &mut state,
        "3",
        "config.apply",
        json!({
            "rev": rev,
            "sections": {
                "buy_sell": {
                    "presets": {"Potion": {"scroll": 0, "row": 2, "count": 4}},
                    "sell_cap": 10
                }
            }
        }),
    );
    assert!(response.ok, "apply: {:?}", response.error);
    assert_eq!(response.result.unwrap()["rev"], rev + 1);

    let root = dir.path().join("config");
    let setup = file::read(&root, def("setup/buy_sell.yaml"))
        .unwrap()
        .unwrap();
    assert_eq!(setup["buy_sell"]["presets"]["Potion"]["count"], 4);
    let ui = file::read(&root, def("machine/ui.yaml")).unwrap().unwrap();
    assert_eq!(ui["ui"]["left"], 10.0, "the untouched file stays");
    assert!(
        !root.join("local.yaml").exists(),
        "no legacy file was created"
    );
}
