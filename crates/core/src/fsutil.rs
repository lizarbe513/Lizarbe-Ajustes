//! Escritura segura de archivos de configuración: escritura atómica, copias
//! de seguridad con rotación y transacciones que se pueden deshacer.

use std::path::{Path, PathBuf};

use anyhow::{Context, Result};

/// Escribe en un temporal del mismo directorio y lo renombra encima.
pub fn write_atomic(path: &Path, content: &str) -> Result<()> {
    let dir = path.parent().unwrap_or(Path::new("."));
    std::fs::create_dir_all(dir).with_context(|| format!("{}", dir.display()))?;
    let name = path.file_name().and_then(|n| n.to_str()).unwrap_or("file");
    let tmp = dir.join(format!(".{name}.lizarbe.tmp"));
    std::fs::write(&tmp, content).with_context(|| format!("{}", tmp.display()))?;
    std::fs::rename(&tmp, path).with_context(|| format!("{}", path.display()))?;
    Ok(())
}

/// Marca de tiempo para nombrar copias de seguridad.
pub fn stamp() -> String {
    chrono::Local::now().format("%Y%m%d-%H%M%S").to_string()
}

/// Copia `path` a `dir/<nombre>.<stamp>`. `None` si el archivo no existe.
pub fn backup(path: &Path, dir: &Path, stamp: &str) -> Result<Option<PathBuf>> {
    if !path.is_file() {
        return Ok(None);
    }
    std::fs::create_dir_all(dir).with_context(|| format!("{}", dir.display()))?;
    let name = path.file_name().and_then(|n| n.to_str()).unwrap_or("file");
    let dest = dir.join(format!("{name}.{stamp}"));
    std::fs::copy(path, &dest).with_context(|| format!("backup {}", dest.display()))?;
    Ok(Some(dest))
}

/// Conserva solo las `keep` copias más recientes de `dir`.
pub fn prune_backups(dir: &Path, keep: usize) {
    let Ok(rd) = std::fs::read_dir(dir) else {
        return;
    };
    let mut files: Vec<_> = rd
        .flatten()
        .filter_map(|e| Some((e.metadata().ok()?.modified().ok()?, e.path())))
        .collect();
    files.sort_by_key(|f| std::cmp::Reverse(f.0));
    for (_, p) in files.into_iter().skip(keep) {
        let _ = std::fs::remove_file(p);
    }
}

/// Grupo de escrituras que se confirman o se deshacen juntas.
///
/// Cada archivo se respalda antes de escribirlo; si después la comprobación
/// falla (p. ej. Hyprland informa errores de configuración), [`rollback`]
/// devuelve todos los archivos a su contenido anterior.
///
/// [`rollback`]: Transaction::rollback
pub struct Transaction {
    backup_dir: PathBuf,
    stamp: String,
    /// Archivo escrito y su contenido anterior (`None` si no existía).
    written: Vec<(PathBuf, Option<String>)>,
    pub backups: Vec<PathBuf>,
}

impl Transaction {
    pub fn new(backup_dir: impl Into<PathBuf>) -> Self {
        Transaction {
            backup_dir: backup_dir.into(),
            stamp: stamp(),
            written: vec![],
            backups: vec![],
        }
    }

    pub fn write(&mut self, path: &Path, content: &str) -> Result<()> {
        if !self.written.iter().any(|(p, _)| p == path) {
            let original = std::fs::read_to_string(path).ok();
            if let Some(b) = backup(path, &self.backup_dir, &self.stamp)? {
                self.backups.push(b);
            }
            self.written.push((path.to_path_buf(), original));
        }
        write_atomic(path, content)
    }

    pub fn files(&self) -> impl Iterator<Item = &Path> {
        self.written.iter().map(|(p, _)| p.as_path())
    }

    /// Da por buenos los cambios y deja solo las `keep` copias más recientes.
    pub fn commit(self, keep: usize) -> Vec<PathBuf> {
        prune_backups(&self.backup_dir, keep);
        self.backups
    }

    /// Restaura el contenido anterior de todos los archivos escritos.
    pub fn rollback(self) -> Result<()> {
        for (path, original) in self.written.into_iter().rev() {
            match original {
                Some(text) => write_atomic(&path, &text)?,
                None => {
                    if path.exists() {
                        std::fs::remove_file(&path)
                            .with_context(|| format!("{}", path.display()))?;
                    }
                }
            }
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn writes_atomically_with_backup() {
        let dir = tempfile::tempdir().unwrap();
        let file = dir.path().join("a.conf");
        let backups = dir.path().join("backups");
        std::fs::write(&file, "old").unwrap();

        let mut tx = Transaction::new(&backups);
        tx.write(&file, "new").unwrap();
        let saved = tx.commit(30);

        assert_eq!(std::fs::read_to_string(&file).unwrap(), "new");
        assert_eq!(saved.len(), 1);
        assert_eq!(std::fs::read_to_string(&saved[0]).unwrap(), "old");
        assert!(
            !dir.path().join(".a.conf.lizarbe.tmp").exists(),
            "no quedan temporales"
        );
    }

    #[test]
    fn rollback_restores_and_removes_new_files() {
        let dir = tempfile::tempdir().unwrap();
        let existing = dir.path().join("existing.lua");
        let created = dir.path().join("sub/created.lua");
        std::fs::write(&existing, "original").unwrap();

        let mut tx = Transaction::new(dir.path().join("backups"));
        tx.write(&existing, "first").unwrap();
        tx.write(&existing, "second").unwrap();
        tx.write(&created, "new").unwrap();
        assert_eq!(tx.backups.len(), 1, "un respaldo por archivo existente");
        tx.rollback().unwrap();

        assert_eq!(std::fs::read_to_string(&existing).unwrap(), "original");
        assert!(!created.exists());
    }

    #[test]
    fn prunes_old_backups() {
        let dir = tempfile::tempdir().unwrap();
        for i in 0..5 {
            std::fs::write(dir.path().join(format!("f.{i}")), "x").unwrap();
        }
        prune_backups(dir.path(), 2);
        assert_eq!(std::fs::read_dir(dir.path()).unwrap().count(), 2);
    }
}
