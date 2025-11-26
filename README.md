# satsuki: PowerDNS frontend for managing subdomains

A Rust-based web frontend for **delegating and managing subdomains** under a configured base domain using **PowerDNS**.
Users can register a subdomain, authenticate using Basic Auth, and manage DNS records through a simple API (to be consumed by a TypeScript web UI).

This project contains:

* A **backend** (`Rust`, `axum`, `tokio`)
* A **PowerDNS integration layer**
* A **SQLite database** containing only user metadata
* A **TypeScript-friendly API** for a frontend
* A **builder pattern** enabling embedding into other binaries

---

## Features

### 🟦 Subdomain registration

Users select a subdomain (e.g., `alice`) → the system provisions:

* A **zone** on the subdomain PowerDNS instance
* A **NS delegation** in the base PowerDNS instance
* A user row in SQLite (with Argon2 password hash)

### 🟦 Authentication using Basic Auth

* Username = the subdomain name
* Password = user-chosen
* Frontend stores credentials in `sessionStorage`
* All API calls use `Authorization: Basic …`

### 🟦 DNS Record Management

Users can read/write DNS RRsets for **their zone only**.
NS at the apex is protected and controlled only via NS-mode endpoints.

### 🟦 Switchable NS Mode

Two modes:

* **Internal NS mode** → subdomain resolves using configured internal nameservers
* **External NS mode** → user provides third-party NS records; hosted zone remains editable but inactive until internal mode is restored

### 🟦 Minimal & secure persistence

Only SQLite contains:

* User/subdomain name
* Argon2 password hash
* External NS settings (if any)

All DNS data stays in PowerDNS.

---

## Architecture

### Components

* **Rust (server)**

  * `axum` web server
  * `sqlx` SQLite backend
  * `reqwest` PowerDNS API client
  * `argon2` password hashing
  * `thiserror`, `anyhow` for errors
  * `regex` + custom validation

* **PowerDNS**

  * One “base” PDNS instance → manages the *parent* zone
  * One “subdomain” PDNS instance → manages each user zone

---

## Configuration

`satsuki-pdns-frontend` uses a flexible builder pattern that is configured from the CLI.

### Example invocation

```sh
satsuki-pdns-frontend \
  --base-domain example.com \
  --db-path ./data/users.sqlite \
  --listen 0.0.0.0:8080 \
  --base-pdns-url http://127.0.0.1:8081/api/v1 \
  --base-pdns-key secret123 \
  --base-pdns-server-id localhost \
  --sub-pdns-url http://127.0.0.1:8082/api/v1 \
  --sub-pdns-key otherkey456 \
  --sub-pdns-server-id localhost \
  --internal-ns ns1.example.net. \
  --internal-ns ns2.example.net.
```

Notes:

* `--base-domain example.com` (without trailing dot)
* Internal NS names must be FQDNs; they are forced to end with `.`

---

## Subdomain Label Rules

User-provided subdomain labels (e.g., `alice` in `alice.example.com`) must satisfy:

* `[a-z0-9-]` only
* Length 1–63
* Cannot start/end with `-`
* Cannot contain `--`
* ASCII only

These rules avoid ambiguous DNS behavior and ensure safety.

---

## API Overview

All API endpoints return JSON.

### Public Endpoints

#### `POST /api/signup`

Register a new subdomain.

```json
{
  "subdomain": "alice",
  "password": "secret"
}
```

#### `POST /api/signin`

Verifies user credentials.

```json
{
  "subdomain": "alice",
  "password": "secret"
}
```

#### `GET /api/subdomain/check?name=alice`

Returns whether subdomain is available.

### Authenticated Endpoints

These require:

```
Authorization: Basic base64("subdomain:password")
```

#### `GET /api/zone`

Returns RRsets (except protected apex NS).

#### `PUT /api/zone`

Replaces all RRsets for the zone.

```json
{
  "records": [
    { "name": "www.alice.example.com.", "rrtype": "A", "ttl": 300, "content": "203.0.113.5" }
  ]
}
```

#### `POST /api/ns-mode/internal`

Switch back to internal NS.

#### `POST /api/ns-mode/external`

```json
{
  "ns": ["ns1.custom-dns.com.", "ns2.custom-dns.com."]
}
```

#### `GET /api/profile`

Returns user settings:

```json
{
  "subdomain": "alice",
  "external_ns": false,
  "external_ns1": null,
  "external_ns2": null
}
```

---

## Database Schema

`migrations/0001_init.sql`:

```sql
CREATE TABLE IF NOT EXISTS users (
  id              INTEGER PRIMARY KEY AUTOINCREMENT,
  subdomain       TEXT NOT NULL UNIQUE,
  password_hash   TEXT NOT NULL,
  external_ns     INTEGER NOT NULL DEFAULT 0,
  external_ns1    TEXT,
  external_ns2    TEXT,
  created_at      TEXT NOT NULL,
  updated_at      TEXT NOT NULL,
  last_login_at   TEXT
);
```

---

## Development Setup

### Requirements

* Rust stable
* SQLite3
* PowerDNS running (2 instances ideally)
* `sqlx-cli` (optional for migrations)

### Running in dev mode

```sh
cargo run -- --base-domain example.com --db-path ./dev.sqlite ...
```

### Database migrations

With `sqlx-cli`:

```sh
sqlx migrate run
```

Or rely on:

```rust
sqlx::migrate!().run(&pool)
```

which runs automatically on startup.

---

## PowerDNS Requirements

### Base PowerDNS instance must contain:

* The **parent zone** for your base domain (`example.com.`)
* Accessible via API (`/servers/{id}/zones/...`)

### Subdomain PowerDNS instance must allow:

* `POST /servers/{id}/zones` to create user zones
* `PATCH /servers/{id}/zones/{zone}` to modify RRsets

---

## Security Notes

* Always serve over **HTTPS**
* Use **Argon2** password hashing (already implemented)
* Never send PowerDNS API keys to the frontend
* Basic Auth is safe **only over HTTPS**
* User-submitted hostnames validated strictly

---

## Future Enhancements

* Web UI (TypeScript, not included here)
* Metrics / health endpoint
* Zone cloning / templates
* Audit logging
* Account deletion workflow

---

## License
Apache-2.0 or MPL-2.0.
