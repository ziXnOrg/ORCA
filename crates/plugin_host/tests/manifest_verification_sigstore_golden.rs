#![allow(missing_docs)]

#[test]
#[ignore = "awaiting real sigstore bundles (offline)"]
fn loads_sigstore_fixture() {
    let p = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("tests/golden/sigstore/valid_bundle.json");
    let s = std::fs::read_to_string(&p).expect("fixture exists and readable");
    assert!(s.trim_start().starts_with('{'), "expected JSON object stub");
}
