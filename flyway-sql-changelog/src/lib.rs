use std::path::{Path};
use std::io::Read;
use std::string::FromUtf8Error;
use std::sync::Arc;
use std::cmp::Ordering;

use serde::{ Deserialize, Serialize };
use std::error::Error;
use std::fmt::{Display, Formatter};

const SINGLE_QUOTE1: u8 = '\'' as u8;
const SINGLE_QUOTE2: u8 = '`' as u8;
//const SINGLE_QUOTE3: u8 = '´' as u8;
const DOUBLE_QUOTE: u8 = '"' as u8;
const SEMICOLON: u8 = ';' as u8;
const BACKSLASH: u8 = '\\' as u8;
const MINUS: u8 = '-' as u8;
const LINEFEED: u8 = '\n' as u8;

/// Kinds of errors that can occur when processing a `ChangelogFile`
#[derive(Debug)]
pub enum ChangelogErrorKind {
    EmptyChangelog,
    /// min_version, requested_min_version
    MinVersionNotFound(String, String),
    /// max_version, requested_max_version
    MaxVersionNotFound(String, String),
    IoError(std::io::Error),
    Other(Box<dyn std::error::Error + Send + Sync>),
}

/// An error that occurred while processing a `ChangelogFile`
#[derive(Debug)]
pub struct ChangelogError {
    kind: ChangelogErrorKind,
}

impl ChangelogError {
    pub fn emtpy_change_log() -> ChangelogError {
        return ChangelogError {
            kind: ChangelogErrorKind::EmptyChangelog,
        };
    }

    pub fn min_version_not_found(actual_min_version: &str, requested_min_version: &str) -> ChangelogError {
        return ChangelogError {
            kind: ChangelogErrorKind::MinVersionNotFound(actual_min_version.to_string(), requested_min_version.to_string()),
        };
    }

    pub fn max_version_not_found(actual_max_version: String, requested_max_version: String) -> ChangelogError {
        return ChangelogError {
            kind: ChangelogErrorKind::MaxVersionNotFound(actual_max_version, requested_max_version),
        };
    }

    pub fn io(io_error: std::io::Error) -> ChangelogError {
        return ChangelogError {
            kind: ChangelogErrorKind::IoError(io_error),
        };
    }

    pub fn other(other_error: Box<dyn std::error::Error + Send + Sync>) -> ChangelogError {
        return ChangelogError {
            kind: ChangelogErrorKind::Other(other_error),
        };
    }

    pub fn kind(&self) -> &ChangelogErrorKind {
        &self.kind
    }
}

impl From<std::io::Error> for ChangelogError {
    fn from(io_error: std::io::Error) -> Self {
        return ChangelogError::io(io_error);
    }
}

impl Display for ChangelogError {
    fn fmt(&self, fmt: &mut Formatter<'_>) -> std::fmt::Result {
        match &self.kind {
            ChangelogErrorKind::EmptyChangelog => {
                return write!(fmt, "Database changelog is empty.");
            }
            ChangelogErrorKind::MinVersionNotFound(actual_min, requested_min) => {
                return write!(fmt, "Requested minimum version {} not found in changelog. Minimum available version is {}.", requested_min, actual_min);
            }
            ChangelogErrorKind::MaxVersionNotFound(actual_max, requested_max) => {
                return write!(fmt, "Requested maximum version {} not found in changelog. Maximum available version is {}.", requested_max, actual_max);
            }
            ChangelogErrorKind::IoError(io_error) => {
                return io_error.fmt(fmt);
            }
            ChangelogErrorKind::Other(other_error) => {
                return other_error.fmt(fmt);
            }
        };
    }
}

impl Error for ChangelogError {
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        match &self.kind {
            ChangelogErrorKind::IoError(io_error) => {
                return Some(io_error);
            },
            ChangelogErrorKind::Other(other_error) => {
                return Some(&**other_error);
            },
            _ => return None
        };
    }
}

pub type Result<T> = std::result::Result<T, ChangelogError>;

/// A changelog file
#[derive(Debug, Clone)]
pub struct ChangelogFile {
    /// The version this `ChangelogFile` represents
    version: String,

