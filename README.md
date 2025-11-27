# satsuki: PowerDNS frontend for managing subdomains

A Rust-based web frontend for **delegating and managing subdomains** under a configured base domain using **PowerDNS**.
Users can register a subdomain, authenticate using Basic Auth, and manage DNS records through both a JSON API **and** the bundled React/Vite frontend under `frontend/`.

This project contains:

* A **backend** (`Rust`, `axum`, `tokio`)
* A **PowerDNS integration layer**
* A **SQLite database** containing only user metadata
* A **TypeScript frontend** (React + Vite) that consumes the API
* A **builder-friendly CLI** (`satsuki-pdns-frontend`) for embedding or standalone use

---

## Quick installation

```bash
# with Rustup
cargo install satsuki
```

Or download pre-built binaries from [Releases](https://github.com/metastable-void/satsuki/releases).

## Features

### 🟦 Subdomain registration

Users select a subdomain (e.g., `alice`) → the system provisions:

* A **zone** on the subdomain PowerDNS instance
* A **NS delegation** in the base PowerDNS instance
* A user row in SQLite (with Argon2 password hash)

### 🟦 Authentication using Basic Auth

* Username = the subdomain name
* Password = user-chosen
* Frontend stores credentials in `localStorage`
* All API calls use `Authorization: Basic …`

### 🟦 DNS Record Management

Users can read/write DNS RRsets for **their zone only**. The UI automatically normalizes relative owners, decodes IDNs for display, and punycodes them again on save. NS at the apex is protected and controlled only via NS-mode endpoints.

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

### Reserved Subdomain Names

A small set of infrastructure-friendly labels (e.g. `www`, `mail`, `ftp`, `smtp`, `email`) plus the RFC 2606/6761 special-use names (`example`, `invalid`, `localhost`, `test`) are blocked by default.  
Override or extend this list through `AppConfig::disallowed_subdomains` if you need different policies.

---

## API Overview

All API endpoints return JSON.

### Public Endpoints

#### `GET /health`

Returns `{"status":"ok"}` so load balancers and the bundled frontend can verify that the process is alive. This endpoint never touches the database or PowerDNS.

#### `POST /api/signup`

Registers a new subdomain. The payload must pass `validate_subdomain_name`, cannot appear in the disallowed list, and the password is Argon2-hashed before storage.

```json
{
  "subdomain": "alice",
  "password": "supers3cret"
}
```

When the request succeeds:

1. A zone is created on the sub-PDNS instance.
2. Apex NS + SOA RRsets inside that zone are replaced with the configured internal values.
3. The parent/base PDNS zone receives an NS delegation.
4. The SQLite row is inserted.

Failures during steps (2)–(4) trigger best-effort cleanup of both PDNS instances. Duplicate subdomains return HTTP 409.

#### `POST /api/signin`

Checks credentials and updates `last_login_at` when successful. Response body is `{"ok": true}` on success and `401` on failures (no session cookies are issued—the caller stores Basic Auth credentials).

```json
{
  "subdomain": "alice",
  "password": "supers3cret"
}
```

#### `GET /api/subdomain/check?name=<label>`

Validates the label and reports availability:

```json
{ "available": true }
```

Reserved labels (e.g. `www`, `mail`, `localhost`, …​) are treated as unavailable even if they are not in the database.

#### `GET /api/about`

Returns basic metadata for the deployment:

```json
{ "base_domain": "example.com" }
```

#### `GET /api/subdomain/list`

Fetches the NS RRsets from the base PowerDNS zone and groups them by owner name (including the apex entry). Example response:

```json
[
  {
    "name": "example.com.",
    "records": ["ns1.example.net.", "ns2.example.net."]
  },
  {
    "name": "custom.example.com.",
    "records": ["ns1.custom-dns.com.", "ns2.custom-dns.com."]
  }
]
```

#### `GET /api/subdomain/soa`

Returns the parent-zone SOA line used by the frontend’s BIND-style helper:

```json
{ "soa": "ns1.example.net. hostmaster.example.net. 2024010101 7200 900 1209600 300" }
```

#### `GET /metrics`

Exports Prometheus text metrics, currently `satsuki_subdomains_total`, which counts unique delegated subdomains (i.e., non-apex NS RRsets in the parent zone):

```
satsuki_subdomains_total 42
```

### Authenticated Endpoints

All authenticated endpoints require:

```
Authorization: Basic base64("subdomain:password")
```

#### `GET /api/zone`

Returns every RRset for the user’s zone **except** the apex NS RRset, which is managed by the NS-mode endpoints. Example:

```json
[
  {
    "name": "www.alice.example.com.",
    "rrtype": "A",
    "ttl": 300,
    "content": "203.0.113.5",
    "priority": null
  }
]
```

#### `PUT /api/zone`

Replaces the submitted RRsets. Records are grouped by `(name, rrtype)` and each group must share the same TTL. Apex NS and SOA changes are rejected to keep the NS-mode flow authoritative.

```json
{
  "records": [
    {
      "name": "www.alice.example.com.",
      "rrtype": "A",
      "ttl": 600,
      "content": "203.0.113.5",
      "priority": null
    }
  ]
}
```

#### `POST /api/ns-mode/internal`

Replaces the parent-zone delegation with the configured internal NS values and clears any stored external NS details in the database. Use this to “bring the zone home” after previously pointing it to third-party nameservers.

#### `POST /api/ns-mode/external`

Switches the parent-zone delegation to user-provided nameservers. The payload must contain 1–6 FQDNs that end with a dot:

```json
{
  "ns": ["ns1.custom-dns.com.", "ns2.custom-dns.com."]
}
```

The accepted NS list is stored in SQLite so the UI can reflect the user’s current configuration.

#### `GET /api/profile`

Returns the logged-in user’s metadata:

```json
{
  "subdomain": "alice",
  "external_ns": false,
  "external_ns1": null,
  "external_ns2": null,
  "external_ns3": null,
  "external_ns4": null,
  "external_ns5": null,
  "external_ns6": null
}
```

#### `POST /api/password/change`

Allows a logged-in user to rotate their password without re-registering. Requires the current password and a new secret (minimum 8 characters):

```json
{
  "current_password": "supers3cret",
  "new_password": "evenB3tter!"
}
```

Invalid current passwords return `401`; successful changes return `{"ok": true}`.

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
  external_ns3    TEXT,
  external_ns4    TEXT,
  external_ns5    TEXT,
  external_ns6    TEXT,
  created_at      TEXT NOT NULL,
  updated_at      TEXT NOT NULL,
  last_login_at   TEXT
);
```

---

## Development Setup

### Requirements

* Rust stable
* PowerDNS running (2 instances ideally)
* `sqlx-cli` (optional for migrations)
* Node.js 18+ (for the React/Vite frontend under `frontend/`) (Optional)

### Frontend (React + Vite) (Optional)

```sh
npm install
npm run dev     # serves frontend from ./frontend
npm run build   # emits static assets into ./dist
```

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

* Metrics / deeper health reporting
* Zone cloning / templates
* Audit logging
* Account deletion workflow

---

## License
Apache-2.0 or MPL-2.0.
