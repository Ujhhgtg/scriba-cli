use std::fs;

use clap::ValueEnum;

#[derive(Clone, Copy, Debug, ValueEnum, PartialEq, Eq)]
pub enum Environment {
    Host,
    Device,
}

impl Environment {
    pub fn detect() -> Self {
        if std::env::consts::OS != "linux" {
            return Environment::Host;
        }

        let os_release = fs::read_to_string("/etc/os-release").unwrap_or_default();

        if os_release.contains("Buildroot") {
            Environment::Device
        } else {
            Environment::Host
        }
    }
}

#[derive(Clone, Debug, ValueEnum)]
pub enum AppFilter {
    User,
    Builtin,
    BuiltinThirdparty,
}

pub const CONFIG_FILE: &str = "/userdisk/scriba/config.toml";
pub const LOGS_DIR: &str = "/userdisk/scriba/logs/";
pub const BIN_DIR: &str = "/userdisk/scriba/bin/";
pub const MODULES_DIR: &str = "/userdisk/scriba/modules/";
pub const MODULES_UPDATE_DIR: &str = "/userdisk/scriba/modules_update/";
