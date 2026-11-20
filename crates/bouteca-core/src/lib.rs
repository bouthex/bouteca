//! # bouteca-core
//!
//! Núcleo de Bouteca, independiente de la interfaz: se compila y se prueba sin ventana.
//!
//! | Módulo        | Responsabilidad                                              |
//! |---------------|--------------------------------------------------------------|
//! | `providers`   | Integraciones reales con cada fuente (trait `Provider`)      |
//! | `query`       | Normalización, sinónimos, traducción, entidades              |
//! | `rank`        | Relevancia y claves de duplicados                            |
//! | `engine`      | Búsqueda federada progresiva, caché, cancelación, límites    |
//! | `db`          | SQLite: favoritos, colecciones, historial, descargas         |
//! | `downloads`   | Descargas validadas, reanudables y sin duplicados            |
//! | `validate`    | Nombres seguros, path traversal, tipo real de archivo        |
//! | `maintenance` | Autolimpieza y auditoría de almacenamiento                   |
//! | `settings`    | Configuración persistente                                    |

pub mod db;
pub mod dedup;
pub mod downloads;
pub mod engine;
pub mod error;
pub mod http;
pub mod intent;
pub mod maintenance;
pub mod model;
pub mod providers;
pub mod quality;
pub mod query;
pub mod rank;
pub mod rights;
pub mod settings;
pub mod validate;
pub mod visual;

pub use error::{CoreError, Result};
