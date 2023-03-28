use std::cell::Cell;
use std::sync::{Arc};
use tokio::sync::Mutex;

use rbatis::Rbatis;
use flyway::{MigrationExecutor, MigrationState, MigrationStateManager, MigrationsError, MigrationStatus};

use async_trait::async_trait;
use rbatis::executor::RBatisTxExecutor;

/// Default table name for the migration state management table
pub const DEFAULT_MIGRATIONS_TABLE: &str = "dbup_migrations";

/// Available driver types supported by Rbatis
pub enum RbatisDbDriverType {
    MySql,
    Pg,
    Sqlite,
    MsSql,
    Other(String),
}

/// Rbatis implementation of `MigrationStateManager` and `MigrationExecutor`
pub struct RbatisMigrationDriver {
    db: Arc<Rbatis>,
    migrations_table_name: String,
    tx: Mutex<Cell<Option<RBatisTxExecutor>>>,
}

impl RbatisMigrationDriver {
    /// Create a new driver
    ///
    ///  * `db`: The `Rbatis` instance for accessing the database
    ///  * `migrations_table_name`: The optional name of the table the migration state information
    ///    should be stored in. If `None`, the `DEFAULT_MIGRATIONS_TABLE` will be used.
    pub fn new(db: Arc<Rbatis>, migrations_table_name: Option<&str>) -> RbatisMigrationDriver {
        return RbatisMigrationDriver {
            db: db.clone(),
            migrations_table_name: migrations_table_name.map(|v| v.to_string())
                .or(Some(DEFAULT_MIGRATIONS_TABLE.to_string()))
                .unwrap(),
            tx: Mutex::new(Cell::new(None)),
        }
    }

    /// The the driver type of the `Rbatis` instance
    ///
    /// This method will get the driver type from `Rbatis` (which is a string) and convert it into
    /// an `RbatisDbDriverType`. `Other(String)` will be used for any database drivers not directly
    /// known to `db-up-rbatis`.
    pub fn driver_type(&self) -> rbatis::Result<RbatisDbDriverType> {
        let db = self.db.clone();
        let driver_type_name = db.driver_type()?;
        let result = match driver_type_name {
            "mssql" => RbatisDbDriverType::MsSql,
            "mysql" => RbatisDbDriverType::MySql,
            "postgres" => RbatisDbDriverType::Pg,
            "sqlite" => RbatisDbDriverType::Sqlite,
            _ => RbatisDbDriverType::Other(driver_type_name.to_string())
        };
        return Ok(result);
    }
}

/// Implementation of the `MigrationStateManager`
#[async_trait]
impl MigrationStateManager for RbatisMigrationDriver {
    async fn prepare(&self) -> flyway::Result<()> {
        println!("preparing migrations table ...");
        let db = self.db.clone();
        let mut db = db.acquire()
            .await
            .or_else(|err| Err(MigrationsError::migration_database_failed(None, Some(err.into()))))?;
        let statement = format!(
            r#"CREATE TABLE IF NOT EXISTS {} (
                version INTEGER PRIMARY KEY,
                status VARCHAR(16)
            );"#, self.migrations_table_name.as_str());
        println!("preparation statement: {}", statement.as_str());
        let _result = db.exec(statement.as_str(), vec![])
            .await
            .or_else(|err| Err(MigrationsError::migration_setup_failed(Some(err.into()))))?;
        println!("preparing migrations table ... done");
        return Ok(());
    }

    async fn lowest_version(&self) -> flyway::Result<Option<MigrationState>> {
        println!("retrieving lowest version ... ");
        let db = self.db.clone();
        let mut db = db.acquire()
            .await
            .or_else(|err| Err(MigrationsError::migration_database_failed(None, Some(err.into()))))?;
        let version: Option<u32> = db.query_decode(format!("SELECT MIN(version) FROM {} WHERE status='deployed';",
                                                           self.migrations_table_name.as_str()).as_str(), vec![])
            .await
            .or_else(|err| Err(MigrationsError::migration_versioning_failed(Some(err.into()))))?;

        println!("retrieving lowest version ... {:?}", &version);
        return Ok(version.and_then(|version|
            Some(MigrationState {
                version,
                status: MigrationStatus::Deployed
            })));
    }

    async fn highest_version(&self) -> flyway::Result<Option<MigrationState>> {
        println!("retrieving highest version ... ");
        let db = self.db.clone();
        let mut db = db.acquire()
            .await
            .or_else(|err| Err(MigrationsError::migration_database_failed(None, Some(err.into()))))?;
        let version: Option<u32> = db.query_decode(format!("SELECT MAX(version) FROM {} WHERE status='deployed';",
                                                           self.migrations_table_name.as_str()).as_str(), vec![])
            .await
            .or_else(|err| Err(MigrationsError::migration_versioning_failed(Some(err.into()))))?;

        println!("retrieving highest version ... {:?}", &version);
        return Ok(version.and_then(|version|
            Some(MigrationState {
                version,
                status: MigrationStatus::Deployed
            })));
    }

