//! RED tests for plugin manifest verification (T-6a-E3-SEC-04)
//! These tests assert failure modes; the current stubbed verifier returns Ok,
//! so these should FAIL until GREEN implements real checks.

use plugin_host::{ManifestVerifier, PluginManifest, VerificationError};

fn wasm_minimal() -> Vec<u8> {
    let wat = "(module)";
    wat::parse_str(wat).expect("WAT -> WASM should succeed")
}

#[test]
fn unsigned_manifest_fails_verification() {
    let wasm = wasm_minimal();
    let manifest = PluginManifest {
        name: "demo".into(),
        version: "1.0.0".into(),
        wasm_digest: "deadbeef".into(),
        signature: None, // unsigned
        sbom_ref: Some("sbom.json".into()),
    };
    let v = ManifestVerifier::new();
    let res = v.verify(&manifest, &wasm);
    assert!(matches!(res, Err(VerificationError::MissingSignature)), "expected MissingSignature, got: {:?}", res);
}

#[test]
fn tampered_manifest_fails_verification() {
    let wasm = wasm_minimal();
    let manifest = PluginManifest {
        name: "demo".into(),
        version: "1.0.0".into(),
        wasm_digest: "0000".into(), // wrong digest
        signature: Some("stub-signature".into()),
        sbom_ref: Some("sbom.json".into()),
    };
    let v = ManifestVerifier::new();
    let res = v.verify(&manifest, &wasm);
    assert!(matches!(res, Err(VerificationError::DigestMismatch)), "expected DigestMismatch, got: {:?}", res);
}

#[test]
fn invalid_signature_fails_verification() {
    let wasm = wasm_minimal();
    let manifest = PluginManifest {
        name: "demo".into(),
        version: "1.0.0".into(),
        wasm_digest: "deadbeef".into(),
        signature: Some("not-a-valid-signature".into()),
        sbom_ref: Some("sbom.json".into()),
    };
    let v = ManifestVerifier::new();
    let res = v.verify(&manifest, &wasm);
    assert!(matches!(res, Err(VerificationError::InvalidSignature)), "expected InvalidSignature, got: {:?}", res);
}

#[test]
fn missing_sbom_fails_policy_check() {
    let wasm = wasm_minimal();
    let manifest = PluginManifest {
        name: "demo".into(),
        version: "1.0.0".into(),
        wasm_digest: "deadbeef".into(),
        signature: Some("stub-signature".into()),
        sbom_ref: None, // missing SBOM per policy
    };
    let v = ManifestVerifier::new();
    let res = v.verify(&manifest, &wasm);
    assert!(matches!(res, Err(VerificationError::MissingSbom)), "expected MissingSbom, got: {:?}", res);
}

