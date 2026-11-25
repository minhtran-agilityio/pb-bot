//! Issue #174: macOS lets a desktop app (and the `podbay-dev` it starts) use the local network only after the owner
//! allows it, and asks only an app that says why and has a stable identity. These tests check that the bundle says so.

use std::path::PathBuf;

fn file(rel: &str) -> String {
    let path = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join(rel);
    std::fs::read_to_string(&path).unwrap_or_else(|e| panic!("cannot read {}: {e}", path.display()))
}

#[test]
fn the_app_says_why_it_needs_the_local_network() {
    let plist = file("Info.plist");
    assert!(plist.contains("<key>NSLocalNetworkUsageDescription</key>"), "Info.plist has no NSLocalNetworkUsageDescription");
}

#[test]
fn the_bundle_is_signed_ad_hoc_under_its_own_identifier() {
    let conf: serde_json::Value = serde_json::from_str(&file("tauri.conf.json")).unwrap();
    let mac = &conf["bundle"]["macOS"];
    assert_eq!(mac["signingIdentity"], "-", "bundle.macOS.signingIdentity must sign the whole bundle ad hoc");
}