    async fn list_versions(&self) -> flyway::Result<Vec<MigrationState>> {
        println!("listing versions ... ");
        let db = self.db.clone();
        let mut db = db.acquire()
            .await
            .or_else(|err| Err(MigrationsError::migration_database_failed(None, Some(err.into()))))?;
        let versions: Vec<u32> = db.query_decode(format!("SELECT version FROM {} WHERE status='deployed' ORDER BY version asc;",
                                                         self.migrations_table_name.as_str()).as_str(), vec![])
            .await
            .or_else(|err| Err(MigrationsError::migration_versioning_failed(Some(err.into()))))?;

        let versions: Vec<MigrationState> = versions.iter()
            .map(|version|
                MigrationState {
                    version: *version,
                    status: MigrationStatus::Deployed
                })
            .collect();

        println!("listing versions ... {:?}", &versions);
        return Ok(versions);
    }

    async fn begin_version(&self, version: u32) -> flyway::Result<()> {
        println!("beginning version ... {}", version);
        let db = self.db.clone();
        let mut db = db.acquire()
            .await
            .or_else(|err| Err(MigrationsError::migration_database_failed(None, Some(err.into()))))?;
        let update_statement = format!(r#"UPDATE {} SET status='in_progress' where version={};"#,
                                       self.migrations_table_name.as_str(), version);
        println!("update statement: {}", update_statement.as_str());
        let update_result = db.exec(update_statement.as_str(), vec![])
            .await
            .or_else(|err| Err(MigrationsError::migration_versioning_failed(Some(err.into()))))?;

        if update_result.rows_affected < 1 {
            let insert_statement = format!(r#"INSERT INTO {}(version, status) VALUES ({}, 'in_progress');"#,
                                           self.migrations_table_name.as_str(), version);
            println!("insert statement: {}", insert_statement.as_str());
            let _insert_result = db.exec(insert_statement.as_str(), vec![])
                .await
                .or_else(|err| Err(MigrationsError::migration_versioning_failed(Some(err.into()))))?;
        }

        return Ok(());
    }

    async fn finish_version(&self, version: u32) -> flyway::Result<()> {
        println!("finishing version ... {}", version);
        let db = self.db.clone();
        let mut db = db.acquire()
            .await
            .or_else(|err| Err(MigrationsError::migration_database_failed(None, Some(err.into()))))?;

        let update_statement = format!(r#"UPDATE {} SET status='deployed' where version={};"#,
                                       self.migrations_table_name.as_str(), version);
        println!("update statement: {}", update_statement.as_str());
        let update_result = db.exec(update_statement.as_str(), vec![])
            .await
            .or_else(|err| Err(MigrationsError::migration_versioning_failed(Some(err.into()))))?;

        if update_result.rows_affected < 1 {
            let insert_statement = format!(r#"INSERT INTO {}(version, status) VALUES ({}, 'deployed');"#,
                                           self.migrations_table_name.as_str(), version);
            println!("insert statement: {}", insert_statement.as_str());
            let _insert_result = db.exec(insert_statement.as_str(), vec![])
                .await
                .or_else(|err| Err(MigrationsError::migration_versioning_failed(Some(err.into()))))?;
        }

        return Ok(());
    }
}

/// Implementation of the `MigrationExecutor`
#[async_trait]
impl MigrationExecutor for RbatisMigrationDriver {
    async fn begin_transaction(&self) -> flyway::Result<()> {
        println!("beginning transaction ...");
        {
            let mut tx_guard = self.tx.lock().await;
            if tx_guard.get_mut().is_some() {
                return Err(MigrationsError::migration_database_failed(None, None));
            }
        }

        let tx = {
            let db = self.db.clone();
            db.acquire_begin()
                .await
                .or_else(|err| Err(MigrationsError::migration_versioning_failed(Some(err.into()))))?
        };

        let tx_guard = self.tx.lock().await;
        tx_guard.set(Some(tx));
        return Ok(());
    }

    async fn execute_changelog_file(&self, changelog_file: flyway::ChangelogFile) -> flyway::Result<()> {
        println!("executing changelog file ... {:?}", &changelog_file);
        let mut tx_guard = self.tx.lock().await;
        let tx = tx_guard.get_mut().as_mut();
        match tx {
            Some(tx) => {
                for statement in changelog_file.iter() {
                    println!("executing statement: {}", statement.statement.as_str());
                    tx.exec(statement.statement.as_str(), vec![])
                        .await
                        .or_else(|err| Err(MigrationsError::migration_versioning_failed(Some(err.into()))))?;
                }
            },
            None => {
                return Err(MigrationsError::migration_database_failed(None, None));
            }
        };
        return Ok(());
    }

    async fn commit_transaction(&self) -> flyway::Result<()> {
        println!("committing transaction ...");
        let mut tx = {
            let tx_guard = self.tx.lock().await;
            tx_guard.replace(None)
        };

        match tx.as_mut() {
            Some(tx) => {
                return tx.commit().await
                    .map(|_| ())
                    .or_else(|err| Err(MigrationsError::migration_database_failed(None, Some(err.into()))));
            }
            None => {
                return Err(MigrationsError::migration_database_failed(None, None));
            }
        }
    }

    async fn rollback_transaction(&self) -> flyway::Result<()> {
        println!("rolling back transaction ...");
        let mut tx = {
            let tx_guard = self.tx.lock().await;
            tx_guard.replace(None)
        };

        match tx.as_mut() {
            Some(tx) => {
                return tx.rollback().await
                    .map(|_| ())
                    .or_else(|err| Err(MigrationsError::migration_database_failed(None, Some(err.into()))));
            }
            None => {
                return Err(MigrationsError::migration_database_failed(None, None));
            }
        }
    }
}