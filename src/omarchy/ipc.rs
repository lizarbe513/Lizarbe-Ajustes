//! Comunicación con el shell en ejecución y con los comandos `omarchy`.

use std::process::{Command, Stdio};

/// Ejecuta un comando y devuelve stdout si terminó bien.
pub fn run(cmd: &str, args: &[&str]) -> Result<String, String> {
    let out = Command::new(cmd)
        .args(args)
        .stdin(Stdio::null())
        .output()
        .map_err(|e| format!("{cmd}: {e}"))?;
    if out.status.success() {
        Ok(String::from_utf8_lossy(&out.stdout).trim().to_string())
    } else {
        let err = String::from_utf8_lossy(&out.stderr).trim().to_string();
        Err(if err.is_empty() {
            format!("{cmd} {}: {}", args.join(" "), out.status)
        } else {
            err
        })
    }
}

pub fn command_exists(cmd: &str) -> bool {
    std::env::var_os("PATH")
        .is_some_and(|paths| std::env::split_paths(&paths).any(|dir| dir.join(cmd).is_file()))
}

/// `true` si el shell de Omarchy responde.
pub fn shell_running() -> bool {
    command_exists("omarchy-shell")
        && run("omarchy-shell", &["shell", "ping"]).is_ok_and(|s| s == "ok")
}

pub fn reload_config() -> Result<(), String> {
    run("omarchy-shell", &["shell", "reloadConfig"]).map(|_| ())
}

pub fn rescan_plugins() -> Result<(), String> {
    run("omarchy-shell", &["shell", "rescanPlugins"]).map(|_| ())
}
