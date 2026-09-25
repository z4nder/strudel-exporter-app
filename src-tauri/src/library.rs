use rusqlite::{params, Connection, OptionalExtension};
use serde::{Deserialize, Serialize};
use std::path::Path;
use std::sync::Mutex;
use std::time::{SystemTime, UNIX_EPOCH};
use tauri::{command, AppHandle, Manager, State};

pub struct LibraryDb(pub Mutex<Connection>);

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct TrackRenderSettings {
    pub range_mode: String,
    pub start_cycle: f64,
    pub end_cycle: f64,
    pub loops: u32,
    pub sample_rate: u32,
    pub max_polyphony: u32,
    pub export_file_name: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct Tag {
    pub id: i64,
    pub name: String,
    pub color: String,
    pub track_count: u32,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct Track {
    pub id: i64,
    pub name: String,
    pub source_path: String,
    pub settings: TrackRenderSettings,
    pub tags: Vec<Tag>,
    pub created_at: i64,
    pub updated_at: i64,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct AlbumSummary {
    pub id: i64,
    pub name: String,
    pub description: String,
    pub cover_path: Option<String>,
    pub sample_rate: u32,
    pub default_gap_seconds: f64,
    pub track_count: u32,
    pub tags: Vec<Tag>,
    pub created_at: i64,
    pub updated_at: i64,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct AlbumTrackSettings {
    pub range_mode: String,
    pub start_cycle: f64,
    pub end_cycle: f64,
    pub loops: u32,
    pub max_polyphony: u32,
    pub gap_after_seconds: f64,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct AlbumTrackItem {
    pub id: i64,
    pub position: u32,
    pub track: Track,
    pub settings: AlbumTrackSettings,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct Album {
    #[serde(flatten)]
    pub summary: AlbumSummary,
    pub tracks: Vec<AlbumTrackItem>,
}

fn now_timestamp() -> i64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs() as i64
}

fn normalize_source_path(path: &str) -> Result<(String, String), String> {
    let canonical = std::fs::canonicalize(path)
        .map_err(|e| format!("não foi possível acessar o arquivo: {e}"))?;
    if !canonical.is_file() {
        return Err("o caminho selecionado não é um arquivo".to_string());
    }
    let extension = canonical
        .extension()
        .and_then(|value| value.to_str())
        .unwrap_or_default();
    if !extension.eq_ignore_ascii_case("strudel") && !extension.eq_ignore_ascii_case("js") {
        return Err("selecione um arquivo .strudel ou .js".to_string());
    }
    std::fs::File::open(&canonical).map_err(|e| format!("não foi possível ler o arquivo: {e}"))?;
    let display = canonical.to_string_lossy().into_owned();
    #[cfg(target_os = "windows")]
    let normalized = display.to_lowercase();
    #[cfg(not(target_os = "windows"))]
    let normalized = display.clone();
    Ok((display, normalized))
}

fn default_track_name(path: &Path) -> String {
    path.file_stem()
        .and_then(|value| value.to_str())
        .filter(|value| !value.trim().is_empty())
        .unwrap_or("track")
        .to_string()
}

fn validate_settings(settings: &TrackRenderSettings) -> Result<(), String> {
    if settings.range_mode != "automatic" && settings.range_mode != "manual" {
        return Err("range_mode deve ser automatic ou manual".to_string());
    }
    if !settings.start_cycle.is_finite() || settings.start_cycle < 0.0 {
        return Err("Start cycle deve ser maior ou igual a 0".to_string());
    }
    if !settings.end_cycle.is_finite() || settings.end_cycle <= settings.start_cycle {
        return Err("End cycle deve ser maior que Start cycle".to_string());
    }
    if !(1..=999).contains(&settings.loops) {
        return Err("loops deve estar entre 1 e 999".to_string());
    }
    if ![44_100, 48_000, 96_000].contains(&settings.sample_rate) {
        return Err("sample rate inválido".to_string());
    }
    if !(1..=256).contains(&settings.max_polyphony) {
        return Err("maximum polyphony deve estar entre 1 e 256".to_string());
    }
    if settings.export_file_name.trim().is_empty() {
        return Err("o nome de exportação não pode ficar vazio".to_string());
    }
    Ok(())
}

pub fn migrate(connection: &Connection) -> Result<(), String> {
    connection
        .execute_batch(
            "PRAGMA foreign_keys = ON;
             CREATE TABLE IF NOT EXISTS tracks (
               id INTEGER PRIMARY KEY AUTOINCREMENT,
               name TEXT NOT NULL,
               source_path TEXT NOT NULL,
               source_path_normalized TEXT NOT NULL UNIQUE,
               created_at INTEGER NOT NULL,
               updated_at INTEGER NOT NULL
             );
             CREATE TABLE IF NOT EXISTS track_render_settings (
               track_id INTEGER PRIMARY KEY REFERENCES tracks(id) ON DELETE CASCADE,
               range_mode TEXT NOT NULL CHECK(range_mode IN ('automatic', 'manual')),
               start_cycle REAL NOT NULL CHECK(start_cycle >= 0),
               end_cycle REAL NOT NULL CHECK(end_cycle > start_cycle),
               loops INTEGER NOT NULL CHECK(loops BETWEEN 1 AND 999),
               sample_rate INTEGER NOT NULL CHECK(sample_rate IN (44100, 48000, 96000)),
               max_polyphony INTEGER NOT NULL CHECK(max_polyphony BETWEEN 1 AND 256),
               export_file_name TEXT NOT NULL,
               updated_at INTEGER NOT NULL
             );
             CREATE TABLE IF NOT EXISTS tags (
               id INTEGER PRIMARY KEY AUTOINCREMENT,
               name TEXT NOT NULL COLLATE NOCASE UNIQUE,
               color TEXT NOT NULL,
               created_at INTEGER NOT NULL,
               updated_at INTEGER NOT NULL
             );
             CREATE TABLE IF NOT EXISTS track_tags (
               track_id INTEGER NOT NULL REFERENCES tracks(id) ON DELETE CASCADE,
               tag_id INTEGER NOT NULL REFERENCES tags(id) ON DELETE CASCADE,
               PRIMARY KEY(track_id, tag_id)
             );
             CREATE TABLE IF NOT EXISTS albums (
               id INTEGER PRIMARY KEY AUTOINCREMENT,
               name TEXT NOT NULL,
               description TEXT NOT NULL DEFAULT '',
               cover_path TEXT,
               sample_rate INTEGER NOT NULL CHECK(sample_rate IN (44100, 48000, 96000)),
               default_gap_seconds REAL NOT NULL CHECK(default_gap_seconds >= 0),
               created_at INTEGER NOT NULL,
               updated_at INTEGER NOT NULL
             );
             CREATE TABLE IF NOT EXISTS album_tracks (
               id INTEGER PRIMARY KEY AUTOINCREMENT,
               album_id INTEGER NOT NULL REFERENCES albums(id) ON DELETE CASCADE,
               track_id INTEGER NOT NULL REFERENCES tracks(id) ON DELETE RESTRICT,
               position INTEGER NOT NULL CHECK(position >= 0),
               range_mode TEXT NOT NULL CHECK(range_mode IN ('automatic', 'manual')),
               start_cycle REAL NOT NULL CHECK(start_cycle >= 0),
               end_cycle REAL NOT NULL CHECK(end_cycle > start_cycle),
               loops INTEGER NOT NULL CHECK(loops BETWEEN 1 AND 999),
               max_polyphony INTEGER NOT NULL CHECK(max_polyphony BETWEEN 1 AND 256),
               gap_after_seconds REAL NOT NULL CHECK(gap_after_seconds >= 0),
               created_at INTEGER NOT NULL,
               updated_at INTEGER NOT NULL,
               UNIQUE(album_id, position)
             );",
        )
        .map_err(|e| e.to_string())
}

fn row_to_track(row: &rusqlite::Row<'_>) -> rusqlite::Result<Track> {
    Ok(Track {
        id: row.get(0)?,
        name: row.get(1)?,
        source_path: row.get(2)?,
        created_at: row.get(3)?,
        updated_at: row.get(4)?,
        settings: TrackRenderSettings {
            range_mode: row.get(5)?,
            start_cycle: row.get(6)?,
            end_cycle: row.get(7)?,
            loops: row.get(8)?,
            sample_rate: row.get(9)?,
            max_polyphony: row.get(10)?,
            export_file_name: row.get(11)?,
        },
        tags: Vec::new(),
    })
}

const TRACK_SELECT: &str = "SELECT t.id, t.name, t.source_path, t.created_at, t.updated_at,
            s.range_mode, s.start_cycle, s.end_cycle, s.loops,
            s.sample_rate, s.max_polyphony, s.export_file_name
       FROM tracks t
       JOIN track_render_settings s ON s.track_id = t.id";

fn find_track(connection: &Connection, id: i64) -> Result<Track, String> {
    let mut track = connection
        .query_row(
            &format!("{TRACK_SELECT} WHERE t.id = ?1"),
            [id],
            row_to_track,
        )
        .optional()
        .map_err(|e| e.to_string())?
        .ok_or_else(|| "Track não encontrada".to_string())?;
    track.tags = tags_for_track(connection, track.id)?;
    Ok(track)
}

fn row_to_tag(row: &rusqlite::Row<'_>) -> rusqlite::Result<Tag> {
    Ok(Tag {
        id: row.get(0)?,
        name: row.get(1)?,
        color: row.get(2)?,
        track_count: row.get(3)?,
    })
}

const TAG_SELECT: &str = "SELECT g.id, g.name, g.color,
            CAST((SELECT COUNT(*) FROM track_tags tt WHERE tt.tag_id = g.id) AS INTEGER)
       FROM tags g";

fn tags_for_track(connection: &Connection, track_id: i64) -> Result<Vec<Tag>, String> {
    let mut statement = connection
        .prepare(&format!(
            "{TAG_SELECT} JOIN track_tags link ON link.tag_id = g.id
             WHERE link.track_id = ?1 ORDER BY g.name COLLATE NOCASE"
        ))
        .map_err(|e| e.to_string())?;
    let tags = statement
        .query_map([track_id], row_to_tag)
        .map_err(|e| e.to_string())?
        .collect::<Result<Vec<_>, _>>()
        .map_err(|e| e.to_string())?;
    Ok(tags)
}

fn list_tags(connection: &Connection) -> Result<Vec<Tag>, String> {
    let mut statement = connection
        .prepare(&format!("{TAG_SELECT} ORDER BY g.name COLLATE NOCASE"))
        .map_err(|e| e.to_string())?;
    let tags = statement
        .query_map([], row_to_tag)
        .map_err(|e| e.to_string())?
        .collect::<Result<Vec<_>, _>>()
        .map_err(|e| e.to_string())?;
    Ok(tags)
}

fn validate_tag(name: &str, color: &str) -> Result<(String, String), String> {
    let name = name.trim();
    if name.is_empty() || name.chars().count() > 40 {
        return Err("o nome da tag deve ter entre 1 e 40 caracteres".to_string());
    }
    let valid_color = color.len() == 7
        && color.starts_with('#')
        && color[1..]
            .chars()
            .all(|character| character.is_ascii_hexdigit());
    if !valid_color {
        return Err("a cor deve usar o formato hexadecimal #RRGGBB".to_string());
    }
    Ok((name.to_string(), color.to_ascii_uppercase()))
}

fn create_tag(connection: &Connection, name: &str, color: &str) -> Result<Tag, String> {
    let (name, color) = validate_tag(name, color)?;
    let now = now_timestamp();
    connection
        .execute(
            "INSERT INTO tags(name, color, created_at, updated_at) VALUES (?1, ?2, ?3, ?3)",
            params![name, color, now],
        )
        .map_err(|error| {
            if error.to_string().contains("UNIQUE constraint failed") {
                "já existe uma tag com esse nome".to_string()
            } else {
                error.to_string()
            }
        })?;
    let id = connection.last_insert_rowid();
    Ok(Tag {
        id,
        name,
        color,
        track_count: 0,
    })
}

fn update_tag(connection: &Connection, id: i64, name: &str, color: &str) -> Result<Tag, String> {
    let (name, color) = validate_tag(name, color)?;
    let changed = connection
        .execute(
            "UPDATE tags SET name = ?1, color = ?2, updated_at = ?3 WHERE id = ?4",
            params![name, color, now_timestamp(), id],
        )
        .map_err(|error| {
            if error.to_string().contains("UNIQUE constraint failed") {
                "já existe uma tag com esse nome".to_string()
            } else {
                error.to_string()
            }
        })?;
    if changed == 0 {
        return Err("Tag não encontrada".to_string());
    }
    list_tags(connection)?
        .into_iter()
        .find(|tag| tag.id == id)
        .ok_or_else(|| "Tag não encontrada".to_string())
}

fn delete_tag(connection: &Connection, id: i64) -> Result<(), String> {
    let changed = connection
        .execute("DELETE FROM tags WHERE id = ?1", [id])
        .map_err(|e| e.to_string())?;
    if changed == 0 {
        return Err("Tag não encontrada".to_string());
    }
    Ok(())
}

fn set_track_tag(
    connection: &Connection,
    track_id: i64,
    tag_id: i64,
    attached: bool,
) -> Result<Track, String> {
    find_track(connection, track_id)?;
    if attached {
        connection
            .execute(
                "INSERT OR IGNORE INTO track_tags(track_id, tag_id) VALUES (?1, ?2)",
                params![track_id, tag_id],
            )
            .map_err(|e| e.to_string())?;
    } else {
        connection
            .execute(
                "DELETE FROM track_tags WHERE track_id = ?1 AND tag_id = ?2",
                params![track_id, tag_id],
            )
            .map_err(|e| e.to_string())?;
    }
    find_track(connection, track_id)
}

fn validate_album_fields(
    name: &str,
    sample_rate: u32,
    default_gap_seconds: f64,
) -> Result<String, String> {
    let name = name.trim();
    if name.is_empty() || name.chars().count() > 120 {
        return Err("o nome do Album deve ter entre 1 e 120 caracteres".to_string());
    }
    if ![44_100, 48_000, 96_000].contains(&sample_rate) {
        return Err("sample rate inválido".to_string());
    }
    if !default_gap_seconds.is_finite() || default_gap_seconds < 0.0 {
        return Err("o intervalo padrão deve ser maior ou igual a 0".to_string());
    }
    Ok(name.to_string())
}

fn validate_album_track_settings(settings: &AlbumTrackSettings) -> Result<(), String> {
    if settings.range_mode != "automatic" && settings.range_mode != "manual" {
        return Err("range_mode deve ser automatic ou manual".to_string());
    }
    if !settings.start_cycle.is_finite() || settings.start_cycle < 0.0 {
        return Err("Start cycle deve ser maior ou igual a 0".to_string());
    }
    if !settings.end_cycle.is_finite() || settings.end_cycle <= settings.start_cycle {
        return Err("End cycle deve ser maior que Start cycle".to_string());
    }
    if !(1..=999).contains(&settings.loops) {
        return Err("loops deve estar entre 1 e 999".to_string());
    }
    if !(1..=256).contains(&settings.max_polyphony) {
        return Err("maximum polyphony deve estar entre 1 e 256".to_string());
    }
    if !settings.gap_after_seconds.is_finite() || settings.gap_after_seconds < 0.0 {
        return Err("o intervalo após a faixa deve ser maior ou igual a 0".to_string());
    }
    Ok(())
}

fn row_to_album_summary(row: &rusqlite::Row<'_>) -> rusqlite::Result<AlbumSummary> {
    Ok(AlbumSummary {
        id: row.get(0)?,
        name: row.get(1)?,
        description: row.get(2)?,
        cover_path: row.get(3)?,
        sample_rate: row.get(4)?,
        default_gap_seconds: row.get(5)?,
        track_count: row.get(6)?,
        tags: Vec::new(),
        created_at: row.get(7)?,
        updated_at: row.get(8)?,
    })
}

const ALBUM_SELECT: &str = "SELECT a.id, a.name, a.description, a.cover_path, a.sample_rate,
            a.default_gap_seconds,
            CAST((SELECT COUNT(*) FROM album_tracks at WHERE at.album_id = a.id) AS INTEGER),
            a.created_at, a.updated_at
       FROM albums a";

fn album_tags(connection: &Connection, album_id: i64) -> Result<Vec<Tag>, String> {
    let mut statement = connection
        .prepare(
            "SELECT DISTINCT t.id, t.name, t.color,
                    CAST((SELECT COUNT(*) FROM track_tags tt2 WHERE tt2.tag_id = t.id) AS INTEGER)
             FROM tags t
             JOIN track_tags tt ON tt.tag_id = t.id
             JOIN album_tracks at ON at.track_id = tt.track_id
             WHERE at.album_id = ?1
             ORDER BY t.name COLLATE NOCASE",
        )
        .map_err(|e| e.to_string())?;
    let tags = statement
        .query_map([album_id], |row| {
            Ok(Tag {
                id: row.get(0)?,
                name: row.get(1)?,
                color: row.get(2)?,
                track_count: row.get(3)?,
            })
        })
        .map_err(|e| e.to_string())?
        .collect::<Result<Vec<_>, _>>()
        .map_err(|e| e.to_string())?;
    Ok(tags)
}

fn list_albums(connection: &Connection) -> Result<Vec<AlbumSummary>, String> {
    let mut statement = connection
        .prepare(&format!(
            "{ALBUM_SELECT} ORDER BY a.updated_at DESC, a.name COLLATE NOCASE"
        ))
        .map_err(|e| e.to_string())?;
    let mut albums = statement
        .query_map([], row_to_album_summary)
        .map_err(|e| e.to_string())?
        .collect::<Result<Vec<_>, _>>()
        .map_err(|e| e.to_string())?;
    drop(statement);
    for album in &mut albums {
        album.tags = album_tags(connection, album.id)?;
    }
    Ok(albums)
}

fn find_album_summary(connection: &Connection, id: i64) -> Result<AlbumSummary, String> {
    let mut album = connection
        .query_row(
            &format!("{ALBUM_SELECT} WHERE a.id = ?1"),
            [id],
            row_to_album_summary,
        )
        .optional()
        .map_err(|e| e.to_string())?
        .ok_or_else(|| "Album não encontrado".to_string())?;
    album.tags = album_tags(connection, album.id)?;
    Ok(album)
}

fn find_album(connection: &Connection, id: i64) -> Result<Album, String> {
    let summary = find_album_summary(connection, id)?;
    let mut statement = connection
        .prepare(
            "SELECT id, position, track_id, range_mode, start_cycle, end_cycle,
                    loops, max_polyphony, gap_after_seconds
               FROM album_tracks WHERE album_id = ?1 ORDER BY position",
        )
        .map_err(|e| e.to_string())?;
    let rows = statement
        .query_map([id], |row| {
            Ok((
                row.get::<_, i64>(0)?,
                row.get::<_, u32>(1)?,
                row.get::<_, i64>(2)?,
                AlbumTrackSettings {
                    range_mode: row.get(3)?,
                    start_cycle: row.get(4)?,
                    end_cycle: row.get(5)?,
                    loops: row.get(6)?,
                    max_polyphony: row.get(7)?,
                    gap_after_seconds: row.get(8)?,
                },
            ))
        })
        .map_err(|e| e.to_string())?
        .collect::<Result<Vec<_>, _>>()
        .map_err(|e| e.to_string())?;
    drop(statement);

    let tracks = rows
        .into_iter()
        .map(|(item_id, position, track_id, settings)| {
            Ok(AlbumTrackItem {
                id: item_id,
                position,
                track: find_track(connection, track_id)?,
                settings,
            })
        })
        .collect::<Result<Vec<_>, String>>()?;
    Ok(Album { summary, tracks })
}

fn create_album(
    connection: &Connection,
    name: &str,
    description: &str,
    cover_path: Option<String>,
    sample_rate: u32,
    default_gap_seconds: f64,
) -> Result<Album, String> {
    let name = validate_album_fields(name, sample_rate, default_gap_seconds)?;
    let now = now_timestamp();
    connection
        .execute(
            "INSERT INTO albums(name, description, cover_path, sample_rate,
                                default_gap_seconds, created_at, updated_at)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?6)",
            params![
                name,
                description.trim(),
                cover_path,
                sample_rate,
                default_gap_seconds,
                now
            ],
        )
        .map_err(|e| e.to_string())?;
    find_album(connection, connection.last_insert_rowid())
}

fn update_album(
    connection: &Connection,
    id: i64,
    name: &str,
    description: &str,
    cover_path: Option<String>,
    sample_rate: u32,
    default_gap_seconds: f64,
) -> Result<Album, String> {
    let name = validate_album_fields(name, sample_rate, default_gap_seconds)?;
    let changed = connection
        .execute(
            "UPDATE albums SET name = ?1, description = ?2, cover_path = ?3,
                               sample_rate = ?4, default_gap_seconds = ?5, updated_at = ?6
             WHERE id = ?7",
            params![
                name,
                description.trim(),
                cover_path,
                sample_rate,
                default_gap_seconds,
                now_timestamp(),
                id
            ],
        )
        .map_err(|e| e.to_string())?;
    if changed == 0 {
        return Err("Album não encontrado".to_string());
    }
    find_album(connection, id)
}

fn delete_album(connection: &Connection, id: i64) -> Result<(), String> {
    let changed = connection
        .execute("DELETE FROM albums WHERE id = ?1", [id])
        .map_err(|e| e.to_string())?;
    if changed == 0 {
        return Err("Album não encontrado".to_string());
    }
    Ok(())
}

fn add_album_track(connection: &Connection, album_id: i64, track_id: i64) -> Result<Album, String> {
    let album = find_album_summary(connection, album_id)?;
    let track = find_track(connection, track_id)?;
    let position: u32 = connection
        .query_row(
            "SELECT COALESCE(MAX(position) + 1, 0) FROM album_tracks WHERE album_id = ?1",
            [album_id],
            |row| row.get(0),
        )
        .map_err(|e| e.to_string())?;
    let now = now_timestamp();
    connection
        .execute(
            "INSERT INTO album_tracks(
               album_id, track_id, position, range_mode, start_cycle, end_cycle,
               loops, max_polyphony, gap_after_seconds, created_at, updated_at
             ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?10)",
            params![
                album_id,
                track_id,
                position,
                track.settings.range_mode,
                track.settings.start_cycle,
                track.settings.end_cycle,
                track.settings.loops,
                track.settings.max_polyphony,
                album.default_gap_seconds,
                now,
            ],
        )
        .map_err(|e| e.to_string())?;
    connection
        .execute(
            "UPDATE albums SET updated_at = ?1 WHERE id = ?2",
            params![now, album_id],
        )
        .map_err(|e| e.to_string())?;
    find_album(connection, album_id)
}

fn update_album_track(
    connection: &Connection,
    item_id: i64,
    settings: &AlbumTrackSettings,
) -> Result<Album, String> {
    validate_album_track_settings(settings)?;
    let album_id: i64 = connection
        .query_row(
            "SELECT album_id FROM album_tracks WHERE id = ?1",
            [item_id],
            |row| row.get(0),
        )
        .optional()
        .map_err(|e| e.to_string())?
        .ok_or_else(|| "Faixa do Album não encontrada".to_string())?;
    let now = now_timestamp();
    connection
        .execute(
            "UPDATE album_tracks SET range_mode = ?1, start_cycle = ?2, end_cycle = ?3,
                                     loops = ?4, max_polyphony = ?5,
                                     gap_after_seconds = ?6, updated_at = ?7
             WHERE id = ?8",
            params![
                settings.range_mode,
                settings.start_cycle,
                settings.end_cycle,
                settings.loops,
                settings.max_polyphony,
                settings.gap_after_seconds,
                now,
                item_id,
            ],
        )
        .map_err(|e| e.to_string())?;
    connection
        .execute(
            "UPDATE albums SET updated_at = ?1 WHERE id = ?2",
            params![now, album_id],
        )
        .map_err(|e| e.to_string())?;
    find_album(connection, album_id)
}

fn remove_album_track(connection: &mut Connection, item_id: i64) -> Result<Album, String> {
    let album_id: i64 = connection
        .query_row(
            "SELECT album_id FROM album_tracks WHERE id = ?1",
            [item_id],
            |row| row.get(0),
        )
        .optional()
        .map_err(|e| e.to_string())?
        .ok_or_else(|| "Faixa do Album não encontrada".to_string())?;
    let transaction = connection.transaction().map_err(|e| e.to_string())?;
    transaction
        .execute("DELETE FROM album_tracks WHERE id = ?1", [item_id])
        .map_err(|e| e.to_string())?;
    let ids = {
        let mut statement = transaction
            .prepare("SELECT id FROM album_tracks WHERE album_id = ?1 ORDER BY position")
            .map_err(|e| e.to_string())?;
        let ids = statement
            .query_map([album_id], |row| row.get::<_, i64>(0))
            .map_err(|e| e.to_string())?
            .collect::<Result<Vec<_>, _>>()
            .map_err(|e| e.to_string())?;
        ids
    };
    for (position, id) in ids.iter().enumerate() {
        transaction
            .execute(
                "UPDATE album_tracks SET position = ?1 WHERE id = ?2",
                params![1_000_000 + position as u32, id],
            )
            .map_err(|e| e.to_string())?;
    }
    for (position, id) in ids.iter().enumerate() {
        transaction
            .execute(
                "UPDATE album_tracks SET position = ?1 WHERE id = ?2",
                params![position as u32, id],
            )
            .map_err(|e| e.to_string())?;
    }
    transaction.commit().map_err(|e| e.to_string())?;
    find_album(connection, album_id)
}

fn reorder_album_tracks(
    connection: &mut Connection,
    album_id: i64,
    item_ids: &[i64],
) -> Result<Album, String> {
    let current_ids = find_album(connection, album_id)?
        .tracks
        .into_iter()
        .map(|item| item.id)
        .collect::<Vec<_>>();
    let mut expected = current_ids.clone();
    let mut received = item_ids.to_vec();
    expected.sort_unstable();
    received.sort_unstable();
    if expected != received {
        return Err("a nova ordem deve conter exatamente as faixas do Album".to_string());
    }
    let transaction = connection.transaction().map_err(|e| e.to_string())?;
    for (position, id) in item_ids.iter().enumerate() {
        transaction
            .execute(
                "UPDATE album_tracks SET position = ?1 WHERE id = ?2 AND album_id = ?3",
                params![1_000_000 + position as u32, id, album_id],
            )
            .map_err(|e| e.to_string())?;
    }
    for (position, id) in item_ids.iter().enumerate() {
        transaction
            .execute(
                "UPDATE album_tracks SET position = ?1 WHERE id = ?2 AND album_id = ?3",
                params![position as u32, id, album_id],
            )
            .map_err(|e| e.to_string())?;
    }
    transaction
        .execute(
            "UPDATE albums SET updated_at = ?1 WHERE id = ?2",
            params![now_timestamp(), album_id],
        )
        .map_err(|e| e.to_string())?;
    transaction.commit().map_err(|e| e.to_string())?;
    find_album(connection, album_id)
}

fn list_tracks(connection: &Connection) -> Result<Vec<Track>, String> {
    let mut statement = connection
        .prepare(&format!(
            "{TRACK_SELECT} ORDER BY t.updated_at DESC, t.name COLLATE NOCASE"
        ))
        .map_err(|e| e.to_string())?;
    let tracks = statement
        .query_map([], row_to_track)
        .map_err(|e| e.to_string())?
        .collect::<Result<Vec<_>, _>>()
        .map_err(|e| e.to_string())?;
    let mut tracks = tracks;
    for track in &mut tracks {
        track.tags = tags_for_track(connection, track.id)?;
    }
    Ok(tracks)
}

fn import_track(connection: &mut Connection, path: &str) -> Result<Track, String> {
    let (source_path, normalized) = normalize_source_path(path)?;
    let name = default_track_name(Path::new(&source_path));
    let now = now_timestamp();
    let transaction = connection.transaction().map_err(|e| e.to_string())?;

    let existing_id = transaction
        .query_row(
            "SELECT id FROM tracks WHERE source_path_normalized = ?1",
            [&normalized],
            |row| row.get::<_, i64>(0),
        )
        .optional()
        .map_err(|e| e.to_string())?;

    let id = if let Some(id) = existing_id {
        transaction
            .execute(
                "UPDATE tracks SET source_path = ?1, updated_at = ?2 WHERE id = ?3",
                params![source_path, now, id],
            )
            .map_err(|e| e.to_string())?;
        id
    } else {
        transaction
            .execute(
                "INSERT INTO tracks(name, source_path, source_path_normalized, created_at, updated_at)
                 VALUES (?1, ?2, ?3, ?4, ?4)",
                params![name, source_path, normalized, now],
            )
            .map_err(|e| e.to_string())?;
        let id = transaction.last_insert_rowid();
        transaction
            .execute(
                "INSERT INTO track_render_settings(
                   track_id, range_mode, start_cycle, end_cycle, loops,
                   sample_rate, max_polyphony, export_file_name, updated_at
                 ) VALUES (?1, 'automatic', 0, 1, 1, 44100, 32, ?2, ?3)",
                params![id, name, now],
            )
            .map_err(|e| e.to_string())?;
        id
    };

    transaction.commit().map_err(|e| e.to_string())?;
    find_track(connection, id)
}

fn save_track_settings(
    connection: &Connection,
    track_id: i64,
    settings: &TrackRenderSettings,
) -> Result<Track, String> {
    validate_settings(settings)?;
    let now = now_timestamp();
    let changed = connection
        .execute(
            "UPDATE track_render_settings
                SET range_mode = ?1, start_cycle = ?2, end_cycle = ?3, loops = ?4,
                    sample_rate = ?5, max_polyphony = ?6, export_file_name = ?7,
                    updated_at = ?8
              WHERE track_id = ?9",
            params![
                settings.range_mode,
                settings.start_cycle,
                settings.end_cycle,
                settings.loops,
                settings.sample_rate,
                settings.max_polyphony,
                settings.export_file_name.trim(),
                now,
                track_id,
            ],
        )
        .map_err(|e| e.to_string())?;
    if changed == 0 {
        return Err("Track não encontrada".to_string());
    }
    connection
        .execute(
            "UPDATE tracks SET updated_at = ?1 WHERE id = ?2",
            params![now, track_id],
        )
        .map_err(|e| e.to_string())?;
    find_track(connection, track_id)
}

fn delete_track(connection: &mut Connection, track_id: i64) -> Result<(), String> {
    let transaction = connection.transaction().map_err(|e| e.to_string())?;
    let now = now_timestamp();
    transaction
        .execute(
            "UPDATE albums SET updated_at = ?1
             WHERE id IN (SELECT album_id FROM album_tracks WHERE track_id = ?2)",
            params![now, track_id],
        )
        .map_err(|e| e.to_string())?;
    transaction
        .execute("DELETE FROM album_tracks WHERE track_id = ?1", [track_id])
        .map_err(|e| e.to_string())?;
    let changed = transaction
        .execute("DELETE FROM tracks WHERE id = ?1", [track_id])
        .map_err(|e| e.to_string())?;
    if changed == 0 {
        return Err("Track não encontrada".to_string());
    }
    transaction.commit().map_err(|e| e.to_string())
}

pub fn open(app: &AppHandle) -> Result<LibraryDb, String> {
    let data_dir = app.path().app_data_dir().map_err(|e| e.to_string())?;
    std::fs::create_dir_all(&data_dir).map_err(|e| e.to_string())?;
    let connection =
        Connection::open(data_dir.join("library.sqlite3")).map_err(|e| e.to_string())?;
    migrate(&connection)?;
    Ok(LibraryDb(Mutex::new(connection)))
}

#[command]
pub fn library_list_tracks(db: State<'_, LibraryDb>) -> Result<Vec<Track>, String> {
    let connection = db.0.lock().map_err(|e| e.to_string())?;
    list_tracks(&connection)
}

#[command]
pub fn library_import_track(path: String, db: State<'_, LibraryDb>) -> Result<Track, String> {
    let mut connection = db.0.lock().map_err(|e| e.to_string())?;
    import_track(&mut connection, &path)
}

#[command]
pub fn library_save_track_settings(
    track_id: i64,
    settings: TrackRenderSettings,
    db: State<'_, LibraryDb>,
) -> Result<Track, String> {
    let connection = db.0.lock().map_err(|e| e.to_string())?;
    save_track_settings(&connection, track_id, &settings)
}

#[command]
pub fn library_delete_track(track_id: i64, db: State<'_, LibraryDb>) -> Result<(), String> {
    let mut connection = db.0.lock().map_err(|e| e.to_string())?;
    delete_track(&mut connection, track_id)
}

#[command]
pub fn library_list_tags(db: State<'_, LibraryDb>) -> Result<Vec<Tag>, String> {
    let connection = db.0.lock().map_err(|e| e.to_string())?;
    list_tags(&connection)
}

#[command]
pub fn library_create_tag(
    name: String,
    color: String,
    db: State<'_, LibraryDb>,
) -> Result<Tag, String> {
    let connection = db.0.lock().map_err(|e| e.to_string())?;
    create_tag(&connection, &name, &color)
}

#[command]
pub fn library_update_tag(
    tag_id: i64,
    name: String,
    color: String,
    db: State<'_, LibraryDb>,
) -> Result<Tag, String> {
    let connection = db.0.lock().map_err(|e| e.to_string())?;
    update_tag(&connection, tag_id, &name, &color)
}

#[command]
pub fn library_delete_tag(tag_id: i64, db: State<'_, LibraryDb>) -> Result<(), String> {
    let connection = db.0.lock().map_err(|e| e.to_string())?;
    delete_tag(&connection, tag_id)
}

#[command]
pub fn library_set_track_tag(
    track_id: i64,
    tag_id: i64,
    attached: bool,
    db: State<'_, LibraryDb>,
) -> Result<Track, String> {
    let connection = db.0.lock().map_err(|e| e.to_string())?;
    set_track_tag(&connection, track_id, tag_id, attached)
}

#[command]
pub fn library_list_albums(db: State<'_, LibraryDb>) -> Result<Vec<AlbumSummary>, String> {
    let connection = db.0.lock().map_err(|e| e.to_string())?;
    list_albums(&connection)
}

#[command]
pub fn library_get_album(album_id: i64, db: State<'_, LibraryDb>) -> Result<Album, String> {
    let connection = db.0.lock().map_err(|e| e.to_string())?;
    find_album(&connection, album_id)
}

#[command]
pub fn library_create_album(
    name: String,
    description: String,
    cover_path: Option<String>,
    sample_rate: u32,
    default_gap_seconds: f64,
    db: State<'_, LibraryDb>,
) -> Result<Album, String> {
    let connection = db.0.lock().map_err(|e| e.to_string())?;
    create_album(
        &connection,
        &name,
        &description,
        cover_path,
        sample_rate,
        default_gap_seconds,
    )
}

#[command]
pub fn library_update_album(
    album_id: i64,
    name: String,
    description: String,
    cover_path: Option<String>,
    sample_rate: u32,
    default_gap_seconds: f64,
    db: State<'_, LibraryDb>,
) -> Result<Album, String> {
    let connection = db.0.lock().map_err(|e| e.to_string())?;
    update_album(
        &connection,
        album_id,
        &name,
        &description,
        cover_path,
        sample_rate,
        default_gap_seconds,
    )
}

#[command]
pub fn library_delete_album(album_id: i64, db: State<'_, LibraryDb>) -> Result<(), String> {
    let connection = db.0.lock().map_err(|e| e.to_string())?;
    delete_album(&connection, album_id)
}

#[command]
pub fn library_add_album_track(
    album_id: i64,
    track_id: i64,
    db: State<'_, LibraryDb>,
) -> Result<Album, String> {
    let connection = db.0.lock().map_err(|e| e.to_string())?;
    add_album_track(&connection, album_id, track_id)
}

#[command]
pub fn library_update_album_track(
    item_id: i64,
    settings: AlbumTrackSettings,
    db: State<'_, LibraryDb>,
) -> Result<Album, String> {
    let connection = db.0.lock().map_err(|e| e.to_string())?;
    update_album_track(&connection, item_id, &settings)
}

#[command]
pub fn library_remove_album_track(item_id: i64, db: State<'_, LibraryDb>) -> Result<Album, String> {
    let mut connection = db.0.lock().map_err(|e| e.to_string())?;
    remove_album_track(&mut connection, item_id)
}

#[command]
pub fn library_reorder_album_tracks(
    album_id: i64,
    item_ids: Vec<i64>,
    db: State<'_, LibraryDb>,
) -> Result<Album, String> {
    let mut connection = db.0.lock().map_err(|e| e.to_string())?;
    reorder_album_tracks(&mut connection, album_id, &item_ids)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;

    fn memory_db() -> Connection {
        let connection = Connection::open_in_memory().unwrap();
        migrate(&connection).unwrap();
        connection
    }

    fn source_fixture(name: &str) -> PathBuf {
        let path = std::env::temp_dir().join(format!(
            "strudel-library-test-{}-{name}.strudel",
            std::process::id()
        ));
        std::fs::write(&path, "s(\"bd sd\")").unwrap();
        path
    }

    #[test]
    fn migration_creates_track_and_settings_with_constraints() {
        let connection = memory_db();
        connection
            .execute(
                "INSERT INTO tracks(name, source_path, source_path_normalized, created_at, updated_at)
                 VALUES ('Forest', '/tmp/forest.strudel', '/tmp/forest.strudel', 1, 1)",
                [],
            )
            .unwrap();
        let id = connection.last_insert_rowid();
        assert!(connection
            .execute(
                "INSERT INTO track_render_settings(
                   track_id, range_mode, start_cycle, end_cycle, loops,
                   sample_rate, max_polyphony, export_file_name, updated_at
                 ) VALUES (?1, 'manual', 2, 6, 10, 48000, 64, 'forest', 1)",
                [id],
            )
            .is_ok());
        assert!(connection
            .execute(
                "UPDATE track_render_settings SET loops = 1000 WHERE track_id = ?1",
                [id],
            )
            .is_err());
    }

    #[test]
    fn deleting_track_cascades_only_database_settings() {
        let connection = memory_db();
        connection
            .execute(
                "INSERT INTO tracks(name, source_path, source_path_normalized, created_at, updated_at)
                 VALUES ('Forest', '/tmp/forest.strudel', '/tmp/forest.strudel', 1, 1)",
                [],
            )
            .unwrap();
        let id = connection.last_insert_rowid();
        connection
            .execute(
                "INSERT INTO track_render_settings VALUES (?1, 'automatic', 0, 8, 1, 44100, 32, 'forest', 1)",
                [id],
            )
            .unwrap();
        connection
            .execute("DELETE FROM tracks WHERE id = ?1", [id])
            .unwrap();
        let settings: i64 = connection
            .query_row("SELECT COUNT(*) FROM track_render_settings", [], |row| {
                row.get(0)
            })
            .unwrap();
        assert_eq!(settings, 0);
    }

    #[test]
    fn imports_once_and_restores_saved_render_settings() {
        let mut connection = memory_db();
        let source = source_fixture("persist");
        let source_string = source.to_string_lossy();

        let imported = import_track(&mut connection, &source_string).unwrap();
        let duplicate = import_track(&mut connection, &source_string).unwrap();
        assert_eq!(duplicate.id, imported.id);
        assert_eq!(list_tracks(&connection).unwrap().len(), 1);

        let settings = TrackRenderSettings {
            range_mode: "manual".to_string(),
            start_cycle: 2.0,
            end_cycle: 6.0,
            loops: 10,
            sample_rate: 48_000,
            max_polyphony: 64,
            export_file_name: "album-version".to_string(),
        };
        save_track_settings(&connection, imported.id, &settings).unwrap();

        let restored = list_tracks(&connection).unwrap().remove(0);
        assert_eq!(restored.settings, settings);

        delete_track(&mut connection, imported.id).unwrap();
        assert!(
            source.exists(),
            "deleting a Track must not delete its source"
        );
        assert!(list_tracks(&connection).unwrap().is_empty());
        std::fs::remove_file(source).unwrap();
    }

    #[test]
    fn manages_tags_and_track_links_without_duplicates() {
        let mut connection = memory_db();
        let source = source_fixture("tags");
        let track = import_track(&mut connection, &source.to_string_lossy()).unwrap();

        let ambient = create_tag(&connection, "Ambient", "#4F8A67").unwrap();
        assert!(create_tag(&connection, "ambient", "#FFFFFF").is_err());
        assert!(create_tag(&connection, "Invalid", "white").is_err());

        let attached = set_track_tag(&connection, track.id, ambient.id, true).unwrap();
        assert_eq!(attached.tags.len(), 1);
        assert_eq!(attached.tags[0].name, "Ambient");
        set_track_tag(&connection, track.id, ambient.id, true).unwrap();
        assert_eq!(list_tags(&connection).unwrap()[0].track_count, 1);

        let updated = update_tag(&connection, ambient.id, "Atmosphere", "#123ABC").unwrap();
        assert_eq!(updated.name, "Atmosphere");
        assert_eq!(
            list_tracks(&connection).unwrap()[0].tags[0].color,
            "#123ABC"
        );

        delete_tag(&connection, ambient.id).unwrap();
        assert!(list_tracks(&connection).unwrap()[0].tags.is_empty());
        assert_eq!(list_tracks(&connection).unwrap().len(), 1);
        std::fs::remove_file(source).unwrap();
    }

    #[test]
    fn manages_album_tracks_with_independent_settings_and_order() {
        let mut connection = memory_db();
        let source_a = source_fixture("album-a");
        let source_b = source_fixture("album-b");
        let track_a = import_track(&mut connection, &source_a.to_string_lossy()).unwrap();
        let track_b = import_track(&mut connection, &source_b.to_string_lossy()).unwrap();
        let album_tag = create_tag(&connection, "Album tag", "#ABCDEF").unwrap();
        set_track_tag(&connection, track_a.id, album_tag.id, true).unwrap();
        let album =
            create_album(&connection, "Night Walk", "Two tracks", None, 48_000, 1.5).unwrap();

        let album = add_album_track(&connection, album.summary.id, track_a.id).unwrap();
        let album = add_album_track(&connection, album.summary.id, track_b.id).unwrap();
        assert_eq!(album.tracks.len(), 2);
        assert_eq!(album.tracks[0].settings.gap_after_seconds, 1.5);
        assert_eq!(album.summary.tags[0].name, "Album tag");

        let first_id = album.tracks[0].id;
        let second_id = album.tracks[1].id;
        let custom = AlbumTrackSettings {
            range_mode: "manual".to_string(),
            start_cycle: 2.0,
            end_cycle: 6.0,
            loops: 4,
            max_polyphony: 64,
            gap_after_seconds: 0.5,
        };
        let album = update_album_track(&connection, first_id, &custom).unwrap();
        assert_eq!(album.tracks[0].settings, custom);
        assert_eq!(
            find_track(&connection, track_a.id).unwrap().settings.loops,
            1
        );

        let album = reorder_album_tracks(&mut connection, album.summary.id, &[second_id, first_id])
            .unwrap();
        assert_eq!(album.tracks[0].track.id, track_b.id);

        let album = remove_album_track(&mut connection, second_id).unwrap();
        assert_eq!(album.tracks.len(), 1);
        assert_eq!(album.tracks[0].position, 0);

        delete_track(&mut connection, track_a.id).unwrap();
        assert!(find_album(&connection, album.summary.id).unwrap().tracks.is_empty());

        delete_album(&connection, album.summary.id).unwrap();
        assert_eq!(list_tracks(&connection).unwrap().len(), 1);
        std::fs::remove_file(source_a).unwrap();
        std::fs::remove_file(source_b).unwrap();
    }
}
