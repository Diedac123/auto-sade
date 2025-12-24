use crate::pdf_extractor::DatosPdf;
use anyhow::{Context, Result};
use calamine::{open_workbook, Reader, Xlsx};
use rust_xlsxwriter::Workbook;
use std::path::Path;

/// Guarda los datos extraídos en un archivo Excel
/// Equivalente a `guardar_dataframe` en Python
pub fn guardar_excel(datos: &[DatosPdf], ruta_salida: &Path) -> Result<()> {
    let mut workbook = Workbook::new();
    
    // Crear nueva hoja
    let worksheet = workbook.add_worksheet();
    worksheet.set_name("CCOO revisar")?;
    
    // Escribir encabezados
    let headers = [
        "CCOO N°",
        "ORGANISMO",
        "Institucional Patrimonial",
        "Fecha",
        "RESULTADO INVENTARIO FISICO",
    ];
    
    for (col, header) in headers.iter().enumerate() {
        worksheet.write_string(0, col as u16, *header)?;
    }
    
    // Escribir datos
    for (row, dato) in datos.iter().enumerate() {
        let row_num = (row + 1) as u32;
        worksheet.write_string(row_num, 0, &dato.ccoo)?;
        worksheet.write_string(row_num, 1, &dato.organismo)?;
        worksheet.write_string(row_num, 2, &dato.patrimonial)?;
        worksheet.write_string(row_num, 3, &dato.fecha)?;
        worksheet.write_string(row_num, 4, &dato.resultado)?;
    }
    
    // Intentar agregar a archivo existente o crear nuevo
    if ruta_salida.exists() {
        // Si el archivo existe, intentamos agregar una nueva hoja
        // Nota: rust_xlsxwriter no soporta edición de archivos existentes directamente
        // Así que creamos un nuevo archivo con sufijo
        let stem = ruta_salida.file_stem()
            .and_then(|s| s.to_str())
            .unwrap_or("output");
        let extension = ruta_salida.extension()
            .and_then(|s| s.to_str())
            .unwrap_or("xlsx");
        let parent = ruta_salida.parent().unwrap_or(Path::new("."));
        
        let timestamp = chrono::Local::now().format("%Y%m%d_%H%M%S");
        let nuevo_nombre = format!("{}_{}.{}", stem, timestamp, extension);
        let nueva_ruta = parent.join(nuevo_nombre);
        
        workbook.save(&nueva_ruta)
            .with_context(|| format!("Error al guardar Excel en {:?}", nueva_ruta))?;
        
        println!("Archivo guardado en {:?}", nueva_ruta);
    } else {
        workbook.save(ruta_salida)
            .with_context(|| format!("Error al guardar Excel en {:?}", ruta_salida))?;
    }
    
    Ok(())
}

/// Lee un archivo Excel existente (para referencia futura)
#[allow(dead_code)]
pub fn leer_excel(ruta: &Path) -> Result<Vec<Vec<String>>> {
    let mut workbook: Xlsx<_> = open_workbook(ruta)
        .with_context(|| format!("Error al abrir Excel: {:?}", ruta))?;
    
    let mut datos = Vec::new();
    
    if let Some(Ok(range)) = workbook.worksheet_range_at(0) {
        for row in range.rows() {
            let fila: Vec<String> = row.iter()
                .map(|cell| cell.to_string())
                .collect();
            datos.push(fila);
        }
    }
    
    Ok(datos)
}
