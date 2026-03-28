CREATE TABLE IF NOT EXISTS projects (
  id INTEGER PRIMARY KEY AUTOINCREMENT,
  name TEXT NOT NULL,
  summary TEXT NOT NULL,
  tags TEXT NOT NULL,
  maintainer_alias TEXT NOT NULL,
  created_at TEXT NOT NULL
);

CREATE TABLE IF NOT EXISTS contributors (
  id INTEGER PRIMARY KEY AUTOINCREMENT,
  alias TEXT NOT NULL,
  skills TEXT NOT NULL,
  contact_hint TEXT NOT NULL,
  created_at TEXT NOT NULL
);

CREATE TABLE IF NOT EXISTS issues (
  id INTEGER PRIMARY KEY AUTOINCREMENT,
  title TEXT NOT NULL,
  normalized_pattern TEXT NOT NULL,
  solution TEXT NOT NULL,
  tags TEXT NOT NULL,
  confidence REAL NOT NULL,
  created_at TEXT NOT NULL
);

CREATE TABLE IF NOT EXISTS insights (
  id INTEGER PRIMARY KEY AUTOINCREMENT,
  insight_type TEXT NOT NULL,
  pattern_hash TEXT NOT NULL,
  recommendation TEXT NOT NULL,
  impact_score REAL NOT NULL,
  source_anonymous_id TEXT NOT NULL,
  created_at TEXT NOT NULL
);

CREATE TABLE IF NOT EXISTS knowledge_resources (
  id INTEGER PRIMARY KEY AUTOINCREMENT,
  kind TEXT NOT NULL,
  title TEXT NOT NULL,
  body TEXT NOT NULL,
  tags TEXT NOT NULL,
  author_alias TEXT NOT NULL,
  created_at TEXT NOT NULL
);
