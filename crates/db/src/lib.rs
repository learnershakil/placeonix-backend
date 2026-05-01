use std::time::Duration;

use placeonix_config::DatabaseConfig;
use sqlx::{
    postgres::{PgConnectOptions, PgPoolOptions},
    PgPool,
};

pub mod control_schema;

#[derive(Clone)]
pub struct DatabasePools {
    control: PgPool,
    tenant: PgPool,
}

impl DatabasePools {
    pub fn control(&self) -> &PgPool {
        &self.control
    }

    pub fn tenant(&self) -> &PgPool {
        &self.tenant
    }

    pub async fn verify_connectivity(&self) -> Result<(), sqlx::Error> {
        sqlx::query("SELECT 1").execute(&self.control).await?;
        sqlx::query("SELECT 1").execute(&self.tenant).await?;
        Ok(())
    }
}

pub async fn connect(config: &DatabaseConfig) -> Result<DatabasePools, sqlx::Error> {
    let control = connect_pool(config, config.control_url.expose()).await?;
    let tenant = connect_pool(config, config.tenant_url.expose()).await?;

    Ok(DatabasePools { control, tenant })
}

async fn connect_pool(config: &DatabaseConfig, database_url: &str) -> Result<PgPool, sqlx::Error> {
    let connect_options = database_url.parse::<PgConnectOptions>()?;

    PgPoolOptions::new()
        .min_connections(config.min_connections)
        .max_connections(config.max_connections)
        .acquire_timeout(Duration::from_secs(config.acquire_timeout_secs))
        .test_before_acquire(true)
        .connect_with(connect_options)
        .await
}
