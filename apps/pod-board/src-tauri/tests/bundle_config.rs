//! Issue #174: macOS lets a desktop app (and the `podbay-dev` it starts) use the local network only after the owner
//! allows it, and shows the app's own reason when it asks. This test checks that the bundle gives one.

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
