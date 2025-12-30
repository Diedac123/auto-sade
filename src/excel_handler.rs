use crate::pdf_extractor::DatosPdf;
use anyhow::{Context, Result};
use calamine::{open_workbook, Reader, Xlsx};
use chrono::NaiveDate;
use rust_xlsxwriter::{Format, Workbook};
use std::path::Path;

/// Convierte una fecha NaiveDate al número serial de Excel
/// Excel usa el sistema de fechas 1900, donde el 1 de enero de 1900 = 1
/// Nota: Excel tiene un bug histórico donde considera 1900 como año bisiesto
fn fecha_a_excel_serial(fecha: &NaiveDate) -> f64 {
    // Fecha base de Excel: 30 de diciembre de 1899
    // (Excel cuenta desde 1, y tiene el bug del año bisiesto 1900)
    let fecha_base = NaiveDate::from_ymd_opt(1899, 12, 30).unwrap();
    let dias = fecha.signed_duration_since(fecha_base).num_days();
    dias as f64
}

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

    // Crear formato de fecha para Excel (dd/mm/yyyy)
    let formato_fecha = Format::new().set_num_format("dd/mm/yyyy");

    // Escribir datos
    for (row, dato) in datos.iter().enumerate() {
        let row_num = (row + 1) as u32;
        worksheet.write_string(row_num, 0, &dato.ccoo)?;
        worksheet.write_string(row_num, 1, &dato.organismo)?;
        worksheet.write_string(row_num, 2, &dato.patrimonial)?;

        // Escribir fecha como número con formato de fecha
        // Excel la reconocerá automáticamente como fecha
        if let Some(ref fecha) = dato.fecha {
            let serial = fecha_a_excel_serial(fecha);
            worksheet.write_number_with_format(row_num, 3, serial, &formato_fecha)?;
        } else {
            worksheet.write_string(row_num, 3, "")?;
        }

        worksheet.write_string(row_num, 4, &dato.resultado)?;
    }

    // Intentar agregar a archivo existente o crear nuevo
    if ruta_salida.exists() {
        // Si el archivo existe, intentamos agregar una nueva hoja
        // Nota: rust_xlsxwriter no soporta edición de archivos existentes directamente
        // Así que creamos un nuevo archivo con sufijo
        let stem = ruta_salida
            .file_stem()
            .and_then(|s| s.to_str())
            .unwrap_or("output");
        let extension = ruta_salida
            .extension()
            .and_then(|s| s.to_str())
            .unwrap_or("xlsx");
        let parent = ruta_salida.parent().unwrap_or(Path::new("."));

        let timestamp = chrono::Local::now().format("%Y%m%d_%H%M%S");
        let nuevo_nombre = format!("{}_{}.{}", stem, timestamp, extension);
        let nueva_ruta = parent.join(nuevo_nombre);

        workbook
            .save(&nueva_ruta)
            .with_context(|| format!("Error al guardar Excel en {:?}", nueva_ruta))?;

        println!("Archivo guardado en {:?}", nueva_ruta);
    } else {
        workbook
            .save(ruta_salida)
            .with_context(|| format!("Error al guardar Excel en {:?}", ruta_salida))?;
    }

    Ok(())
}

/// Lee un archivo Excel existente (para referencia futura)
#[allow(dead_code)]
pub fn leer_excel(ruta: &Path) -> Result<Vec<Vec<String>>> {
    let mut workbook: Xlsx<_> =
        open_workbook(ruta).with_context(|| format!("Error al abrir Excel: {:?}", ruta))?;

    let mut datos = Vec::new();

    if let Some(Ok(range)) = workbook.worksheet_range_at(0) {
        for row in range.rows() {
            let fila: Vec<String> = row.iter().map(|cell| cell.to_string()).collect();
            datos.push(fila);
        }
    }

    Ok(datos)
}
