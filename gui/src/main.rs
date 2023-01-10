use eframe::NativeOptions;
use serde::{Deserialize, Serialize};

#[derive(Debug, Default, Deserialize, Serialize)]
pub struct Main {
    login: String,
    version: String,
}

#[derive(Debug, Default, Deserialize, Serialize)]
pub struct Settings {
    java_path: String,
    game_width: u32,
    game_height: u32,
}

#[derive(Debug, Default, Deserialize, Serialize)]
struct MyApp {
    main: Main,
    settings: Settings,
}

impl MyApp {
    fn new(cc: &eframe::CreationContext<'_>) -> Self {
        cc.storage
            .and_then(|storage| eframe::get_value(storage, eframe::APP_KEY))
            .unwrap_or_default()
    }
}

impl eframe::App for MyApp {
    fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        egui::Window::new("Main").show(ctx, |ui| {
            egui::TextEdit::singleline(&mut self.main.login)
                .hint_text("Login")
                .show(ui);
            egui::ComboBox::from_label("Version")
                .selected_text(&self.main.version)
                .show_ui(ui, |ui| {});
            if ui.button("Run game").clicked() {
                // TODO : run and go to log
            }
        });
        egui::Window::new("Settings").show(ctx, |ui| {
            egui::TextEdit::singleline(&mut self.settings.java_path)
                .hint_text("Java binary path")
                .show(ui);
        });
    }

    fn save(&mut self, storage: &mut dyn eframe::Storage) {
        eframe::set_value(storage, eframe::APP_KEY, self);
    }
}

fn main() {
    tracing_subscriber::fmt::init();

    let native_options = NativeOptions {
        follow_system_theme: true,
        ..Default::default()
    };
    eframe::run_native(
        env!("CARGO_PKG_NAME"),
        native_options,
        Box::new(|cc| Box::new(MyApp::new(cc))),
    );
}
