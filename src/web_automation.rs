use crate::config::Config;
use anyhow::{Context, Result};
use chromiumoxide::browser::{Browser, BrowserConfig};
use futures::StreamExt;
use std::path::PathBuf;
use std::time::Duration;
use tokio::time::sleep;

/// Configura un perfil temporal con preferencias para desactivar traducción
fn setup_custom_profile() -> Result<PathBuf> {
    let mut temp_dir = std::env::temp_dir();
    let unique_id = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)?
        .as_nanos();
    temp_dir.push(format!("auto_sade_profile_{}", unique_id));

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
/// Retorna true si las descargas terminaron, false si se agotó el tiempo
async fn esperar_descargas_completas(ruta_descargas: &PathBuf, timeout_secs: u64) -> bool {
    let inicio = std::time::Instant::now();
    let timeout = Duration::from_secs(timeout_secs);
    let tiempo_estabilidad = Duration::from_secs(2); // Debe estar estable 2 segundos sin .crdownload
    let mut inicio_estabilidad: Option<std::time::Instant> = None;

    loop {
        // Verificar si hay archivos .crdownload
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
            // Si hay pendientes, reseteamos el contador de estabilidad
            inicio_estabilidad = None;
        } else {
            // Si no hay pendientes, iniciamos o chequeamos el contador de estabilidad
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

/// Resultado de la descarga de comunicaciones
#[derive(Debug, Default)]
pub struct ResultadoDescarga {
    pub comunicaciones_procesadas: u32,
    pub total_comunicaciones: u32,
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
    let credenciales = config
        .get_credenciales(usuario_id)
        .context("Credenciales de usuario no encontradas")?;

    // Obtener carpeta de descargas
    let ruta_descargas = dirs::download_dir().unwrap_or_else(|| PathBuf::from("."));

    // Configurar perfil personalizado para preferencias
    let user_data_dir = setup_custom_profile()?;

    // Configurar navegador con opciones para permitir descargas inseguras
    let browser_config = BrowserConfig::builder()
        .user_data_dir(&user_data_dir)
        .with_head() // Mostrar navegador (no headless)
        // Suprimir popups y diálogos
        .arg("--no-first-run")
        .arg("--no-default-browser-check")
        .arg("--disable-session-crashed-bubble")
        .arg("--disable-session-crashed-bubble")
        .arg("--disable-infobars")
        .arg("--disable-restore-session-state") // Evita restaurar sesión anterior
        .arg("--disable-background-networking")
        .arg("--hide-crash-restore-bubble") // Oculta popup de restauración
        .arg("--lang=es-419") // Forzar idioma español
        // Desactiva popup de traducción y otras características no deseadas en una sola bandera
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
        .map_err(|e| anyhow::anyhow!("Error al configurar navegador: {}", e))?;

    let (browser, mut handler) = Browser::launch(browser_config)
        .await
        .context("Error al iniciar navegador")?;

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

    // Buscar campos de texto
    let mut inputs = page.find_elements(".form-control.z-textbox").await?;

    // Si no hay campos de login, probablemente hay una sesión activa - hacer logout
    if inputs.len() < 2 {
        on_status("Sesión existente detectada, cerrando sesión...");

        // Buscar y hacer clic en el botón de logout
        let logout_btn = page
            .find_elements(".z-icon-sign-out.texto-header-unificado.z-span")
            .await?;
        if !logout_btn.is_empty() {
            logout_btn[0].click().await?;

            // Esperar a que se complete el logout
            sleep(Duration::from_secs(2)).await;

            // Navegar de nuevo a la página para tener un estado limpio
            on_status("Navegando a SADE nuevamente...");
            page.goto("http://euc.gcba.gob.ar/ccoo-web/")
                .await
                .context("Error al navegar a SADE después del logout")?;

            sleep(Duration::from_secs(2)).await;

            // Volver a buscar los campos de login
            inputs = page.find_elements(".form-control.z-textbox").await?;
        }
    }

    // Ahora hacer login
    if inputs.len() >= 2 {
        on_status("Ingresando credenciales...");
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
    } else {
        anyhow::bail!("No se encontraron los campos de login después de intentar logout");
    }

    sleep(Duration::from_secs(3)).await;

    // Navegar a Bandeja CO
    on_status("Navegando a Bandeja CO...");
    let tabs = page.find_elements(".z-tab-text").await?;
    if tabs.len() > 3 {
        tabs[3].click().await?;
    }

    sleep(Duration::from_secs(2)).await;

    // Seleccionar ver 100 elementos
    let botones = page.find_elements(".boton-sin-caja.z-button").await?;
    if botones.len() > 27 {
        botones[27].click().await?;
    }

    sleep(Duration::from_secs(4)).await;

    // Calcular páginas a avanzar
    let paginas_completas = (inicio - 1) / 100;

    if paginas_completas > 0 {
        on_status(&format!("Avanzando a página {}...", paginas_completas + 1));
        for _ in 0..paginas_completas {
            let next_btns = page.find_elements(".z-paging-button.z-paging-next").await?;
            if next_btns.len() > 5 {
                next_btns[5].click().await?;
                sleep(Duration::from_secs(2)).await;
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

        sleep(Duration::from_secs(1)).await;

        // Hacer clic en la comunicación
        let search_icons = page.find_elements(".z-icon-search.z-span").await?;
        if search_icons.len() > indice_actual {
            if let Err(e) = search_icons[indice_actual].click().await {
                eprintln!(
                    "Error al hacer clic en comunicación {}: {}",
                    num_comunicacion, e
                );
                continue;
            }
        }

        sleep(Duration::from_secs(1)).await;

        // Descargar archivos adjuntos
        loop {
            sleep(Duration::from_secs(1)).await;
            let download_icons = page.find_elements(".z-icon-download").await?;

            if download_icons.is_empty() {
                break;
            }

            let cantidad_archivos = download_icons.len() - 1; // Menos el primero que no se descarga

            // Descargar todos los archivos excepto el primero
            for i in 1..download_icons.len() {
                if let Err(e) = download_icons[i].click().await {
                    eprintln!("Error descargando archivo {}: {}", i, e);
                }
                // Espera mínima entre clics (solo para que el navegador procese)
                sleep(Duration::from_millis(300)).await;
            }

            // Espera inicial de 3s para asegurar que Chrome cree los archivos .crdownload
            sleep(Duration::from_secs(3)).await;

            // Esperar a que las descargas terminen (verificando archivos .crdownload)
            // Timeout reducido a 10s por pedido del usuario
            let timeout_descarga = 1;
            if !esperar_descargas_completas(&ruta_descargas, timeout_descarga).await {
                eprintln!("Advertencia: Algunas descargas pueden no haber terminado");
            }

            // Verificar si hay más páginas de adjuntos
            let next_btns = page.find_elements(".z-paging-button.z-paging-next").await?;
            if next_btns.len() > 1 {
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

        sleep(Duration::from_secs(1)).await;

        // Si es la última comunicación, esperamos 10 segundos extra antes de cerrar
        if num_comunicacion == final_ {
            on_status("Esperando 10s extra por seguridad al ser la última comunicación...");
            sleep(Duration::from_secs(10)).await;
        }

        comunicaciones_procesadas += 1;
    }

    // Espera final breve antes de cerrar
    on_status("Finalizando...");
    sleep(Duration::from_secs(3)).await;

    // Cerrar navegador
    drop(browser);
    handle.abort();

    // Dar tiempo al SO para liberar los archivos (Windows suele ser lento liberando locks)
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

    Ok(ResultadoDescarga {
        comunicaciones_procesadas,
        total_comunicaciones,
    })
}
