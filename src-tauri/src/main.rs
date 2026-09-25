#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

use tauri::command;

#[command]
fn read_strudel_file(path: String) -> Result<String, String> {
    std::fs::read_to_string(&path).map_err(|e| e.to_string())
}

#[command]
async fn save_wav_bytes(bytes: Vec<u8>, path: String) -> Result<(), String> {
    if bytes.len() < 44 || &bytes[0..4] != b"RIFF" || &bytes[8..12] != b"WAVE" {
        return Err("dados recebidos não são um arquivo WAV válido".to_string());
    }

    let final_path = if path.to_lowercase().ends_with(".wav") {
        path
    } else {
        format!("{}.wav", path)
    };

    log::info!("saving Strudel WAV: path={} bytes={}", final_path, bytes.len());
    std::fs::write(&final_path, bytes).map_err(|e| e.to_string())
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
        .invoke_handler(tauri::generate_handler![read_strudel_file, save_wav_bytes])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