    /// The full code of this `ChangelogFile`
    content: Arc<String>,
}

/// Internal state of the `SqlStatementIterator`
#[derive(Debug, Clone)]
enum SqlStatementIteratorState {
    /// Top-level state
    Normal,
    /// The parser is inside a quoted region
    ///
    /// The argument is the type of quote used.
    Quoted(u8),
    /// The parser is inside an escape sequence
    ///
    /// The argument is the type of quote in which the escape appeared.
    Escaped(u8),
    /// The parser is inside a comment
    ///
    /// First argument is the `SqlStatementIteratorState` from before the comment started.
    /// Second argument is the contents of the comment.
    Comment(Box<SqlStatementIteratorState>, Vec<u8>)
}

/// The annotation of an SQL statement
///
/// Changelog files support annotating SQL statements so special error- and transaction-handling
/// may be applied to the statement. Support for those annotations is not guaranteed by
/// driver implementations.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SqlStatementAnnotation {
    /// Continue the migration if the annotated statement fails
    may_fail: Option<bool>,
}

/// A single, optionally annotated, SQL statement
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SqlStatement {
    /// The optional annotation of of the statement
    pub annotation: Option<SqlStatementAnnotation>,
    /// The actual SQL statement
    pub statement: String,
}

/// An iterator for a `ChangelogFile`
#[derive(Debug, Clone)]
pub struct SqlStatementIterator {
    /// `Arc` reference to the content of the changelog
    content: Arc<String>,
    /// Current position inside the content
    position: usize,
    /// Current state of the iterator
    state: SqlStatementIteratorState,
}

impl ChangelogFile {
    /// Load `ChangelogFile` from a given path
    pub fn from_path(path: &Path) -> Result<ChangelogFile> {
        let mut version = "".to_string();
        let basename_opt = path.components().last();
        if let Some(basename) = basename_opt {
            let basename = basename.as_os_str().to_str().unwrap();
            let index_opt = basename.find("_");
            if let Some(index) = index_opt {
                if index > 0 {
                    version = (&basename[0..index]).to_string();
                }
            }
        }

        return std::fs::read_to_string(path)
            .map(|content| ChangelogFile {
                version,
                content: Arc::new(content)
            })
            .or_else(|err| Err(err.into()));
    }

    /// Create `ChangelogFile` from a version and a string containing the contents
    pub fn from_string(version: &str, sql: &str) -> Result<ChangelogFile> {
        return Ok(ChangelogFile {
            version: version.to_string(),
            content: Arc::new(sql.to_string())
        });
    }

    /// Create an iterator for the statements of this `ChangelogFile`
    pub fn iter(&self) -> SqlStatementIterator {
        return SqlStatementIterator::from_shared_string(self.content.clone());
    }

    /// Get the version of this `ChangelogFile`
    pub fn version(&self) -> &str {
        return self.version.as_str();
    }

    /// Get the raw text of the `ChangelogFile`
    pub fn content(&self) -> &str {
        return self.content.as_str();
    }
}

impl PartialEq<Self> for ChangelogFile {
    #[inline]
    fn eq(&self, other: &Self) -> bool {
        return self.version.eq(&other.version) &&
            self.content.eq(&other.content);
    }
}

impl PartialOrd<Self> for ChangelogFile {
    #[inline]
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        return self.version.as_bytes().partial_cmp(other.version.as_bytes());
    }
}

impl Eq for ChangelogFile { }

impl Ord for ChangelogFile {
    fn cmp(&self, other: &Self) -> Ordering {
        return self.version.as_bytes().cmp(other.version.as_bytes());
    }
}

impl SqlStatementIterator {
    /// Create object by reading content from a given path
    pub fn from_path(path: &Path) -> Result<SqlStatementIterator> {
        let mut text = String::new();
        std::fs::File::open(path)?.read_to_string(&mut text)?;

        return Ok(Self::from_str(text.as_str()));
    }

    /// Create object from a string
    pub fn from_str(content: &str) -> SqlStatementIterator {
        return Self::from_shared_string(Arc::new(content.to_string()));
    }

