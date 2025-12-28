use anyhow::Result;
use std::collections::HashMap;
use std::env;
use std::path::PathBuf;

/// Credenciales de usuario para SADE
#[derive(Debug, Clone)]
pub struct Credenciales {
    pub usuario: String,
    pub password: String,
}

/// Configuración de la aplicación
#[derive(Debug, Clone)]
pub struct Config {
    pub ruta_archivos: PathBuf,
    pub ruta_excel: PathBuf,
    pub usuarios: HashMap<String, Credenciales>,
}

/// Obtiene el directorio donde está el ejecutable
fn obtener_directorio_exe() -> PathBuf {
    env::current_exe()
        .ok()
        .and_then(|p| p.parent().map(|p| p.to_path_buf()))
        .unwrap_or_else(|| PathBuf::from("."))
}

impl Config {
    /// Carga la configuración - las rutas se basan en el directorio del ejecutable
    pub fn from_env() -> Result<Self> {
        let dir_exe = obtener_directorio_exe();

        // RUTA_ARCHIVOS = directorio del exe (donde están los PDFs)
        let ruta_archivos = dir_exe.clone();

        // RUTA_EXCEL = archivo "Listado RDP a copiar.xlsx" en el directorio del exe
        let ruta_excel = dir_exe.join("Listado RDP a copiar.xlsx");

        let mut usuarios = HashMap::new();

        // Cargar credenciales de ERICA
        if let (Ok(user), Ok(pass)) = (env::var("SADE_USER_ERICA"), env::var("SADE_PASSWORD_ERICA"))
        {
            usuarios.insert(
                "1".to_string(),
                Credenciales {
                    usuario: user,
                    password: pass,
                },
            );
        }

        // Cargar credenciales de CECILIA
        if let (Ok(user), Ok(pass)) = (
            env::var("SADE_USER_CECILIA"),
            env::var("SADE_PASSWORD_CECILIA"),
        ) {
            usuarios.insert(
                "2".to_string(),
                Credenciales {
                    usuario: user,
                    password: pass,
                },
            );
        }

        // Verificar que hay al menos un usuario configurado
        if usuarios.is_empty() {
            anyhow::bail!("No se encontraron credenciales de usuario en el archivo .env");
        }

        // Crear subcarpetas necesarias si no existen
        let _ = std::fs::create_dir_all(ruta_archivos.join("Procesados"));
        let _ = std::fs::create_dir_all(ruta_archivos.join("Revisar"));

        Ok(Config {
            ruta_archivos,
            ruta_excel,
            usuarios,
        })
    }

    /// Obtiene las credenciales para un usuario específico
    pub fn get_credenciales(&self, usuario_id: &str) -> Option<&Credenciales> {
        self.usuarios.get(usuario_id)
    }
}

impl Default for Config {
    /// Crea una configuración por defecto basada en el directorio del ejecutable
    fn default() -> Self {
        let dir_exe = obtener_directorio_exe();
        Config {
            ruta_archivos: dir_exe.clone(),
            ruta_excel: dir_exe.join("Listado RDP a copiar.xlsx"),
            usuarios: HashMap::new(),
        }
    }
}
