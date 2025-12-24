use anyhow::{Context, Result};
use std::collections::HashMap;
use std::env;
use std::path::PathBuf;

/// Credenciales de usuario para SADE
#[derive(Debug, Clone)]
pub struct Credenciales {
    pub usuario: String,
    pub password: String,
}

/// Configuración de la aplicación cargada desde variables de entorno
#[derive(Debug, Clone)]
pub struct Config {
    pub ruta_archivos: PathBuf,
    pub ruta_excel: PathBuf,
    pub ruta_icono: Option<PathBuf>,
    pub usuarios: HashMap<String, Credenciales>,
}

impl Config {
    /// Carga la configuración desde las variables de entorno
    pub fn from_env() -> Result<Self> {
        let ruta_archivos = env::var("RUTA_ARCHIVOS")
            .context("Variable de entorno RUTA_ARCHIVOS no encontrada. Asegúrate de que el archivo .env existe y contiene esta variable.")?;
        
        let ruta_excel = env::var("RUTA_EXCEL")
            .context("Variable de entorno RUTA_EXCEL no encontrada. Asegúrate de que el archivo .env existe y contiene esta variable.")?;
        
        let ruta_icono = env::var("RUTA_ICONO").ok().map(PathBuf::from);
        
        let mut usuarios = HashMap::new();
        
        // Cargar credenciales de ERICA
        if let (Ok(user), Ok(pass)) = (
            env::var("SADE_USER_ERICA"),
            env::var("SADE_PASSWORD_ERICA"),
        ) {
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
        
        Ok(Config {
            ruta_archivos: PathBuf::from(ruta_archivos),
            ruta_excel: PathBuf::from(ruta_excel),
            ruta_icono,
            usuarios,
        })
    }
    
    /// Obtiene las credenciales para un usuario específico
    pub fn get_credenciales(&self, usuario_id: &str) -> Option<&Credenciales> {
        self.usuarios.get(usuario_id)
    }
}

impl Default for Config {
    /// Crea una configuración vacía/por defecto cuando no hay .env
    fn default() -> Self {
        Config {
            ruta_archivos: PathBuf::new(),
            ruta_excel: PathBuf::new(),
            ruta_icono: None,
            usuarios: HashMap::new(),
        }
    }
}
