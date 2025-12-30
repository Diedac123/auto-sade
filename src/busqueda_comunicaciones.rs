//! Módulo para buscar y descargar comunicaciones desde un archivo Excel
//!
//! Equivalente Rust del script Python `busqueda_comunicaciones.py`

use crate::config::Config;
use anyhow::{Context, Result};
use calamine::{open_workbook, Reader, Xlsx};
use chromiumoxide::browser::{Browser, BrowserConfig};
use futures::StreamExt;
use std::path::{Path, PathBuf};
use std::time::Duration;
use tokio::time::sleep;

/// Resultado de la búsqueda de comunicaciones
#[derive(Debug, Default)]
pub struct ResultadoBusqueda {
    pub comunicaciones_descargadas: usize,
    pub total_comunicaciones: usize,
}

/// Lee un archivo Excel y devuelve las comunicaciones (CCOO N°) que no tienen organismo asignado
pub fn obtener_comunicaciones_sin_organismo(path: &Path) -> Result<Vec<String>> {
    let mut workbook: Xlsx<_> = open_workbook(path)
        .with_context(|| format!("No se pudo abrir el archivo Excel: {:?}", path))?;

    // Obtener la primera hoja
    let sheet_name = workbook
        .sheet_names()
        .first()
        .cloned()
        .context("El archivo Excel no tiene hojas")?;

    let range = workbook
        .worksheet_range(&sheet_name)
        .context("No se pudo leer la hoja de cálculo")?;

    // Encontrar los índices de las columnas
    let headers: Vec<String> = range
        .rows()
        .next()
        .context("El archivo está vacío")?
        .iter()
        .map(|cell| cell.to_string().trim().to_string())
        .collect();

    let idx_ccoo = headers
        .iter()
        .position(|h| h == "CCOO N°")
        .context("No se encontró la columna 'CCOO N°'")?;

    let idx_organismo = headers
        .iter()
        .position(|h| h == "ORGANISMO")
        .context("No se encontró la columna 'ORGANISMO'")?;

    // Filtrar filas donde ORGANISMO está vacío
    let mut comunicaciones: Vec<String> = Vec::new();

    for row in range.rows().skip(1) {
        // Saltar encabezado
        let organismo = row
            .get(idx_organismo)
            .map(|c| c.to_string())
            .unwrap_or_default();
        let ccoo = row
            .get(idx_ccoo)
            .map(|c| c.to_string().trim().to_string())
            .unwrap_or_default();

        // Si organismo está vacío y ccoo tiene valor
        if organismo.trim().is_empty() && !ccoo.is_empty() {
            comunicaciones.push(ccoo);
        }
    }

    Ok(comunicaciones)
}

/// Configura un perfil temporal con preferencias para desactivar traducción y permitir descargas
fn setup_custom_profile() -> Result<PathBuf> {
    let mut temp_dir = std::env::temp_dir();
    let unique_id = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)?
        .as_nanos();
    temp_dir.push(format!("busqueda_ccoo_profile_{}", unique_id));

    let default_dir = temp_dir.join("Default");
    std::fs::create_dir_all(&default_dir).context("No se pudo crear directorio del perfil")?;

    let prefs_path = default_dir.join("Preferences");
    let prefs_content = r#"{
        "translate": { "enabled": false },
        "profile": { 
            "password_manager_enabled": false,
            "default_content_setting_values": { "automatic_downloads": 1 }
        },
        "credentials_enable_service": false
    }"#;

    std::fs::write(prefs_path, prefs_content)
        .context("No se pudo escribir archivo de preferencias")?;

    Ok(temp_dir)
}

