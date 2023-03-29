use std::cell::Cell;
use std::ops::DerefMut;
use std::sync::{Arc};
use std::time::Duration;
use tokio::sync::Mutex;
use serde::{Deserialize, Serialize};

use rbatis::{Error, Rbatis};
use flyway::{MigrationExecutor, MigrationState, MigrationStateManager, MigrationsError, MigrationStatus, ChangelogFile};
use rbs::{to_value, Value};
use async_trait::async_trait;
use rbatis::executor::RBatisTxExecutor;
use rbatis::rbatis_codegen::ops::AsProxy;
use rbatis::rbdc::datetime::DateTime;
use rbatis::rbdc::timestamp::Timestamp;

/// Default table name for the migration state management table
pub const DEFAULT_MIGRATIONS_TABLE: &str = "flyway_migrations";



#[derive(Clone, Debug, Serialize, Deserialize)]
struct MigrationInfo {
    ts:DateTime,
    version: u32,
    name: Option<String>,
    checksum: Option<String>,
    status:Option<String>,
}
/// Available driver types supported by Rbatis
pub enum RbatisDbDriverType {
    MySql,
    Pg,
    Sqlite,
    MsSql,
    TDengine,
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
            "Taos"=>RbatisDbDriverType::TDengine,
            _ => RbatisDbDriverType::Other(driver_type_name.to_string())
        };
        return Ok(result);
    }
}

/// Implementation of the `MigrationStateManager`
#[async_trait]
impl MigrationStateManager for RbatisMigrationDriver {
    async fn prepare(&self) -> flyway::Result<()> {
        log::debug!("Preparing Migrations Table ...");
        let db = self.db.clone();
        let mut statement = format!(
            r#"CREATE TABLE IF NOT EXISTS {} (
                version INTEGER PRIMARY KEY,
                ts       varchar(255) null,
                name     varchar(255) null,
                checksum   varchar(255) null,
                status VARCHAR(16)
            );"#, self.migrations_table_name.as_str());

        match self.driver_type(){
            Ok(db_type) => {
                match db_type {
                    RbatisDbDriverType::MySql => {
                        log::debug!("数据库类型:MySql",);

                    }
                    RbatisDbDriverType::Pg => {}
                    RbatisDbDriverType::Sqlite => {}
                    RbatisDbDriverType::MsSql => {}
                    RbatisDbDriverType::TDengine => {
                        log::debug!("数据库类型:TDengine",);
                      statement=format!(
                          r#"
                          CREATE TABLE IF NOT EXISTS {} (`ts` TIMESTAMP, `version` int,`name` nchar(255) , `checksum` nchar(255), `status` nchar(255))
                          "#
                          , self.migrations_table_name.as_str())
                    }
                    RbatisDbDriverType::Other(_) => {}
                }
            }
            Err(_) => {}
        }


        let mut db = db.acquire()
            .await
            .or_else(|err| Err(MigrationsError::migration_database_failed(None, Some(err.into()))))?;

        log::debug!("Preparation Statement: {}", statement.as_str());
        let _result = db.exec(statement.as_str(), vec![])
            .await
            .or_else(|err| Err(MigrationsError::migration_setup_failed(Some(err.into()))))?;
        log::debug!("Preparing Migrations Table ... done");
        return Ok(());
    }

    async fn lowest_version(&self) -> flyway::Result<Option<MigrationState>> {
        log::debug!("Retrieving lowest version ... ");
        let db = self.db.clone();
        let mut db = db.acquire()
            .await
            .or_else(|err| Err(MigrationsError::migration_database_failed(None, Some(err.into()))))?;
        let version: Option<u32> = db.query_decode(format!("SELECT MIN(version) FROM {} WHERE status='deployed';",
                                                           self.migrations_table_name.as_str()).as_str(), vec![])
            .await
            .or_else(|err| Err(MigrationsError::migration_versioning_failed(Some(err.into()))))?;

        log::debug!("Retrieving lowest version ... {:?}", &version);
        return Ok(version.and_then(|version|
            Some(MigrationState {
                version,
                status: MigrationStatus::Deployed
            })));
    }

