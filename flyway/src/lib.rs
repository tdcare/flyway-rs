use std::error::Error;
use std::fmt::{Display, Formatter};
use std::sync::Arc;
use async_trait::async_trait;

pub use flyway_codegen::{ migrations };
pub use flyway_sql_changelog::{Result as ChangelogResult, *};

/// Kinds of errors produced by the migration code
#[derive(Debug)]
pub enum MigrationsErrorKind {
    /// The migration failed during a single migration step.
    ///
    /// This will usually happen when the lines (steps) of a changelog file are processed.
    MigrationDatabaseStepFailed(Option<Box<dyn Error + Send + Sync>>),

    /// There was a general database-related problem
    MigrationDatabaseFailed(Option<Box<dyn Error + Send + Sync>>),

    /// Could not set up migration metadata
    ///
    /// This usually means that the migration state management could not be set up, e.g. because
    /// there was an error while creating the migrations state table.
    MigrationSetupFailed(Option<Box<dyn Error + Send + Sync>>),

    /// There was a problem beginning/finishing a version
    MigrationVersioningFailed(Option<Box<dyn Error + Send + Sync>>),

    /// Some kind of error that has no specific representation
    CustomErrorMessage(String, Option<Box<dyn Error + Send + Sync>>),
}

/// Represents errors produced by migration code
#[derive(Debug)]
pub struct MigrationsError {
    /// The kind of error that occurred
    kind: MigrationsErrorKind,

    /// The last successfully deployed version
    last_successful_version: Option<u32>,
}

impl MigrationsError {
    pub fn migration_database_step_failed(last_successful_version: Option<u32>,
                                          cause: Option<Box<dyn Error + Send + Sync>>) -> MigrationsError {
        return MigrationsError {
            kind: MigrationsErrorKind::MigrationDatabaseStepFailed(cause),
            last_successful_version
        };
    }

    pub fn migration_database_failed(last_successful_version: Option<u32>,
                                     cause: Option<Box<dyn Error + Send + Sync>>) -> MigrationsError {
        return MigrationsError {
            kind: MigrationsErrorKind::MigrationDatabaseFailed(cause),
            last_successful_version
        };
    }

    pub fn migration_setup_failed(cause: Option<Box<dyn Error + Send + Sync>>) -> MigrationsError {
        return MigrationsError {
            kind: MigrationsErrorKind::MigrationSetupFailed(cause),
            last_successful_version: None,
        };
    }

    pub fn migration_versioning_failed(cause: Option<Box<dyn Error + Send + Sync>>) -> MigrationsError {
        return MigrationsError {
            kind: MigrationsErrorKind::MigrationVersioningFailed(cause),
            last_successful_version: None,
        };
    }

    pub fn custom_message(message: &str, last_successful_version: Option<u32>,
                          cause: Option<Box<dyn Error + Send + Sync>>) -> MigrationsError {
        return MigrationsError {
            kind: MigrationsErrorKind::CustomErrorMessage(message.to_string(), cause),
            last_successful_version,
        };
    }

    pub fn kind(&self) -> &MigrationsErrorKind {
        &self.kind
    }

    pub fn last_successful_version(&self) -> Option<u32> {
        self.last_successful_version
    }
}

pub type Result<T> = std::result::Result<T, MigrationsError>;

impl Display for MigrationsError {
    fn fmt(&self, fmt: &mut Formatter<'_>) -> std::fmt::Result {
        match &self.kind {
            MigrationsErrorKind::MigrationDatabaseStepFailed(err_opt) => {
                let mut result = write!(fmt, "Migration step failed.");
                if err_opt.is_some() {
                    result = write!(fmt, "\nCaused by: {}", err_opt.as_ref().unwrap());
                }
                return result;
            },
            MigrationsErrorKind::MigrationDatabaseFailed(err_opt) => {
                let mut result = write!(fmt, "Migration failed.");
                if err_opt.is_some() {
                    result = write!(fmt, "\nCaused by: {}", err_opt.as_ref().unwrap());
                }
                return result;
            },
            MigrationsErrorKind::MigrationSetupFailed(err_opt) => {
                let mut result = write!(fmt, "Migration setup failed.");
                if err_opt.is_some() {
                    result = write!(fmt, "\nCaused by: {}", err_opt.as_ref().unwrap());
                }
                return result;
            },
            MigrationsErrorKind::MigrationVersioningFailed(err_opt) => {
                let mut result = write!(fmt, "Migration versioning failed.");
                if err_opt.is_some() {
                    result = write!(fmt, "\nCaused by: {}", err_opt.as_ref().unwrap());
                }
                return result;
            },
            MigrationsErrorKind::CustomErrorMessage(message, err_opt) => {
                let mut result = write!(fmt, "{}", message.as_str());
                if err_opt.is_some() {
                    result = write!(fmt, "\nCaused by: {}", err_opt.as_ref().unwrap());
                }
                return result;
            }
        };
    }
}

