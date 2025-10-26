#![allow(missing_docs)]

use base64::engine::general_purpose::STANDARD as B64;
use base64::Engine;
use der::Encode;
use p256::{
    ecdsa::SigningKey as P256SigningKey, pkcs8::EncodePrivateKey, SecretKey as P256SecretKey,
};
use x509_cert::der::pem::LineEnding;
use x509_cert::der::EncodePem;
// use pkcs8::LineEnding; // not needed; we write PEM manually via pem crate
use rcgen::{
    BasicConstraints, Certificate, CertificateParams, CustomExtension, DistinguishedName, DnType,
    ExtendedKeyUsagePurpose, IsCa, KeyPair, KeyUsagePurpose, SanType, PKCS_ECDSA_P256_SHA256,
};
use serde_json::json;
use sha2::{Digest, Sha256};
use std::{fs, path::PathBuf};
use tls_codec::{SerializeBytes, TlsByteVecU16, TlsByteVecU24, TlsSerializeBytes, TlsSize};
use x509_cert::ext::pkix::sct::{
    DigitallySigned as SctDigitallySigned, HashAlgorithm, LogId, SerializedSct, SignatureAlgorithm,
    SignatureAndHashAlgorithm, SignedCertificateTimestamp, SignedCertificateTimestampList,
    Version as SctVersion,
};
use x509_cert::spki::EncodePublicKey;
use x509_cert::{der::Decode as DerDecode, Certificate as X509Certificate};

fn write(path: &PathBuf, data: &[u8]) {
    if let Some(parent) = path.parent() {
        let _ = fs::create_dir_all(parent);
    }
    fs::write(path, data).expect("write");
}

// Minimal TLS structs to construct the SCT signing payload (RFC6962) matching sigstore-rs
#[derive(PartialEq, Debug, TlsSerializeBytes, TlsSize)]
#[repr(u8)]
enum SignatureType {
    CertificateTimestamp = 0,
    #[allow(dead_code)]
    TreeHash = 1,
}
#[derive(PartialEq, Debug, TlsSerializeBytes, TlsSize)]
#[repr(u16)]
enum LogEntryType {
    #[allow(dead_code)]
    X509Entry = 0,
    PrecertEntry = 1,
}
#[derive(PartialEq, Debug, TlsSerializeBytes, TlsSize)]
struct PreCert {
    issuer_key_hash: [u8; 32],
    tbs_certificate: TlsByteVecU24,
}
#[derive(PartialEq, Debug, TlsSerializeBytes, TlsSize)]
#[repr(u16)]
enum SignedEntry {
    #[tls_codec(discriminant = "LogEntryType::PrecertEntry")]
    PrecertEntry(PreCert),
}
#[derive(PartialEq, Debug, TlsSerializeBytes, TlsSize)]
struct SCTSignedPayload {
    version: SctVersion,
    signature_type: SignatureType,
    timestamp: u64,
    signed_entry: SignedEntry,
    extensions: TlsByteVecU16,
}

fn sct_signed_payload(
    leaf_der: &[u8],
    issuer_spki_der: &[u8],
    timestamp: u64,
) -> anyhow::Result<Vec<u8>> {
    let cert = X509Certificate::from_der(leaf_der)?;
    // issuer key hash is SHA-256 over DER-encoded SPKI
    let issuer_key_hash: [u8; 32] = Sha256::digest(issuer_spki_der).into();
    // Precert TBS = leaf TBS with SCT extension removed (no-op if absent)
    let mut tbs_precert = cert.tbs_certificate.clone();
    tbs_precert.extensions = tbs_precert.extensions.map(|exts| {
        exts.iter()
            .filter(|v| v.extn_id != const_oid::db::rfc6962::CT_PRECERT_SCTS)
            .cloned()
            .collect()
    });
    let mut tbs_precert_der = Vec::new();
    tbs_precert.encode_to_vec(&mut tbs_precert_der)?;

    let payload = SCTSignedPayload {
        version: SctVersion::V1,
        signature_type: SignatureType::CertificateTimestamp,
        timestamp,
        signed_entry: SignedEntry::PrecertEntry(PreCert {
            issuer_key_hash,
            tbs_certificate: tbs_precert_der.as_slice().into(),
        }),
        extensions: TlsByteVecU16::from_slice(&[]),
    };
    Ok(payload.tls_serialize()?)
}

