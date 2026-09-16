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
        .compile_fds(descriptors)?;
    println!("cargo:rerun-if-changed=proto/olrht/v1/serving.proto");
    Ok(())
}
