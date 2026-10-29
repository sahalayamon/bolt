//! SQLite persistence: trusted devices, transfer history, settings.
//!
//! Only metadata is stored (names, sizes, timestamps, status). File contents
//! and full local paths are never written to the database.

use bolt_security::{DeviceId, TrustError, TrustStore, TrustedDevice};
use rusqlite::{params, Connection, OptionalExtension};
use std::{path::Path, sync::Mutex};
use thiserror::Error;

#[derive(Debug, Error)]
pub enum StoreError {
    #[error("database error: {0}")]
    Db(#[from] rusqlite::Error),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Direction {
    Sent,
    Received,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Status {
    Completed,
    Failed,
    Cancelled,
    Rejected,
}

impl Status {
    fn as_str(self) -> &'static str {
        match self {
            Self::Completed => "completed",
            Self::Failed => "failed",
            Self::Cancelled => "cancelled",
            Self::Rejected => "rejected",
        }
    }
    fn parse(s: &str) -> Self {
        match s {
            "completed" => Self::Completed,
            "cancelled" => Self::Cancelled,
            "rejected" => Self::Rejected,
            _ => Self::Failed,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct HistoryEntry {
    pub transfer_id: String,
    pub direction: Direction,
    pub peer_id: String,
    pub peer_name: String,
    pub status: Status,
    pub file_count: u32,
    /// Name of the first file (or folder) for display; not a path.
    pub title: String,
    pub total_bytes: u64,
    pub started_unix: i64,
    pub finished_unix: i64,
    pub error: Option<String>,
}

#[derive(Debug, Clone, Default)]
pub struct HistoryQuery {
    pub text: Option<String>,
    pub status: Option<Status>,
    pub direction: Option<Direction>,
    pub limit: Option<u32>,
}

pub struct Store {
    conn: Mutex<Connection>,
}

const SCHEMA_VERSION: i64 = 1;

impl Store {
    pub fn open(path: &Path) -> Result<Self, StoreError> {
        Self::init(Connection::open(path)?)
    }

    pub fn open_in_memory() -> Result<Self, StoreError> {
        Self::init(Connection::open_in_memory()?)
    }

    fn init(conn: Connection) -> Result<Self, StoreError> {
        conn.pragma_update(None, "journal_mode", "WAL")?;
        conn.pragma_update(None, "foreign_keys", "ON")?;
        let v: i64 = conn.query_row("PRAGMA user_version", [], |r| r.get(0))?;
        if v < 1 {
            conn.execute_batch(
                "CREATE TABLE trusted_devices (
                     id TEXT PRIMARY KEY, name TEXT NOT NULL,
                     paired_at INTEGER NOT NULL, last_seen INTEGER);
                 CREATE TABLE history (
                     transfer_id TEXT PRIMARY KEY, direction TEXT NOT NULL,
                     peer_id TEXT NOT NULL, peer_name TEXT NOT NULL, status TEXT NOT NULL,
                     file_count INTEGER NOT NULL, title TEXT NOT NULL, total_bytes INTEGER NOT NULL,
                     started INTEGER NOT NULL, finished INTEGER NOT NULL, error TEXT);
                 CREATE INDEX history_finished ON history(finished DESC);
                 CREATE TABLE settings (key TEXT PRIMARY KEY, value TEXT NOT NULL);",
            )?;
            conn.pragma_update(None, "user_version", SCHEMA_VERSION)?;
        }
        Ok(Self {
            conn: Mutex::new(conn),
        })
    }

    fn db(&self) -> std::sync::MutexGuard<'_, Connection> {
        self.conn.lock().unwrap_or_else(|p| p.into_inner())
    }

    // ---- history -------------------------------------------------------

    pub fn record_transfer(&self, e: &HistoryEntry) -> Result<(), StoreError> {
        self.db().execute(
            "INSERT OR REPLACE INTO history VALUES (?1,?2,?3,?4,?5,?6,?7,?8,?9,?10,?11)",
            params![
                e.transfer_id,
                if e.direction == Direction::Sent {
                    "sent"
                } else {
                    "received"
                },
                e.peer_id,
                e.peer_name,
                e.status.as_str(),
                e.file_count,
                e.title,
                e.total_bytes as i64,
                e.started_unix,
                e.finished_unix,
                e.error
            ],
        )?;
        Ok(())
    }

    pub fn history(&self, q: &HistoryQuery) -> Result<Vec<HistoryEntry>, StoreError> {
        let db = self.db();
        let mut stmt = db.prepare(
            "SELECT transfer_id,direction,peer_id,peer_name,status,file_count,title,total_bytes,started,finished,error
             FROM history
             WHERE (?1 IS NULL OR title LIKE '%'||?1||'%' ESCAPE '\\' OR peer_name LIKE '%'||?1||'%' ESCAPE '\\')
               AND (?2 IS NULL OR status = ?2) AND (?3 IS NULL OR direction = ?3)
             ORDER BY finished DESC LIMIT ?4",
        )?;
        let text = q.text.as_ref().map(|t| {
            t.replace('\\', "\\\\")
                .replace('%', "\\%")
                .replace('_', "\\_")
        });
        let dir = q.direction.map(|d| {
            if d == Direction::Sent {
                "sent"
            } else {
                "received"
            }
        });
        let rows = stmt.query_map(
            params![
                text,
                q.status.map(Status::as_str),
                dir,
                q.limit.unwrap_or(500)
            ],
            |r| {
                Ok(HistoryEntry {
                    transfer_id: r.get(0)?,
                    direction: if r.get::<_, String>(1)? == "sent" {
                        Direction::Sent
                    } else {
                        Direction::Received
                    },
                    peer_id: r.get(2)?,
                    peer_name: r.get(3)?,
                    status: Status::parse(&r.get::<_, String>(4)?),
                    file_count: r.get(5)?,
                    title: r.get(6)?,
                    total_bytes: r.get::<_, i64>(7)? as u64,
                    started_unix: r.get(8)?,
                    finished_unix: r.get(9)?,
                    error: r.get(10)?,
                })
            },
        )?;
        Ok(rows.collect::<Result<_, _>>()?)
    }

    pub fn clear_history(&self) -> Result<usize, StoreError> {
        Ok(self.db().execute("DELETE FROM history", [])?)
    }

    // ---- settings ------------------------------------------------------

    pub fn get_setting(&self, key: &str) -> Result<Option<String>, StoreError> {
        Ok(self
            .db()
            .query_row("SELECT value FROM settings WHERE key=?1", [key], |r| {
                r.get(0)
            })
            .optional()?)
    }

    pub fn set_setting(&self, key: &str, value: &str) -> Result<(), StoreError> {
        self.db().execute(
            "INSERT INTO settings(key,value) VALUES(?1,?2) ON CONFLICT(key) DO UPDATE SET value=excluded.value",
            [key, value],
        )?;
        Ok(())
    }
}

fn terr(e: rusqlite::Error) -> TrustError {
    TrustError(e.to_string())
}

impl TrustStore for Store {
    fn is_trusted(&self, id: &DeviceId) -> bool {
        self.db()
            .query_row(
                "SELECT 1 FROM trusted_devices WHERE id=?1",
                [id.as_str()],
                |_| Ok(()),
            )
            .optional()
            .map(|o| o.is_some())
            .unwrap_or_else(|e| {
                tracing::error!("trust lookup failed, treating as untrusted: {e}");
                false // fail closed
            })
    }

    fn trust(&self, d: TrustedDevice) -> Result<(), TrustError> {
        self.db()
            .execute(
                "INSERT INTO trusted_devices VALUES(?1,?2,?3,?4)
                 ON CONFLICT(id) DO UPDATE SET name=excluded.name, last_seen=excluded.last_seen",
                params![d.id.as_str(), d.name, d.paired_at_unix, d.last_seen_unix],
            )
            .map(|_| ())
            .map_err(terr)
    }

    fn revoke(&self, id: &DeviceId) -> Result<(), TrustError> {
        self.db()
            .execute("DELETE FROM trusted_devices WHERE id=?1", [id.as_str()])
            .map(|_| ())
            .map_err(terr)
    }

    fn list(&self) -> Result<Vec<TrustedDevice>, TrustError> {
        let db = self.db();
        let mut stmt = db
            .prepare("SELECT id,name,paired_at,last_seen FROM trusted_devices ORDER BY name")
            .map_err(terr)?;
        let rows = stmt
            .query_map([], |r| {
                Ok((
                    r.get::<_, String>(0)?,
                    r.get::<_, String>(1)?,
                    r.get::<_, i64>(2)?,
                    r.get::<_, Option<i64>>(3)?,
                ))
            })
            .map_err(terr)?;
        let mut out = vec![];
        for row in rows {
            let (id, name, paired_at_unix, last_seen_unix) = row.map_err(terr)?;
            if let Ok(id) = DeviceId::parse(&id) {
                out.push(TrustedDevice {
                    id,
                    name,
                    paired_at_unix,
                    last_seen_unix,
                });
            }
        }
        Ok(out)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn entry(id: &str, title: &str, status: Status, finished: i64) -> HistoryEntry {
        HistoryEntry {
            transfer_id: id.into(),
            direction: Direction::Received,
            peer_id: "p".into(),
            peer_name: "Pixel 9".into(),
            status,
            file_count: 1,
            title: title.into(),
            total_bytes: 10,
            started_unix: finished - 5,
            finished_unix: finished,
            error: None,
        }
    }

    #[test]
    fn history_search_filter_order_clear() {
        let s = Store::open_in_memory().unwrap();
        s.record_transfer(&entry("1", "holiday.mp4", Status::Completed, 100))
            .unwrap();
        s.record_transfer(&entry("2", "report_final.pdf", Status::Failed, 200))
            .unwrap();
        s.record_transfer(&entry("3", "100%.txt", Status::Cancelled, 300))
            .unwrap();

        let all = s.history(&HistoryQuery::default()).unwrap();
        assert_eq!(
            all.iter()
                .map(|e| e.transfer_id.as_str())
                .collect::<Vec<_>>(),
            ["3", "2", "1"]
        );
        let q = HistoryQuery {
            text: Some("holiday".into()),
            ..Default::default()
        };
        assert_eq!(s.history(&q).unwrap().len(), 1);
        let q = HistoryQuery {
            text: Some("%".into()),
            ..Default::default()
        };
        assert_eq!(
            s.history(&q).unwrap().len(),
            1,
            "LIKE wildcards are escaped"
        );
        let q = HistoryQuery {
            status: Some(Status::Failed),
            ..Default::default()
        };
        assert_eq!(s.history(&q).unwrap()[0].transfer_id, "2");
        let q = HistoryQuery {
            text: Some("pixel".into()),
            ..Default::default()
        };
        assert_eq!(
            s.history(&q).unwrap().len(),
            3,
            "search matches device name, case-insensitive"
        );
        assert_eq!(s.clear_history().unwrap(), 3);
        assert!(s.history(&HistoryQuery::default()).unwrap().is_empty());
    }

    #[test]
    fn trust_persists_across_reopen_and_revokes() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("db.sqlite");
        let id = DeviceId::parse(&"c".repeat(64)).unwrap();
        {
            let s = Store::open(&path).unwrap();
            assert!(!s.is_trusted(&id));
            s.trust(TrustedDevice {
                id: id.clone(),
                name: "Laptop".into(),
                paired_at_unix: 5,
                last_seen_unix: None,
            })
            .unwrap();
        }
        let s = Store::open(&path).unwrap();
        assert!(s.is_trusted(&id));
        assert_eq!(s.list().unwrap()[0].name, "Laptop");
        s.revoke(&id).unwrap();
        assert!(!s.is_trusted(&id));
    }

    #[test]
    fn settings_roundtrip_and_overwrite() {
        let s = Store::open_in_memory().unwrap();
        assert_eq!(s.get_setting("theme").unwrap(), None);
        s.set_setting("theme", "dark").unwrap();
        s.set_setting("theme", "light").unwrap();
        assert_eq!(s.get_setting("theme").unwrap().as_deref(), Some("light"));
    }
}
