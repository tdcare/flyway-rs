use std::path::{Path, PathBuf};
use crate::{MigrationStore, ChangelogFile};

/// Runtime migration store that loads SQL migration scripts from the filesystem dynamically.
///
/// Unlike the compile-time `#[migrations]` macro which embeds migration files into the binary,
/// `RuntimeMigrationStore` reads migration files at runtime from a specified directory.
/// This provides flexibility for scenarios where migrations need to be updated without
/// recompiling the application.
///
/// # Example
///
/// ```no_run
/// use flyway::RuntimeMigrationStore;
///
/// // Create a runtime migration store pointing to the migrations directory
/// let store = RuntimeMigrationStore::new("migrations/mysql");
///
/// // Optionally validate the directory exists
/// if let Err(e) = store.validate() {
///     eprintln!("Migration directory validation failed: {}", e);
/// }
///
/// // Use with MigrationRunner
/// // let runner = MigrationRunner::new(store, state_manager, executor, false);
/// ```
///
/// # File Naming Convention
///
/// Migration files must follow the naming pattern: `V<version>_<name>.sql`
/// - `<version>`: A positive integer representing the migration version
/// - `<name>`: A descriptive name for the migration (can contain underscores)
///
/// Examples:
/// - `V1_Create_Users_Table.sql`
/// - `V2_Add_Email_Column.sql`
/// - `V10_Create_Indexes.sql`
///
/// # Error Handling
///
/// The store gracefully handles errors:
/// - If the migration directory doesn't exist, returns an empty list and logs a warning
/// - If individual files fail to parse, they are skipped with a warning logged
/// - Invalid version numbers cause the file to be skipped
pub struct RuntimeMigrationStore {
    /// Directory path containing migration SQL files
    migration_dir: PathBuf,
}

impl RuntimeMigrationStore {
    /// Creates a new `RuntimeMigrationStore` with the specified migration directory.
    ///
    /// # Arguments
    ///
    /// * `dir` - Path to the directory containing migration SQL files.
    ///           Can be any type convertible to `PathBuf` (e.g., `&str`, `String`, `Path`).
    ///
    /// # Example
    ///
    /// ```
    /// use flyway::RuntimeMigrationStore;
    ///
    /// let store = RuntimeMigrationStore::new("migrations");
    /// let store = RuntimeMigrationStore::new(std::path::PathBuf::from("db/migrations"));
    /// ```
    pub fn new(dir: impl Into<PathBuf>) -> Self {
        Self {
            migration_dir: dir.into(),
        }
    }

    /// Validates that the migration directory exists and is accessible.
    ///
    /// This method checks if the configured migration directory exists and is readable.
    /// It's recommended to call this during application initialization to catch configuration
    /// errors early.
    ///
    /// # Returns
    ///
    /// * `Ok(())` - Directory exists and is accessible
    /// * `Err(String)` - Description of what went wrong
    ///
    /// # Example
    ///
    /// ```no_run
    /// use flyway::RuntimeMigrationStore;
    ///
    /// let store = RuntimeMigrationStore::new("migrations");
    /// match store.validate() {
    ///     Ok(()) => println!("Migration directory is valid"),
    ///     Err(e) => eprintln!("Invalid migration directory: {}", e),
    /// }
    /// ```
    pub fn validate(&self) -> Result<(), String> {
        if !self.migration_dir.exists() {
            return Err(format!(
                "Migration directory does not exist: {:?}",
                self.migration_dir
            ));
        }

        if !self.migration_dir.is_dir() {
            return Err(format!(
                "Migration path is not a directory: {:?}",
                self.migration_dir
            ));
        }

        Ok(())
    }

