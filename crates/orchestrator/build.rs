fn main() {
    // Ensure `protoc` is available in CI and dev environments without
    // requiring system packages. Falls back to vendored binary if PROTOC
    // is not already set by the environment.
    if std::env::var_os("PROTOC").is_none() {
        if let Ok(pb) = protoc_bin_vendored::protoc_bin_path() {
            std::env::set_var("PROTOC", pb);
        }
    }

    let proto = "../../Docs/API/orca_v1.proto";
    tonic_build::configure()
        .build_server(true)
        .build_client(true)
        .type_attribute(".", "#[derive(serde::Serialize, serde::Deserialize)]")
        .type_attribute(".", "#[serde(default)]")
        .compile(&[proto], &["../../Docs/API"])
        .expect("proto build failed");
    println!("cargo:rerun-if-changed={}", proto);
}
