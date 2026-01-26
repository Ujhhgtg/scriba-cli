use anyhow::bail;
use anyhow::{Context, Result, anyhow};
use libc::{MS_BIND, mount};
use std::collections::HashMap;
use std::fs;
use std::fs::File;
use std::fs::create_dir_all;
use std::fs::rename;
use std::path::Path;
use std::path::PathBuf;
use std::{ffi::CString, os::unix::ffi::OsStrExt};
use tempfile::tempdir;
use tracing::info;
use tracing::warn;
use zip::ZipArchive;

use crate::process;

pub fn read_module_prop(path: &std::path::Path) -> anyhow::Result<HashMap<String, String>> {
    let content = fs::read_to_string(path)?;
    let mut map = HashMap::new();
    for line in content.lines() {
        if let Some((k, v)) = line.split_once('=') {
            map.insert(k.trim().to_string(), v.trim().to_string());
        }
    }

    validate_prop(&map, "id", PropType::String)?;
    validate_prop(&map, "name", PropType::String)?;
    validate_prop(&map, "description", PropType::String)?;
    validate_prop(&map, "version", PropType::Int)?;

    // Handle optional skip_mount (default: false)
    if let Some(skip_mount_val) = map.get("skip_mount") {
        match skip_mount_val.to_lowercase().as_str() {
            "true" | "false" => {}
            _ => return Err(anyhow!("property skip_mount must be 'true' or 'false'")),
        }
    } else {
        map.insert("skip_mount".to_string(), "false".to_string());
    }

    let id = map.get("id").unwrap();
    let dir_name = path
        .parent()
        .and_then(|p| p.file_name())
        .and_then(|n| n.to_str())
        .ok_or_else(|| anyhow!("cannot get directory name"))?;
    if id != dir_name {
        return Err(anyhow!(
            "id '{}' does not match directory name '{}'",
            id,
            dir_name
        ));
    }

    Ok(map)
}

#[derive(Clone, Copy)]
#[allow(dead_code)]
enum PropType {
    String,
    Int,
    Bool,
}

fn validate_prop(
    map: &HashMap<String, String>,
    key: &str,
    prop_type: PropType,
) -> anyhow::Result<()> {
    let value = map
        .get(key)
        .ok_or_else(|| anyhow!("missing required property: {}", key))?;
    match prop_type {
        PropType::String => {
            if value.trim().is_empty() {
                return Err(anyhow!("property {} cannot be empty", key));
            }
        }
        PropType::Int => {
            value
                .parse::<i32>()
                .map_err(|_| anyhow!("property {} must be a valid integer", key))?;
        }
        PropType::Bool => match value.to_lowercase().as_str() {
            "true" | "false" => {}
            _ => return Err(anyhow!("property {} must be 'true' or 'false'", key)),
        },
    }
    Ok(())
}

pub fn run_script(module_dir: &std::path::Path, script: &str) -> anyhow::Result<()> {
    let script_path = module_dir.join(script);
    if script_path.exists() {
        let status = process::run_with_output("sh", &[script_path.to_str().unwrap()])?;
        if !status.success() {
            bail!(
                "script {} failed with exit code {:?}",
                script,
                status.code()
            );
        }
    } else {
        bail!("script {script} does not exist")
    }

    Ok(())
}

pub fn delete_dir(path: &std::path::Path) -> anyhow::Result<()> {
    info!("deleting dir {path:?}");
    if path.exists() {
        fs::remove_dir_all(path)?;
    }
    Ok(())
}

pub fn move_dir(src: &std::path::Path, dst: &std::path::Path) -> anyhow::Result<()> {
    info!("moving {src:?} to {dst:?}");
    if dst.exists() {
        delete_dir(dst)?;
    } else {
        create_dir_all(dst)?;
    }
    rename(src, dst)?;
    Ok(())
}

