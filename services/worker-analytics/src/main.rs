use placeonix_config::AppConfig;
use placeonix_worker_core::{WorkerKind, WorkerRuntimePlan};
use tracing::info;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let config = AppConfig::from_env("placeonix-worker-analytics")?;
    let _telemetry = telemetry::init(&config.service.name)?;
    let plan = WorkerRuntimePlan::for_worker(WorkerKind::Analytics);
    info!(
        service = %config.service.name,
        environment = %config.service.environment,
        streams = ?plan.streams(),
        consumer_group = %plan.consumer_group(),
        dead_letter_stream = %plan.dead_letter_stream(),
        "analytics worker starting"
    );
    Ok(())
}