fn main() -> anyhow::Result<()> {
    // Output paths
    let root = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../../crates/plugin_host/tests/golden/sigstore");
    let trust_dir = root.join("trust");

    // 1) Create a self-signed CA (Fulcio root substitute)
    let mut ca_params = CertificateParams::new(vec![]);
    ca_params.alg = &PKCS_ECDSA_P256_SHA256;
    ca_params.is_ca = IsCa::Ca(BasicConstraints::Constrained(0));
    let mut ca_dn = DistinguishedName::new();
    ca_dn.push(DnType::CommonName, "ORCA Test Fulcio Root");
    ca_params.distinguished_name = ca_dn;
    let ca_cert = Certificate::from_params(ca_params)?;
    write(&trust_dir.join("fulcio_root.pem"), ca_cert.serialize_pem().unwrap().as_bytes());
    let ca_der = ca_cert.serialize_der()?;

    // 2) Deterministic P-256 leaf keypair (signatures and cert share this key)
    let leaf_seed = Sha256::digest(b"orca-leaf-key-seed");
    let leaf_sk = P256SecretKey::from_slice(leaf_seed.as_ref()).expect("valid scalar");
    let leaf_pkcs8 = leaf_sk.to_pkcs8_der().unwrap();
    let leaf_kp1 = KeyPair::from_der(leaf_pkcs8.as_bytes()).unwrap();
    let leaf_kp2 = KeyPair::from_der(leaf_pkcs8.as_bytes()).unwrap();

    // 3) Deterministic CTFE keypair and write public key PEM for trust
    let ctfe_seed = Sha256::digest(b"orca-ctfe-key-seed");
    let ctfe_signing = P256SigningKey::from(P256SecretKey::from_slice(ctfe_seed.as_ref())?);
    let ctfe_vk = ctfe_signing.verifying_key();
    let ctfe_spki_der = ctfe_vk.to_public_key_der()?; // DER SubjectPublicKeyInfo
                                                      // Write PUBLIC KEY PEM for CTFE (SubjectPublicKeyInfo)
    let ctfe_pem = pem::Pem::new("PUBLIC KEY", ctfe_spki_der.as_ref().to_vec());
    write(&trust_dir.join("ctfe_pubkey.pem"), pem::encode(&ctfe_pem).as_bytes());

    // 4) Leaf certificate (SAN=email, OIDC issuer extension) signed by CA (initial, no SCT)
    let mut leaf_params = CertificateParams::new(vec![]);
    leaf_params.alg = &PKCS_ECDSA_P256_SHA256;
    leaf_params.key_pair = Some(leaf_kp1);
    let mut leaf_dn = DistinguishedName::new();
    leaf_dn.push(DnType::CommonName, "ORCA Test Leaf");
    leaf_params.distinguished_name = leaf_dn;
    leaf_params.subject_alt_names = vec![SanType::Rfc822Name("test@example.com".to_string())];
    // Fulcio OIDC issuer extension OID 1.3.6.1.4.1.57264.1.1 with raw ASCII content (not DER)
    let issuer_raw = b"https://fulcio.sigstore.dev".to_vec();
    leaf_params
        .custom_extensions
        .push(CustomExtension::from_oid_content(&[1, 3, 6, 1, 4, 1, 57264, 1, 1], issuer_raw));
    // Required key usage and extended key usage for code signing
    leaf_params.key_usages = vec![KeyUsagePurpose::DigitalSignature];
    leaf_params.extended_key_usages = vec![ExtendedKeyUsagePurpose::CodeSigning];
    let leaf_cert_no_sct = Certificate::from_params(leaf_params)?;
    let leaf_der_no_sct = leaf_cert_no_sct.serialize_der_with_signer(&ca_cert)?;

    // 5) Construct an embedded SCT for the leaf and rebuild the leaf cert with SCT extension
    let nb_secs = {
        let cert = X509Certificate::from_der(&leaf_der_no_sct)?;
        cert.tbs_certificate.validity.not_before.to_unix_duration().as_secs()
    };
    // issuer_key_hash uses DER-encoded SubjectPublicKeyInfo of the issuer (CA)
    let ca_x509 = X509Certificate::from_der(&ca_der)?;
    let mut ca_spki_der = Vec::new();
    ca_x509.tbs_certificate.subject_public_key_info.encode_to_vec(&mut ca_spki_der)?;
    let sct_payload = sct_signed_payload(&leaf_der_no_sct, &ca_spki_der, nb_secs + 1)?;
    let sct_raw: p256::ecdsa::Signature = <p256::ecdsa::SigningKey as signature::Signer<
        p256::ecdsa::Signature,
    >>::sign(&ctfe_signing, &sct_payload);
    let sct_sig: p256::ecdsa::DerSignature = sct_raw.to_der();
    let log_id: [u8; 32] = Sha256::digest(ctfe_spki_der.as_bytes()).into();
    let sct = SignedCertificateTimestamp {
        version: SctVersion::V1,
        log_id: LogId { key_id: log_id },
        timestamp: nb_secs + 1,
        extensions: TlsByteVecU16::from_slice(&[]),
        signature: SctDigitallySigned {
            algorithm: SignatureAndHashAlgorithm {
                hash: HashAlgorithm::Sha256,
                signature: SignatureAlgorithm::Ecdsa,
            },
            signature: TlsByteVecU16::from_slice(sct_sig.as_bytes()),
        },
    };
    let serialized =
        SerializedSct::new(sct).map_err(|e| anyhow::anyhow!("sct serialize: {:?}", e))?;
    let sct_list = SignedCertificateTimestampList::new(&[serialized])
        .map_err(|e| anyhow::anyhow!("sct list: {:?}", e))?;
    let sct_ext_der = sct_list.to_der()?;

    // Rebuild leaf with SCT extension
    let mut leaf_with_sct_params = CertificateParams::new(vec![]);
    leaf_with_sct_params.alg = &PKCS_ECDSA_P256_SHA256;
    let mut leaf_dn2 = DistinguishedName::new();
    leaf_dn2.push(DnType::CommonName, "ORCA Test Leaf");
    leaf_with_sct_params.distinguished_name = leaf_dn2;
    leaf_with_sct_params.subject_alt_names =
        vec![SanType::Rfc822Name("test@example.com".to_string())];
    leaf_with_sct_params.custom_extensions.push(CustomExtension::from_oid_content(
        &[1, 3, 6, 1, 4, 1, 57264, 1, 1],
        b"https://fulcio.sigstore.dev".to_vec(),
    ));
    leaf_with_sct_params.key_usages = vec![KeyUsagePurpose::DigitalSignature];
    leaf_with_sct_params.extended_key_usages = vec![ExtendedKeyUsagePurpose::CodeSigning];
    // CT Precert SCT list extension: 1.3.6.1.4.1.11129.2.4.2 (non-critical)
    leaf_with_sct_params
        .custom_extensions
        .push(CustomExtension::from_oid_content(&[1, 3, 6, 1, 4, 1, 11129, 2, 4, 2], sct_ext_der));
    leaf_with_sct_params.key_pair = Some(leaf_kp2);
    let leaf_cert = Certificate::from_params(leaf_with_sct_params)?;
    let leaf_der = leaf_cert.serialize_der_with_signer(&ca_cert)?;

    // 6) Compute digest of the test wasm and sign it with the leaf key
    let wasm_path = root.join("test_plugin.wasm");
    let wasm = fs::read(&wasm_path).expect("read test wasm");
    let digest = Sha256::digest(&wasm);
    let leaf_signer = P256SigningKey::from(leaf_sk);
    // Sigstore verifies against the prehashed SHA-256 digest; sign as prehash to match
    let sig_raw_ecdsa: p256::ecdsa::Signature =
        <p256::ecdsa::SigningKey as ecdsa::signature::hazmat::PrehashSigner<
            p256::ecdsa::Signature,
        >>::sign_prehash(&leaf_signer, &digest)
        .expect("sign prehash");
    // Use DER-encoded ECDSA signature bytes, which sigstore expects for ECDSA_P256_SHA256_ASN1
    let sig_der = sig_raw_ecdsa.to_der();

    // Build PEM for leaf using x509-cert (LF line endings) to match sigstore expected formatting
    let x509_leaf = X509Certificate::from_der(&leaf_der)?;
    let leaf_pem_text = x509_leaf.to_pem(LineEnding::LF)?;
    let base64_pem = B64.encode(leaf_pem_text.as_bytes());

    // Build canonicalized hashedrekord body expected by verifier
    let hashedrekord = json!({
        "kind": "hashedrekord",
        "apiVersion": "0.0.1",
        "spec": {
            "signature": { "content": B64.encode(sig_der.as_bytes()), "publicKey": { "content": base64_pem } },
            "data": { "hash": { "algorithm": "sha256", "value": hex::encode(&digest) } }
        }
    });
    let canonicalized_body_b64 = B64.encode(serde_json::to_vec(&hashedrekord)?);

    // integratedTime must fall within cert validity; use not_before + 1s
    let nb_secs = {
        let cert = X509Certificate::from_der(&leaf_der)?;
        cert.tbs_certificate.validity.not_before.to_unix_duration().as_secs()
    };

    // 7) Build three bundle variants (valid, tampered, invalid-signature)
    let valid = json!({
        "mediaType": "application/vnd.dev.sigstore.bundle+json;version=0.1",
        "messageSignature": {
            "messageDigest": { "algorithm": "SHA2_256", "digest": B64.encode(&digest) },
            "signature": B64.encode(sig_der.as_bytes())
        },
        "verificationMaterial": {
            "x509CertificateChain": { "certificates": [ { "rawBytes": B64.encode(&leaf_der) } ] },
            "tlogEntries": [
                {
                    "logIndex": 1,
                    "logId": { "keyId": B64.encode(b"orca-test-log") },
                    "kindVersion": { "kind": "hashedrekord", "version": "0.0.1" },
                    "integratedTime": nb_secs + 1,
                    "inclusionPromise": { "signedEntryTimestamp": B64.encode(b"dummy-set") },
                    "canonicalizedBody": canonicalized_body_b64
                }
            ]
        }
    });

    let valid_path = root.join("valid_bundle.json");
    write(&valid_path, serde_json::to_vec_pretty(&valid)?.as_slice());

    // Tampered: alter digest base64 (flip first char)
    let mut tampered = valid.clone();
    if let Some(d) = tampered["messageSignature"]["messageDigest"]["digest"].as_str() {
        let mut s = d.as_bytes().to_vec();
        if let Some(b) = s.first_mut() {
            *b = b'A';
        }
        tampered["messageSignature"]["messageDigest"]["digest"] =
            serde_json::Value::String(String::from_utf8(s).unwrap());
    }
    write(&root.join("tampered_bundle.json"), serde_json::to_vec_pretty(&tampered)?.as_slice());

    // Invalid signature: not base64
    let mut invalid = valid.clone();
    invalid["messageSignature"]["signature"] = serde_json::Value::String("!!!not-base64!!!".into());
    write(&root.join("invalid_signature.json"), serde_json::to_vec_pretty(&invalid)?.as_slice());

    Ok(())
}
