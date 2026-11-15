---
title: "Sync Server API (draft)"
description: "The record-store API a self-hosted Reach sync server would implement, matching what the sync engine already does with Turso."
---

Reach's sync engine (`src-tauri/src/vault/sync_engine.rs`) talks to two kinds of backend. A **record store** keeps one sealed record per secret and a sequence number, so a device can ask for only what changed. A **blob store** keeps one sealed file per vault. Turso is a record store today. A self-hosted Reach sync server would be a record store too, and this page describes the API it would offer. No server exists yet.

The server never sees plaintext. A record is an opaque blob sealed by the client (XChaCha20-Poly1305 under a key derived from the vault's master key); its id is a keyed hash of the real one. The server only orders and stores them.

## Concepts

- **Vault**: identified by the vault id from its header. A vault has a header (the master key wrapped for its owner) and members (the master key wrapped for each member's public key), both opaque to the server apart from user ids.
- **Record**: `{ id: string, seq: integer, sealed: bytes }`. `id` is unique within a vault. `seq` is assigned by the server, strictly increasing within a vault.
- **Cursor**: the highest `seq` a device has seen.

## Endpoints

All requests are HTTPS, authenticated per device (a device key registered for the user, signing each request), and scoped to vaults the user is a member of.

| Method | Path | Does |
| --- | --- | --- |
| `GET` | `/v1/vaults/{vault}/header` | The header and member rows, as the client stored them |
| `PUT` | `/v1/vaults/{vault}/members/{user}` | Add or replace a member row (owner or admin only) |
| `DELETE` | `/v1/vaults/{vault}/members/{user}` | Remove a member, and revoke that user's access |
| `GET` | `/v1/vaults/{vault}/records?since={cursor}` | Every record with `seq > cursor`, in `seq` order, plus the new cursor |
| `POST` | `/v1/vaults/{vault}/records` | Push records, with the cursor the client merged up to |

### Push

Request:

```json
{ "base": 41, "records": [{ "id": "9f2c…", "sealed": "base64…" }] }
```

If the vault's highest `seq` is still `base`, the server stores each record (replacing any with the same `id`), gives them `base + 1`, `base + 2`, … in order, and answers `200` with `{ "cursor": 42 }`. Otherwise it stores nothing and answers `409`; the client pulls, merges, and pushes again. This is exactly what the Turso backend does inside one transaction.

### Notifications (optional)

`GET /v1/vaults/{vault}/events` as a server-sent event stream that sends the new highest `seq` whenever it changes, so devices sync at once instead of every minute.

## Why this shape

- Revoking a member is real: the server stops serving the vault to that user, which a shared Turso token cannot do.
- The merge rules stay entirely on the client, so the server is small and has nothing to get wrong about conflicts.
- A server that rolls a record back is caught: the version inside each sealed record only grows, and clients refuse an older one.