    async fn highest_version(&self) -> flyway::Result<Option<MigrationState>> {
        log::debug!("Retrieving highest version ... ");
        let db = self.db.clone();
        let mut db = db.acquire()
            .await
            .or_else(|err| Err(MigrationsError::migration_database_failed(None, Some(err.into()))))?;
        let version: Option<u32> = db.query_decode(format!("SELECT MAX(version) FROM {} WHERE status='deployed';",
                                                           self.migrations_table_name.as_str()).as_str(), vec![])
            .await
            .or_else(|err| Err(MigrationsError::migration_versioning_failed(Some(err.into()))))?;

        log::debug!("Retrieving highest version ... {:?}", &version);
        return Ok(version.and_then(|version|
            Some(MigrationState {
                version,
                status: MigrationStatus::Deployed
            })));
    }

    async fn list_versions(&self) -> flyway::Result<Vec<MigrationState>> {
        log::debug!("Listing versions ... ");
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

        log::debug!("Listing versions ... {:?}", &versions);
        return Ok(versions);
    }

    async fn begin_version(&self, changelog_file: &ChangelogFile) -> flyway::Result<()> {
        log::debug!("Beginning version ... {}", changelog_file.version);
        let db = self.db.clone();
        let mut db = db.acquire()
            .await
            .or_else(|err| Err(MigrationsError::migration_database_failed(None, Some(err.into()))))?;

       match   self.driver_type(){
           Ok(db_type) => {
               match db_type {
                   RbatisDbDriverType::TDengine => {
                       let mut ts:i64=DateTime::utc().unix_timestamp_millis()+changelog_file.version.parse::<i64>().unwrap_or_default();
                       let ts_select=format!(r#"select ts,version from {} where status='in_progress' and version=? limit 1;"#, self.migrations_table_name.as_str());
                       match   db.query_decode::<Vec<MigrationInfo>>(ts_select.as_str(),vec![to_value!(changelog_file.version.clone())]).await{
                           Ok(result) => {
                               // println!("{:?}",result);
                              if result.first().is_some(){
                                  let mut time=result.first().unwrap().ts.clone().deref_mut().clone().set_offset(-16*60*60);
                                   ts=time.unix_timestamp_millis();
                              }
                           }
                           Err(e) => {
                               log::error!("数据异常:{}",e.to_string())
                           }
                       };


                       let insert_statement = format!(r#"INSERT INTO {}(ts,version,name,checksum, status) VALUES (?,?,?,?, 'in_progress');"#,
                                                      self.migrations_table_name.as_str());
                       log::debug!("Insert statement: {}", insert_statement.as_str());
                       let _insert_result = db.exec(insert_statement.as_str(), vec![to_value!(ts),to_value!(changelog_file.version.clone()),to_value!(changelog_file.name.clone()),to_value!(changelog_file.checksum.clone())])
                           .await
                           .or_else(|err| Err(MigrationsError::migration_versioning_failed(Some(err.into()))))?;
                       return Ok(());
                   }
                 _ => {}
               }
           }
           Err(_) => {}
       }

        let update_statement = format!(r#"UPDATE {} SET status='in_progress' where version={};"#,
                                       self.migrations_table_name.as_str(), changelog_file.version);
        log::debug!("Update statement: {}", update_statement.as_str());
        let update_result = db.exec(update_statement.as_str(), vec![])
            .await
            .or_else(|err| Err(MigrationsError::migration_versioning_failed(Some(err.into()))))?;

        if update_result.rows_affected < 1 {
            let  ts:i64=DateTime::utc().unix_timestamp_millis()+changelog_file.version.parse::<i64>().unwrap_or_default();

            let insert_statement = format!(r#"INSERT INTO {}(ts,version,name,checksum, status) VALUES (?,?,?,?, 'in_progress');"#,
                                           self.migrations_table_name.as_str());
            log::debug!("Insert statement: {}", insert_statement.as_str());
            let _insert_result = db.exec(insert_statement.as_str(), vec![to_value!(ts),to_value!(changelog_file.version.clone()),to_value!(changelog_file.name.clone()),to_value!(changelog_file.checksum.clone())])
                .await
                .or_else(|err| Err(MigrationsError::migration_versioning_failed(Some(err.into()))))?;
        }

        return Ok(());
    }

    async fn finish_version(&self, changelog_file: &ChangelogFile) -> flyway::Result<()> {
        log::debug!("Finishing version ... {}", changelog_file.version);
        let db = self.db.clone();
        let mut db = db.acquire()
            .await
            .or_else(|err| Err(MigrationsError::migration_database_failed(None, Some(err.into()))))?;


        match   self.driver_type(){
            Ok(db_type) => {
                match db_type {
                    RbatisDbDriverType::TDengine => {
                        let mut ts:i64=DateTime::utc().unix_timestamp_millis()+changelog_file.version.parse::<i64>().unwrap_or_default();
                        let ts_select=format!(r#"select ts,version from {} where status='in_progress' and version=? limit 1;"#, self.migrations_table_name.as_str());
                        match   db.query_decode::<Vec<MigrationInfo>>(ts_select.as_str(),vec![to_value!(changelog_file.version.clone())]).await{
                            Ok(result) => {
                                if result.first().is_some(){
                                    let mut time=result.first().unwrap().ts.clone().deref_mut().clone().set_offset(-16*60*60);

                                    ts=time.unix_timestamp_millis();                               }
                            }
                            Err(e) => {
                                log::error!("数据异常:{}",e.to_string())
                            }
                        };

                        let insert_statement = format!(r#"INSERT INTO {}(ts,version,name,checksum, status) VALUES (?,?,?, 'deployed');"#,
                                                       self.migrations_table_name.as_str());
                        log::debug!("Insert statement: {}", insert_statement.as_str());
                        let _insert_result = db.exec(insert_statement.as_str(), vec![to_value!(ts),to_value!(changelog_file.version.clone()),to_value!(changelog_file.name.clone()),to_value!(changelog_file.checksum.clone())])
                            .await
                            .or_else(|err| Err(MigrationsError::migration_versioning_failed(Some(err.into()))))?;
                        return Ok(());
                    }
                    _ => {}
                }
            }
            Err(_) => {}
        }


        let update_statement = format!(r#"UPDATE {} SET status='deployed' where version={};"#,
                                       self.migrations_table_name.as_str(), changelog_file.version);
        log::debug!("Update statement: {}", update_statement.as_str());
        let update_result = db.exec(update_statement.as_str(), vec![])
            .await
            .or_else(|err| Err(MigrationsError::migration_versioning_failed(Some(err.into()))))?;

        if update_result.rows_affected < 1 {
            let  ts:i64=DateTime::utc().unix_timestamp_millis()+changelog_file.version.parse::<i64>().unwrap_or_default();

            let insert_statement = format!(r#"INSERT INTO {}(ts,version,name,checksum, status) VALUES (?,?,?,?, 'in_progress');"#,
                                           self.migrations_table_name.as_str());
            log::debug!("Insert statement: {}", insert_statement.as_str());
            let _insert_result = db.exec(insert_statement.as_str(), vec![to_value!(ts),to_value!(changelog_file.version.clone()),to_value!(changelog_file.name.clone()),to_value!(changelog_file.checksum.clone())])
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
        log::debug!("Beginning transaction ...");
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

    async fn execute_changelog_file(&self, changelog_file: &flyway::ChangelogFile) -> flyway::Result<()> {
        log::debug!("Executing changelog file ... {:?}", &changelog_file);
        let mut tx_guard = self.tx.lock().await;
        let tx = tx_guard.get_mut().as_mut();
        match tx {
            Some(tx) => {
                for statement in changelog_file.iter() {
                    log::debug!("Executing statement: {}", statement.statement.as_str());
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
        log::debug!("Committing transaction ...");
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
        log::debug!("Rolling back transaction ...");
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