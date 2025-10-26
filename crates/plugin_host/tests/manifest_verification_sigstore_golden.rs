#![allow(missing_docs)]

use plugin_host::{ManifestVerifier, PluginManifest, VerificationError};
use sha2::{Digest, Sha256};

fn fixtures_dir() -> std::path::PathBuf {
    std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/golden/sigstore")
}

fn read_fixture(name: &str) -> String {
    std::fs::read_to_string(fixtures_dir().join(name)).expect("fixture exists and readable")
}

fn wasm_bytes() -> Vec<u8> {
    // Deterministic minimal module.
    let wat = "(module)";
    wat::parse_str(wat).expect("WAT -> WASM should succeed")
}

#[test]
fn sigstore_valid_bundle_verifies_offline() {
    let wasm = wasm_bytes();
    let digest_hex = hex::encode(Sha256::digest(&wasm));
    let manifest = PluginManifest {
        name: "demo".into(),
        version: "1.0.0".into(),
        wasm_digest: digest_hex,
        signature: Some(read_fixture("valid_bundle.json")),
        sbom_ref: Some("sbom.json".into()),
    };
    let v = ManifestVerifier::new();
    let res = v.verify(&manifest, &wasm);
    assert!(res.is_ok(), "expected Ok for valid bundle, got: {res:?}");
}

#[test]
fn sigstore_tampered_bundle_fails() {
    let wasm = wasm_bytes();
    let digest_hex = hex::encode(Sha256::digest(&wasm));
    let manifest = PluginManifest {
        name: "demo".into(),
        version: "1.0.0".into(),
        wasm_digest: digest_hex,
        signature: Some(read_fixture("tampered_bundle.json")),
        sbom_ref: Some("sbom.json".into()),
    };
    let v = ManifestVerifier::new();
    let res = v.verify(&manifest, &wasm);
    assert!(
        matches!(res, Err(VerificationError::InvalidSignature)),
        "expected InvalidSignature, got: {res:?}"
    );
}

#[test]
fn sigstore_invalid_signature_fails() {
    let wasm = wasm_bytes();
    let digest_hex = hex::encode(Sha256::digest(&wasm));
    let manifest = PluginManifest {
        name: "demo".into(),
        version: "1.0.0".into(),
        wasm_digest: digest_hex,
        signature: Some(read_fixture("invalid_signature.json")),
        sbom_ref: Some("sbom.json".into()),
    };
    let v = ManifestVerifier::new();
    let res = v.verify(&manifest, &wasm);
    assert!(
        matches!(res, Err(VerificationError::InvalidSignature)),
        "expected InvalidSignature, got: {res:?}"
    );
}

#[test]
fn sigstore_missing_trust_root_fails() {
    let wasm = wasm_bytes();
    let digest_hex = hex::encode(Sha256::digest(&wasm));
    let manifest = PluginManifest {
        name: "demo".into(),
        version: "1.0.0".into(),
        wasm_digest: digest_hex,
        signature: Some(read_fixture("valid_bundle.json")),
        sbom_ref: Some("sbom.json".into()),
    };
    let v = ManifestVerifier::new();
    let res = v.verify(&manifest, &wasm);
    assert!(
        matches!(res, Err(VerificationError::InvalidSignature)),
        "expected InvalidSignature (no trust root), got: {res:?}"
    );
}
