CREATE TABLE IF NOT EXISTS developers (
  id INTEGER PRIMARY KEY AUTOINCREMENT,
  username TEXT NOT NULL UNIQUE,
  email TEXT NOT NULL UNIQUE,
  signing_public_key TEXT NOT NULL,
  created_at TEXT NOT NULL
);

CREATE TABLE IF NOT EXISTS api_keys (
  api_key TEXT PRIMARY KEY,
  developer_id INTEGER NOT NULL,
  created_at TEXT NOT NULL,
  FOREIGN KEY(developer_id) REFERENCES developers(id)
);

CREATE TABLE IF NOT EXISTS packages (
  id INTEGER PRIMARY KEY AUTOINCREMENT,
  name TEXT NOT NULL UNIQUE,
  owner_developer_id INTEGER NOT NULL,
  description TEXT NOT NULL,
  created_at TEXT NOT NULL,
  FOREIGN KEY(owner_developer_id) REFERENCES developers(id)
);

CREATE TABLE IF NOT EXISTS package_versions (
  id INTEGER PRIMARY KEY AUTOINCREMENT,
  package_id INTEGER NOT NULL,
  version TEXT NOT NULL,
  nyx_toml TEXT NOT NULL,
  source_sha256 TEXT NOT NULL,
  signature TEXT NOT NULL,
  verified INTEGER NOT NULL,
  verification_report TEXT NOT NULL,
  published_at TEXT NOT NULL,
  UNIQUE(package_id, version),
  FOREIGN KEY(package_id) REFERENCES packages(id)
);

CREATE TABLE IF NOT EXISTS package_dependencies (
  id INTEGER PRIMARY KEY AUTOINCREMENT,
  package_version_id INTEGER NOT NULL,
  dep_name TEXT NOT NULL,
  dep_req TEXT NOT NULL,
  FOREIGN KEY(package_version_id) REFERENCES package_versions(id)
);

CREATE TABLE IF NOT EXISTS package_tags (
  id INTEGER PRIMARY KEY AUTOINCREMENT,
  package_version_id INTEGER NOT NULL,
  tag TEXT NOT NULL,
  FOREIGN KEY(package_version_id) REFERENCES package_versions(id)
);

CREATE TABLE IF NOT EXISTS build_artifacts (
  id INTEGER PRIMARY KEY AUTOINCREMENT,
  package_version_id INTEGER NOT NULL,
  target TEXT NOT NULL,
  artifact_sha256 TEXT NOT NULL,
  build_status TEXT NOT NULL,
  created_at TEXT NOT NULL,
  FOREIGN KEY(package_version_id) REFERENCES package_versions(id)
);

CREATE TABLE IF NOT EXISTS package_downloads (
  id INTEGER PRIMARY KEY AUTOINCREMENT,
  package_id INTEGER NOT NULL,
  version TEXT NOT NULL,
  downloaded_at TEXT NOT NULL,
  FOREIGN KEY(package_id) REFERENCES packages(id)
);

CREATE TABLE IF NOT EXISTS mirrors (
  id INTEGER PRIMARY KEY AUTOINCREMENT,
  region TEXT NOT NULL,
  endpoint TEXT NOT NULL UNIQUE,
  health TEXT NOT NULL,
  last_sync_at TEXT NOT NULL
);