impl Error for MigrationsError {
    // fn source(&self) -> Option<&(dyn Error + 'static)> {
    //     match &self.kind {
    //         MigrationsErrorKind::MigrationDatabaseStepFailed(err_opt) => {
    //             if err_opt.is_some() {
    //                 let err = err_opt.as_ref().unwrap();
    //                 return Some(err);
    //             }
    //         },
    //         MigrationsErrorKind::MigrationDatabaseFailed(err_opt) => {
    //             if err_opt.is_some() {
    //                 let err = err_opt.as_ref().unwrap();
    //                 return Some(err);
    //             }
    //         },
    //         MigrationsErrorKind::MigrationSetupFailed(err_opt) => {
    //             if err_opt.is_some() {
    //                 let err = err_opt.as_ref().unwrap();
    //                 return Some(err);
    //             }
    //         },
    //         MigrationsErrorKind::MigrationVersioningFailed(err_opt) => {
    //             if err_opt.is_some() {
    //                 let err = err_opt.as_ref().unwrap();
    //                 return Some(err);
    //             }
    //         },
    //         // MigrationsErrorKind::MigrationQueryArgumentReplacementFailed(err_opt) => {
    //         //     if err_opt.is_some() {
    //         //         let err = err_opt.as_ref().unwrap();
    //         //         return Some(err);
    //         //     }
    //         // },
    //     };
    //     return None;
    // }
}

/// Status of a migration.
#[derive(Debug, Clone)]
pub enum MigrationStatus {
    /// Migration is in progress.
    ///
    /// The migration of this version has been started, but not finished yet. Depending on the
    /// database driver and transaction management, this status may never actually land in the
    /// database.
    InProgress,

    /// Migration has been finished.
    Deployed,
}

/// The minimal information for a migration version
#[derive(Debug, Clone)]
pub struct MigrationState {
    /// The version of the migration
    pub version: u32,

    /// The status of the migration
    pub status: MigrationStatus,
}

/// Trait for state management
///
/// This should be implemented by DB drivers so that db-up can manage installed schema versions.
#[async_trait]
pub trait MigrationStateManager {
    /// Prepare the DB for migration state management
    ///
    /// This will be called before any other methods to ensure that the dateabase is prepared
    /// for state management. For most drivers, this method will simply ensure that a state
    /// management table exists.
    async fn prepare(&self) -> Result<()>;

    /// Get the lowest deployed version
    async fn lowest_version(&self) -> Result<Option<MigrationState>>;

    /// Get the highest deployed version
    async fn highest_version(&self) -> Result<Option<MigrationState>>;

    /// Get a list of all deployed versions
    async fn list_versions(&self) -> Result<Vec<MigrationState>>;

    /// Begin a new version
    async fn begin_version(&self, changelog_file: &ChangelogFile) -> Result<()>;

    /// Finish a new version
    ///
    /// This will usually just set the status of the migration version to `Deployed`
    async fn finish_version(&self, changelog_file: &ChangelogFile) -> Result<()>;
}

/// Trait for executing migrations
///
/// This should be implemented by DB drivers so that db-up can execute migrations on the
/// database.
#[async_trait]
pub trait MigrationExecutor {
    async fn begin_transaction(&self) -> Result<()>;
    async fn execute_changelog_file(&self, changelog_file: &ChangelogFile) -> Result<()>;
    async fn commit_transaction(&self) -> Result<()>;
    async fn rollback_transaction(&self) -> Result<()>;
}

/// Struct for running migrations on a database
pub struct MigrationRunner<S, M, E> {
    /// The migration store containing the changelog files
    store: S,

