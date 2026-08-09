//! SQLite-backed persistent memory store.
//!
//! [`MemoryStore`] is the single access point for all persistent data:
//! - **doc cache** – cached command explanations (man pages, tldr, AI).
//! - **facts** – arbitrary key/value knowledge partitioned by category.
//! - **tool usage** – record of every tool invocation.
//! - **conversation summaries** – compressed snapshots of AI sessions.

use chrono::{Duration, Utc};
use serde::{Deserialize, Serialize};
use sqlx::{sqlite::SqliteConnectOptions, SqlitePool};
use std::path::Path;
use std::str::FromStr;
use tracing::{debug, instrument};
use uuid::Uuid;

use toolshub_core::{Error, Result};

// ── Public data types ──────────────────────────────────────────────────────────

/// A cached documentation entry.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DocCacheEntry {
    pub id: String,
    pub tool_name: String,
    pub content: String,
    pub source: String,
    pub cached_at: String,
    pub expires_at: String,
}

/// A stored fact (key/value pair in a category).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Fact {
    pub id: String,
    pub category: String,
    pub key: String,
    pub value: String,
    pub confidence: f64,
    pub created_at: String,
    pub updated_at: String,
}

/// A recorded tool invocation.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ToolUsage {
    pub id: String,
    pub tool_name: String,
    pub command: String,
    pub directory: String,
    pub exit_code: i64,
    pub used_at: String,
}

/// A compressed AI conversation summary.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ConversationSummary {
    pub id: String,
    pub summary: String,
    pub topics: Vec<String>,
    pub created_at: String,
}

// ── MemoryStore ────────────────────────────────────────────────────────────────

/// The shared SQLite-backed memory store.
#[derive(Clone, Debug)]
pub struct MemoryStore {
    pool: SqlitePool,
}

impl MemoryStore {
    /// Open (or create) the database at `db_path` and run any pending migrations.
    ///
    /// Alias for [`MemoryStore::open`] — both names are accepted.
    pub async fn new(db_path: &Path) -> Result<Self> {
        Self::open(db_path).await
    }

    /// Open (or create) the database at `db_path` and run any pending migrations.
    pub async fn open(db_path: &Path) -> Result<Self> {
        // Ensure the parent directory exists.
        if let Some(parent) = db_path.parent() {
            std::fs::create_dir_all(parent)
                .map_err(|e| Error::Database(format!("Failed to create data dir: {e}")))?;
        }

        let options = SqliteConnectOptions::from_str(&format!(
            "sqlite://{}",
            db_path.to_string_lossy()
        ))
        .map_err(|e| Error::Database(format!("Invalid DB path: {e}")))?
        .create_if_missing(true);

        let pool = SqlitePool::connect_with(options)
            .await
            .map_err(|e| Error::Database(format!("Failed to open DB: {e}")))?;

        // Run embedded migrations.
        sqlx::migrate!("src/migrations")
            .run(&pool)
            .await
            .map_err(|e| Error::Database(format!("Migration failed: {e}")))?;

        debug!("MemoryStore opened at {}", db_path.display());
        Ok(Self { pool })
    }

    // ── Doc Cache ──────────────────────────────────────────────────────────────

    /// Retrieve a cached doc entry for `tool`, if it exists and has not expired.
    #[instrument(skip(self))]
    pub async fn get_cached_doc(&self, tool: &str) -> Result<Option<DocCacheEntry>> {
        let now = Utc::now().to_rfc3339();

        let row = sqlx::query!(
            r#"
            SELECT id, tool_name, content, source, cached_at, expires_at
            FROM   doc_cache
            WHERE  tool_name = ?
              AND  expires_at > ?
            "#,
            tool,
            now,
        )
        .fetch_optional(&self.pool)
        .await
        .map_err(|e| Error::Database(format!("get_cached_doc: {e}")))?;

        Ok(row.map(|r| DocCacheEntry {
            id: r.id,
            tool_name: r.tool_name,
            content: r.content,
            source: r.source,
            cached_at: r.cached_at,
            expires_at: r.expires_at,
        }))
    }

