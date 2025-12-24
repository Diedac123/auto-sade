use anyhow::{Context, Result};
use chrono::NaiveDateTime;
use lopdf::Document;
use pdf_extract::extract_text;
use regex::Regex;
use std::fs;
use std::path::Path;

/// Datos extraídos de un PDF
#[derive(Debug, Clone)]
pub struct DatosPdf {
    pub ccoo: String,
    pub organismo: String,
    pub patrimonial: String,
    pub fecha: String,
    pub resultado: String,
}

/// Extrae el organismo de las anotaciones del PDF
/// Equivalente a `extraer_organismo` en Python (usa pikepdf)
pub fn extraer_organismo(ruta_pdf: &Path) -> Result<String> {
    let doc = Document::load(ruta_pdf)
        .with_context(|| format!("Error al cargar PDF: {:?}", ruta_pdf))?;
    
    // Iterar sobre las páginas
    for page_id in doc.page_iter() {
        if let Ok(page) = doc.get_dictionary(page_id) {
            // Intentar obtener anotaciones
            let annots = match page.get(b"Annots") {
                Ok(a) => a,
                Err(_) => continue,
            };
            
            // Las anotaciones pueden ser un array directo o una referencia
            let annots_array = if let Ok(arr) = annots.as_array() {
                arr.clone()
            } else if let Ok(ref_id) = annots.as_reference() {
                if let Ok(arr_obj) = doc.get_object(ref_id) {
                    if let Ok(arr) = arr_obj.as_array() {
                        arr.clone()
                    } else {
                        continue;
                    }
                } else {
                    continue;
                }
            } else {
                continue;
            };
            
            for annot_ref in &annots_array {
                // Obtener el diccionario de la anotación
                let annot = if let Ok(ref_id) = annot_ref.as_reference() {
                    match doc.get_dictionary(ref_id) {
                        Ok(d) => d,
                        Err(_) => continue,
                    }
                } else if let Ok(d) = annot_ref.as_dict() {
                    d
                } else {
                    continue;
                };
                
                // Obtener el nombre de la anotación (/T)
                let annot_name = if let Ok(t_value) = annot.get(b"T") {
                    // Puede ser name o string
                    if let Ok(name) = t_value.as_name_str() {
                        name.to_string()
                    } else if let Ok(bytes) = t_value.as_str() {
                        String::from_utf8_lossy(bytes).to_string()
                    } else {
                        continue;
                    }
                } else {
                    continue;
                };
                
                // Buscar "reparticion_0"
                if annot_name == "reparticion_0" {
                    if let Ok(v_value) = annot.get(b"V") {
                        // El valor puede ser string o bytes
                        let value = if let Ok(bytes) = v_value.as_str() {
                            String::from_utf8_lossy(bytes).to_string()
                        } else if let Ok(name) = v_value.as_name_str() {
                            name.to_string()
                        } else {
                            continue;
                        };
                        
                        // Tomar solo la primera línea
                        let organismo = value
                            .lines()
                            .next()
                            .unwrap_or("")
                            .to_string();
                        
                        if !organismo.is_empty() {
                            return Ok(organismo);
                        }
                    }
                }
            }
        }
    }
    Ok(String::new())
}

/// Convierte la fecha del PDF al formato dd/mm/aaaa
/// Equivalente a `convertir_fecha_pdf` en Python
pub fn convertir_fecha_pdf(fecha_pdf: &str) -> Result<String> {
    // Eliminar prefijo "D:" y zona horaria
    let fecha_limpia = fecha_pdf
        .strip_prefix("D:")
        .unwrap_or(fecha_pdf)
        .split('-')
        .next()
        .unwrap_or("")
        .split('+')
        .next()
        .unwrap_or("");
    
    if fecha_limpia.len() < 14 {
        return Ok(String::new());
    }
    
    // Parsear la fecha (formato: YYYYMMDDHHMMSS)
    let fecha_parseada = NaiveDateTime::parse_from_str(fecha_limpia, "%Y%m%d%H%M%S")
        .context("Error al parsear fecha del PDF")?;
    
    Ok(fecha_parseada.format("%d/%m/%Y").to_string())
}

/// Extrae el código patrimonial del texto
/// Equivalente a `extraer_patrimonial` en Python
pub fn extraer_patrimonial(texto: &str) -> Option<String> {
    let patron = Regex::new(r"\d\.\d{2}\.\d\.\d\.\d{3,5}\.\d\.\d").ok()?;
    patron.find(texto).map(|m| m.as_str().to_string())
}

