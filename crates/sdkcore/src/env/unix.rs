use std::collections::HashMap;
use util::sdk::Sdk;
use crate::env::EnvOperation;

pub struct UnixEnvOperation{}

impl EnvOperation for UnixEnvOperation {
    fn set_sdk_envs(&self, envs: &HashMap<String, String>) -> anyhow::Result<()> {
        todo!()
    }

    fn add_sdk_path(&self, sdk_path: &str) -> anyhow::Result<()> {
        todo!()
    }

    fn get_path(&self) -> anyhow::Result<String> {
        todo!()
    }
}