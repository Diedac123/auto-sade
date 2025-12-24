use anyhow::{Context, Result};
use regex::Regex;
use std::fs;
use std::path::Path;

/// Resultado de la operación de mover archivos
#[derive(Debug, Default)]
pub struct ResultadoMover {
    pub archivos_movidos: usize,
    pub archivos_eliminados: usize,
}

/// Mueve archivos PDF desde la carpeta de descargas al destino
/// y elimina archivos de organismos específicos
/// Equivalente a `mover_archivos` en Python
pub fn mover_archivos(ruta_descarga: &Path, ruta_destino: &Path) -> Result<ResultadoMover> {
    let mut resultado = ResultadoMover::default();
    
    // Patrón para archivos a mover: NO-YYYY-NNNN-GCABA-XXX.pdf
    let patron_mover = Regex::new(r"^NO-\d{4}-\d+-GCABA-[A-Za-z0-9]+\.pdf$")
        .context("Error al compilar regex de mover")?;
    
    // Patrón para archivos a eliminar (organismos específicos)
    let patron_eliminar = Regex::new(
        r"^NO-\d{4}-\d+-GCABA-(DGSOCAI|DGCG|MGEYA|UAIMHF|DGADCYP|EAIT|DGTES|OGEPU|PG|DGAIGA)\.pdf$"
    ).context("Error al compilar regex de eliminar")?;
    
    // Asegurar que el directorio destino existe
    fs::create_dir_all(ruta_destino)?;
    
    // Mover archivos que coinciden con el patrón
    let entries = fs::read_dir(ruta_descarga)
        .with_context(|| format!("Error al leer directorio de descargas: {:?}", ruta_descarga))?;
    
    for entry in entries.flatten() {
        let path = entry.path();
        if !path.is_file() {
            continue;
        }
        
        let nombre = match path.file_name().and_then(|n| n.to_str()) {
            Some(n) => n,
            None => continue,
        };
        
        if patron_mover.is_match(nombre) {
            let ruta_final = ruta_destino.join(nombre);
            match fs::rename(&path, &ruta_final) {
                Ok(_) => resultado.archivos_movidos += 1,
                Err(e) => eprintln!("Error moviendo {}: {}", nombre, e),
            }
        }
    }
    
    // Eliminar archivos específicos del destino
    let entries = fs::read_dir(ruta_destino)
        .with_context(|| format!("Error al leer directorio destino: {:?}", ruta_destino))?;
    
    for entry in entries.flatten() {
        let path = entry.path();
        if !path.is_file() {
            continue;
        }
        
        let nombre = match path.file_name().and_then(|n| n.to_str()) {
            Some(n) => n,
            None => continue,
        };
        
        if patron_eliminar.is_match(nombre) {
            match fs::remove_file(&path) {
                Ok(_) => resultado.archivos_eliminados += 1,
                Err(e) => eprintln!("Error eliminando {}: {}", nombre, e),
            }
        }
    }
    
    Ok(resultado)
}

/// Obtiene la ruta de la carpeta de descargas del usuario
pub fn obtener_ruta_descargas() -> Option<std::path::PathBuf> {
    dirs::download_dir()
}
