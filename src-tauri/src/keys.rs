//! Almacenamiento de claves de API.
//!
//! - Windows: Administrador de credenciales (cifrado por el sistema con la cuenta del usuario).
//! - Otros sistemas (solo desarrollo): archivo `keys.json` en la carpeta de datos, con permisos 600.
//!
//! Las claves nunca se escriben en `settings.json`, en logs ni en el repositorio.

use bouteca_core::engine::KeyStore;
use parking_lot::RwLock;
use std::collections::HashMap;
use std::path::PathBuf;

const SERVICE: &str = "Bouteca";

pub struct SecureKeys {
    cache: RwLock<HashMap<String, Option<String>>>,
    #[allow(dead_code)]
    fallback_file: PathBuf,
}

impl SecureKeys {
    pub fn new(data_dir: PathBuf) -> Self {
        Self { cache: RwLock::new(HashMap::new()), fallback_file: data_dir.join("keys.json") }
    }

    pub fn set(&self, provider: &str, key: Option<&str>) -> Result<(), String> {
        let key = key.map(str::trim).filter(|k| !k.is_empty());
        if let Some(k) = key {
            if k.len() > 256 || k.chars().any(|c| c.is_control() || c.is_whitespace()) {
                return Err("la clave tiene un formato inválido".into());
            }
        }
        self.store(provider, key)?;
        self.cache.write().insert(provider.to_string(), key.map(str::to_string));
        Ok(())
    }

    #[cfg(windows)]
    fn store(&self, provider: &str, key: Option<&str>) -> Result<(), String> {
        let entry = keyring::Entry::new(SERVICE, provider).map_err(|e| e.to_string())?;
        match key {
            Some(k) => entry.set_password(k).map_err(|e| e.to_string()),
            None => match entry.delete_credential() {
                Ok(()) | Err(keyring::Error::NoEntry) => Ok(()),
                Err(e) => Err(e.to_string()),
            },
        }
    }

    #[cfg(windows)]
    fn load(&self, provider: &str) -> Option<String> {
        keyring::Entry::new(SERVICE, provider).ok()?.get_password().ok()
    }

    #[cfg(not(windows))]
    fn read_file(&self) -> HashMap<String, String> {
        std::fs::read(&self.fallback_file).ok().and_then(|b| serde_json::from_slice(&b).ok()).unwrap_or_default()
    }

    #[cfg(not(windows))]
    fn store(&self, provider: &str, key: Option<&str>) -> Result<(), String> {
        let _ = SERVICE;
        let mut map = self.read_file();
        match key {
            Some(k) => {
                map.insert(provider.to_string(), k.to_string());
            }
            None => {
                map.remove(provider);
            }
        }
        if let Some(d) = self.fallback_file.parent() {
            std::fs::create_dir_all(d).map_err(|e| e.to_string())?;
        }
        std::fs::write(&self.fallback_file, serde_json::to_vec(&map).unwrap()).map_err(|e| e.to_string())?;
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            let _ = std::fs::set_permissions(&self.fallback_file, std::fs::Permissions::from_mode(0o600));
        }
        Ok(())
    }

    #[cfg(not(windows))]
    fn load(&self, provider: &str) -> Option<String> {
        self.read_file().remove(provider)
    }
}

impl KeyStore for SecureKeys {
    fn get(&self, provider: &str) -> Option<String> {
        if let Some(v) = self.cache.read().get(provider) {
            return v.clone();
        }
        let v = self.load(provider);
        self.cache.write().insert(provider.to_string(), v.clone());
        v
    }
}
