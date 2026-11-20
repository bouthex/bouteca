//! Configuración persistente (JSON en la carpeta de datos de la app). Las claves de API NO
//! se guardan aquí: van al Administrador de credenciales de Windows.

use crate::engine::EngineSettings;
use crate::error::Result;
use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct AppSettings {
    pub engine: EngineSettings,
    /// Carpeta raíz de descargas. `None` = Descargas/Bouteca del usuario.
    pub download_dir: Option<PathBuf>,
    pub organize_by_kind: bool,
    pub detect_duplicates: bool,
    /// Atajo global (formato de Tauri, p. ej. "CommandOrControl+Shift+B"); `None` = desactivado.
    pub global_shortcut: Option<String>,
    pub minimize_to_tray: bool,
    pub launch_at_startup: bool,
    /// Reproducir GIFs/videos de la cuadrícula al pasar el mouse (si no, solo miniatura).
    pub hover_previews: bool,
    pub autoplay_gifs: bool,
    /// Límite para la caché del WebView (miniaturas). Se vacía al superarlo.
    pub web_cache_limit_mb: u64,
    pub auto_cleanup: bool,
    /// "low" | "high": tamaño de miniaturas pedidas a los proveedores que lo permiten.
    pub thumbnail_quality: String,
    pub theme: String,
}

impl Default for AppSettings {
    fn default() -> Self {
        Self {
            engine: EngineSettings::default(),
            download_dir: None,
            organize_by_kind: true,
            detect_duplicates: true,
            global_shortcut: Some("CommandOrControl+Shift+B".into()),
            minimize_to_tray: false,
            launch_at_startup: false,
            hover_previews: true,
            autoplay_gifs: false,
            web_cache_limit_mb: 150,
            auto_cleanup: true,
            thumbnail_quality: "low".into(),
            theme: "dark".into(),
        }
    }
}

impl AppSettings {
    pub fn load(path: &Path) -> Self {
        match std::fs::read(path) {
            Ok(bytes) => serde_json::from_slice(&bytes).unwrap_or_else(|e| {
                tracing::warn!("configuración dañada, se usan valores por defecto: {e}");
                // Conservamos la copia dañada para no perder datos del usuario.
                let _ = std::fs::rename(path, path.with_extension("json.bak"));
                Self::default()
            }),
            Err(_) => Self::default(),
        }
    }

    /// Escritura atómica: archivo temporal + rename, así un corte de luz no deja JSON a medias.
    pub fn save(&self, path: &Path) -> Result<()> {
        if let Some(dir) = path.parent() {
            std::fs::create_dir_all(dir)?;
        }
        let tmp = path.with_extension("json.tmp");
        std::fs::write(&tmp, serde_json::to_vec_pretty(self).unwrap_or_default())?;
        std::fs::rename(&tmp, path)?;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn guarda_y_carga() {
        let tmp = tempfile::tempdir().unwrap();
        let p = tmp.path().join("settings.json");
        let mut s = AppSettings::default();
        s.minimize_to_tray = true;
        s.save(&p).unwrap();
        let l = AppSettings::load(&p);
        assert!(l.minimize_to_tray);
        assert!(!l.launch_at_startup, "inicio con Windows desactivado por defecto");
        std::fs::write(&p, b"{roto").unwrap();
        let d = AppSettings::load(&p);
        assert!(!d.minimize_to_tray);
        assert!(p.with_extension("json.bak").exists());
    }
}
