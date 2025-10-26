#![allow(missing_docs)]

use plugin_host::{ManifestVerifier, PluginManifest, VerificationError};
use proptest::prelude::*;
use sha2::{Digest, Sha256};

fn digest_hex(bytes: &[u8]) -> String {
    hex::encode(Sha256::digest(bytes))
}

proptest! {
    #![proptest_config(ProptestConfig { cases: 100, .. ProptestConfig::default() })]

    // Case-insensitive hex should verify when signature policy is disabled.
    #[test]
    fn digest_hex_case_insensitive(wasm in proptest::collection::vec(any::<u8>(), 0..256)) {
        let hex = digest_hex(&wasm);
        let upper = hex.to_ascii_uppercase();
        let mixed: String = hex.chars().enumerate().map(|(i, c)| if i % 2 == 0 { c.to_ascii_uppercase() } else { c }).collect();

        let v = ManifestVerifier { require_signed_plugins: false };

        let man_upper = PluginManifest { name: "p".into(), version: "1".into(), wasm_digest: upper, signature: None, sbom_ref: None };
        prop_assert!(v.verify(&man_upper, &wasm).is_ok());

        let man_mixed = PluginManifest { name: "p".into(), version: "1".into(), wasm_digest: mixed, signature: None, sbom_ref: None };
        prop_assert!(v.verify(&man_mixed, &wasm).is_ok());
    }

    // Leading/trailing whitespace must be trimmed from manifest.wasm_digest.
    #[test]
    fn digest_whitespace_trimmed(wasm in proptest::collection::vec(any::<u8>(), 0..256)) {
        let hex = digest_hex(&wasm);
        let spaced = format!("  {hex}  ");
        let v = ManifestVerifier { require_signed_plugins: false };
        let man = PluginManifest { name: "p".into(), version: "1".into(), wasm_digest: spaced, signature: None, sbom_ref: None };
        prop_assert!(v.verify(&man, &wasm).is_ok());
    }

    // When signatures are required but missing, expect MissingSignature.
    #[test]
    fn missing_signature_when_required(wasm in proptest::collection::vec(any::<u8>(), 0..256)) {
        let hex = digest_hex(&wasm);
        let v = ManifestVerifier { require_signed_plugins: true };
        let man = PluginManifest { name: "p".into(), version: "1".into(), wasm_digest: hex, signature: None, sbom_ref: None };
        let res = v.verify(&man, &wasm);
        prop_assert!(matches!(res, Err(VerificationError::MissingSignature)));
    }

    // When signature present but SBOM missing and policy requires, expect MissingSbom.
    #[test]
    fn missing_sbom_when_required(wasm in proptest::collection::vec(any::<u8>(), 0..256)) {
        let hex = digest_hex(&wasm);
        let v = ManifestVerifier { require_signed_plugins: true };
        let man = PluginManifest { name: "p".into(), version: "1".into(), wasm_digest: hex, signature: Some("AQ==".into()), sbom_ref: None };
        let res = v.verify(&man, &wasm);
        prop_assert!(matches!(res, Err(VerificationError::MissingSbom)));
    }

    // Invalid base64 signature strings should return InvalidSignature (policy off to reach signature path).
    #[test]
    fn invalid_base64_signature_fails(
        wasm in proptest::collection::vec(any::<u8>(), 0..256),
        bad in "[^A-Za-z0-9+/=]{1,16}"
    ) {
        let hex = digest_hex(&wasm);
        let v = ManifestVerifier { require_signed_plugins: false };
        let man = PluginManifest { name: "p".into(), version: "1".into(), wasm_digest: hex, signature: Some(bad), sbom_ref: Some("sbom.json".into()) };
        let res = v.verify(&man, &wasm);
        prop_assert!(matches!(res, Err(VerificationError::InvalidSignature)));
    }
}
