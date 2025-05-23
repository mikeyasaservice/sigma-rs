fn main() -> Result<(), Box<dyn std::error::Error>> {
    // Check if service feature is enabled through environment variable
    let service_enabled = std::env::var("CARGO_FEATURE_SERVICE").is_ok();
    
    if service_enabled {
        tonic_build::configure()
            .build_server(true)
            .build_client(true)
            .compile(&["proto/sigma.proto"], &["proto"])?;
    }
    Ok(())
}