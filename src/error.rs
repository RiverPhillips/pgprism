use thiserror::Error;

#[derive(Debug, Error)]
pub enum Error {
    #[error("failed to parse config")]
    ConfigParseError(#[from] toml::de::Error),
    #[error("failed to read config file")]
    Io(#[from] std::io::Error),
}
