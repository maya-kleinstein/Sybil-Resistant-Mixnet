fn main () -> Result<(), Box<dyn std::error::Error>> {
    tonic_build::compile_protos("client/client.proto")?;
    Ok(())
}