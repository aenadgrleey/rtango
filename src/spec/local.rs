use serde::{Deserialize, Serialize};

use super::spec::DeployMode;

#[derive(Debug, Default, Serialize, Deserialize)]
pub struct LocalConfig {
    #[serde(default)]
    pub mode: Option<DeployMode>,
}