    /// Insert or replace a cached doc entry.
    #[instrument(skip(self, entry))]
    pub async fn cache_doc(&self, entry: &DocCacheEntry) -> Result<()> {
        sqlx::query!(
            r#"
            INSERT INTO doc_cache (id, tool_name, content, source, cached_at, expires_at)
            VALUES (?, ?, ?, ?, ?, ?)
            ON CONFLICT (tool_name) DO UPDATE SET
                content    = excluded.content,
                source     = excluded.source,
                cached_at  = excluded.cached_at,
                expires_at = excluded.expires_at
            "#,
            entry.id,
            entry.tool_name,
            entry.content,
            entry.source,
            entry.cached_at,
            entry.expires_at,
        )
        .execute(&self.pool)
        .await
        .map_err(|e| Error::Database(format!("cache_doc: {e}")))?;

        debug!("Cached doc for '{}' (source={})", entry.tool_name, entry.source);
        Ok(())
    }

    /// Convenience wrapper: cache a doc with a computed TTL in hours.
    pub async fn set_cached_doc(
        &self,
        tool: &str,
        content: &str,
        source: &str,
        ttl_hours: i64,
    ) -> Result<()> {
        let now = Utc::now();
        let entry = DocCacheEntry {
            id: Uuid::new_v4().to_string(),
            tool_name: tool.to_string(),
            content: content.to_string(),
            source: source.to_string(),
            cached_at: now.to_rfc3339(),
            expires_at: (now + Duration::hours(ttl_hours)).to_rfc3339(),
        };
        self.cache_doc(&entry).await
    }

    /// Delete the cached doc for `tool`, if one exists.
    #[instrument(skip(self))]
    pub async fn evict_cached_doc(&self, tool: &str) -> Result<()> {
        sqlx::query!("DELETE FROM doc_cache WHERE tool_name = ?", tool)
            .execute(&self.pool)
            .await
            .map_err(|e| Error::Database(format!("evict_cached_doc: {e}")))?;
        Ok(())
    }

    /// List all cached doc entries (including expired ones, for inspection).
    pub async fn list_cached_docs(&self) -> Result<Vec<DocCacheEntry>> {
        let rows = sqlx::query!(
            "SELECT id, tool_name, content, source, cached_at, expires_at FROM doc_cache ORDER BY cached_at DESC"
        )
        .fetch_all(&self.pool)
        .await
        .map_err(|e| Error::Database(format!("list_cached_docs: {e}")))?;

        Ok(rows
            .into_iter()
            .map(|r| DocCacheEntry {
                id: r.id,
                tool_name: r.tool_name,
                content: r.content,
                source: r.source,
                cached_at: r.cached_at,
                expires_at: r.expires_at,
            })
            .collect())
    }

    // ── Facts ──────────────────────────────────────────────────────────────────

    /// Retrieve a fact by category and key.
    #[instrument(skip(self))]
    pub async fn get_fact(&self, category: &str, key: &str) -> Result<Option<String>> {
        let row = sqlx::query!(
            "SELECT value FROM facts WHERE category = ? AND key = ?",
            category,
            key,
        )
        .fetch_optional(&self.pool)
        .await
        .map_err(|e| Error::Database(format!("get_fact: {e}")))?;

        Ok(row.map(|r| r.value))
    }

    /// Get all facts for a given category.
    #[instrument(skip(self))]
    pub async fn get_facts_by_category(&self, category: &str) -> Result<Vec<Fact>> {
        let rows = sqlx::query!(
            r#"
            SELECT id, category, key, value, confidence, created_at, updated_at
            FROM   facts
            WHERE  category = ?
            ORDER BY key
            "#,
            category,
        )
        .fetch_all(&self.pool)
        .await
        .map_err(|e| Error::Database(format!("get_facts_by_category: {e}")))?;

        Ok(rows
            .into_iter()
            .map(|r| Fact {
                id: r.id,
                category: r.category,
                key: r.key,
                value: r.value,
                confidence: r.confidence,
                created_at: r.created_at,
                updated_at: r.updated_at,
            })
            .collect())
    }

