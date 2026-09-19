//! Compiles the protocol with `protox`, in pure Rust, so nothing outside
//! cargo is installed; keeps the descriptor set for reflection.

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let out = std::path::PathBuf::from(std::env::var("OUT_DIR")?);
    let descriptors = protox::compile(["olrht/v1/serving.proto"], ["proto"])?;
    let bytes = prost::Message::encode_to_vec(&descriptors);
    std::fs::write(out.join("olrht.bin"), &bytes)?;
    tonic_prost_build::configure()
        .build_server(true)
        .build_client(true)
        // The JSON surface (serving plan §7) serves these same messages,
        // serialized as JSON instead of protobuf, so the two transports
        // cannot drift: one shape, one mapper in `convert`, two
        // encodings. Field names are the proto's, which prost spells in
        // snake case, so nothing renames them. The oneof is an enum, and
        // its variants are renamed back to the proto's field names.
        .type_attribute(".olrht.v1", "#[derive(::serde::Serialize)]")
        .type_attribute(
            ".olrht.v1.ResolveResponse.outcome",
            "#[serde(rename_all = \"snake_case\")]",
        )
        .type_attribute(
            ".olrht.v1.PathResponse.outcome",
            "#[serde(rename_all = \"snake_case\")]",
        )
        .compile_fds(descriptors)?;
    println!("cargo:rerun-if-changed=proto/olrht/v1/serving.proto");
    Ok(())
}