    /// Scans the migration directory and loads all valid migration files.
    ///
    /// This is the core implementation that:
    /// 1. Reads all entries in the migration directory
    /// 2. Filters for files matching the `V*.sql` pattern
    /// 3. Parses version numbers from filenames
    /// 4. Loads each file using `ChangelogFile::from_path()`
    /// 5. Sorts migrations by version number
    ///
    /// # Implementation Notes
    ///
    /// - Files that don't match the expected naming pattern are silently skipped
    /// - Files with invalid version numbers are skipped with a warning logged
    /// - File loading errors are logged but don't prevent other migrations from loading
    /// - The returned list is sorted by version number in ascending order
    ///
    /// # Returns
    ///
    /// A vector of `ChangelogFile` sorted by version number. Returns an empty vector
    /// if the directory doesn't exist or contains no valid migration files.
    fn load_migrations(&self) -> Vec<ChangelogFile> {
        // Check if directory exists
        if !self.migration_dir.exists() {
            log::warn!(
                "Migration directory does not exist: {:?}. Returning empty migration list.",
                self.migration_dir
            );
            return Vec::new();
        }

        if !self.migration_dir.is_dir() {
            log::warn!(
                "Migration path is not a directory: {:?}. Returning empty migration list.",
                self.migration_dir
            );
            return Vec::new();
        }

        // Read directory entries
        let entries = match std::fs::read_dir(&self.migration_dir) {
            Ok(entries) => entries,
            Err(e) => {
                log::warn!(
                    "Failed to read migration directory {:?}: {}. Returning empty migration list.",
                    self.migration_dir,
                    e
                );
                return Vec::new();
            }
        };

        // Process each entry
        let mut migrations: Vec<ChangelogFile> = entries
            .filter_map(|entry| {
                // Skip entries that failed to read
                let entry = match entry {
                    Ok(e) => e,
                    Err(e) => {
                        log::warn!("Failed to read directory entry: {}", e);
                        return None;
                    }
                };

                let path = entry.path();

                // Only process files
                if !path.is_file() {
                    return None;
                }

                // Get filename
                let filename = match path.file_name().and_then(|n| n.to_str()) {
                    Some(name) => name.to_string(),
                    None => {
                        log::warn!("Invalid filename: {:?}", path);
                        return None;
                    }
                };

                // Filter for V*.sql pattern
                if !filename.starts_with('V') || !filename.ends_with(".sql") {
                    return None;
                }

                // Parse version and name from filename
                // Expected format: V<version>_<name>.sql
                let underscore_pos = match filename.find('_') {
                    Some(pos) if pos > 1 && pos < filename.len() - "V.sql".len() => pos,
                    _ => {
                        log::warn!(
                            "Invalid migration filename format (missing underscore): {}. Skipping.",
                            filename
                        );
                        return None;
                    }
                };

                let version_str = &filename[1..underscore_pos];
                
                // Validate that version part contains only digits
                if !version_str.chars().all(|c| c.is_ascii_digit()) {
                    log::warn!(
                        "Invalid version in filename '{}' (contains non-digit characters). Skipping.",
                        filename
                    );
                    return None;
                }

                // Parse version number
                let version: u64 = match version_str.parse() {
                    Ok(v) => v,
                    Err(e) => {
                        log::warn!(
                            "Failed to parse version from filename '{}': {}. Skipping.",
                            filename,
                            e
                        );
                        return None;
                    }
                };

                Some((path, version, filename))
            })
            .filter_map(|(path, version, filename)| {
                // Extract name from filename (between underscore and .sql)
                let underscore_pos = filename.find('_').unwrap();
                let _name = &filename[(underscore_pos + 1)..(filename.len() - ".sql".len())];

                // Load the changelog file
                match ChangelogFile::from_path(&path) {
                    Ok(changelog) => {
                        // Verify the parsed version matches the filename version
                        if changelog.version != version {
                            log::warn!(
                                "Version mismatch in file '{}': filename says {}, file content says {}. Using filename version.",
                                filename,
                                version,
                                changelog.version
                            );
                        }
                        Some(changelog)
                    }
                    Err(e) => {
                        log::warn!(
                            "Failed to load migration file '{}': {}. Skipping.",
                            filename,
                            e
                        );
                        None
                    }
                }
            })
            .collect();

        // Sort migrations by version
        migrations.sort_by(|a, b| a.version.cmp(&b.version));

        log::debug!(
            "Loaded {} migrations from {:?}",
            migrations.len(),
            self.migration_dir
        );

        migrations
    }

    /// Returns the configured migration directory path.
    ///
    /// # Example
    ///
    /// ```
    /// use flyway::RuntimeMigrationStore;
    ///
    /// let store = RuntimeMigrationStore::new("migrations");
    /// assert_eq!(store.migration_dir(), std::path::Path::new("migrations"));
    /// ```
    pub fn migration_dir(&self) -> &Path {
        &self.migration_dir
    }
}

