use serde::{Deserialize, Serialize};

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq)]
pub struct General {
    #[serde(default = "General::default_worker_threads")]
    pub worker_threads: usize,
}

impl General {
    fn default_worker_threads() -> usize {
        num_cpus::get()
    }
}

impl Default for General {
    fn default() -> Self {
        Self {
            worker_threads: General::default_worker_threads(),
        }
    }
}

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq, Default)]
pub struct Config {
    #[serde(default = "General::default")]
    pub general: General,
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
        };
        let serialized = toml::to_string(&original).expect("serialization failed");
        let deserialized: Config = toml::from_str(&serialized).expect("deserialization failed");
        assert_eq!(original, deserialized);
    }
}
