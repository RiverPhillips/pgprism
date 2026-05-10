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