    /// List all stored facts.
    pub async fn list_all_facts(&self) -> Result<Vec<Fact>> {
        let rows = sqlx::query!(
            "SELECT id, category, key, value, confidence, created_at, updated_at FROM facts ORDER BY category, key"
        )
        .fetch_all(&self.pool)
        .await
        .map_err(|e| Error::Database(format!("list_all_facts: {e}")))?;

        Ok(rows
            .into_iter()
            .map(|r| Fact {
                id: r.id,
                category: r.category,
                key: r.key,
                value: r.value,
                confidence: r.confidence,
                created_at: r.created_at,
                updated_at: r.updated_at,
            })
            .collect())
    }

    /// Insert or replace a fact.
    #[instrument(skip(self, value))]
    pub async fn set_fact(
        &self,
        category: &str,
        key: &str,
        value: &str,
        confidence: f64,
    ) -> Result<()> {
        let id = Uuid::new_v4().to_string();
        let now = Utc::now().to_rfc3339();

        sqlx::query!(
            r#"
            INSERT INTO facts (id, category, key, value, confidence, created_at, updated_at)
            VALUES (?, ?, ?, ?, ?, ?, ?)
            ON CONFLICT (category, key) DO UPDATE SET
                value      = excluded.value,
                confidence = excluded.confidence,
                updated_at = excluded.updated_at
            "#,
            id,
            category,
            key,
            value,
            confidence,
            now,
            now,
        )
        .execute(&self.pool)
        .await
        .map_err(|e| Error::Database(format!("set_fact: {e}")))?;

        Ok(())
    }

    /// Delete a specific fact.
    pub async fn delete_fact(&self, category: &str, key: &str) -> Result<()> {
        sqlx::query!(
            "DELETE FROM facts WHERE category = ? AND key = ?",
            category,
            key,
        )
        .execute(&self.pool)
        .await
        .map_err(|e| Error::Database(format!("delete_fact: {e}")))?;
        Ok(())
    }

    // ── Tool Usage ─────────────────────────────────────────────────────────────

    /// Record a tool invocation using a [`ToolUsage`] struct.
    #[instrument(skip(self, usage))]
    pub async fn record_tool_usage(&self, usage: &ToolUsage) -> Result<()> {
        sqlx::query!(
            r#"
            INSERT INTO tool_usage (id, tool_name, command, directory, exit_code, used_at)
            VALUES (?, ?, ?, ?, ?, ?)
            "#,
            usage.id,
            usage.tool_name,
            usage.command,
            usage.directory,
            usage.exit_code,
            usage.used_at,
        )
        .execute(&self.pool)
        .await
        .map_err(|e| Error::Database(format!("record_tool_usage: {e}")))?;

        Ok(())
    }

    /// List recent tool usages, limited by `limit`.
    pub async fn list_tool_usage(&self, limit: i64) -> Result<Vec<ToolUsage>> {
        let rows = sqlx::query!(
            r#"
            SELECT id, tool_name, command, directory, exit_code, used_at
            FROM   tool_usage
            ORDER  BY used_at DESC
            LIMIT  ?
            "#,
            limit,
        )
        .fetch_all(&self.pool)
        .await
        .map_err(|e| Error::Database(format!("list_tool_usage: {e}")))?;

        Ok(rows
            .into_iter()
            .map(|r| ToolUsage {
                id: r.id,
                tool_name: r.tool_name,
                command: r.command,
                directory: r.directory,
                exit_code: r.exit_code,
                used_at: r.used_at,
            })
            .collect())
    }

    // ── Conversation Summaries ─────────────────────────────────────────────────

