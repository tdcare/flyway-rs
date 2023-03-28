# flyway-rs copy from db-up

`db-up` is a collection of Rust crates for loading and executing database
migrations.

It supposed to be an alternative to [refinery](https://github.com/rust-db/refinery)
and was created because `refinery` is pretty closed when it comes to database drivers. Basically
it is not possible to create database driver crates for `refinery` without creating either a
fork or including the driver inside the `refinery` crate. The reason is that the
`refinery::Migration::applied(...)` method is not public, which prevents other crates from implementing
the `refinery::AsyncMigrate` trait and reading [this issue](https://github.com/rust-db/refinery/issues/248)
it seems the authors are not motivated to change this behaviour.

`db-up` consists of multiple crates:
* Top-level crates:
    * `db-up`: The main crate. Contains the migration runner and re-exports necessary
      macros and structs from other db-up crates.
    * `db-up-rbatis`: A driver for executing DB migrations via the
      [Rbatis](https://github.com/rbatis/rbatis) database library.
* Other crates:
    * `db-up-codegen`: Contains the `migrations` attribute macro
    * `db-up-sql-changelog`: Contains the `ChangelogFile` struct that can load
      SQL files and split them into separate, annotated statements
      via a `SqlStatementIterator`.

## Status

This crate has some known (and probably some unknown) limitations and stability issues:

* The transaction management is not finished yet. At the moment, only a
  "one transaction per changelog" mode is implemented, but no "one transaction for all changes"
  mode. I'm not sure if anyone will need the latter, but it i plan to implement it at some point.
* The "last successful version" is not set correctly at many places, especially when producing
  errors.
* The `iter()` implementation for `ChangelogFile` is not conforming to the Rust standards
  yet.
* For now, there is only an Rbatis driver implementation available.
* The Rbatis driver in `db-up-rbatis` uses one set of queries for all database drivers supported
  by Rbatis. As far as i can tell from e.g. `refinery`, some database systems (specifically MSSQL)
  support or even need a different syntax for state management.
* More examples should be added.
* More tests should be added.

## Usage

All the crates in this project are libraries. The included tests can be started via:

```sh
~$ cd db-up
~/db-up$ cargo test
```

To use the crates inside your project, the following steps should be taken:

1. Include the necessary crates in your `Cargo.toml` (get available versions
   from [crates.io](https://crates.io/crates/db-up)):
```toml
# Add the db-up dependency
[dependency.db-up]
version = "<version>"

# Add the db-up-rbatis dependency in order to run migrations via Rbatis. At the time
# of writing, this is the only supported database driver.
[dependency.db-up-rbatis]
version = "<version>"

# Add Rbatis dependencies ...
```
2. E.g in your `main.rs`:
```rust
use db_up::{MigrationExecutor, MigrationState, MigrationStateManager, MigrationStore, migrations, MigrationRunner};
use db_up_rbatis::RbatisMigrationDriver;
use rbatis::Rbatis;

// Load migrations (SQL files) from `examples/migrations` and make them available via
// `Migrations::changelog()`. The generated class can be used for `MigrationRunner::migrate(...)`.
#[migrations("examples/migrations")]
pub struct Migrations {
}

async fn run(rbatis: Arc<Rbatis>) -> Result<()> {
    let migration_driver = Arc::new(RbatisMigrationDriver::new(rbatis.clone(), None));
    let migration_runner = MigrationRunner::new(
        Migrations {},
        migration_driver.clone(),
        migration_driver.clone()
    );
    migration_runner.migrate().await?;
}

// Add main method that creates an `Rbatis` instance and calls the `run(...)` method.
// ...

```

# License

The project is licensed under the [BSD 3-clause license](LICENSE.txt).