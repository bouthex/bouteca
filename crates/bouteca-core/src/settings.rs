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
    /// "system" (respeta la preferencia de Windows), "reduced" o "off".
    pub motion: String,
    /// Calidad de descarga por defecto (solo entre archivos que la fuente publica de verdad).
    pub preferred_quality: crate::quality::QualityPref,
    /// Combinaciones de filtros guardadas por el usuario (la interfaz define su contenido).
    pub saved_filters: Vec<SavedFilter>,
    pub updates: UpdateSettings,
}

/// Actualizaciones: canal estable por defecto; instalar siempre requiere confirmación.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(default)]
pub struct UpdateSettings {
    pub check_on_start: bool,
    /// Descargar (y verificar) en segundo plano al encontrar una versión nueva.
    pub auto_download: bool,
    /// "stable" | "beta".
    pub channel: UpdateChannel,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "lowercase")]
pub enum UpdateChannel {
    #[default]
    Stable,
    Beta,
}

impl Default for UpdateSettings {
    fn default() -> Self {
        Self { check_on_start: true, auto_download: true, channel: UpdateChannel::Stable }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct SavedFilter {
    pub name: String,
    pub data: serde_json::Value,
}

impl AppSettings {
    /// Límites para datos que vienen de la interfaz.
    pub fn validate(&self) -> std::result::Result<(), String> {
        if self.saved_filters.len() > 20 {
            return Err("se pueden guardar hasta 20 combinaciones de filtros".into());
        }
        for f in &self.saved_filters {
            let n = f.name.trim().chars().count();
            if n == 0 || n > 40 {
                return Err("el nombre del filtro debe tener entre 1 y 40 caracteres".into());
            }
            if serde_json::to_string(&f.data).map(|s| s.len()).unwrap_or(usize::MAX) > 2048 {
                return Err("filtro guardado demasiado grande".into());
            }
        }
        if self.engine.favorite_providers.len() > 64 {
            return Err("demasiadas fuentes favoritas".into());
        }
        Ok(())
    }
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
            motion: "system".into(),
            preferred_quality: Default::default(),
            saved_filters: Vec::new(),
            updates: UpdateSettings::default(),
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
mod tests_validate {
    use super::*;

    #[test]
    fn limites_de_filtros_guardados() {
        let mut s = AppSettings::default();
        assert!(s.validate().is_ok());
        s.saved_filters.push(SavedFilter { name: "Música sin créditos".into(), data: serde_json::json!({"rights": "sin_atribucion"}) });
        assert!(s.validate().is_ok());
        s.saved_filters.push(SavedFilter { name: " ".into(), data: serde_json::json!({}) });
        assert!(s.validate().is_err());
        s.saved_filters.pop();
        s.saved_filters.push(SavedFilter { name: "grande".into(), data: serde_json::json!({"x": "a".repeat(3000)}) });
        assert!(s.validate().is_err());
        // Archivos de configuración viejos (sin los campos nuevos) siguen cargando.
        let old: AppSettings = serde_json::from_str(r#"{"engine":{"primary_lang":"es","extra_langs":[],"disabled_providers":[],"safe_search":true,"include_unlicensed":false,"per_page":24}}"#).unwrap();
        assert!(old.engine.group_duplicates && old.saved_filters.is_empty());
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn guarda_y_carga() {
        let tmp = tempfile::tempdir().unwrap();
        let p = tmp.path().join("settings.json");
        let s = AppSettings { minimize_to_tray: true, ..Default::default() };
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
