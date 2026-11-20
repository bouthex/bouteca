use thiserror::Error;

#[derive(Debug, Error)]
pub enum CoreError {
    #[error("sin conexión o error de red: {0}")]
    Network(String),
    #[error("el proveedor respondió HTTP {status}: {message}")]
    Http { status: u16, message: String },
    #[error("límite de consultas alcanzado en {provider}")]
    RateLimited { provider: String, retry_after_secs: Option<u64> },
    #[error("{0} necesita una clave de API (gratuita) para funcionar")]
    MissingKey(String),
    #[error("respuesta inesperada del proveedor: {0}")]
    Parse(String),
    #[error("archivo descargado no válido: {0}")]
    InvalidFile(String),
    #[error("espacio insuficiente en disco")]
    DiskFull,
    #[error("permiso denegado: {0}")]
    Permission(String),
    #[error("operación cancelada")]
    Cancelled,
    #[error("no encontrado: {0}")]
    NotFound(String),
    #[error("base de datos: {0}")]
    Db(String),
    #[error("E/S: {0}")]
    Io(String),
    #[error("{0}")]
    Other(String),
}

impl From<rusqlite::Error> for CoreError {
    fn from(e: rusqlite::Error) -> Self {
        CoreError::Db(e.to_string())
    }
}

impl From<std::io::Error> for CoreError {
    fn from(e: std::io::Error) -> Self {
        use std::io::ErrorKind::*;
        match e.kind() {
            PermissionDenied => CoreError::Permission(e.to_string()),
            // StorageFull es estable desde Rust 1.83.
            StorageFull => CoreError::DiskFull,
            _ => CoreError::Io(e.to_string()),
        }
    }
}

impl From<reqwest::Error> for CoreError {
    fn from(e: reqwest::Error) -> Self {
        if let Some(s) = e.status() {
            CoreError::Http { status: s.as_u16(), message: e.to_string() }
        } else if e.is_decode() {
            CoreError::Parse(e.to_string())
        } else {
            CoreError::Network(e.to_string())
        }
    }
}

impl serde::Serialize for CoreError {
    fn serialize<S: serde::Serializer>(&self, s: S) -> std::result::Result<S::Ok, S::Error> {
        s.serialize_str(&self.to_string())
    }
}

pub type Result<T> = std::result::Result<T, CoreError>;
