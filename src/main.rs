// Ocultar ventana de consola en Windows (solo en modo release)
#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

mod busqueda_comunicaciones;
mod config;
mod excel_handler;
mod file_processor;
mod gui;
mod pdf_extractor;
mod web_automation;

use std::env;

fn main() -> eframe::Result<()> {
    // Intentar cargar .env desde múltiples ubicaciones

    // 1. Junto al ejecutable
    if let Ok(exe_path) = env::current_exe() {
        if let Some(exe_dir) = exe_path.parent() {
            let env_path = exe_dir.join(".env");
            if env_path.exists() {
                let _ = dotenvy::from_path(&env_path);
            }
        }
    }

    // 2. Directorio actual de trabajo
    let _ = dotenvy::dotenv();

    // 3. Directorio del proyecto (solo funciona durante desarrollo)
    #[cfg(debug_assertions)]
    {
        let manifest_dir = env!("CARGO_MANIFEST_DIR");
        let env_path = std::path::Path::new(manifest_dir).join(".env");
        let _ = dotenvy::from_path(&env_path);
    }

    // Ejecutar la GUI
    gui::run()
}
