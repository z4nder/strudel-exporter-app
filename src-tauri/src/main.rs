#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

use serde::Deserialize;
use std::io::{BufWriter, Write};
use std::path::PathBuf;
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

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct AlbumAudioSegment {
    base_bytes: Vec<u8>,
    loops: u32,
    gap_seconds: f64,
}

#[derive(Debug, Clone, Copy, PartialEq)]
struct WavFormat {
    audio_format: u16,
    channels: u16,
    sample_rate: u32,
    byte_rate: u32,
    block_align: u16,
    bits_per_sample: u16,
}

fn wav_format(bytes: &[u8]) -> Result<WavFormat, String> {
    if bytes.len() < 44 || &bytes[12..16] != b"fmt " {
        return Err("WAV base não contém um chunk fmt canônico".to_string());
    }
    Ok(WavFormat {
        audio_format: u16::from_le_bytes(bytes[20..22].try_into().map_err(|_| "fmt inválido")?),
        channels: u16::from_le_bytes(bytes[22..24].try_into().map_err(|_| "fmt inválido")?),
        sample_rate: u32::from_le_bytes(bytes[24..28].try_into().map_err(|_| "fmt inválido")?),
        byte_rate: u32::from_le_bytes(bytes[28..32].try_into().map_err(|_| "fmt inválido")?),
        block_align: u16::from_le_bytes(bytes[32..34].try_into().map_err(|_| "fmt inválido")?),
        bits_per_sample: u16::from_le_bytes(bytes[34..36].try_into().map_err(|_| "fmt inválido")?),
    })
}

fn write_album_wav<W: Write>(mut writer: W, segments: &[AlbumAudioSegment]) -> Result<u64, String> {
    let first = segments.first().ok_or("o Album não possui faixas")?;
    let format = wav_format(&first.base_bytes)?;
    let (_, first_data_start, _) = wav_data_chunk(&first.base_bytes)?;
    let mut output_data_size = 0usize;
    let mut parsed_segments = Vec::with_capacity(segments.len());

    for segment in segments {
        if !(1..=999).contains(&segment.loops) {
            return Err("a quantidade de loops deve estar entre 1 e 999".to_string());
        }
        if !segment.gap_seconds.is_finite() || segment.gap_seconds < 0.0 {
            return Err("o intervalo entre faixas deve ser maior ou igual a 0".to_string());
        }
        if wav_format(&segment.base_bytes)? != format {
            return Err("todas as faixas do Album devem usar o mesmo formato de áudio".to_string());
        }
        let (_, data_start, data_size) = wav_data_chunk(&segment.base_bytes)?;
        let repeated_size = data_size
            .checked_mul(segment.loops as usize)
            .ok_or("o Album excedeu o tamanho suportado")?;
        let gap_frames = (segment.gap_seconds * format.sample_rate as f64).round() as usize;
        let gap_size = gap_frames
            .checked_mul(format.block_align as usize)
            .ok_or("o Album excedeu o tamanho suportado")?;
        output_data_size = output_data_size
            .checked_add(repeated_size)
            .and_then(|size| size.checked_add(gap_size))
            .ok_or("o Album excedeu o tamanho suportado")?;
        parsed_segments.push((segment, data_start, data_size, gap_size));
    }

    let output_data_size_u32 = u32::try_from(output_data_size)
        .map_err(|_| "o Album excede o limite de 4 GiB do formato RIFF/WAV")?;
    let riff_size = first_data_start
        .checked_sub(8)
        .and_then(|header| header.checked_add(output_data_size))
        .ok_or("o Album excedeu o tamanho suportado")?;
    let riff_size_u32 = u32::try_from(riff_size)
        .map_err(|_| "o Album excede o limite de 4 GiB do formato RIFF/WAV")?;

    let mut header = first.base_bytes[..first_data_start].to_vec();
    header[4..8].copy_from_slice(&riff_size_u32.to_le_bytes());
    let data_size_offset = first_data_start - 4;
    header[data_size_offset..data_size_offset + 4]
        .copy_from_slice(&output_data_size_u32.to_le_bytes());
    writer.write_all(&header).map_err(|e| e.to_string())?;

    let silence = [0u8; 8192];
    for (segment, data_start, data_size, gap_size) in parsed_segments {
        let pcm = &segment.base_bytes[data_start..data_start + data_size];
        for _ in 0..segment.loops {
            writer.write_all(pcm).map_err(|e| e.to_string())?;
        }
        let mut remaining = gap_size;
        while remaining > 0 {
            let chunk = remaining.min(silence.len());
            writer
                .write_all(&silence[..chunk])
                .map_err(|e| e.to_string())?;
            remaining -= chunk;
        }
    }
    writer.flush().map_err(|e| e.to_string())?;
    Ok((first_data_start + output_data_size) as u64)
}

