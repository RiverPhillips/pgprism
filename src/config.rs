use std::{
    fs,
    net::{IpAddr, Ipv4Addr},
    path::Path,
    str::FromStr,
};

use serde::{Deserialize, Serialize};

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq)]
#[serde(default)]
pub struct General {
    pub worker_threads: usize,
}

impl Default for General {
    fn default() -> Self {
        Self {
            worker_threads: std::thread::available_parallelism()
                .map(|n| n.get())
                .unwrap_or(1),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct DownstreamConfig {
    pub listener_port: u16,
    pub listener_address: IpAddr,
}

impl DownstreamConfig {
    pub const DEFAULT_LISTENER_PORT: u16 = 50002;
    pub const DEFAULT_LISTENER_ADDRESS: IpAddr = IpAddr::V4(Ipv4Addr::new(0, 0, 0, 0));
}

impl Default for DownstreamConfig {
    fn default() -> Self {
        Self {
            listener_port: Self::DEFAULT_LISTENER_PORT,
            listener_address: Self::DEFAULT_LISTENER_ADDRESS,
        }
    }
}

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq, Default)]
#[serde(default)]
pub struct Config {
    pub general: General,
    pub downstream: DownstreamConfig,
}

impl Config {
    pub fn load(path: &Path) -> Result<Self, crate::error::Error> {
        let contents = fs::read_to_string(path)?;
        Self::from_str(&contents)
    }
}

impl FromStr for Config {
    type Err = crate::error::Error;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        toml::from_str(s).map_err(Self::Err::ConfigParseError)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn default_worker_threads_matches_cpu_count() {
        assert_eq!(Config::default().general.worker_threads, num_cpus::get());
    }

    #[test]
    fn toml_roundtrip() {
        let original = Config {
            general: General { worker_threads: 4 },
            downstream: DownstreamConfig {
                listener_port: 50003,
                listener_address: "127.0.0.1".parse().unwrap(),
            },
        };
        let serialized = toml::to_string(&original).expect("serialization failed");
        let deserialized: Config = Config::from_str(&serialized).expect("deserialization failed");
        assert_eq!(original, deserialized);
    }
}
