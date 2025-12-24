use crate::config::Config;
use anyhow::{Context, Result};
use chromiumoxide::browser::{Browser, BrowserConfig};
use futures::StreamExt;
use std::path::PathBuf;
use std::time::Duration;
use tokio::time::sleep;

/// Resultado de la descarga de comunicaciones
#[derive(Debug, Default)]
pub struct ResultadoDescarga {
    pub comunicaciones_procesadas: u32,
    pub total_comunicaciones: u32,
}

/// Espera a que no haya archivos .crdownload en la carpeta de descargas
async fn esperar_descargas_completas(ruta_descargas: &PathBuf, timeout_secs: u64) -> bool {
    let inicio = std::time::Instant::now();
    let timeout = Duration::from_secs(timeout_secs);
    
    loop {
        if inicio.elapsed() > timeout {
            eprintln!("Timeout esperando descargas");
            return false;
        }
        
        let mut hay_descargando = false;
        
        if let Ok(entries) = std::fs::read_dir(ruta_descargas) {
            for entry in entries.flatten() {
                if let Some(ext) = entry.path().extension() {
                    if ext == "crdownload" || ext == "tmp" {
                        hay_descargando = true;
                        break;
                    }
                }
            }
        }
        
        if !hay_descargando {
            return true;
        }
        
        sleep(Duration::from_millis(500)).await;
    }
}

