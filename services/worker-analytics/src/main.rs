use placeonix_config::AppConfig;
use tracing::info;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let config = AppConfig::from_env("placeonix-worker-analytics")?;
    let _telemetry = telemetry::init(&config.service.name)?;
    info!(
        service = %config.service.name,
        environment = %config.service.environment,
        "analytics worker starting"
    );
    Ok(())
}
