use crate::config::Config;
use crate::excel_handler;
use crate::file_processor;
use crate::pdf_extractor;
use crate::web_automation;
use eframe::egui;
use std::sync::{Arc, Mutex};

/// Estado de la aplicación
#[derive(Debug, Clone, PartialEq)]
pub enum EstadoApp {
    Listo,
    Procesando(String),
    Finalizado(String),
    Error(String),
}

/// Aplicación principal
pub struct AutoSadeApp {
    config: Option<Config>,
    usuario: String,
    comunicacion_inicio: String,
    comunicacion_final: String,
    estado: Arc<Mutex<EstadoApp>>,
    botones_habilitados: Arc<Mutex<bool>>,
}

impl Default for AutoSadeApp {
    fn default() -> Self {
        let (config, estado_inicial) = match Config::from_env() {
            Ok(cfg) => (Some(cfg), EstadoApp::Listo),
            Err(e) => {
                eprintln!("Error al cargar configuración: {}", e);
                (None, EstadoApp::Error(format!("Error de configuración: {}", e)))
            }
        };
        
        Self {
            config,
            usuario: String::new(),
            comunicacion_inicio: String::new(),
            comunicacion_final: String::new(),
            estado: Arc::new(Mutex::new(estado_inicial)),
            botones_habilitados: Arc::new(Mutex::new(true)),
        }
    }
}

impl AutoSadeApp {
    pub fn new(_cc: &eframe::CreationContext<'_>) -> Self {
        Self::default()
    }
    
    fn actualizar_estado(&self, nuevo_estado: EstadoApp) {
        if let Ok(mut estado) = self.estado.lock() {
            *estado = nuevo_estado;
        }
    }
    
    fn habilitar_botones(&self, habilitado: bool) {
        if let Ok(mut hab) = self.botones_habilitados.lock() {
            *hab = habilitado;
        }
    }
    
    fn botones_estan_habilitados(&self) -> bool {
        self.botones_habilitados.lock().map(|h| *h).unwrap_or(true)
    }
    
    fn obtener_estado(&self) -> EstadoApp {
        self.estado.lock().map(|e| e.clone()).unwrap_or(EstadoApp::Listo)
    }
}