impl MigrationStore for RuntimeMigrationStore {
    /// Returns all migration changelogs loaded from the filesystem.
    ///
    /// This method scans the configured migration directory, loads all valid SQL files,
    /// and returns them sorted by version number.
    ///
    /// # Returns
    ///
    /// A vector of `ChangelogFile` instances, sorted by version in ascending order.
    /// Returns an empty vector if no valid migrations are found.
    ///
    /// # Implementation Details
    ///
    /// - Delegates to `load_migrations()` for the actual file scanning logic
    /// - Errors during file loading are logged but don't cause panics
    /// - The result is cached per call (not persisted between calls)
    fn changelogs(&self) -> Vec<ChangelogFile> {
        self.load_migrations()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs::{self, File};
    use std::io::Write;
    use tempfile::TempDir;

    fn create_test_migration(dir: &Path, version: u64, name: &str, content: &str) {
        let filename = format!("V{}_{}.sql", version, name);
        let path = dir.join(filename);
        let mut file = File::create(&path).unwrap();
        writeln!(file, "{}", content).unwrap();
    }

    #[test]
    fn test_runtime_store_creation() {
        let store = RuntimeMigrationStore::new("test/migrations");
        assert_eq!(store.migration_dir(), Path::new("test/migrations"));
    }

    #[test]
    fn test_validate_success() {
        let temp_dir = TempDir::new().unwrap();
        let store = RuntimeMigrationStore::new(temp_dir.path());
        assert!(store.validate().is_ok());
    }

    #[test]
    fn test_validate_nonexistent_directory() {
        let store = RuntimeMigrationStore::new("/nonexistent/path/that/should/not/exist");
        assert!(store.validate().is_err());
    }

    #[test]
    fn test_validate_file_not_directory() {
        let temp_dir = TempDir::new().unwrap();
        let file_path = temp_dir.path().join("not_a_dir.sql");
        File::create(&file_path).unwrap();
        
        let store = RuntimeMigrationStore::new(&file_path);
        assert!(store.validate().is_err());
    }

    #[test]
    fn test_load_empty_directory() {
        let temp_dir = TempDir::new().unwrap();
        let store = RuntimeMigrationStore::new(temp_dir.path());
        let migrations = store.changelogs();
        assert_eq!(migrations.len(), 0);
    }

    #[test]
    fn test_load_single_migration() {
        let temp_dir = TempDir::new().unwrap();
        create_test_migration(
            temp_dir.path(),
            1,
            "Create_Users",
            "CREATE TABLE users (id INT PRIMARY KEY);"
        );

        let store = RuntimeMigrationStore::new(temp_dir.path());
        let migrations = store.changelogs();
        
        assert_eq!(migrations.len(), 1);
        assert_eq!(migrations[0].version, 1);
        assert_eq!(migrations[0].name, "Create_Users");
    }

    #[test]
    fn test_load_multiple_migrations_sorted() {
        let temp_dir = TempDir::new().unwrap();
        create_test_migration(temp_dir.path(), 3, "Add_Index", "CREATE INDEX idx ON users(name);");
        create_test_migration(temp_dir.path(), 1, "Create_Users", "CREATE TABLE users (id INT);");
        create_test_migration(temp_dir.path(), 2, "Add_Email", "ALTER TABLE users ADD email VARCHAR;");

        let store = RuntimeMigrationStore::new(temp_dir.path());
        let migrations = store.changelogs();
        
        assert_eq!(migrations.len(), 3);
        assert_eq!(migrations[0].version, 1);
        assert_eq!(migrations[1].version, 2);
        assert_eq!(migrations[2].version, 3);
    }

    #[test]
    fn test_skip_invalid_filenames() {
        let temp_dir = TempDir::new().unwrap();
        
        // Valid migration
        create_test_migration(temp_dir.path(), 1, "Valid", "SELECT 1;");
        
        // Invalid: doesn't start with V
        let invalid_path = temp_dir.path().join("invalid.sql");
        File::create(&invalid_path).unwrap();
        
        // Invalid: doesn't end with .sql
        let invalid_path2 = temp_dir.path().join("V2_test.txt");
        File::create(&invalid_path2).unwrap();
        
        // Invalid: no underscore
        let invalid_path3 = temp_dir.path().join("V3.sql");
        File::create(&invalid_path3).unwrap();

        let store = RuntimeMigrationStore::new(temp_dir.path());
        let migrations = store.changelogs();
        
        assert_eq!(migrations.len(), 1);
        assert_eq!(migrations[0].version, 1);
    }

    #[test]
    fn test_nonexistent_directory_returns_empty() {
        let store = RuntimeMigrationStore::new("/nonexistent/migrations/path");
        let migrations = store.changelogs();
        assert_eq!(migrations.len(), 0);
    }

    #[test]
    fn test_migration_content_loaded_correctly() {
        let temp_dir = TempDir::new().unwrap();
        let sql_content = "CREATE TABLE test (\n    id INT PRIMARY KEY,\n    name VARCHAR(100)\n);";
        create_test_migration(temp_dir.path(), 1, "Test_Table", sql_content);

        let store = RuntimeMigrationStore::new(temp_dir.path());
        let migrations = store.changelogs();
        
        assert_eq!(migrations.len(), 1);
        assert!(migrations[0].content.contains("CREATE TABLE test"));
        assert!(migrations[0].content.contains("id INT PRIMARY KEY"));
    }
}
