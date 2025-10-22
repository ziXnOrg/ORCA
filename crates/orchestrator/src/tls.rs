use std::fs::File;
use std::io::BufReader;
use rustls::{Certificate, PrivateKey, RootCertStore, ServerConfig};
use rustls_pemfile::{certs, pkcs8_private_keys};
use tonic::transport::ServerTlsConfig;

fn load_cert_chain(path: &str) -> anyhow::Result<Vec<Certificate>> {
    let mut reader = BufReader::new(File::open(path)?);
    Ok(certs(&mut reader)?.into_iter().map(Certificate).collect())
}

fn load_private_key(path: &str) -> anyhow::Result<PrivateKey> {
    let mut reader = BufReader::new(File::open(path)?);
    let mut keys = pkcs8_private_keys(&mut reader)?;
    anyhow::ensure!(!keys.is_empty(), "no private keys found");
    Ok(PrivateKey(keys.remove(0)))
}

fn load_ca(path: &str) -> anyhow::Result<RootCertStore> {
    let mut store = RootCertStore::empty();
    let mut reader = BufReader::new(File::open(path)?);
    let added = store.add_pem_file(&mut reader).map(|(added, _)| added)?;
    anyhow::ensure!(added > 0, "no CA certs added");
    Ok(store)
}

pub fn server_tls_from_env() -> anyhow::Result<ServerTlsConfig> {
    let cert = std::env::var("AGENT_TLS_CERT_FILE")?;
    let key = std::env::var("AGENT_TLS_KEY_FILE")?;
    let ca = std::env::var("AGENT_TLS_CA_FILE")?;

    let cert_chain = load_cert_chain(&cert)?;
    let private_key = load_private_key(&key)?;
    let client_roots = load_ca(&ca)?;

    let mut cfg = ServerConfig::builder().with_safe_defaults().with_client_cert_verifier(std::sync::Arc::new(rustls::server::AllowAnyAuthenticatedClient::new(client_roots))).with_single_cert(cert_chain, private_key)?;
    cfg.alpn_protocols = vec![b"h2".to_vec()];

    Ok(ServerTlsConfig::new().rustls_server_config(std::sync::Arc::new(cfg)))
}
