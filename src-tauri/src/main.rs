#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

mod wav;

use tauri::command;

#[command]
fn read_strudel_file(path: String) -> Result<String, String> {
    std::fs::read_to_string(&path).map_err(|e| e.to_string())
}

#[command]
async fn save_wav(samples: Vec<f32>, sample_rate: u32, path: String) -> Result<(), String> {
    let encoded = wav::encode_wav(&samples, sample_rate, 2);
    let final_path = if path.to_lowercase().ends_with(".wav") {
        path
    } else {
        format!("{}.wav", path)
    };
    std::fs::write(&final_path, encoded).map_err(|e| e.to_string())
}

fn main() {
    tauri::Builder::default()
        .plugin(
            tauri_plugin_log::Builder::new()
                .targets([
                    tauri_plugin_log::Target::new(tauri_plugin_log::TargetKind::Stdout),
                    tauri_plugin_log::Target::new(tauri_plugin_log::TargetKind::LogDir {
                        file_name: Some("strudel-exporter".to_string()),
                    }),
                ])
                .level(log::LevelFilter::Debug)
                .build(),
        )
        .plugin(tauri_plugin_dialog::init())
        .invoke_handler(tauri::generate_handler![read_strudel_file, save_wav])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
