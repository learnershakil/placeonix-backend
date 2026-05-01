use std::{
    env,
    error::Error,
    fmt,
    net::{AddrParseError, SocketAddr},
    str::FromStr,
};

use url::{ParseError as UrlParseError, Url};

const DEFAULT_HTTP_BIND_ADDR: &str = "0.0.0.0:8080";
const DEFAULT_HTTP_REQUEST_TIMEOUT_SECS: &str = "30";
const DEFAULT_HTTP_MAX_BODY_BYTES: &str = "1048576";
const DEFAULT_CONTROL_DATABASE_URL: &str =
    "postgresql://placeonix:placeonix_dev@localhost:5432/placeonix_control";
const DEFAULT_TENANT_DATABASE_URL: &str =
    "postgresql://placeonix:placeonix_dev@localhost:5433/placeonix_tenant";
const DEFAULT_DB_MAX_CONNECTIONS: &str = "10";
const DEFAULT_DB_MIN_CONNECTIONS: &str = "0";
const DEFAULT_DB_ACQUIRE_TIMEOUT_SECS: &str = "3";
const DEFAULT_REDIS_URL: &str = "redis://localhost:6379";
const DEFAULT_RATE_LIMIT_WINDOW_SECS: &str = "60";
const DEFAULT_RATE_LIMIT_IP_REQUESTS: &str = "120";
const DEFAULT_RATE_LIMIT_USER_REQUESTS: &str = "600";
const DEFAULT_RATE_LIMIT_ROUTE_REQUESTS: &str = "3000";
const DEFAULT_NATS_URL: &str = "nats://localhost:4222";
const DEFAULT_S3_ENDPOINT: &str = "http://localhost:9000";
const DEFAULT_S3_BUCKET: &str = "placeonix";
const DEFAULT_S3_ACCESS_KEY: &str = "placeonix";
const DEFAULT_S3_SECRET_KEY: &str = "placeonix_dev";

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AppConfig {
    pub service: ServiceConfig,
    pub http: HttpConfig,
    pub databases: DatabaseConfig,
    pub redis_url: SecretString,
    pub rate_limits: RateLimitConfig,
    pub nats_url: SecretString,
    pub object_storage: ObjectStorageConfig,
}

impl AppConfig {
    pub fn from_env(service_name: impl Into<String>) -> Result<Self, ConfigError> {
        Self::from_source(service_name, |key| env::var(key).ok())
    }

