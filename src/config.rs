use std::{
    fs, io,
    path::{Path, PathBuf},
};

use config::{Config, File};
use serde::Deserialize;

use crate::defs::{CONFIG_FILE, Environment};

#[derive(Debug, Deserialize)]
pub struct AppConfig {}

impl Default for AppConfig {
    fn default() -> Self {
        Self {}
    }
}

fn config_path(environment: Environment) -> PathBuf {
    match environment {
        Environment::Device => PathBuf::from(CONFIG_FILE),

        Environment::Host => {
            if let Ok(xdg) = std::env::var("XDG_CONFIG_HOME") {
                Path::new(&xdg).join("scriba/config.toml")
            } else if let Ok(home) = std::env::var("HOME") {
                Path::new(&home).join(".config/scriba/config.toml")
            } else {
                PathBuf::from("scriba/config.toml")
            }
        }
    }
}

fn ensure_config_file(path: &Path) -> io::Result<()> {
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)?;
    }

    if !path.exists() {
        fs::write(
            path,
            "# scriba configuration\n\
             \n",
        )?;
    }

    Ok(())
}

pub fn load_config(environment: Environment) -> AppConfig {
    let path = config_path(environment);

    ensure_config_file(&path)
        .unwrap_or_else(|e| panic!("failed to prepare config file {:?}: {e}", path));

    let config = Config::builder()
        .add_source(File::from(path))
        .build()
        .expect("failed to load config");

    config.try_deserialize::<AppConfig>().unwrap_or_default()
}
