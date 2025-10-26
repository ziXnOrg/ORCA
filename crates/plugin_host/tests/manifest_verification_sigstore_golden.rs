#![allow(missing_docs)]

use base64::Engine;
use plugin_host::{ManifestVerifier, PluginManifest, SigstoreOptions, VerificationError};
use sha2::{Digest, Sha256};

fn fixtures_dir() -> std::path::PathBuf {
    std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/golden/sigstore")
}

fn read_fixture(name: &str) -> String {
    std::fs::read_to_string(fixtures_dir().join(name)).expect("fixture exists and readable")
}

fn read_bytes(name: &str) -> Vec<u8> {
    std::fs::read(fixtures_dir().join(name)).expect("fixture exists and readable")
}

fn wasm_bytes() -> Vec<u8> {
    read_bytes("test_plugin.wasm")
}

fn sigstore_opts() -> SigstoreOptions {
    SigstoreOptions {
        fulcio_cert_pem: read_bytes("trust/fulcio_root.pem"),
        rekor_key_pem: None,
        ctfe_key_pem: read_bytes("trust/ctfe_pubkey.pem"),
        issuer_allowlist: vec!["https://fulcio.sigstore.dev".to_string()],
        san_allowlist: vec!["test@example.com".to_string()],
    }
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

    // Triaging: verify directly with sigstore to surface error cause in CI logs
    let bundle_str = read_fixture("valid_bundle.json");
    let bundle: Result<sigstore::bundle::Bundle, _> = serde_json::from_str(&bundle_str);
    if let Ok(bundle) = bundle {
        let trust = {
            let pem = read_bytes("trust/fulcio_root.pem");
            let text = std::str::from_utf8(&pem).unwrap();
            let body: String = text
                .lines()
                .filter_map(|l| {
                    let t = l.trim();
                    if t.starts_with("---") || t.is_empty() {
                        None
                    } else {
                        Some(t)
                    }
                })
                .collect();
            let der_vec =
                base64::engine::general_purpose::STANDARD.decode(body.as_bytes()).unwrap();
            let der_static: &'static [u8] = Box::leak(der_vec.into_boxed_slice());
            let mut ctfe_map = std::collections::BTreeMap::new();
            // load CTFE public key (SPKI) PEM and decode to DER bytes
            let ctfe_pem = read_bytes("trust/ctfe_pubkey.pem");
            let ctfe_text = std::str::from_utf8(&ctfe_pem).unwrap();
            let mut ctfe_b64 = String::new();
            for l in ctfe_text.lines() {
                let t = l.trim();
                if t.starts_with("---") || t.is_empty() {
                    continue;
                }
                ctfe_b64.push_str(t);
            }
            let ctfe_der =
                base64::engine::general_purpose::STANDARD.decode(ctfe_b64.as_bytes()).unwrap();
            ctfe_map.insert("ctfe-0".to_string(), ctfe_der);
            sigstore::trust::ManualTrustRoot {
                fulcio_certs: vec![rustls_pki_types::CertificateDer::from(der_static)],
                rekor_keys: Default::default(),
                ctfe_keys: ctfe_map,
            }
        };
        let verifier = sigstore::bundle::verify::blocking::Verifier::new(
            sigstore::rekor::apis::configuration::Configuration::default(),
            trust,
        )
        .unwrap();
        let policy = sigstore::bundle::verify::policy::Identity::new(
            "test@example.com",
            "https://fulcio.sigstore.dev",
        );
        let mut hasher = Sha256::new();
        hasher.update(&wasm);
        match verifier.verify_digest(hasher, bundle, &policy, true) {
            Ok(()) => println!("[triage] direct sigstore verify OK"),
            Err(e) => println!("[triage] direct sigstore verify ERR: {e:?}"),
        }
    } else {
        println!("[triage] bundle JSON parse ERR: {bundle:?}");
    }

    let v = ManifestVerifier::with_sigstore(sigstore_opts());
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
