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
        // And the TypeScript the browser client is written against, from
        // these same types: `src/ts.rs` writes
        // `web/src/generated/serving.ts` from them. It is generated here
        // and not from the protocol, because the JSON is these types
        // rendered by serde and is not proto3 JSON — proto3 would spell
        // the field `publicId` and send a `uint64` as a string, and this
        // surface sends `public_id` and `"total": 74`. Generating from
        // the layer that writes the bytes is the only way the two cannot
        // drift; ts-rs reads the same serde attributes above, so the
        // renamed oneof variants come out renamed.
        //
        // `cfg(test)`: ts-rs is a dev-dependency, so the derive exists
        // only when this crate is built for its own tests.
        .type_attribute(
            ".olrht.v1",
            "#[cfg_attr(test, derive(::ts_rs::TS), ts(export_to = \"serving.ts\"))]",
        )
        // Everything lands in one file, so two types cannot share a
        // name. Both oneofs are `Outcome` in Rust, and `Record` and
        // `Date` are TypeScript's own — the names here are the ones the
        // client already used by hand. A TypeScript type name is not on
        // the wire; no field is renamed.
        .type_attribute(
            ".olrht.v1.ResolveResponse.outcome",
            "#[cfg_attr(test, ts(rename = \"ResolveOutcome\"))]",
        )
        .type_attribute(
            ".olrht.v1.PathResponse.outcome",
            "#[cfg_attr(test, ts(rename = \"PathOutcome\"))]",
        )
        .type_attribute(
            ".olrht.v1.Record",
            "#[cfg_attr(test, ts(rename = \"SourceRecord\"))]",
        )
        .type_attribute(
            ".olrht.v1.Date",
            "#[cfg_attr(test, ts(rename = \"PubDate\"))]",
        )
        .compile_fds(descriptors)?;
    println!("cargo:rerun-if-changed=proto/olrht/v1/serving.proto");
    Ok(())
}