    pub fn from_source<F>(service_name: impl Into<String>, get: F) -> Result<Self, ConfigError>
    where
        F: Fn(&str) -> Option<String>,
    {
        let service_name = service_name.into();
        if service_name.trim().is_empty() {
            return Err(ConfigError::MissingRequired {
                key: "service_name",
            });
        }

        let environment = get_or_default(&get, "APP_ENV", "local")
            .parse::<DeploymentEnvironment>()
            .map_err(|value| ConfigError::InvalidEnvironment { value })?;
        let bind_addr = parse_addr(
            "HTTP_BIND_ADDR",
            get_or_default(&get, "HTTP_BIND_ADDR", DEFAULT_HTTP_BIND_ADDR),
        )?;
        let request_timeout_secs = parse_positive_u64(
            "HTTP_REQUEST_TIMEOUT_SECS",
            get_or_default(
                &get,
                "HTTP_REQUEST_TIMEOUT_SECS",
                DEFAULT_HTTP_REQUEST_TIMEOUT_SECS,
            ),
        )?;
        let max_body_bytes = parse_positive_usize(
            "HTTP_MAX_BODY_BYTES",
            get_or_default(&get, "HTTP_MAX_BODY_BYTES", DEFAULT_HTTP_MAX_BODY_BYTES),
        )?;

        let control_url = required_secret_url(
            "CONTROL_DATABASE_URL",
            get_or_default(&get, "CONTROL_DATABASE_URL", DEFAULT_CONTROL_DATABASE_URL),
        )?;
        let tenant_url = required_secret_url(
            "TENANT_DATABASE_URL",
            get_or_default(&get, "TENANT_DATABASE_URL", DEFAULT_TENANT_DATABASE_URL),
        )?;
        let max_connections = parse_positive_u32(
            "DB_MAX_CONNECTIONS",
            get_or_default(&get, "DB_MAX_CONNECTIONS", DEFAULT_DB_MAX_CONNECTIONS),
        )?;
        let min_connections = parse_u32(
            "DB_MIN_CONNECTIONS",
            get_or_default(&get, "DB_MIN_CONNECTIONS", DEFAULT_DB_MIN_CONNECTIONS),
        )?;
        if min_connections > max_connections {
            return Err(ConfigError::InvalidNumber {
                key: "DB_MIN_CONNECTIONS",
                value: min_connections.to_string(),
                reason: "must be less than or equal to DB_MAX_CONNECTIONS",
            });
        }
        let acquire_timeout_secs = parse_positive_u64(
            "DB_ACQUIRE_TIMEOUT_SECS",
            get_or_default(
                &get,
                "DB_ACQUIRE_TIMEOUT_SECS",
                DEFAULT_DB_ACQUIRE_TIMEOUT_SECS,
            ),
        )?;
        let redis_url = required_secret_url(
            "REDIS_URL",
            get_or_default(&get, "REDIS_URL", DEFAULT_REDIS_URL),
        )?;
        let rate_limits = RateLimitConfig {
            window_secs: parse_positive_u64(
                "RATE_LIMIT_WINDOW_SECS",
                get_or_default(
                    &get,
                    "RATE_LIMIT_WINDOW_SECS",
                    DEFAULT_RATE_LIMIT_WINDOW_SECS,
                ),
            )?,
            per_ip_requests: parse_positive_u32(
                "RATE_LIMIT_IP_REQUESTS",
                get_or_default(
                    &get,
                    "RATE_LIMIT_IP_REQUESTS",
                    DEFAULT_RATE_LIMIT_IP_REQUESTS,
                ),
            )?,
            per_user_requests: parse_positive_u32(
                "RATE_LIMIT_USER_REQUESTS",
                get_or_default(
                    &get,
                    "RATE_LIMIT_USER_REQUESTS",
                    DEFAULT_RATE_LIMIT_USER_REQUESTS,
                ),
            )?,
            per_route_requests: parse_positive_u32(
                "RATE_LIMIT_ROUTE_REQUESTS",
                get_or_default(
                    &get,
                    "RATE_LIMIT_ROUTE_REQUESTS",
                    DEFAULT_RATE_LIMIT_ROUTE_REQUESTS,
                ),
            )?,
        };
        let nats_url = required_secret_url(
            "NATS_URL",
            get_or_default(&get, "NATS_URL", DEFAULT_NATS_URL),
        )?;

        let endpoint = get_or_default(&get, "S3_ENDPOINT", DEFAULT_S3_ENDPOINT);
        parse_url("S3_ENDPOINT", &endpoint)?;

        let bucket = non_empty(
            "S3_BUCKET",
            get_or_default(&get, "S3_BUCKET", DEFAULT_S3_BUCKET),
        )?;
        let access_key = required_secret(
            "S3_ACCESS_KEY",
            get_or_default(&get, "S3_ACCESS_KEY", DEFAULT_S3_ACCESS_KEY),
        )?;
        let secret_key = required_secret(
            "S3_SECRET_KEY",
            get_or_default(&get, "S3_SECRET_KEY", DEFAULT_S3_SECRET_KEY),
        )?;

        Ok(Self {
            service: ServiceConfig {
                name: service_name,
                environment,
            },
            http: HttpConfig {
                bind_addr,
                request_timeout_secs,
                max_body_bytes,
            },
            databases: DatabaseConfig {
                control_url,
                tenant_url,
                max_connections,
                min_connections,
                acquire_timeout_secs,
            },
            redis_url,
            rate_limits,
            nats_url,
            object_storage: ObjectStorageConfig {
                endpoint,
                bucket,
                access_key,
                secret_key,
            },
        })
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ServiceConfig {
    pub name: String,
    pub environment: DeploymentEnvironment,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct HttpConfig {
    pub bind_addr: SocketAddr,
    pub request_timeout_secs: u64,
    pub max_body_bytes: usize,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DatabaseConfig {
    pub control_url: SecretString,
    pub tenant_url: SecretString,
    pub max_connections: u32,
    pub min_connections: u32,
    pub acquire_timeout_secs: u64,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RateLimitConfig {
    pub window_secs: u64,
    pub per_ip_requests: u32,
    pub per_user_requests: u32,
    pub per_route_requests: u32,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ObjectStorageConfig {
    pub endpoint: String,
    pub bucket: String,
    pub access_key: SecretString,
    pub secret_key: SecretString,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DeploymentEnvironment {
    Local,
    Test,
    Staging,
    Production,
}

impl fmt::Display for DeploymentEnvironment {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let value = match self {
            Self::Local => "local",
            Self::Test => "test",
            Self::Staging => "staging",
            Self::Production => "production",
        };
        f.write_str(value)
    }
}

impl FromStr for DeploymentEnvironment {
    type Err = String;

    fn from_str(value: &str) -> Result<Self, Self::Err> {
        match value.trim().to_ascii_lowercase().as_str() {
            "local" | "dev" | "development" => Ok(Self::Local),
            "test" => Ok(Self::Test),
            "staging" | "stage" => Ok(Self::Staging),
            "prod" | "production" => Ok(Self::Production),
            _ => Err(value.to_owned()),
        }
    }
}

#[derive(Clone, PartialEq, Eq)]
pub struct SecretString(String);

impl SecretString {
    pub fn new(value: impl Into<String>) -> Self {
        Self(value.into())
    }

    pub fn expose(&self) -> &str {
        &self.0
    }
}

impl fmt::Debug for SecretString {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("SecretString(**redacted**)")
    }
}

impl fmt::Display for SecretString {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("**redacted**")
    }
}

#[derive(Debug)]
pub enum ConfigError {
    MissingRequired {
        key: &'static str,
    },
    InvalidEnvironment {
        value: String,
    },
    InvalidBindAddr {
        key: &'static str,
        value: String,
        source: AddrParseError,
    },
    InvalidUrl {
        key: &'static str,
        value: String,
        source: UrlParseError,
    },
    InvalidNumber {
        key: &'static str,
        value: String,
        reason: &'static str,
    },
}

impl fmt::Display for ConfigError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::MissingRequired { key } => write!(f, "missing required config value `{key}`"),
            Self::InvalidEnvironment { value } => {
                write!(
                    f,
                    "invalid APP_ENV `{value}`; expected local, test, staging, or production"
                )
            }
            Self::InvalidBindAddr { key, value, .. } => {
                write!(f, "invalid socket address for `{key}`: `{value}`")
            }
            Self::InvalidUrl { key, value, .. } => {
                write!(f, "invalid URL for `{key}`: `{value}`")
            }
            Self::InvalidNumber { key, value, reason } => {
                write!(f, "invalid number for `{key}`: `{value}`; {reason}")
            }
        }
    }
}

impl Error for ConfigError {
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        match self {
            Self::InvalidBindAddr { source, .. } => Some(source),
            Self::InvalidUrl { source, .. } => Some(source),
            Self::MissingRequired { .. }
            | Self::InvalidEnvironment { .. }
            | Self::InvalidNumber { .. } => None,
        }
    }
}