/// Descarga comunicaciones desde SADE
/// Equivalente a `descargar_comunicaciones` en Python
pub async fn descargar_comunicaciones(
    inicio: u32,
    final_: u32,
    usuario_id: &str,
    config: &Config,
    on_status: impl Fn(&str),
) -> Result<ResultadoDescarga> {
    let credenciales = config.get_credenciales(usuario_id)
        .context("Credenciales de usuario no encontradas")?;
    
    // Obtener carpeta de descargas
    let ruta_descargas = dirs::download_dir()
        .unwrap_or_else(|| PathBuf::from("."));
    
    // Configurar navegador con opciones para permitir descargas inseguras
    let browser_config = BrowserConfig::builder()
        .with_head()  // Mostrar navegador (no headless)
        .arg("--disable-web-security")
        .arg("--allow-running-insecure-content")
        .arg("--disable-features=IsolateOrigins,site-per-process")
        .arg("--disable-site-isolation-trials")
        .arg("--disable-features=BlockInsecurePrivateNetworkRequests")
        .arg("--unsafely-treat-insecure-origin-as-secure=http://euc.gcba.gob.ar")
        .arg("--ignore-certificate-errors")
        .arg("--disable-popup-blocking")
        .arg(format!("--download.default_directory={}", ruta_descargas.display()))
        .build()
        .map_err(|e| anyhow::anyhow!("Error al configurar navegador: {}", e))?;
    
    let (browser, mut handler) = Browser::launch(browser_config)
        .await
        .context("Error al iniciar navegador")?;
    
    // Manejar eventos del navegador en segundo plano
    let handle = tokio::spawn(async move {
        while let Some(_event) = handler.next().await {}
    });
    
    let page = browser.new_page("about:blank")
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
    
    // Buscar campos de texto
    let inputs = page.find_elements(".form-control.z-textbox").await?;
    if inputs.len() >= 2 {
        inputs[0].click().await?.type_str(&credenciales.usuario).await?;
        inputs[1].click().await?.type_str(&credenciales.password).await?;
    }
    
    // Click en botón de login
    let login_btn = page.find_element(".btn.btn-default.z-button").await?;
    login_btn.click().await?;
    
    sleep(Duration::from_secs(2)).await;
    
    // Navegar a Bandeja CO
    on_status("Navegando a Bandeja CO...");
    let tabs = page.find_elements(".z-tab-text").await?;
    if tabs.len() > 3 {
        tabs[3].click().await?;
    }
    
    sleep(Duration::from_secs(1)).await;
    
    // Seleccionar ver 100 elementos
    let botones = page.find_elements(".boton-sin-caja.z-button").await?;
    if botones.len() > 27 {
        botones[27].click().await?;
    }
    
    sleep(Duration::from_millis(500)).await;
    
    // Calcular páginas a avanzar
    let paginas_completas = (inicio - 1) / 100;
    
    if paginas_completas > 0 {
        on_status(&format!("Avanzando a página {}...", paginas_completas + 1));
        for _ in 0..paginas_completas {
            let next_btns = page.find_elements(".z-paging-button.z-paging-next").await?;
            if next_btns.len() > 5 {
                next_btns[5].click().await?;
                sleep(Duration::from_secs(1)).await;
            }
        }
    }
    
    // Procesar comunicaciones
    let mut comunicaciones_procesadas = 0u32;
    let total_comunicaciones = final_ - inicio + 1;
    
    for num_comunicacion in inicio..=final_ {
        on_status(&format!(
            "Descargando comunicación {} ({} de {})",
            num_comunicacion,
            comunicaciones_procesadas + 1,
            total_comunicaciones
        ));
        
        let indice_actual = ((num_comunicacion - 1) % 100) as usize;
        
        // Si llegamos al índice 0 y no es la primera comunicación, avanzar página
        if indice_actual == 0 && num_comunicacion != inicio {
            let next_btns = page.find_elements(".z-paging-button.z-paging-next").await?;
            if next_btns.len() > 5 {
                next_btns[5].click().await?;
                sleep(Duration::from_secs(1)).await;
            }
        }
        
        sleep(Duration::from_millis(500)).await;
        
        // Hacer clic en la comunicación
        let search_icons = page.find_elements(".z-icon-search.z-span").await?;
        if search_icons.len() > indice_actual {
            if let Err(e) = search_icons[indice_actual].click().await {
                eprintln!("Error al hacer clic en comunicación {}: {}", num_comunicacion, e);
                continue;
            }
        }
        
        sleep(Duration::from_secs(1)).await;
        
        // Descargar archivos adjuntos
        loop {
            sleep(Duration::from_millis(500)).await;
            let download_icons = page.find_elements(".z-icon-download").await?;
            
            if download_icons.is_empty() {
                break;
            }
            
            // Descargar todos los archivos excepto el primero
            for i in 1..download_icons.len() {
                if let Err(e) = download_icons[i].click().await {
                    eprintln!("Error descargando archivo {}: {}", i, e);
                }
                // Esperar más tiempo entre descargas
                sleep(Duration::from_secs(1)).await;
            }
            
            // Esperar a que terminen las descargas antes de continuar
            on_status(&format!(
                "Comunicación {} - esperando descargas...",
                num_comunicacion
            ));
            esperar_descargas_completas(&ruta_descargas, 30).await;
            
            // Verificar si hay más páginas de adjuntos - simplificado
            let next_btns = page.find_elements(".z-paging-button.z-paging-next").await?;
            if next_btns.len() > 1 {
                // Intentar hacer clic en el segundo botón de siguiente
                if next_btns[1].click().await.is_err() {
                    break;
                }
                sleep(Duration::from_secs(1)).await;
            } else {
                break;
            }
        }
        
        // Volver a la lista
        let volver_btns = page.find_elements(".btn.z-button").await?;
        if !volver_btns.is_empty() {
            volver_btns[0].click().await?;
        }
        
        sleep(Duration::from_millis(500)).await;
        comunicaciones_procesadas += 1;
    }
    
    // Esperar a que todas las descargas terminen antes de cerrar
    on_status("Finalizando descargas pendientes...");
    esperar_descargas_completas(&ruta_descargas, 60).await;
    
    // Cerrar navegador limpiamente
    on_status("Cerrando navegador...");
    sleep(Duration::from_secs(2)).await;
    drop(browser);
    handle.abort();
    
    Ok(ResultadoDescarga {
        comunicaciones_procesadas,
        total_comunicaciones,
    })
}