    /// Create object from an `Arc<String>`
    pub fn from_shared_string(content: Arc<String>) -> SqlStatementIterator {
        return SqlStatementIterator {
            content,
            position: 0,
            state: SqlStatementIteratorState::Normal,
        };
    }

    /// Get the next byte of the content
    fn next_byte(&mut self) -> Option<u8> {
        if self.position < self.content.len() {
            let ch = self.content.as_bytes()[self.position];
            self.position += 1;
            return Some(ch);
        }

        return None;
    }
}

impl Iterator for SqlStatementIterator {
    type Item = SqlStatement;

    fn next(&mut self) -> Option<Self::Item> {
        // println!("READING next statement: position={}, state={:?}", self.position, &self.state);

        //let mut len = 0;
        let mut statement: Vec<u8> = Vec::new();
        let mut annotation: Vec<u8> = Vec::new();

        let mut ch = self.next_byte();

        while ch.is_some() {
            //len += 1;
            let current_char = ch.unwrap();
            ch = self.next_byte();

            //println!("ch={}", current_char);

            match current_char {
                LINEFEED => {
                    match &self.state {
                        SqlStatementIteratorState::Comment(prev_state, comment) => {
                            let comment_string: String = String::from_utf8(comment.to_vec())
                                .or_else::<FromUtf8Error, _>(|_: FromUtf8Error| Ok("(non-utf8)".to_string()))
                                .unwrap();

                            let comment_string = comment_string.trim_start();
                            if comment_string.starts_with("--! ") {
                                let comment_string = &comment_string[4..comment_string.len()];
                                // println!("annotation line: {}", comment_string);
                                for byte in comment_string.as_bytes() {
                                    annotation.push(*byte);
                                }
                            } else {
                                // println!("SQL comment: {}", comment_string);
                            }
                            self.state = *prev_state.clone();
                        },
                        _ => {
                            statement.push(current_char);
                        }
                    }
                },
                MINUS => {
                    match &self.state {
                        SqlStatementIteratorState::Normal => {
                            self.state = SqlStatementIteratorState::Comment(Box::new(self.state.clone()), "-".to_string().into_bytes());
                        },
                        SqlStatementIteratorState::Comment(prev_state, comment) => {
                            self.state = SqlStatementIteratorState::Comment(
                                prev_state.clone(),
                                comment.to_vec().into_iter().chain(vec![current_char].into_iter()).collect()
                            );
                        },
                        _ => {
                            statement.push(current_char);
                        }
                    };
                },
                SINGLE_QUOTE1 => {
                    match &self.state {
                        SqlStatementIteratorState::Normal => {
                            statement.push(current_char);
                            self.state = SqlStatementIteratorState::Quoted(SINGLE_QUOTE1);
                        },
                        SqlStatementIteratorState::Escaped(q) => {
                            statement.push(current_char);
                            self.state = SqlStatementIteratorState::Quoted(*q);
                        },
                        SqlStatementIteratorState::Quoted(q) => {
                            if current_char == *q {
                                statement.push(current_char);
                                self.state = SqlStatementIteratorState::Normal;
                            }
                        },
                        SqlStatementIteratorState::Comment(prev_state, comment) => {
                            if comment.len() < 2 {
                                let mut comment_clone = comment.clone();
                                statement.append(&mut comment_clone);
                                self.state = *prev_state.clone();
                            } else {
                                self.state = SqlStatementIteratorState::Comment(
                                    prev_state.clone(),
                                    comment.to_vec().into_iter().chain(vec![current_char].into_iter()).collect()
                                );
                            }
                        }
                    }
                },
                SINGLE_QUOTE2 => {
                    match &self.state {
                        SqlStatementIteratorState::Normal => {
                            statement.push(current_char);
                            self.state = SqlStatementIteratorState::Quoted(SINGLE_QUOTE1);
                        },
                        SqlStatementIteratorState::Escaped(q) => {
                            statement.push(current_char);
                            self.state = SqlStatementIteratorState::Quoted(*q);
                        },
                        SqlStatementIteratorState::Quoted(q) => {
                            statement.push(current_char);
                            if current_char == *q {
                                self.state = SqlStatementIteratorState::Normal;
                            }
                        },
                        SqlStatementIteratorState::Comment(prev_state, comment) => {
                            if comment.len() < 2 {
                                let mut comment_clone = comment.clone();
                                statement.append(&mut comment_clone);
                                self.state = *prev_state.clone();
                            } else {
                                self.state = SqlStatementIteratorState::Comment(
                                    prev_state.clone(),
                                    comment.to_vec().into_iter().chain(vec![current_char].into_iter()).collect()
                                );
                            }
                        }
                    }
                },
                DOUBLE_QUOTE => {
                    match &self.state {
                        SqlStatementIteratorState::Normal => {
                            statement.push(current_char);
                            self.state = SqlStatementIteratorState::Quoted(SINGLE_QUOTE1);
                        },
                        SqlStatementIteratorState::Escaped(q) => {
                            statement.push(current_char);
                            self.state = SqlStatementIteratorState::Quoted(*q);
                        },
                        SqlStatementIteratorState::Quoted(q) => {
                            statement.push(current_char);
                            if current_char == *q {
                                self.state = SqlStatementIteratorState::Normal;
                            }
                        },
                        SqlStatementIteratorState::Comment(prev_state, comment) => {
                            if comment.len() < 2 {
                                let mut comment_clone = comment.clone();
                                statement.append(&mut comment_clone);
                                self.state = *prev_state.clone();
                            } else {
                                self.state = SqlStatementIteratorState::Comment(
                                    prev_state.clone(),
                                    comment.to_vec().into_iter().chain(vec![current_char].into_iter()).collect()
                                );
                            }
                        }
                    }
                },
                SEMICOLON => {
                    match &self.state {
                        SqlStatementIteratorState::Quoted(_) => {
                            statement.push(current_char);
                        },
                        SqlStatementIteratorState::Comment(prev_state, comment) => {
                            if comment.len() < 2 {
                                let mut comment_clone = comment.clone();
                                statement.append(&mut comment_clone);
                                self.state = *prev_state.clone();
                            } else {
                                self.state = SqlStatementIteratorState::Comment(
                                    prev_state.clone(),
                                    comment.to_vec().into_iter().chain(vec![current_char].into_iter()).collect()
                                );
                            }
                        },
                        _ => {
                            break;
                        }
                    };
                },
                BACKSLASH => {
                    match &self.state {
                        SqlStatementIteratorState::Quoted(q) => {
                            statement.push(current_char);
                            self.state = SqlStatementIteratorState::Escaped(*q);
                        },
                        SqlStatementIteratorState::Escaped(q) => {
                            statement.push(current_char);
                            self.state = SqlStatementIteratorState::Quoted(*q);
                        },
                        SqlStatementIteratorState::Comment(prev_state, comment) => {
                            if comment.len() < 2 {
                                let mut comment_clone = comment.clone();
                                statement.append(&mut comment_clone);
                                self.state = *prev_state.clone();
                            } else {
                                self.state = SqlStatementIteratorState::Comment(
                                    prev_state.clone(),
                                    comment.to_vec().into_iter().chain(vec![current_char].into_iter()).collect()
                                );
                            }
                        },
                        _ => {
                            statement.push(current_char);
                        }
                    };
                },
                _ => {
                    match &self.state {
                        SqlStatementIteratorState::Comment(prev_state, comment) => {
                            if comment.len() < 2 {
                                let mut comment_clone = comment.clone();
                                statement.append(&mut comment_clone);
                                self.state = *prev_state.clone();
                            } else {
                                self.state = SqlStatementIteratorState::Comment(
                                    prev_state.clone(),
                                    comment.to_vec().into_iter().chain(vec![current_char].into_iter()).collect()
                                );
                            }
                        },
                        _ => {
                            statement.push(current_char);
                        }
                    }
                }
            }
        }

        for byte in statement.as_slice() {
            if *byte > 127 {
                println!("invalid byte: {:#02x}", byte);
            }
        }

        // println!("FINISHED READING: statement={}", String::from_utf8(statement.clone()).unwrap());
        if statement.len() > 0 {
            //self.position += len;
            // println!("FINISHED READING: position={}", self.position);
            return String::from_utf8(statement)
                .map(|value| value.trim().to_string())
                .ok()
                .map_or_else(|| None, |value| {
                    if value.len() > 0 {
                        // println!("annotation length: {}", annotation.len());
                        let annotation = if annotation.len() > 0 {
                            serde_yaml::from_slice::<SqlStatementAnnotation>(annotation.as_slice())
                                .or_else(|err| {
                                    // println!("Error parsing annotations: {:?}", err);
                                    return Err(err);
                                })
                                .ok()
                        } else {
                            None
                        };
                        // println!("returning annotation: {:?}", &annotation);
                        // println!("returning statement:  {}", &value);
                        let result = SqlStatement {
                            statement: value,
                            annotation
                        };
                        Some(result)
                    } else {
                        None
                    }
                });
        } else {
            return None;
        }
    }
}

