fn main() -> Result<(), Box<dyn std::error::Error>> {
    // Always build protobuf for gRPC service
    tonic_build::configure()
        .build_server(true)
        .build_client(true)
        .compile(&["proto/sigma.proto"], &["proto"])?;
    Ok(())
}