/// Determina el resultado del inventario basado en patrones de texto
/// Equivalente a `extraer_resultado` en Python
pub fn extraer_resultado(texto: &str) -> String {
    let texto_sin_espacios = texto.replace(' ', "");
    
    let patrones = [
        r"(?i)sinnovedad",
        r"(?i)sinnovedades",
        r"(?i)encuentransincambio",
        r"(?i)encuentrasincambio",
        r"(?i)noexistennovedad",
        r"(?i)nosurgennovedades",
        r"(?i)nopresentanovedad",
        r"(?i)EXCEDENTESNIFALTANTESANOTIFICAR",
        r"(?i)noexistediferenciaentreloobranteenelSigaf",
        r"(?i)nohabiéndoseenprincipioverificadodiferencias",
        r"(?i)nohabiendoseenprincipioverificadodiferencias",
        r"(?i)seencuentraninventariadoscomobienesmuebles",
        r"(?i)noposeebienesasignadosporelSIGAFWEB",
        r"(?i)noseregistrannovedad",
        r#"(?i)noregistra"excedentes"ni"faltantes""#,
        r"(?i)noregistraexcedentesnifaltantes",
        r"(?i)Sinnovedadesalrespecto",
        r"(?i)Hasidoverificadayseencuentracorrecta",
        r"(?i)sinencontrarnovedades",
        r"(?i)sinvariaciones",
        r"(?i)SINNOVEDA",
        r"(?i)noregistranovedad",
        r"(?i)nohubonovedad",
        r"(?i)SinExcedentesySinFaltantes",
        r"(?i)NosehanlocalizadobienesExcedentesy/oFaltantes",
        r"(?i)S/NOVEDAD",
        r"(?i)nohaynovedad",
        r"(?i)nosehanverificadonovedades",
        r"(?i)novedadalguna",
        r"(?i)nohahabidonovedades",
        r"(?i)noposeesaldosenbienesprecarios",
        r"(?i)sinvariacion",
        r"(?i)noseregistraronnovedades",
        r"(?i)sinmodificacion",
        r"(?i)noregistramovimientos",
        r"(?i)notienenovedad",
        r"(?i)nohabiéndoseencontradodiferencias",
        r"(?i)noarrojanovedad",
        r"(?i)notuvonovedad",
    ];
    
    for patron in &patrones {
        if let Ok(regex) = Regex::new(patron) {
            if regex.is_match(&texto_sin_espacios) {
                return "Sin novedad".to_string();
            }
        }
    }
    
    "Con novedades (ver)".to_string()
}

/// Procesa todos los archivos PDF en una carpeta
/// Equivalente a `procesar_pdfs` en Python
pub fn procesar_pdfs(ruta_archivos: &Path) -> Result<Vec<DatosPdf>> {
    let mut lista_datos = Vec::new();
    
    // Crear directorios de destino si no existen
    let dir_procesados = ruta_archivos.join("Procesados");
    let dir_revisar = ruta_archivos.join("Revisar");
    fs::create_dir_all(&dir_procesados)?;
    fs::create_dir_all(&dir_revisar)?;
    
    let entries = fs::read_dir(ruta_archivos)
        .with_context(|| format!("Error al leer directorio: {:?}", ruta_archivos))?;
    
    for entry in entries.flatten() {
        let path = entry.path();
        
        // Solo procesar archivos PDF
        if path.extension().and_then(|e| e.to_str()) != Some("pdf") {
            continue;
        }
        
        let archivo_pdf = path.file_name()
            .and_then(|n| n.to_str())
            .unwrap_or("");
        
        let ccoo = path.file_stem()
            .and_then(|n| n.to_str())
            .unwrap_or("")
            .to_string();
        
        // Extraer texto del PDF
        let texto = match extract_text(&path) {
            Ok(t) => t.replace('\n', " ")
                .chars()
                .filter(|c| !c.is_control())
                .collect::<String>(),
            Err(_) => continue,
        };
        
        // Extraer fecha de metadatos
        let fecha = if let Ok(doc) = Document::load(&path) {
            if let Some(info) = doc.trailer.get(b"Info").ok().and_then(|i| i.as_reference().ok()) {
                if let Ok(info_dict) = doc.get_dictionary(info) {
                    info_dict.get(b"ModDate")
                        .ok()
                        .and_then(|d| d.as_str().ok())
                        .and_then(|s| {
                            let s_str = String::from_utf8_lossy(s);
                            convertir_fecha_pdf(&s_str).ok()
                        })
                        .unwrap_or_default()
                } else {
                    String::new()
                }
            } else {
                String::new()
            }
        } else {
            String::new()
        };
        
        // Extraer datos
        let organismo = extraer_organismo(&path).unwrap_or_default();
        let patrimonial = extraer_patrimonial(&texto).unwrap_or_default();
        let resultado = extraer_resultado(&texto);
        
        lista_datos.push(DatosPdf {
            ccoo,
            organismo,
            patrimonial,
            fecha,
            resultado: resultado.clone(),
        });
        
        // Mover archivo según resultado
        let destino = if resultado == "Sin novedad" {
            dir_procesados.join(archivo_pdf)
        } else {
            dir_revisar.join(archivo_pdf)
        };
        
        if let Err(e) = fs::rename(&path, &destino) {
            eprintln!("Error al mover archivo {}: {}", archivo_pdf, e);
        }
    }
    
    Ok(lista_datos)
}
