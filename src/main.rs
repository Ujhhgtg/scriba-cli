mod cli;
mod config;
mod defs;
mod logging;
mod module;
mod process;

use std::fs;
use std::io;
use std::path::Path;

use clap::CommandFactory;
use clap::Parser;
use clap_complete::generate;
use tracing::error;
use tracing::info;
use tracing::warn;

use crate::cli::AppCommand;
use crate::cli::Cli;
use crate::cli::InternalCommand;
use crate::cli::ModuleCommand;
use crate::cli::TopLevel;
use crate::defs::BIN_DIR;
use crate::defs::Environment;
use crate::defs::LOGS_DIR;
use crate::defs::MODULES_DIR;
use crate::defs::MODULES_UPDATE_DIR;

/* =========================
 * Main
 * ========================= */

fn main() -> anyhow::Result<()> {
    logging::init_logging();

    let cli = Cli::parse();
    let environment = cli.force_env.unwrap_or_else(Environment::detect);

    // Host forwarding via adb if exactly one device
    if environment == Environment::Host {
        // let devices = adb::list_devices();
        // if devices.len() == 1 {
        //     let device = &devices[0];

        //     // Reconstruct the CLI args to pass to adb shell
        //     // skip argv[0], keep the rest
        //     let args: Vec<String> = std::env::args().skip(1).collect();

        //     let status = adb::shell_run(device, crate_name!(), args);
        //     if let Err(err) = status {
        //         error!(err);
        //         return Err(anyhow!("failed to execute adb shell: {err}"));
        //     }
        // } else {
        //     error!("no or more than one connected devices");
        //     return Err(anyhow!("no or more than one connected devices"));
        // }

        // return Ok(());

        error!("not supported");
        return Ok(());
    }

    fs::create_dir_all(Path::new(BIN_DIR))?;
    fs::create_dir_all(Path::new(LOGS_DIR))?;
    fs::create_dir_all(Path::new(MODULES_DIR))?;
    fs::create_dir_all(Path::new(MODULES_UPDATE_DIR))?;

    let _config = config::load_config(environment);

    match cli.command {
        Some(TopLevel::App { command }) => match command {
            AppCommand::Install { path } => {
                info!("installing app from {path}");
                process::run_with_output("miniapp_cli", &["install", &path])?;
            }

            AppCommand::Uninstall { app_id } => {
                info!("uninstalling app {app_id}");
                process::run_with_output("miniapp_cli", &["uninstall", &app_id.to_string()])?;
            }

            AppCommand::Run { app_id, page } => {
                info!("running app {app_id}");
                if let Some(page) = page {
                    process::run_with_output(
                        "miniapp_cli",
                        &["start", &app_id.to_string(), &page.to_string()],
                    )?;
                } else {
                    process::run_with_output("miniapp_cli", &["start", &app_id.to_string()])?;
                }
            }

            AppCommand::List { filter } => {
                info!("listing apps with filters: {filter:?}");
                error!("unimplemented")
            }
        },

        Some(TopLevel::Module { command }) => match command {
            ModuleCommand::Install { path, clean } => {
                info!("installing module from {path} (clean={clean})");

                // extract module & read id
                let temp_dir = module::unzip_module(Path::new(&path))?;
                info!("extracting module to {temp_dir:?}");
                let prop = module::read_module_prop(&temp_dir.join("module.prop"))?;
                let module_id = prop
                    .get("id")
                    .ok_or_else(|| anyhow::anyhow!("module.prop missing id"))?;

                // if module already exists in update dir, delete it
                let target_dir = Path::new(MODULES_UPDATE_DIR).join(module_id);
                if target_dir.exists() {
                    warn!("same module {module_id} exists in update dir, removing it first");
                    module::delete_dir(&target_dir)?;
                }

                // move module to update dir
                let target_dir = std::path::Path::new(MODULES_UPDATE_DIR).join(module_id);
                info!("moving module from temp dir to {target_dir:?}");
                module::move_dir(&temp_dir, &target_dir)?;
                info!("running install.sh");
                module::run_script(&target_dir, "install.sh")?;

                info!("module {module_id} installed to update dir");
            }

            ModuleCommand::Uninstall { module_id } => {
                info!("uninstalling module {module_id}");

                let module_dir = std::path::Path::new(MODULES_DIR).join(&module_id);
                let update_dir = std::path::Path::new(MODULES_UPDATE_DIR).join(&module_id);

                // if module is being updated, remove it first
                if update_dir.exists() {
                    module::delete_dir(&update_dir)?;
                    info!("module {module_id} removed from update dir");
                    return Ok(());
                }

                // if module is installed
                if module_dir.exists() {
                    // unflag uninstall
                    if fs::read_to_string(module_dir.join("uninstall.flag")).is_ok() {
                        fs::remove_file(module_dir.join("uninstall.flag"))?;
                        info!("module {module_id} unmarked for uninstall");
                    } else {
                        // flag uninstall
                        module::run_script(&module_dir, "uninstall.sh")?;
                        fs::write(module_dir.join("uninstall.flag"), "")?;
                        info!("module {module_id} marked for uninstall");
                    }
                } else {
                    error!("module is not installed or being updated");
                }
            }

            ModuleCommand::List => {
                module::list_modules(MODULES_DIR, "installed modules:");
                module::list_modules(MODULES_UPDATE_DIR, "pending update modules:");
            }
        },

        Some(TopLevel::Internal { command }) => match command {
            InternalCommand::BootComplete => {
                info!("executing boot complete logic");

                // 1. Unlock adb shell by creating /tmp/.adb_auth_verified
                info!("unlocking adb shell");
                let adb_auth_path = "/tmp/.adb_auth_verified";
                if let Err(e) = fs::File::create(adb_auth_path) {
                    warn!(
                        "failed to unlock adb shell by creating {}: {}",
                        adb_auth_path, e
                    );
                } else {
                    info!("adb shell unlocked by creating {}", adb_auth_path);
                }

                // 2. Remove uninstall flagged modules
                info!("removing uninstall flagged modules");
                let entries = std::fs::read_dir(MODULES_DIR).unwrap();
                for entry in entries {
                    let entry = entry.unwrap();
                    let path = entry.path();
                    if path.join("uninstall.flag").exists() {
                        info!("removing {path:?}");
                        if let Err(e) = module::delete_dir(&path) {
                            warn!("failed to delete module dir {path:?}: {e}");
                        }
                    }
                }

                // 3. Move update modules
                info!("installing update pending modules");
                let entries = std::fs::read_dir(MODULES_UPDATE_DIR).unwrap();
                for entry in entries {
                    let entry = entry.unwrap();
                    let path = entry.path();
                    info!("updating {path:?}");
                    let target = std::path::Path::new(MODULES_DIR).join(path.file_name().unwrap());
                    if let Err(e) = module::move_dir(&path, &target) {
                        warn!("failed to move update module {path:?} to {target:?}: {e}");
                    }
                }

                if Path::new("/userdisk/Favorite/safe_mode.flag").exists() {
                    warn!("safe mode flag exists, not initializing modules");
                    return Ok(());
                }

                // 4. Initialize modules
                info!("initializing modules");
                for entry in std::fs::read_dir(MODULES_DIR).unwrap() {
                    let entry = entry?;
                    let path = entry.path();
                    info!("initializing {path:?}");

                    // read props
                    let props = module::read_module_prop(&path.join("module.prop"));
                    if let Err(err) = props {
                        error!("module {path:?} has invalid properties: {err}, skipping");
                        continue;
                    }
                    let props = props.unwrap();

                    info!(
                        "module info: {}, {}, {}, {}",
                        props["id"], props["name"], props["description"], props["version"]
                    );

                    // disable
                    if path.join("disable.flag").exists() {
                        warn!("module {path:?} is disabled, not initializing it");
                        continue;
                    }

                    // mount
                    info!("mounting module {path:?}");
                    if props
                        .get("skip_mount")
                        .map(|s| s.as_str())
                        .unwrap_or("false")
                        != "true"
                    {
                        if let Err(err) = module::mount_module(&path) {
                            warn!("failed to mount module: {err}");
                            continue;
                        }
                    } else {
                        info!("module has skip_mount, not mounting module")
                    }

                    // execute boot-complete.sh
                    info!("executing boot-complete.sh in {path:?}");
                    if path.join("boot-complete.sh").exists() {
                        if let Err(e) = module::run_script(&path, "boot-complete.sh") {
                            warn!("failed to run boot-complete.sh for {path:?}: {e}");
                            continue;
                        }
                    } else {
                        warn!("boot-complete.sh does not exist")
                    }
                }

                // let _ = fs::write("/userdisk/Favorite/safe_mode.flag", "");
            }
        },

        Some(TopLevel::Completion { shell }) => {
            let mut cmd = Cli::command();
            let bin_name = cmd.get_name().to_string();
            generate(shell, &mut cmd, bin_name, &mut io::stdout());
        }

        None => {
            Cli::command().print_help().unwrap();
        }
    }

    Ok(())
}
