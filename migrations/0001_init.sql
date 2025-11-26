-- migrations/0001_init.sql
CREATE TABLE IF NOT EXISTS users (
  id              INTEGER PRIMARY KEY AUTOINCREMENT,
  subdomain       TEXT NOT NULL UNIQUE,
  password_hash   TEXT NOT NULL,
  external_ns     INTEGER NOT NULL DEFAULT 0,
  external_ns1    TEXT,
  external_ns2    TEXT,
  external_ns3    TEXT,
  external_ns4    TEXT,
  external_ns5    TEXT,
  external_ns6    TEXT,
  created_at      TEXT NOT NULL,
  updated_at      TEXT NOT NULL,
  last_login_at   TEXT
);
