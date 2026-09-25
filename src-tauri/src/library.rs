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
pub struct Track {
    pub id: i64,
    pub name: String,
    pub source_path: String,
    pub settings: TrackRenderSettings,
    pub created_at: i64,
    pub updated_at: i64,
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
    })
}

const TRACK_SELECT: &str = "SELECT t.id, t.name, t.source_path, t.created_at, t.updated_at,
            s.range_mode, s.start_cycle, s.end_cycle, s.loops,
            s.sample_rate, s.max_polyphony, s.export_file_name
       FROM tracks t
       JOIN track_render_settings s ON s.track_id = t.id";

fn find_track(connection: &Connection, id: i64) -> Result<Track, String> {
    connection
        .query_row(
            &format!("{TRACK_SELECT} WHERE t.id = ?1"),
            [id],
            row_to_track,
        )
        .optional()
        .map_err(|e| e.to_string())?
        .ok_or_else(|| "Track não encontrada".to_string())
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

fn delete_track(connection: &Connection, track_id: i64) -> Result<(), String> {
    let changed = connection
        .execute("DELETE FROM tracks WHERE id = ?1", [track_id])
        .map_err(|e| e.to_string())?;
    if changed == 0 {
        return Err("Track não encontrada".to_string());
    }
    Ok(())
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
    let connection = db.0.lock().map_err(|e| e.to_string())?;
    delete_track(&connection, track_id)
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

        delete_track(&connection, imported.id).unwrap();
        assert!(
            source.exists(),
            "deleting a Track must not delete its source"
        );
        assert!(list_tracks(&connection).unwrap().is_empty());
        std::fs::remove_file(source).unwrap();
    }
}
