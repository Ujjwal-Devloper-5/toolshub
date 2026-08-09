-- ── toolshub-memory: initial schema ──────────────────────────────────────────

-- facts: arbitrary key/value knowledge store, partitioned by category
CREATE TABLE IF NOT EXISTS facts (
    id          TEXT    NOT NULL PRIMARY KEY,
    category    TEXT    NOT NULL,
    key         TEXT    NOT NULL,
    value       TEXT    NOT NULL,
    confidence  REAL    NOT NULL DEFAULT 1.0,
    created_at  TEXT    NOT NULL,
    updated_at  TEXT    NOT NULL,
    UNIQUE (category, key)
);

CREATE INDEX IF NOT EXISTS idx_facts_category     ON facts (category);
CREATE INDEX IF NOT EXISTS idx_facts_category_key ON facts (category, key);

-- ─────────────────────────────────────────────────────────────────────────────

-- tool_usage: records every invocation of a terminal tool
CREATE TABLE IF NOT EXISTS tool_usage (
    id          TEXT     NOT NULL PRIMARY KEY,
    tool_name   TEXT     NOT NULL,
    command     TEXT     NOT NULL,
    directory   TEXT     NOT NULL,
    exit_code   INTEGER  NOT NULL,
    used_at     TEXT     NOT NULL
);

CREATE INDEX IF NOT EXISTS idx_tool_usage_tool_name ON tool_usage (tool_name);
CREATE INDEX IF NOT EXISTS idx_tool_usage_used_at   ON tool_usage (used_at DESC);

-- ─────────────────────────────────────────────────────────────────────────────

-- conversation_summaries: compressed snapshots of past AI sessions
CREATE TABLE IF NOT EXISTS conversation_summaries (
    id          TEXT NOT NULL PRIMARY KEY,
    summary     TEXT NOT NULL,
    topics      TEXT NOT NULL,   -- JSON array of topic strings
    created_at  TEXT NOT NULL
);

CREATE INDEX IF NOT EXISTS idx_conv_summaries_created_at ON conversation_summaries (created_at DESC);

-- ─────────────────────────────────────────────────────────────────────────────

-- doc_cache: cached man-page / help-text content for tools
CREATE TABLE IF NOT EXISTS doc_cache (
    id          TEXT NOT NULL PRIMARY KEY,
    tool_name   TEXT NOT NULL UNIQUE,
    content     TEXT NOT NULL,
    source      TEXT NOT NULL,
    cached_at   TEXT NOT NULL,
    expires_at  TEXT NOT NULL
);

CREATE INDEX IF NOT EXISTS idx_doc_cache_tool_name  ON doc_cache (tool_name);
CREATE INDEX IF NOT EXISTS idx_doc_cache_expires_at ON doc_cache (expires_at);

-- ─────────────────────────────────────────────────────────────────────────────

-- package_index: all packages known to local package managers
CREATE TABLE IF NOT EXISTS package_index (
    id          TEXT NOT NULL PRIMARY KEY,
    name        TEXT NOT NULL,
    version     TEXT NOT NULL,
    manager     TEXT NOT NULL,
    path        TEXT NOT NULL,
    indexed_at  TEXT NOT NULL,
    UNIQUE (name, manager)
);

CREATE INDEX IF NOT EXISTS idx_package_index_name    ON package_index (name);
CREATE INDEX IF NOT EXISTS idx_package_index_manager ON package_index (manager);

-- ─────────────────────────────────────────────────────────────────────────────

-- learning_progress: tracks the user's progress studying individual tools
CREATE TABLE IF NOT EXISTS learning_progress (
    id              TEXT     NOT NULL PRIMARY KEY,
    tool_name       TEXT     NOT NULL UNIQUE,
    level           TEXT     NOT NULL,
    exercises_done  INTEGER  NOT NULL DEFAULT 0,
    last_studied    TEXT     NOT NULL
);

CREATE INDEX IF NOT EXISTS idx_learning_progress_tool_name ON learning_progress (tool_name);
