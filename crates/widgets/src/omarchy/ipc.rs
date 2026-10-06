//! Comunicación con el shell en ejecución y con los comandos `omarchy`.

pub use lizarbe_core::ipc::{command_exists, run};

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