    /// The state manager
    ///
    /// This is an `Arc` so that the state manager and the executor can, but are not required
    /// to be, the same object.
    state_manager: Arc<M>,

    /// The migration executor
    ///
    /// This is an `Arc` so that the state manager and the executor can, but are not required
    /// to be, the same object.
    executor: Arc<E>,
}

/// Struct storing the changelogs needed for the migrations
///
/// Implementations of this trait will usually be generated by the `migrations` macro, but can
/// also be created manually.
pub trait MigrationStore {
    fn changelogs(&self) -> Vec<ChangelogFile>;
}

impl<S, M, E> MigrationRunner<S, M, E>
    where S: MigrationStore,
          M: MigrationStateManager,
          E: MigrationExecutor {

    /// Create a new `MigrationRunner`
    pub fn new(store: S, state_manager: Arc<M>, executor: Arc<E>) -> Self {
        return Self {
            store, state_manager, executor
        };
    }

    /// Migrate with a separate transaction for each changelog
    ///
    /// This will execute each migration inside its own DB transaction. Therefore, if an error
    /// occurs and the method returns prematurely, all versions that have been successfully
    /// deployed will stay in the database.
    pub async fn migrate(&self) -> Result<Option<u32>> {
        self.state_manager.prepare().await?;
        let mut current_highest_version = self.state_manager.highest_version()
            .await?
            .map(|state| state.version);
        let mut migrations: Vec<ChangelogFile> = self.store.changelogs().into_iter()
            .filter(|migration| {
                let version: u32 = migration.version()
                    .parse()
                    .expect("Version must be an integer");
                return current_highest_version.map(|highest_version| version > highest_version)
                    .or(Some(true))
                    .unwrap();
            })
            .collect::<Vec<ChangelogFile>>();
        log::debug!("Sorting migrations ...");
        migrations.sort_by(|a, b| a.version().cmp(b.version()));
        let migrations = migrations;

        log::debug!("Running migrations ... {:?}", &migrations);
        for changelog in migrations.into_iter() {
            let version: u32 = changelog.version().parse().unwrap();

            self.state_manager.begin_version(&changelog).await?;
            self.executor.begin_transaction().await?;
            let result = self.executor
                .execute_changelog_file(&changelog)
                .await;

            match result {
                Ok(_) => {
                    self.executor.commit_transaction().await?;
                    self.state_manager.finish_version(&changelog).await?;
                    current_highest_version = Some(version);
                },
                Err(err) => {
                    let _result = self.executor.rollback_transaction().await
                        .or::<MigrationsError>(Ok(()))
                        .unwrap();
                    return Err(err);
                }
            }
        }

        return Ok(current_highest_version);
    }

    // /// Migrate with a single transaction for all changelogs
    //
    // /// This will execute all migrations inside one big DB transaction. Therefore, if an error
    // /// occurs and the method returns prematurely, none of the changes will stay inside
    // /// the database.
    // pub async fn migrate_single_transaction(&self) -> Result<Option<u32>> {
    //     self.state_manager.prepare().await?;
    //     let mut current_highest_version = self.state_manager.highest_version()
    //         .await?
    //         .map(|state| state.version);
    //     let mut migrations: Vec<ChangelogFile> = self.store.changelogs().into_iter()
    //         .filter(|migration| {
    //             let version: u32 = migration.version()
    //                 .parse()
    //                 .expect("Version must be an integer");
    //             return current_highest_version.map(|highest_version| version > highest_version)
    //                 .or(Some(true))
    //                 .unwrap();
    //         })
    //         .collect::<Vec<ChangelogFile>>();
    //     migrations.sort_by(|a, b| a.version().cmp(b.version()));
    //     let migrations = migrations;
    //
    //     self.executor.begin_transaction().await?;
    //     for changelog in migrations.into_iter() {
    //         let version: u32 = changelog.version().parse().unwrap();
    //
    //         let result = self.executor
    //             .execute_changelog_file(changelog)
    //             .await;
    //         match result {
    //             Ok(_) => {
    //                 current_highest_version = Some(version);
    //             },
    //             Err(err) => {
    //                 self.executor.rollback_transaction();
    //                 return Err(err);
    //             }
    //         }
    //     }
    //     self.executor.commit_transaction().await?;
    //
    //     return Ok(current_highest_version);
    // }
}