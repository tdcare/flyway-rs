# flyway-rs

[![Crates.io](https://img.shields.io/crates/v/flyway.svg)](https://crates.io/crates/flyway)
[![License: MIT](https://img.shields.io/badge/license-MIT-blue.svg)](LICENSE)

[中文文档](README_CN.md)

`flyway-rs` is a collection of Rust crates for loading and executing database migrations, inspired by [Flyway](https://flywaydb.org/) in Java.

It was created as an alternative to [refinery](https://github.com/rust-db/refinery), because `refinery` has a closed driver architecture — the `refinery::Migration::applied(...)` method is not public, preventing external crates from implementing the `refinery::AsyncMigrate` trait. See [this issue](https://github.com/rust-db/refinery/issues/248) for more details.

## Features

- **Multi-database support**: MySQL, PostgreSQL, SQLite, MSSql, TDengine
- **Compile-time migration embedding**: SQL files are parsed and embedded into the binary at compile time via procedural macros — no runtime file I/O needed
- **Runtime migration loading**: Dynamically load SQL migration scripts from filesystem at runtime using `RuntimeMigrationStore` — perfect for development and flexible deployment scenarios
- **Version-based migration management**: Automatic tracking of applied migrations with version state management
- **Transaction support**: Each migration runs within its own database transaction, with automatic rollback on failure
- **Fail-continue mode**: Optionally continue executing subsequent migrations when a single migration fails
- **SQL statement annotations**: Support `--! may_fail: true` annotations to allow individual SQL statements to fail without aborting the migration
- **Multi-database runtime dispatch**: Dynamically select migration scripts based on the detected database type at runtime
- **Pluggable driver architecture**: Easy to implement custom database drivers via the `MigrationStateManager` and `MigrationExecutor` traits

## Crate Structure

| Crate | Description |
|---|---|
| [`flyway`](https://crates.io/crates/flyway) | Core crate. Contains the migration runner, traits (`MigrationStore`, `MigrationStateManager`, `MigrationExecutor`), and re-exports macros from sub-crates. |
| [`flyway-rbatis`](https://crates.io/crates/flyway-rbatis) | Database driver implementation using [Rbatis](https://github.com/rbatis/rbatis). Supports MySQL, PostgreSQL, SQLite, MSSql, and TDengine. |
| [`flyway-codegen`](https://crates.io/crates/flyway-codegen) | Procedural macro crate. Provides the `#[migrations(...)]` attribute macro for compile-time migration loading. |
| [`flyway-sql-changelog`](https://crates.io/crates/flyway-sql-changelog) | SQL file parsing library. Handles SQL splitting, checksum calculation, and statement annotation parsing. |

## Quick Start

### 1. Add Dependencies

```toml
[dependencies]
flyway = "0.7.0"
flyway-rbatis = "0.7.0"
rbatis = "4.9"
rbdc-mysql = "4.9"    # or rbdc-pg, etc.
tokio = { version = "1", features = ["full"] }
```

### 2. Create Migration Files

Place SQL files in a directory following the naming convention `V<version>_<description>.sql`:

```
migrations/
├── V1_Create_DeviceData.sql
├── V2_Create_PatientUseDevice.sql
├── V3__Create_VitalSign.sql
└── V6__Add_PatientNo.sql
```

Each file can contain multiple SQL statements separated by semicolons.

### 3. Write Migration Code

```rust
use std::sync::Arc;
use rbatis::RBatis;
use rbdc_mysql::driver::MysqlDriver;
use flyway::{MigrationRunner, MigrationsError, migrations};
use flyway_rbatis::RbatisMigrationDriver;

// Load migration SQL files at compile time
#[migrations("migrations/mysql/")]
pub struct Migrations {}

async fn run(rb: Arc<RBatis>) -> Result<(), MigrationsError> {
    let driver = Arc::new(RbatisMigrationDriver::new(rb.clone(), None));
    let runner = MigrationRunner::new(
        Migrations {},
        driver.clone(),
        driver.clone(),
        true, // fail_continue: continue on migration failure
    );
    runner.migrate().await?;
    Ok(())
}

#[tokio::main]
async fn main() {
    let rb = RBatis::new();
    rb.init(MysqlDriver {}, "mysql://root:123456@localhost:3306/test").unwrap();
    run(Arc::new(rb)).await.expect("Migration failed");
}
```

## Multi-Database Support

`flyway-rs` supports runtime database type detection and automatic migration script dispatch. Organize migration scripts by database dialect:

```
migrations/
├── mysql/
│   ├── V1_Create_DeviceData.sql
│   └── V2_Create_PatientUseDevice.sql
├── postgres/
│   ├── V1_Create_DeviceData.sql
│   └── V2_Create_PatientUseDevice.sql
└── taos/
    ├── V1_Create_DeviceData.sql
    └── V2_Create_PatientUseDevice.sql
```

Define separate migration stores for each database type:

```rust
use flyway::migrations;

#[migrations("migrations/mysql/")]
pub struct MysqlMigrations {}

#[migrations("migrations/postgres/")]
pub struct PgMigrations {}

#[migrations("migrations/taos/")]
pub struct TaosMigrations {}
```

Then use `RbatisMigrationDriver::driver_type()` to detect the database at runtime and select the appropriate migration store. See the [multi_db example](example/src/multi_db.rs) for a complete implementation.

## Compile-time Embedding vs Runtime Loading

flyway-rs supports two ways to load migration scripts, which can be flexibly chosen according to the scenario:

### Compile-time Embedding (`#[migrations]` macro)

```rust
#[migrations("migrations/mysql/")]
pub struct Migrations {}
```

**Advantages**:
- ⚡ Faster startup (no filesystem I/O)
- 🔒 Higher security (SQL cannot be tampered with)
- 📦 Single-file deployment, simplified operations
- ✅ Compile-time validation of file format

**Use Cases**:
- Production environment deployment
- Embedded devices or containerized applications
- Migration scripts version-controlled with code
- Scenarios with high startup performance requirements

### Runtime Loading (`RuntimeMigrationStore`)

```rust
let store = RuntimeMigrationStore::new("migrations/mysql");
```

**Advantages**:
- 🔄 Update migrations without recompilation
- 🎯 Dynamically select migration directory based on environment
- 🔧 Facilitates external system management of migration scripts
- 💡 More flexible for development and debugging

**Use Cases**:
- Rapid iteration in development environments
- Centralized migration management in microservice architectures
- A/B testing different migration strategies
- Migration scripts generated by external systems

### Hybrid Mode

You can automatically switch modes based on environment variables:

```rust
fn create_store() -> Box<dyn MigrationStore> {
    if std::env::var("USE_RUNTIME").unwrap_or_default() == "true" {
        Box::new(RuntimeMigrationStore::new("migrations/"))
    } else {
        Box::new(CompileTimeMigrations {})
    }
}
```

For a complete example, see [`example/src/hybrid_mode.rs`](example/src/hybrid_mode.rs).

## SQL Annotations

You can annotate SQL statements using special comments to control error handling:

```sql
--! may_fail: true
CREATE INDEX idx_patient_no ON VitalSign(patient_no);
```

When `may_fail: true` is set, the migration will continue even if that specific statement fails.

## Migration State Table

`flyway-rs` automatically creates a migration tracking table (default name: `flyway_migrations`) in your database to track which versions have been applied. The table schema adapts to each database dialect automatically.

## Running Tests

```sh
cd flyway
cargo test
```

## Examples

See the [`example`](example/) directory for complete working examples:

- [`mysql.rs`](example/src/mysql.rs) — MySQL migration example
- [`taos.rs`](example/src/taos.rs) — TDengine migration example
- [`multi_db.rs`](example/src/multi_db.rs) — Multi-database runtime dispatch example
- [`runtime_mysql.rs`](example/src/runtime_mysql.rs) — Runtime loading example
- [`hybrid_mode.rs`](example/src/hybrid_mode.rs) — Hybrid mode example (auto-select based on environment variable)

### Runtime Loading Examples

```bash
# Load migration scripts from directory at runtime
cargo run --bin runtime_mysql

# Hybrid mode: auto-select based on environment variable
cargo run --bin hybrid_mode
USE_RUNTIME_MIGRATIONS=true cargo run --bin hybrid_mode
```

## License

This project is licensed under the [MIT License](LICENSE).
