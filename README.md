# clove1db

A lightweight embedded database framework for Rust — built on redb with layered cache, versioned backup, and domain-driven storage.

## Features

- 🗄️ **Embedded Storage**: Built on [redb](https://github.com/cberner/redb) — no external server needed
- ⚡ **Layered Cache**: In-memory cache via [moka](https://github.com/moka-rs/moka) with TTL and idle expiry
- 🔁 **Versioned Backup**: Every write/delete is recorded — restore any entity to any previous version
- 🧩 **Domain-Driven**: Clean separation via `Entity`, `InputDto`, `OutputDto`, `Repository`, `Domain`
- 📦 **Multi-Database**: Multiple isolated DB files in a single `Storage` instance
- 📡 **Event System**: Built-in event emitter with `on_info`, `on_warn`, `on_error` hooks
- 🦀 **Async**: Fully async via [tokio](https://tokio.rs)

## Install

```toml
[dependencies]
clove1db = "0.0.7"
```

## Quick Start

```rust
use clove1db::{
    storage::{DatabaseConfig, Storage, StorageConfig},
    entity::Entity,
    dto::{InputDto, OutputDto},
    units::Result,
};
use serde::{Deserialize, Serialize};

// 1. Define your entity
#[derive(Debug, Clone, Serialize, Deserialize)]
struct User {
    id:   String,
    name: String,
}

impl Entity for User {
    fn entity_id(&self) -> &str { &self.id }
}

// 2. Build storage
let storage = Storage::builder(StorageConfig::default())
    .add_database(
        DatabaseConfig::new("users_db", "users")
            .cache(10_000, 300, 60)
            .register::<User>("users")
    )
    .build()
    .await?;

// 3. Use domain
let domain = storage.domain::<User>();

let user = domain.create::<CreateUserDto, UserResponse>(input).await?;
let found = domain.get::<UserResponse>(&user.id).await?;
let list  = domain.list::<UserResponse>().await?;
domain.delete(&user.id).await?;
```

## Backup & Versioning

```rust
// Every write is recorded automatically
domain.update::<CreateUserDto, UserResponse>(&id, input).await?;

// View history
let bm      = storage.db_manager("users_db").backup_manager.as_ref().unwrap();
let history = bm.history(TableDefinition::new("users"), &id)?;

// Restore to specific version
domain.restore_by_version(&id, 1).await?;

// Restore to a point in time
domain.restore_at(&id, timestamp_ms).await?;
```

## Multi-Database

```rust
let storage = Storage::builder(StorageConfig::default())
    // DB 1 — default path (next to exe)
    .add_database(
        DatabaseConfig::new("users_db", "users")
            .register::<User>("users")
    )
    // DB 2 — custom path + backup enabled
    .add_database(
        DatabaseConfig::new("catalog_db", "catalog")
            .dir_path(PathBuf::from("./data"))
            .backup_enabled(true)
            .register::<Product>("products")
            .register::<Order>("orders")
    )
    .build()
    .await?;
```

## License

Licensed under either of

- MIT license ([LICENSE-MIT](LICENSE-MIT))