#[cfg(test)]
mod test {
    use std::path::Path;
    use crate::ChangelogFile;

    #[test]
    pub fn test_load_changelog_file1() {
        let path = Path::new(".").join("examples/migrations/V1_test1.sql");
        let result = ChangelogFile::from_path(&path);
        match result {
            Ok(changelog) => {
                assert_eq!(changelog.version, "V1");
                assert!(changelog.content().trim_start().starts_with("CREATE TABLE lorem"));
                assert!(changelog.content().trim_end().ends_with("ipsum VARCHAR(16));"));
            }
            Err(err) => {
                assert!(false, "Changelog file loading failed: {}", err);
            }
        }
    }

    #[test]
    pub fn test_load_changelog_file2() {
        let path = Path::new(".").join("examples/migrations/V2_test2.sql");
        let result = ChangelogFile::from_path(&path);
        match result {
            Ok(changelog) => {
                assert_eq!(changelog.version, "V2");
                assert!(changelog.content().trim_start().starts_with("CREATE INDEX idx_lorem_ipsum"));
                assert!(changelog.content().trim_end().ends_with("sit INTEGER, ahmed BIGINT);"));
            }
            Err(err) => {
                assert!(false, "Changelog file loading failed: {}", err);
            }
        }
    }

    #[test]
    pub fn test_changelog_file1_iterator() {
        let path = Path::new(".").join("examples/migrations/V1_test1.sql");
        let result = ChangelogFile::from_path(&path);
        match result {
            Ok(changelog) => {
                let mut iterator = changelog.iter();
                let statement1 = iterator.next();
                assert!(statement1.is_some(), "Found first statement.");
                assert_eq!(statement1.unwrap().statement.trim(),
                           "CREATE TABLE lorem(id SERIAL, ipsum VARCHAR(16))",
                           "Correct first statement returned.");
                let statement2 = iterator.next();
                assert!(statement2.is_none(), "Only one statement found in iterator.");
            }
            Err(err) => {
                assert!(false, "Changelog file loading failed: {}", err);
            }
        }
    }

    #[test]
    pub fn test_changelog_file2_iterator() {
        let path = Path::new(".").join("examples/migrations/V2_test2.sql");
        let result = ChangelogFile::from_path(&path);
        match result {
            Ok(changelog) => {
                let mut iterator = changelog.iter();
                let statement1 = iterator.next();
                assert!(statement1.is_some(), "Found first statement.");
                assert_eq!(statement1.unwrap().statement.trim(),
                           "CREATE INDEX idx_lorem_ipsum ON lorem(ipsum)",
                           "Correct first statement returned.");
                let statement2 = iterator.next();
                assert!(statement2.is_some(), "Found second statement.");
                assert_eq!(statement2.unwrap().statement.trim(),
                           "CREATE TABLE dolor(id BIGSERIAL PRIMARY KEY, sit INTEGER, ahmed BIGINT)",
                           "Correct second statement returned.");
                let statement3 = iterator.next();
                assert!(statement3.is_none(), "Exactly two statements found in iterator.");
            }
            Err(err) => {
                assert!(false, "Changelog file loading failed: {}", err);
            }
        }
    }
}