    /// Store a compressed conversation summary.
    pub async fn add_conversation_summary(
        &self,
        summary: &str,
        topics: &[String],
    ) -> Result<String> {
        let id = Uuid::new_v4().to_string();
        let now = Utc::now().to_rfc3339();
        let topics_json = serde_json::to_string(topics)
            .map_err(|e| Error::Database(format!("topics serialise: {e}")))?;

        sqlx::query!(
            r#"
            INSERT INTO conversation_summaries (id, summary, topics, created_at)
            VALUES (?, ?, ?, ?)
            "#,
            id,
            summary,
            topics_json,
            now,
        )
        .execute(&self.pool)
        .await
        .map_err(|e| Error::Database(format!("add_conversation_summary: {e}")))?;

        Ok(id)
    }

    /// List the most recent conversation summaries.
    pub async fn list_conversation_summaries(&self, limit: i64) -> Result<Vec<ConversationSummary>> {
        let rows = sqlx::query!(
            r#"
            SELECT id, summary, topics, created_at
            FROM   conversation_summaries
            ORDER  BY created_at DESC
            LIMIT  ?
            "#,
            limit,
        )
        .fetch_all(&self.pool)
        .await
        .map_err(|e| Error::Database(format!("list_conversation_summaries: {e}")))?;

        let mut result = Vec::with_capacity(rows.len());
        for r in rows {
            let topics: Vec<String> =
                serde_json::from_str(&r.topics).unwrap_or_default();
            result.push(ConversationSummary {
                id: r.id,
                summary: r.summary,
                topics,
                created_at: r.created_at,
            });
        }
        Ok(result)
    }

    // ── Maintenance ────────────────────────────────────────────────────────────

    /// Delete all data from all tables.
    pub async fn clear_all(&self) -> Result<()> {
        sqlx::query!("DELETE FROM doc_cache").execute(&self.pool).await
            .map_err(|e| Error::Database(format!("clear doc_cache: {e}")))?;
        sqlx::query!("DELETE FROM facts").execute(&self.pool).await
            .map_err(|e| Error::Database(format!("clear facts: {e}")))?;
        sqlx::query!("DELETE FROM tool_usage").execute(&self.pool).await
            .map_err(|e| Error::Database(format!("clear tool_usage: {e}")))?;
        sqlx::query!("DELETE FROM conversation_summaries").execute(&self.pool).await
            .map_err(|e| Error::Database(format!("clear summaries: {e}")))?;
        Ok(())
    }

    /// Evict all expired doc cache entries.
    pub async fn prune_expired_docs(&self) -> Result<u64> {
        let now = Utc::now().to_rfc3339();
        let res = sqlx::query!("DELETE FROM doc_cache WHERE expires_at <= ?", now)
            .execute(&self.pool)
            .await
            .map_err(|e| Error::Database(format!("prune_expired_docs: {e}")))?;
        Ok(res.rows_affected())
    }

    /// Return the number of rows in each table as a diagnostic summary.
    pub async fn stats(&self) -> Result<MemoryStats> {
        let facts = sqlx::query_scalar!("SELECT COUNT(*) FROM facts")
            .fetch_one(&self.pool).await
            .map_err(|e| Error::Database(format!("stats facts: {e}")))?;
        let doc_cache = sqlx::query_scalar!("SELECT COUNT(*) FROM doc_cache")
            .fetch_one(&self.pool).await
            .map_err(|e| Error::Database(format!("stats doc_cache: {e}")))?;
        let tool_usage = sqlx::query_scalar!("SELECT COUNT(*) FROM tool_usage")
            .fetch_one(&self.pool).await
            .map_err(|e| Error::Database(format!("stats tool_usage: {e}")))?;
        let summaries = sqlx::query_scalar!("SELECT COUNT(*) FROM conversation_summaries")
            .fetch_one(&self.pool).await
            .map_err(|e| Error::Database(format!("stats summaries: {e}")))?;

        Ok(MemoryStats {
            facts: facts as u64,
            doc_cache: doc_cache as u64,
            tool_usage: tool_usage as u64,
            conversation_summaries: summaries as u64,
        })
    }
}

/// Diagnostic statistics for the memory store.
#[derive(Debug, Clone)]
pub struct MemoryStats {
    pub facts: u64,
    pub doc_cache: u64,
    pub tool_usage: u64,
    pub conversation_summaries: u64,
}