/// Espera hasta que no haya archivos .crdownload en la carpeta de descargas
async fn esperar_descargas_completas(ruta_descargas: &PathBuf, timeout_secs: u64) -> bool {
    let inicio = std::time::Instant::now();
    let timeout = Duration::from_secs(timeout_secs);
    let tiempo_estabilidad = Duration::from_secs(2);
    let mut inicio_estabilidad: Option<std::time::Instant> = None;

    loop {
        let hay_pendientes = if let Ok(entries) = std::fs::read_dir(ruta_descargas) {
            entries.filter_map(|e| e.ok()).any(|entry| {
                entry
                    .path()
                    .extension()
                    .map(|ext| ext == "crdownload")
                    .unwrap_or(false)
            })
        } else {
            false
        };

        if hay_pendientes {
            inicio_estabilidad = None;
        } else {
            match inicio_estabilidad {
                None => {
                    inicio_estabilidad = Some(std::time::Instant::now());
                }
                Some(instante) => {
                    if instante.elapsed() >= tiempo_estabilidad {
                        return true;
                    }
                }
            }
        }

        if inicio.elapsed() >= timeout {
            return false;
        }

        sleep(Duration::from_millis(500)).await;
    }
}

/// Busca y descarga comunicaciones desde SADE
pub async fn buscar_comunicaciones(
    comunicaciones: &[String],
    usuario_id: &str,
    config: &Config,
    on_status: impl Fn(&str),
) -> Result<ResultadoBusqueda> {
    let total = comunicaciones.len();
    if total == 0 {
        return Ok(ResultadoBusqueda {
            comunicaciones_descargadas: 0,
            total_comunicaciones: 0,
        });
    }

    let credenciales = config
        .get_credenciales(usuario_id)
        .context("Usuario no encontrado en la configuración")?;

    on_status("Iniciando navegador...");

    // Obtener carpeta de descargas del usuario
    let ruta_descargas = dirs::download_dir().unwrap_or_else(|| PathBuf::from("."));

    // Configurar perfil personalizado para preferencias
    let user_data_dir = setup_custom_profile()?;

    // Configurar navegador con opciones para permitir descargas (igual que web_automation)
    let browser_config = BrowserConfig::builder()
        .user_data_dir(&user_data_dir)
        .with_head() // Mostrar navegador (no headless)
        // Suprimir popups y diálogos
        .arg("--no-first-run")
        .arg("--no-default-browser-check")
        .arg("--disable-session-crashed-bubble")
        .arg("--disable-infobars")
        .arg("--disable-restore-session-state")
        .arg("--disable-background-networking")
        .arg("--hide-crash-restore-bubble")
        .arg("--lang=es-419")
        // Desactiva popup de traducción y otras características no deseadas
        .arg("--disable-features=Translate,TranslateUI,DownloadBubble,DownloadBubbleV2,IsolateOrigins,site-per-process,BlockInsecurePrivateNetworkRequests")
        // Permitir descargas múltiples automáticamente
        .arg("--safebrowsing-disable-download-protection")
        // Seguridad relajada para la página
        .arg("--disable-web-security")
        .arg("--allow-running-insecure-content")
        .arg("--disable-site-isolation-trials")
        .arg("--unsafely-treat-insecure-origin-as-secure=http://euc.gcba.gob.ar")
        .arg("--ignore-certificate-errors")
        .arg("--disable-popup-blocking")
        .arg(format!(
            "--download.default_directory={}",
            ruta_descargas.display()
        ))
        .build()
        .map_err(|e| anyhow::anyhow!("Error configurando navegador: {}", e))?;

    let (browser, mut handler) = Browser::launch(browser_config)
        .await
        .context("Error al iniciar el navegador")?;

    // Manejar eventos del navegador en segundo plano
    let handle = tokio::spawn(async move { while let Some(_event) = handler.next().await {} });

    let page = browser
        .new_page("about:blank")
        .await
        .context("Error al crear página")?;

    // Navegar a SADE
    on_status("Navegando a SADE...");
    page.goto("http://euc.gcba.gob.ar/ccoo-web/")
        .await
        .context("Error al navegar a SADE")?;

    sleep(Duration::from_secs(2)).await;

    // Login
    on_status("Iniciando sesión...");

    let inputs = page
        .find_elements(".form-control.z-textbox")
        .await
        .context("No se encontraron campos de login")?;

    if inputs.len() >= 2 {
        inputs[0]
            .click()
            .await?
            .type_str(&credenciales.usuario)
            .await?;
        inputs[1]
            .click()
            .await?
            .type_str(&credenciales.password)
            .await?;

        // Click en botón de login
        let login_btn = page.find_element(".btn.btn-default.z-button").await?;
        login_btn.click().await?;
    }

    sleep(Duration::from_secs(3)).await;

    let mut descargadas = 0;

    // Procesar cada comunicación
    for (idx, comunicacion) in comunicaciones.iter().enumerate() {
        on_status(&format!(
            "Descargando comunicación {} de {}: {}",
            idx + 1,
            total,
            comunicacion
        ));

        // Buscar campo de texto para número de comunicación
        if let Ok(textboxes) = page.find_elements(".z-textbox").await {
            if !textboxes.is_empty() {
                // Limpiar y escribir número de comunicación
                textboxes[0].click().await.ok();

                // Seleccionar todo el texto y reemplazarlo usando JavaScript
                let js_code = format!(
                    r#"
                    (function() {{
                        var input = document.querySelectorAll('.z-textbox')[0];
                        if (input) {{
                            input.value = '{}';
                            input.dispatchEvent(new Event('input', {{ bubbles: true }}));
                            input.dispatchEvent(new Event('change', {{ bubbles: true }}));
                        }}
                    }})();
                    "#,
                    comunicacion
                );

                let _ = page.evaluate(js_code).await;

                sleep(Duration::from_millis(500)).await;

                // Click en botón de búsqueda (tercer z-button)
                if let Ok(buttons) = page.find_elements(".z-button").await {
                    if buttons.len() > 2 {
                        buttons[2].click().await.ok();
                        sleep(Duration::from_secs(2)).await;
                    }
                }

                // Click en botón para ver detalles (índice 29 según Python)
                if let Ok(detail_btns) = page.find_elements(".boton-sin-caja.z-button").await {
                    if detail_btns.len() > 29 {
                        detail_btns[29].click().await.ok();
                        sleep(Duration::from_secs(2)).await;
                    }
                }

                // Click en botón de descarga
                if let Ok(download_btns) = page.find_elements(".z-icon-download").await {
                    if !download_btns.is_empty() {
                        download_btns[0].click().await.ok();

                        // Espera mínima para que el navegador procese
                        sleep(Duration::from_millis(300)).await;

                        // Espera inicial para asegurar que Chrome cree los archivos .crdownload
                        sleep(Duration::from_secs(1)).await;

                        // Esperar a que las descargas terminen
                        if esperar_descargas_completas(&ruta_descargas, 30).await {
                            descargadas += 1;
                        }
                    }
                }

                // Volver a la lista
                if let Ok(back_btns) = page.find_elements(".btn.z-button").await {
                    if !back_btns.is_empty() {
                        back_btns[0].click().await.ok();
                        sleep(Duration::from_secs(1)).await;
                    }
                }
            }
        }
    }

    on_status("Cerrando navegador...");

    // Cerrar navegador
    drop(browser);
    handle.abort();

    // Dar tiempo al SO para liberar los archivos
    sleep(Duration::from_secs(2)).await;

    // Limpiar perfil temporal con reintentos
    let mut clean_retries = 5;
    while clean_retries > 0 {
        if let Err(e) = std::fs::remove_dir_all(&user_data_dir) {
            if clean_retries == 1 {
                eprintln!(
                    "Advertencia: No se pudo limpiar el perfil temporal tras varios intentos: {}",
                    e
                );
            } else {
                sleep(Duration::from_secs(1)).await;
            }
        } else {
            break;
        }
        clean_retries -= 1;
    }

    Ok(ResultadoBusqueda {
        comunicaciones_descargadas: descargadas,
        total_comunicaciones: total,
    })
}
