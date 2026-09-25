#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

use std::io::{BufWriter, Write};
use tauri::{command, Manager};

mod library;

#[command]
fn read_strudel_file(path: String) -> Result<String, String> {
    std::fs::read_to_string(&path).map_err(|e| e.to_string())
}

fn wav_data_chunk(bytes: &[u8]) -> Result<(usize, usize, usize), String> {
    if bytes.len() < 12 || &bytes[0..4] != b"RIFF" || &bytes[8..12] != b"WAVE" {
        return Err("dados recebidos não são um WAV RIFF válido".to_string());
    }

    let mut offset = 12usize;
    while offset.checked_add(8).is_some_and(|end| end <= bytes.len()) {
        let chunk_size = u32::from_le_bytes(
            bytes[offset + 4..offset + 8]
                .try_into()
                .map_err(|_| "chunk WAV inválido")?,
        ) as usize;
        let data_start = offset + 8;
        let data_end = data_start
            .checked_add(chunk_size)
            .ok_or("tamanho do chunk WAV excedeu o limite")?;
        if data_end > bytes.len() {
            return Err("chunk WAV truncado".to_string());
        }
        if &bytes[offset..offset + 4] == b"data" {
            return Ok((offset + 4, data_start, chunk_size));
        }
        offset = data_end + (chunk_size % 2);
    }

    Err("WAV não contém um chunk de áudio data".to_string())
}

fn write_looped_wav<W: Write>(mut writer: W, base_bytes: &[u8], loops: u32) -> Result<u64, String> {
    if !(1..=999).contains(&loops) {
        return Err("a quantidade de loops deve estar entre 1 e 999".to_string());
    }

    let (data_size_offset, data_start, base_data_size) = wav_data_chunk(base_bytes)?;
    let output_data_size = base_data_size
        .checked_mul(loops as usize)
        .ok_or("o WAV final excedeu o tamanho suportado")?;
    let riff_size = data_start
        .checked_sub(8)
        .and_then(|header| header.checked_add(output_data_size))
        .ok_or("o WAV final excedeu o tamanho suportado")?;
    let output_data_size_u32 = u32::try_from(output_data_size)
        .map_err(|_| "o WAV final excede o limite de 4 GiB do formato RIFF/WAV")?;
    let riff_size_u32 = u32::try_from(riff_size)
        .map_err(|_| "o WAV final excede o limite de 4 GiB do formato RIFF/WAV")?;

    let mut header = base_bytes[..data_start].to_vec();
    header[4..8].copy_from_slice(&riff_size_u32.to_le_bytes());
    header[data_size_offset..data_size_offset + 4]
        .copy_from_slice(&output_data_size_u32.to_le_bytes());
    writer.write_all(&header).map_err(|e| e.to_string())?;

    let base_pcm = &base_bytes[data_start..data_start + base_data_size];
    for _ in 0..loops {
        writer.write_all(base_pcm).map_err(|e| e.to_string())?;
    }
    writer.flush().map_err(|e| e.to_string())?;

    Ok((data_start + output_data_size) as u64)
}

#[command]
async fn save_looped_wav(base_bytes: Vec<u8>, loops: u32, path: String) -> Result<(), String> {
    let final_path = if path.to_lowercase().ends_with(".wav") {
        path
    } else {
        format!("{}.wav", path)
    };

    tauri::async_runtime::spawn_blocking(move || {
        let file = std::fs::File::create(&final_path).map_err(|e| e.to_string())?;
        let bytes_written = write_looped_wav(BufWriter::new(file), &base_bytes, loops)?;
        log::info!(
            "saved looped Strudel WAV: path={} base_bytes={} loops={} output_bytes={}",
            final_path,
            base_bytes.len(),
            loops,
            bytes_written,
        );
        Ok(())
    })
    .await
    .map_err(|e| e.to_string())?
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
        .setup(|app| {
            let db = library::open(app.handle()).map_err(|error| {
                Box::<dyn std::error::Error>::from(format!(
                    "failed to open library database: {error}"
                ))
            })?;
            app.manage(db);
            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            read_strudel_file,
            save_looped_wav,
            library::library_list_tracks,
            library::library_import_track,
            library::library_save_track_settings,
            library::library_delete_track,
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}

#[cfg(test)]
mod tests {
    use super::*;

    fn tiny_wav() -> Vec<u8> {
        let mut wav = Vec::new();
        wav.extend_from_slice(b"RIFF");
        wav.extend_from_slice(&40u32.to_le_bytes());
        wav.extend_from_slice(b"WAVEfmt ");
        wav.extend_from_slice(&16u32.to_le_bytes());
        wav.extend_from_slice(&1u16.to_le_bytes());
        wav.extend_from_slice(&1u16.to_le_bytes());
        wav.extend_from_slice(&44_100u32.to_le_bytes());
        wav.extend_from_slice(&88_200u32.to_le_bytes());
        wav.extend_from_slice(&2u16.to_le_bytes());
        wav.extend_from_slice(&16u16.to_le_bytes());
        wav.extend_from_slice(b"data");
        wav.extend_from_slice(&4u32.to_le_bytes());
        wav.extend_from_slice(&[1, 2, 3, 4]);
        wav
    }

    #[test]
    fn repeats_only_pcm_and_updates_riff_header() {
        let mut output = Vec::new();
        let written = write_looped_wav(&mut output, &tiny_wav(), 3).unwrap();

        assert_eq!(written, 56);
        assert_eq!(u32::from_le_bytes(output[4..8].try_into().unwrap()), 48);
        assert_eq!(u32::from_le_bytes(output[40..44].try_into().unwrap()), 12);
        assert_eq!(&output[44..], &[1, 2, 3, 4, 1, 2, 3, 4, 1, 2, 3, 4]);
    }

    #[test]
    fn rejects_out_of_range_loop_count() {
        assert!(write_looped_wav(Vec::new(), &tiny_wav(), 0).is_err());
        assert!(write_looped_wav(Vec::new(), &tiny_wav(), 1_000).is_err());
    }
}
