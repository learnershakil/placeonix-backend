use tracing::info;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let _telemetry = telemetry::init("placeonix-worker-analytics")?;
    info!("analytics worker starting");
    Ok(())
}