fn get_or_default<F>(get: &F, key: &'static str, default: &'static str) -> String
where
    F: Fn(&str) -> Option<String>,
{
    get(key).unwrap_or_else(|| default.to_owned())
}

fn parse_addr(key: &'static str, value: String) -> Result<SocketAddr, ConfigError> {
    value
        .parse()
        .map_err(|source| ConfigError::InvalidBindAddr { key, value, source })
}

fn required_secret_url(key: &'static str, value: String) -> Result<SecretString, ConfigError> {
    parse_url(key, &value)?;
    Ok(SecretString::new(value))
}

fn required_secret(key: &'static str, value: String) -> Result<SecretString, ConfigError> {
    non_empty(key, value).map(SecretString::new)
}

fn non_empty(key: &'static str, value: String) -> Result<String, ConfigError> {
    if value.trim().is_empty() {
        Err(ConfigError::MissingRequired { key })
    } else {
        Ok(value)
    }
}

fn parse_positive_u64(key: &'static str, value: String) -> Result<u64, ConfigError> {
    let parsed = value
        .parse::<u64>()
        .map_err(|_| ConfigError::InvalidNumber {
            key,
            value: value.clone(),
            reason: "expected a positive integer",
        })?;

    if parsed == 0 {
        Err(ConfigError::InvalidNumber {
            key,
            value,
            reason: "must be greater than zero",
        })
    } else {
        Ok(parsed)
    }
}

fn parse_u32(key: &'static str, value: String) -> Result<u32, ConfigError> {
    value
        .parse::<u32>()
        .map_err(|_| ConfigError::InvalidNumber {
            key,
            value,
            reason: "expected a non-negative integer",
        })
}

fn parse_positive_u32(key: &'static str, value: String) -> Result<u32, ConfigError> {
    let parsed = parse_u32(key, value.clone())?;
    if parsed == 0 {
        Err(ConfigError::InvalidNumber {
            key,
            value,
            reason: "must be greater than zero",
        })
    } else {
        Ok(parsed)
    }
}

fn parse_positive_usize(key: &'static str, value: String) -> Result<usize, ConfigError> {
    let parsed = parse_positive_u64(key, value.clone())?;
    usize::try_from(parsed).map_err(|_| ConfigError::InvalidNumber {
        key,
        value,
        reason: "value exceeds this platform's usize range",
    })
}

