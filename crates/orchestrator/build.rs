fn main() {
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
