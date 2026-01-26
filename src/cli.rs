use clap::builder::Styles;
use clap::builder::styling::AnsiColor;
use clap::builder::styling::Effects;
use clap::{Parser, Subcommand, crate_description, crate_name, crate_version};
use clap_complete::Shell;
use std::str::FromStr;
use tracing::warn;

use crate::defs::AppFilter;
use crate::defs::Environment;

#[derive(Parser)]
#[command(name = crate_name!(),
    version = crate_version!(),
    about = crate_description!(),
    styles = Styles::styled()
        .header(AnsiColor::BrightGreen.on_default() | Effects::BOLD | Effects::UNDERLINE)
        .usage(AnsiColor::Cyan.on_default() | Effects::BOLD)
        .literal(AnsiColor::BrightCyan.on_default() | Effects::BOLD)
        .placeholder(AnsiColor::Cyan.on_default()))]
pub struct Cli {
    /// Force execution environment (host or device)
    #[arg(long, global = true, value_enum)]
    pub force_env: Option<Environment>,

    #[command(subcommand)]
    pub command: Option<TopLevel>,
}

#[derive(Subcommand)]
pub enum TopLevel {
    /// Manage applications ('miniapps')
    App {
        #[command(subcommand)]
        command: AppCommand,
    },

    /// Manage modules
    Module {
        #[command(subcommand)]
        command: ModuleCommand,
    },

    /// Internal commands
    Internal {
        #[command(subcommand)]
        command: InternalCommand,
    },

    /// Generate shell completion
    Completion {
        #[arg(value_enum)]
        shell: Shell,
    },
}

/* =========================
 * Internal commands
 * ========================= */

#[derive(Subcommand)]
pub enum InternalCommand {
    /// Execute boot complete logic
    BootComplete,
}

/* =========================
 * App commands
 * ========================= */

#[derive(Subcommand)]
pub enum AppCommand {
    /// Install an application
    Install {
        /// Path to the application package (.amr)
        path: String,
    },

    /// Uninstall an application
    Uninstall {
        /// ID of application
        #[arg(value_parser = parse_app_id)]
        app_id: u64,
    },

    /// Start an application
    Run {
        /// ID of application
        #[arg(value_parser = parse_app_id)]
        app_id: u64,

        /// Initial page to open
        #[arg(long)]
        page: Option<String>,
    },

    /// List installed applications
    List {
        /// Filter of applications
        #[arg(
            long,
            value_delimiter = ',',
            value_enum,
            default_values_t = vec![AppFilter::User]
        )]
        filter: Vec<AppFilter>,
    },
}

/* =========================
 * Module commands
 * ========================= */

#[derive(Subcommand)]
pub enum ModuleCommand {
    /// Install or update a module
    Install {
        /// Path to module archive
        path: String,

        /// Clear module data before install/update
        #[arg(long)]
        clean: bool,
    },

    /// Uninstall a module
    Uninstall {
        /// Module identifier
        #[arg(value_parser = parse_module_id)]
        module_id: String,
    },

    /// List installed modules
    List,
}

fn parse_app_id(value: &str) -> Result<u64, String> {
    let id = u64::from_str(value).map_err(|_| "app id must be an integer".to_string())?;

    if value.len() != 16 || !value.starts_with("80") {
        warn!(
            "warning: app id `{}` is unusual (expected 16 digits starting with \"80\")",
            value
        );
    }

    Ok(id)
}

fn parse_module_id(value: &str) -> Result<String, String> {
    if value.chars().all(|c| c.is_ascii_alphanumeric() || c == '_') {
        Ok(value.to_string())
    } else {
        Err("module id must contain only letters, numbers, or underscore".to_string())
    }
}