fn parse_url(key: &'static str, value: &str) -> Result<Url, ConfigError> {
    Url::parse(value).map_err(|source| ConfigError::InvalidUrl {
        key,
        value: value.to_owned(),
        source,
    })
}

#[cfg(test)]
mod tests {
    use super::{AppConfig, ConfigError, DeploymentEnvironment, SecretString};

    #[test]
    fn loads_local_defaults() {
        let config = AppConfig::from_source("placeonix-api", |_| None).expect("config loads");

        assert_eq!(config.service.name, "placeonix-api");
        assert_eq!(config.service.environment, DeploymentEnvironment::Local);
        assert_eq!(config.http.bind_addr.to_string(), "0.0.0.0:8080");
        assert_eq!(config.http.request_timeout_secs, 30);
        assert_eq!(config.http.max_body_bytes, 1_048_576);
        assert_eq!(config.databases.max_connections, 10);
        assert_eq!(config.databases.min_connections, 0);
        assert_eq!(config.databases.acquire_timeout_secs, 3);
        assert_eq!(config.rate_limits.window_secs, 60);
        assert_eq!(config.rate_limits.per_ip_requests, 120);
        assert_eq!(config.rate_limits.per_user_requests, 600);
        assert_eq!(config.rate_limits.per_route_requests, 3000);
        assert_eq!(config.object_storage.bucket, "placeonix");
    }

    #[test]
    fn applies_environment_overrides() {
        let config = AppConfig::from_source("placeonix-api", |key| match key {
            "APP_ENV" => Some("production".to_owned()),
            "HTTP_BIND_ADDR" => Some("127.0.0.1:9009".to_owned()),
            "HTTP_REQUEST_TIMEOUT_SECS" => Some("12".to_owned()),
            "HTTP_MAX_BODY_BYTES" => Some("2048".to_owned()),
            "DB_MAX_CONNECTIONS" => Some("24".to_owned()),
            "DB_MIN_CONNECTIONS" => Some("2".to_owned()),
            "DB_ACQUIRE_TIMEOUT_SECS" => Some("1".to_owned()),
            "RATE_LIMIT_WINDOW_SECS" => Some("10".to_owned()),
            "RATE_LIMIT_IP_REQUESTS" => Some("20".to_owned()),
            "RATE_LIMIT_USER_REQUESTS" => Some("30".to_owned()),
            "RATE_LIMIT_ROUTE_REQUESTS" => Some("40".to_owned()),
            "S3_BUCKET" => Some("placeonix-prod".to_owned()),
            _ => None,
        })
        .expect("config loads");

        assert_eq!(
            config.service.environment,
            DeploymentEnvironment::Production
        );
        assert_eq!(config.http.bind_addr.to_string(), "127.0.0.1:9009");
        assert_eq!(config.http.request_timeout_secs, 12);
        assert_eq!(config.http.max_body_bytes, 2048);
        assert_eq!(config.databases.max_connections, 24);
        assert_eq!(config.databases.min_connections, 2);
        assert_eq!(config.databases.acquire_timeout_secs, 1);
        assert_eq!(config.rate_limits.window_secs, 10);
        assert_eq!(config.rate_limits.per_ip_requests, 20);
        assert_eq!(config.rate_limits.per_user_requests, 30);
        assert_eq!(config.rate_limits.per_route_requests, 40);
        assert_eq!(config.object_storage.bucket, "placeonix-prod");
    }

    #[test]
    fn rejects_min_connections_above_max() {
        let error = AppConfig::from_source("placeonix-api", |key| match key {
            "DB_MAX_CONNECTIONS" => Some("2".to_owned()),
            "DB_MIN_CONNECTIONS" => Some("3".to_owned()),
            _ => None,
        })
        .expect_err("invalid pool sizing is rejected");

        assert!(matches!(
            error,
            ConfigError::InvalidNumber {
                key: "DB_MIN_CONNECTIONS",
                ..
            }
        ));
    }

    #[test]
    fn rejects_invalid_url_values() {
        let error = AppConfig::from_source("placeonix-api", |key| match key {
            "REDIS_URL" => Some("not-a-url".to_owned()),
            _ => None,
        })
        .expect_err("invalid URL is rejected");

        assert!(matches!(
            error,
            ConfigError::InvalidUrl {
                key: "REDIS_URL",
                ..
            }
        ));
    }

    #[test]
    fn redacts_secret_values() {
        let secret = SecretString::new("super-secret");

        assert_eq!(format!("{secret}"), "**redacted**");
        assert_eq!(format!("{secret:?}"), "SecretString(**redacted**)");
        assert_eq!(secret.expose(), "super-secret");
    }
}