pub fn unzip_module(zip_path: &Path) -> anyhow::Result<PathBuf> {
    let file = File::open(zip_path)?;
    let mut archive = ZipArchive::new(file)?;
    let tmp_dir = tempdir()?;
    for i in 0..archive.len() {
        let mut file = archive.by_index(i)?;
        let outpath = tmp_dir.path().join(file.mangled_name());

        if file.name().ends_with('/') {
            std::fs::create_dir_all(&outpath)?;
        } else {
            if let Some(parent) = outpath.parent() {
                std::fs::create_dir_all(parent)?;
            }
            std::io::copy(&mut file, &mut std::fs::File::create(&outpath)?)?;
        }
    }
    Ok(tmp_dir.keep())
}

fn bind_mount_file(src: &Path, dst: &Path) -> Result<()> {
    info!("mounting {src:?} on {dst:?}");

    let src_c = CString::new(src.as_os_str().as_bytes()).context("invalid src path")?;
    let dst_c = CString::new(dst.as_os_str().as_bytes()).context("invalid dst path")?;

    let ret = unsafe {
        mount(
            src_c.as_ptr(),
            dst_c.as_ptr(),
            std::ptr::null(),
            MS_BIND,
            std::ptr::null(),
        )
    };

    if ret != 0 {
        return Err(anyhow!(
            "bind mount failed: {} -> {} ({})",
            src.display(),
            dst.display(),
            std::io::Error::last_os_error()
        ));
    }

    Ok(())
}

fn walk_and_bind_files(base_system_dir: &Path, current_dir: &Path) -> Result<()> {
    for entry in fs::read_dir(current_dir)? {
        let entry = entry?;
        let src_path = entry.path();
        let meta = fs::symlink_metadata(&src_path)?;

        let rel = src_path
            .strip_prefix(base_system_dir)
            .context("strip prefix failed")?;
        let dst_path = Path::new("/").join(rel);

        if meta.is_dir() {
            // If the directory does not exist on /, prune the subtree
            if !dst_path.exists() {
                warn!(
                    "directory {:?} does not exist on /, skipping subtree",
                    dst_path
                );
                continue;
            }

            // Recurse, but DO NOT bind the directory itself
            walk_and_bind_files(base_system_dir, &src_path)?;
            continue;
        }

        if meta.is_file() {
            // Target file must already exist on readonly root
            if !dst_path.exists() {
                warn!("file {:?} does not exist on /, skipping", dst_path);
                continue;
            }

            bind_mount_file(&src_path, &dst_path)?;
            continue;
        }

        // Skip symlinks, devices, sockets, fifos, etc.
        warn!("skipping unsupported entry {:?}", src_path);
    }

    Ok(())
}

pub fn mount_module(module_dir: &Path) -> Result<()> {
    if !module_dir.is_dir() {
        bail!("module dir does not exist");
    }

    let system_dir = module_dir.join("system");
    if !system_dir.is_dir() {
        bail!("system dir does not exist or is invalid");
    }

    walk_and_bind_files(&system_dir, &system_dir)?;
    Ok(())
}

pub fn list_modules(dir: &str, label: &str) {
    info!("{label}");
    match fs::read_dir(dir) {
        Ok(entries) => {
            let mut found = false;
            for entry in entries.filter_map(|entry| entry.ok()) {
                let prop_path = entry.path().join("module.prop");
                if prop_path.exists() {
                    if let Ok(m) = read_module_prop(&prop_path) {
                        info!(
                            "{} - {} v{} ({})",
                            m.get("id").unwrap_or(&"?".to_string()),
                            m.get("name").unwrap_or(&"?".to_string()),
                            m.get("version").unwrap_or(&"?".to_string()),
                            m.get("description").unwrap_or(&"".to_string())
                        );
                        found = true;
                    }
                }
            }
            if !found {
                info!("  (no modules found)");
            }
        }
        Err(e) => {
            warn!("failed to read directory {}: {}", dir, e);
        }
    }
}
