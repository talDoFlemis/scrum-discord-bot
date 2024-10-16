use mongodb::options::{ClientOptions, Credential, ServerAddress, Tls, TlsOptions};
use opentelemetry::KeyValue;
use opentelemetry_sdk::Resource;
use secrecy::{ExposeSecret, SecretString};
use serde_aux::field_attributes::deserialize_number_from_string;
use std::convert::{TryFrom, TryInto};

#[derive(serde::Deserialize, Clone)]
pub struct Settings {
    pub database: DatabaseSettings,
    pub application: ApplicationSettings,
    pub http: HttpSettings,
    pub otel: OpenTelemetrySettings,
    pub prometheus: PrometheusSettings,
    pub env: Environment,
}

#[derive(serde::Deserialize, Clone)]
pub struct HttpSettings {
    #[serde(deserialize_with = "deserialize_number_from_string")]
    pub port: u16,
    pub host: String,
    pub prefix: String,
    #[serde(deserialize_with = "deserialize_number_from_string")]
    pub timeout: u64,
}

#[derive(serde::Deserialize, Clone)]
pub struct ApplicationSettings {
    pub name: String,
    pub version: String,
}

#[derive(serde::Deserialize, Clone)]
pub struct DatabaseSettings {
    pub username: String,
    pub password: SecretString,
    #[serde(deserialize_with = "deserialize_number_from_string")]
    pub port: u16,
    pub hosts: Vec<String>,
    pub database: String,
    pub ssl: bool,
}

impl DatabaseSettings {
    pub fn connect_options(&self) -> anyhow::Result<ClientOptions> {
        let ssl_mode = if self.ssl {
            Some(Tls::Enabled(TlsOptions::default()))
        } else {
            None
        };

        let credential = Credential::builder()
            .username(self.username.clone())
            .password(Some(self.password.expose_secret().into()))
            .build();

        let mut hosts = Vec::with_capacity(self.hosts.len());
        for host in self.hosts.iter() {
            let parsed_host = ServerAddress::parse(host)?;
            hosts.push(parsed_host);
        }

        Ok(ClientOptions::builder()
            .hosts(hosts)
            .credential(Some(credential))
            .default_database(self.database.clone())
            .tls(ssl_mode)
            .app_name(Some("scrum-discord-bot".into()))
            .build())
    }
}

#[derive(serde::Deserialize, Clone)]
pub struct OpenTelemetrySettings {
    pub endpoint: String,
    pub enable: bool,
}

impl Settings {
    pub fn get_resource(&self) -> Resource {
        Resource::default().merge(&Resource::new(vec![
            KeyValue::new(
                opentelemetry_semantic_conventions::resource::SERVICE_NAME,
                self.application.name.clone(),
            ),
            KeyValue::new(
                opentelemetry_semantic_conventions::resource::SERVICE_VERSION,
                self.application.version.clone(),
            ),
        ]))
    }
}

#[derive(serde::Deserialize, Clone)]
pub struct PrometheusSettings {
    #[serde(deserialize_with = "deserialize_number_from_string")]
    pub port: u16,
    pub path: String,
}

pub fn get_configuration() -> Result<Settings, config::ConfigError> {
    let base_path = std::env::current_dir().expect("Failed to determine the current directory");
    let configuration_directory = base_path.join("config");

    // Detect the running environment.
    // Default to `local` if unspecified.
    let environment: Environment = std::env::var("APP_ENVIRONMENT")
        .unwrap_or_else(|_| "local".into())
        .try_into()
        .expect("Failed to parse APP_ENVIRONMENT.");

    let environment_filename = format!("{}.yaml", environment.as_str());

    let settings = config::Config::builder()
        .add_source(config::File::from(
            configuration_directory.join("base.yaml"),
        ))
        .add_source(config::File::from(
            configuration_directory.join(environment_filename),
        ))
        .add_source(
            config::Environment::with_prefix("APP")
                .prefix_separator("_")
                .separator("_"),
        )
        .build()?;

    let mut settings_parsed = settings.try_deserialize::<Settings>()?;

    settings_parsed.env = environment;

    Ok(settings_parsed)
}

/// The possible runtime environment for our application.
#[derive(Clone, serde::Deserialize)]
pub enum Environment {
    Local,
    Production,
}

impl Environment {
    pub fn as_str(&self) -> &'static str {
        match self {
            Environment::Local => "local",
            Environment::Production => "production",
        }
    }
}

impl TryFrom<String> for Environment {
    type Error = String;

    fn try_from(s: String) -> Result<Self, Self::Error> {
        match s.to_lowercase().as_str() {
            "local" => Ok(Self::Local),
            "production" => Ok(Self::Production),
            other => Err(format!(
                "{} is not a supported environment. Use either `local` or `production`.",
                other
            )),
        }
    }
}
