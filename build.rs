fn main() -> Result<(), Box<dyn std::error::Error>> {
    println!("cargo:rerun-if-changed=proto/mycqu_service/");
    tonic_prost_build::configure()
        .build_client(false)
        .compile_protos(&["proto/mycqu_service/mycqu_service.proto"], &["proto"])?;
    Ok(())
}
