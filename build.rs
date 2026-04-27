fn main() -> Result<(), Box<dyn std::error::Error>> {
    tonic_prost_build::compile_protos("proto/filer.proto")?;
    tonic_prost_build::compile_protos("proto/iam.proto")?;
    Ok(())
}