impl eframe::App for AutoSadeApp {
    fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        egui::CentralPanel::default().show(ctx, |ui| {
            // Obtener el ancho disponible para centrar contenido
            let panel_width = ui.available_width();
            let content_width = 340.0_f32.min(panel_width - 40.0);
            
            ui.vertical_centered(|ui| {
                ui.add_space(30.0);
                
                // Título con estilo
                ui.label(
                    egui::RichText::new("Automatización Comunicaciones SADE")
                        .heading()
                        .size(22.0)
                );
                
                ui.add_space(25.0);
                
                // Frame de inputs centrado
                ui.allocate_ui_with_layout(
                    egui::vec2(content_width, 0.0),
                    egui::Layout::top_down(egui::Align::Center),
                    |ui| {
                        egui::Frame::default()
                            .inner_margin(egui::Margin::same(20.0))
                            .fill(ui.style().visuals.extreme_bg_color)
                            .rounding(egui::Rounding::same(10.0))
                            .stroke(egui::Stroke::new(1.0, ui.style().visuals.widgets.noninteractive.bg_stroke.color))
                            .show(ui, |ui| {
                                ui.set_width(content_width - 40.0);
                                
                                egui::Grid::new("input_grid")
                                    .num_columns(2)
                                    .spacing([15.0, 12.0])
                                    .show(ui, |ui| {
                                        ui.label("Usuario (1=Erica, 2=Cecilia):");
                                        let usuario_response = ui.add(egui::TextEdit::singleline(&mut self.usuario)
                                            .desired_width(80.0)
                                            .horizontal_align(egui::Align::Center));
                                        // Dar foco al campo de usuario cuando está vacío
                                        if self.usuario.is_empty() && self.comunicacion_inicio.is_empty() {
                                            usuario_response.request_focus();
                                        }
                                        ui.end_row();
                                        
                                        ui.label("Comunicación Inicial:");
                                        ui.add(egui::TextEdit::singleline(&mut self.comunicacion_inicio)
                                            .desired_width(80.0)
                                            .horizontal_align(egui::Align::Center));
                                        ui.end_row();
                                        
                                        ui.label("Comunicación Final:");
                                        ui.add(egui::TextEdit::singleline(&mut self.comunicacion_final)
                                            .desired_width(80.0)
                                            .horizontal_align(egui::Align::Center));
                                        ui.end_row();
                                    });
                            });
                    },
                );
                
                ui.add_space(25.0);
                
                // Botones con ancho uniforme
                let button_width = 180.0;
                let botones_habilitados = self.botones_estan_habilitados();
                
                ui.add_enabled_ui(botones_habilitados, |ui| {
                    if ui.add_sized([button_width, 32.0], egui::Button::new("⬇  Descargar")).clicked() {
                        self.habilitar_botones(false);
                        self.actualizar_estado(EstadoApp::Procesando("Descargando comunicaciones...".to_string()));
                        
                        let inicio: u32 = self.comunicacion_inicio.parse().unwrap_or(1);
                        let final_: u32 = self.comunicacion_final.parse().unwrap_or(1);
                        let usuario = self.usuario.clone();
                        let config = self.config.clone();
                        let estado = Arc::clone(&self.estado);
                        let botones = Arc::clone(&self.botones_habilitados);
                        
                        std::thread::spawn(move || {
                            let rt = tokio::runtime::Runtime::new().unwrap();
                            let resultado = rt.block_on(async {
                                if let Some(cfg) = config {
                                    web_automation::descargar_comunicaciones(
                                        inicio,
                                        final_,
                                        &usuario,
                                        &cfg,
                                        |msg| {
                                            if let Ok(mut e) = estado.lock() {
                                                *e = EstadoApp::Procesando(msg.to_string());
                                            }
                                        },
                                    ).await
                                } else {
                                    Err(anyhow::anyhow!("Configuración no disponible"))
                                }
                            });
                            
                            if let Ok(mut e) = estado.lock() {
                                *e = match resultado {
                                    Ok(r) => EstadoApp::Finalizado(
                                        format!("{} de {} comunicaciones procesadas", 
                                            r.comunicaciones_procesadas, r.total_comunicaciones)
                                    ),
                                    Err(e) => EstadoApp::Error(e.to_string()),
                                };
                            }
                            
                            if let Ok(mut b) = botones.lock() {
                                *b = true;
                            }
                        });
                    }
                    
                    ui.add_space(8.0);
                    
                    if ui.add_sized([button_width, 32.0], egui::Button::new("📁  Mover archivos")).clicked() {
                        self.habilitar_botones(false);
                        self.actualizar_estado(EstadoApp::Procesando("Moviendo archivos...".to_string()));
                        
                        if let Some(config) = &self.config {
                            let ruta_descarga = file_processor::obtener_ruta_descargas()
                                .unwrap_or_default();
                            let ruta_destino = config.ruta_archivos.clone();
                            
                            match file_processor::mover_archivos(&ruta_descarga, &ruta_destino) {
                                Ok(resultado) => {
                                    let neto = resultado.archivos_movidos.saturating_sub(resultado.archivos_eliminados);
                                    self.actualizar_estado(EstadoApp::Finalizado(
                                        format!("{} movidos, {} eliminados", neto, resultado.archivos_eliminados)
                                    ));
                                }
                                Err(e) => {
                                    self.actualizar_estado(EstadoApp::Error(e.to_string()));
                                }
                            }
                        }
                        
                        self.habilitar_botones(true);
                    }
                    
                    ui.add_space(8.0);
                    
                    if ui.add_sized([button_width, 32.0], egui::Button::new("⚙  Procesar archivos")).clicked() {
                        self.habilitar_botones(false);
                        self.actualizar_estado(EstadoApp::Procesando("Procesando PDFs...".to_string()));
                        
                        if let Some(config) = &self.config {
                            match pdf_extractor::procesar_pdfs(&config.ruta_archivos) {
                                Ok(datos) => {
                                    match excel_handler::guardar_excel(&datos, &config.ruta_excel) {
                                        Ok(_) => {
                                            self.actualizar_estado(EstadoApp::Finalizado(
                                                format!("{} archivos procesados", datos.len())
                                            ));
                                        }
                                        Err(e) => {
                                            self.actualizar_estado(EstadoApp::Error(
                                                format!("Error al guardar Excel: {}", e)
                                            ));
                                        }
                                    }
                                }
                                Err(e) => {
                                    self.actualizar_estado(EstadoApp::Error(e.to_string()));
                                }
                            }
                        }
                        
                        self.habilitar_botones(true);
                    }
                });
                
                ui.add_space(20.0);
                
                // Estado
                let estado = self.obtener_estado();
                let (texto, color) = match estado {
                    EstadoApp::Listo => ("Listo", egui::Color32::GRAY),
                    EstadoApp::Procesando(ref msg) => (msg.as_str(), egui::Color32::YELLOW),
                    EstadoApp::Finalizado(ref msg) => (msg.as_str(), egui::Color32::GREEN),
                    EstadoApp::Error(ref msg) => (msg.as_str(), egui::Color32::RED),
                };
                
                ui.label(egui::RichText::new(texto).color(color));
            });
        });
        
        // Solicitar repintado continuo mientras está procesando
        if matches!(self.obtener_estado(), EstadoApp::Procesando(_)) {
            ctx.request_repaint();
        }
    }
}

/// Ejecuta la aplicación GUI
pub fn run() -> eframe::Result<()> {
    let options = eframe::NativeOptions {
        viewport: egui::ViewportBuilder::default()
            .with_inner_size([400.0, 450.0])
            .with_min_inner_size([350.0, 400.0]),
        ..Default::default()
    };
    
    eframe::run_native(
        "SADE",
        options,
        Box::new(|cc| Ok(Box::new(AutoSadeApp::new(cc)))),
    )
}