fn album_output_paths(path: &str) -> (PathBuf, PathBuf, PathBuf, PathBuf) {
    let wav_path = if path.to_lowercase().ends_with(".wav") {
        PathBuf::from(path)
    } else {
        PathBuf::from(format!("{path}.wav"))
    };
    let mut manifest_path = wav_path.clone();
    manifest_path.set_extension("json");
    let wav_temp = PathBuf::from(format!("{}.part", wav_path.to_string_lossy()));
    let manifest_temp = PathBuf::from(format!("{}.part", manifest_path.to_string_lossy()));
    (wav_path, manifest_path, wav_temp, manifest_temp)
}

#[command]
async fn save_album_wav(
    segments: Vec<AlbumAudioSegment>,
    path: String,
    manifest_json: String,
) -> Result<(), String> {
    tauri::async_runtime::spawn_blocking(move || {
        let (wav_path, manifest_path, wav_temp, manifest_temp) = album_output_paths(&path);
        let result = (|| {
            let file = std::fs::File::create(&wav_temp).map_err(|e| e.to_string())?;
            let output_bytes = write_album_wav(BufWriter::new(file), &segments)?;
            std::fs::write(&manifest_temp, manifest_json.as_bytes()).map_err(|e| e.to_string())?;
            std::fs::rename(&wav_temp, &wav_path).map_err(|e| e.to_string())?;
            std::fs::rename(&manifest_temp, &manifest_path).map_err(|e| e.to_string())?;
            log::info!(
                "saved Strudel Album: wav={} manifest={} tracks={} bytes={}",
                wav_path.display(),
                manifest_path.display(),
                segments.len(),
                output_bytes,
            );
            Ok(())
        })();
        if result.is_err() {
            let _ = std::fs::remove_file(&wav_temp);
            let _ = std::fs::remove_file(&manifest_temp);
        }
        result
    })
    .await
    .map_err(|e| e.to_string())?
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
            save_album_wav,
            library::library_list_tracks,
            library::library_import_track,
            library::library_save_track_settings,
            library::library_delete_track,
            library::library_list_tags,
            library::library_create_tag,
            library::library_update_tag,
            library::library_delete_tag,
            library::library_set_track_tag,
            library::library_list_albums,
            library::library_get_album,
            library::library_create_album,
            library::library_update_album,
            library::library_delete_album,
            library::library_add_album_track,
            library::library_update_album_track,
            library::library_remove_album_track,
            library::library_reorder_album_tracks,
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

    #[test]
    fn concatenates_album_segments_loops_and_silence() {
        let first = AlbumAudioSegment {
            base_bytes: tiny_wav(),
            loops: 2,
            gap_seconds: 0.001,
        };
        let second = AlbumAudioSegment {
            base_bytes: tiny_wav(),
            loops: 1,
            gap_seconds: 0.0,
        };
        let mut output = Vec::new();
        let written = write_album_wav(&mut output, &[first, second]).unwrap();

        // 12 PCM bytes + round(44.1 gap frames) * 2-byte mono block alignment.
        assert_eq!(u32::from_le_bytes(output[40..44].try_into().unwrap()), 100);
        assert_eq!(written, 144);
        assert_eq!(&output[44..52], &[1, 2, 3, 4, 1, 2, 3, 4]);
        assert!(output[52..140].iter().all(|byte| *byte == 0));
        assert_eq!(&output[140..144], &[1, 2, 3, 4]);
    }
}
