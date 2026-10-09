//! 检索库里的向量（施工 R-5 下，`docs/blueprint/recall.md` 第三条第 1、2 款）：`vectors(key, model, data)`，一条照一个模型一份，
//! `data` 是 f32 的小端字节（`miyu_recall::vector`）。条目拿掉、换了字的时候向量跟着拿掉（`recall.rs` 的 `put`、`remove_key`、
//! `forget`）。
//!
//! 查的时候把这个模型的向量全读出来逐条算点积（旧版的规模约一万条，几毫秒）：慢了先量，再看要不要另想办法。

use rusqlite::{OptionalExtension, params};

use miyu_kernel::time::Timestamp;
use miyu_recall::vector::{dot, from_bytes, to_bytes};

use super::{DbError, RecallIndex, integer};

/// 以前的库（施工 R-5 下以前建的，同一个版本号）开的时候补上向量表：照原样建，已经有的不动。
pub(super) const ADD: &str = "CREATE TABLE IF NOT EXISTS vectors (
    key TEXT NOT NULL,
    model TEXT NOT NULL,
    data BLOB NOT NULL,
    PRIMARY KEY (key, model)
) WITHOUT ROWID";

/// 照向量找到的一条。
#[derive(Debug, Clone, PartialEq)]
pub struct Near {
    /// 条目的键。
    pub key: String,
    /// 有多像：点积，-1 到 1。
    pub similar: f32,
    /// 条目的字。
    pub text: String,
    /// 条目放进来时的时刻。
    pub at: Timestamp,
}

impl RecallIndex {
    /// 放进条目 `key` 照模型 `model` 的向量，有旧的换掉。条目已经没了的（补的时候被拿掉了）不放。
    ///
    /// # Errors
    ///
    /// 写不进。
    pub fn put_vector(&self, key: &str, model: &str, vector: &[f32]) -> Result<(), DbError> {
        let data = to_bytes(vector);
        self.write(|tx| {
            tx.execute(
                "INSERT OR REPLACE INTO vectors (key, model, data)
                 SELECT ?1, ?2, ?3 WHERE EXISTS (SELECT 1 FROM items WHERE key = ?1)",
                params![key, model, data],
            )?;
            Ok(())
        })
    }

    /// 还没有模型 `model` 的向量的条目，照放进来的先后，`after` 那一行以后的、最多 `limit` 条：（行号、键、字）。补的一方照
    /// 最后一条的行号接着往后取，这一回算不出的不会挡住后面的。
    ///
    /// # Errors
    ///
    /// 读不了。
    pub fn missing(
        &self,
        model: &str,
        after: i64,
        limit: usize,
    ) -> Result<Vec<(i64, String, String)>, DbError> {
        let db = self.lock();
        let Some(db) = db.as_ref() else {
            return Ok(Vec::new());
        };
        let limit = integer(limit as u64, "limit")?;
        let mut select = db.prepare(
            "SELECT items.id, items.key, items.text FROM items
             WHERE items.id > ?2
               AND NOT EXISTS (SELECT 1 FROM vectors WHERE vectors.key = items.key AND vectors.model = ?1)
             ORDER BY items.id LIMIT ?3",
        )?;
        let rows = select.query_map(params![model, after, limit], |row| {
            Ok((row.get(0)?, row.get(1)?, row.get(2)?))
        })?;
        Ok(rows.collect::<Result<_, _>>()?)
    }

    /// 和 `query`（同一个模型、归一化过的）最像的、最多 `limit` 条，最像的在前。读不出来的向量（长度不对）当不像，跳过。
    ///
    /// # Errors
    ///
    /// 读不了；条目的时刻读不出。
    pub fn nearest(&self, model: &str, query: &[f32], limit: usize) -> Result<Vec<Near>, DbError> {
        let db = self.lock();
        let Some(db) = db.as_ref() else {
            return Ok(Vec::new());
        };
        let mut select = db.prepare("SELECT key, data FROM vectors WHERE model = ?1")?;
        let rows = select.query_map([model], |row| {
            Ok((row.get::<_, String>(0)?, row.get::<_, Vec<u8>>(1)?))
        })?;
        let mut scored = Vec::new();
        for row in rows {
            let (key, data) = row?;
            if let Some(vector) = from_bytes(&data) {
                scored.push((dot(query, &vector), key));
            }
        }
        scored.sort_by(|(a, key_a), (b, key_b)| b.total_cmp(a).then(key_a.cmp(key_b)));
        scored.truncate(limit);
        let mut near = Vec::with_capacity(scored.len());
        for (similar, key) in scored {
            let item: Option<(String, i64)> = db
                .query_row("SELECT text, at FROM items WHERE key = ?1", [&key], |row| {
                    Ok((row.get(0)?, row.get(1)?))
                })
                .optional()?;
            let Some((text, at)) = item else { continue };
            let at = Timestamp::from_unix_millis(at)
                .ok_or_else(|| DbError::Bad(format!("time {at}")))?;
            near.push(Near {
                key,
                similar,
                text,
                at,
            });
        }
        Ok(near)
    }
}
