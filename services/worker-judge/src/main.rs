use tracing::info;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let _telemetry = telemetry::init("placeonix-worker-judge")?;
    info!("judge worker starting");
    Ok(())
}
