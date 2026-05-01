use tracing::info;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let _telemetry = telemetry::init("placeonix-worker-media")?;
    info!("media worker starting");
    Ok(())
}
