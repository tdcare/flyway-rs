use proc_macro::TokenStream;
use std::env;
use std::num::ParseIntError;
use std::path::PathBuf;
use std::str::FromStr;
use proc_macro2::Span;
use quote::quote;
use syn::{LitStr};
use syn::__private::TokenStream2;

use flyway_sql_changelog::ChangelogFile;

/// Represents migration files loaded from a directory
#[derive(Debug, Clone)]
struct MigrationInfo {
    version: u32,
    filename: String,
    name: String,
}

/// Attribute macro for automatically generating a `flyway::MigrationStore`
///
/// The macro takes one required literal string parameter representing the directory containing
/// the migration files. Each file must be named like `V<version>_<name>.sql`, where `<version>`
/// is a valid integer and `<name>` is some name describing what the migration does.
///
/// Example:
/// ```ignore
/// use flyway_codegen::migrations;
///
/// #[migrations("examples/migrations/")]
/// struct Migrations {}
///
/// pub fn main() {
///     let migration_store = Migrations {};
///     println!("migrations: {:?}", migration_store.changelogs());
/// }
/// ```
#[proc_macro_attribute]
pub fn migrations(args: TokenStream, input: TokenStream) -> TokenStream {
    // println!("metadata: {:?}", &args);
    // println!("input:    {:?}", &input);

    let input_clone = input.clone();
    let input_struct = syn::parse_macro_input!(input_clone as syn::ItemStruct);
    // println!("input struct: {:?}", &input_struct);

    let path = if args.is_empty() {
        map_to_crate_root(None)
    } else {
        let migrations_path = syn::parse_macro_input!(args as LitStr).value();
        map_to_crate_root(Some(migrations_path.as_str()))
    };
    println!("migrations path: {:?}", path);

    let migrations = get_migrations(&path)
        .expect("Error while gathering migration file information.");
    println!("migrations: {:?}", &migrations);

    let migration_tokens: Vec<TokenStream2> = migrations.iter()
        .map(|migration| {
            let name = migration.name.as_str();
            let version = migration.version;
            let filename = migration.filename.as_str();
            let file_path = path.clone().join(filename).display().to_string();
            let content = std::fs::read_to_string(file_path.as_str())
                .expect(format!("Could not read migration file: {}", file_path).as_str());

            // just check if the changelog can be loaded correctly:
            let _changelog = ChangelogFile::from_string(version.to_string().as_str(), name,content.as_str())
                .expect(format!("Migration file is not a valid SQL changelog file: {}", file_path).as_str());

            quote! {
                (#version, #name.to_string(), #content)
            }
        })
        .collect();

    let struct_name = syn::Ident::new(input_struct.ident.to_string().as_str(), Span::call_site());
    // println!("struct_name: {}", &struct_name);
    let result = quote! {
        impl flyway::MigrationStore for #struct_name {
            fn changelogs(&self) -> Vec<flyway::ChangelogFile> {
                use flyway::ChangelogFile;

                let mut result: Vec<ChangelogFile> = [#(#migration_tokens),*].iter()
                .map(|migration| {
                    ChangelogFile::from_string(migration.0.to_string().as_str(),migration.1.to_string().as_str(), migration.2).unwrap()
                })
                .collect();
                return result;
            }
        }
    };
    // println!("result: {}", result.to_string());

    let input: TokenStream2 = input.into();
    return quote! {
        #input
        #result
    }.into();
}

/// Map a path to the root of the crate
fn map_to_crate_root(path: Option<&str>) -> PathBuf {
    let root = env::var("CARGO_MANIFEST_DIR")
        .map(|root| PathBuf::from(root))
        .expect("Missing CARGO_MANIFEST_DIR environment variable. Cannot obtain crate root.");
    let result = path.map(|path| root.join(PathBuf::from_str(path)
        .expect("Could not parse filename.")))
        .or(Some(root))
        .unwrap();
    return result;
}

/// List migrations contained inside a directory
fn get_migrations(path: &PathBuf) -> Result<Vec<MigrationInfo>, std::io::Error> {
    let result: Vec<MigrationInfo> = std::fs::read_dir(path)?
        .filter(|entry| entry.is_ok())
        .map(|entry| entry.unwrap().file_name().to_str().map(|v| v.to_string()))
        .filter(|filename| filename.is_some())
        .map(|filename| filename.unwrap())
        .filter(|filename| filename.starts_with("V") && filename.ends_with(".sql"))
        .map(|filename| {
            let index = filename.find("_");
            let mut version = "";
            let mut name = "";
            if let Some(index) = index {
                if index > 1 && index < filename.len() - "V.sql".len() {
                    if filename[1..index].chars().all(|ch| ch >= '0' && ch <= '9') {
                        version = &filename[1..index];
                        name = &filename[(index + 1)..(filename.len() - ".sql".len())];
                    }
                }
            }

            return if version.is_empty() {
                None
            } else {
                let result: Result<Option<u32>, ParseIntError> = version.parse::<u32>()
                    .map(|version| Some(version))
                    .or(Ok(None));

                let result = result.unwrap()
                    .map(|version| {
                        MigrationInfo {
                            version,
                            filename: filename.to_string(),
                            name: name.to_string()
                        }
                    });
                return result
            };
        })
        .filter(|info| info.is_some())
        .map(|info| info.unwrap())
        .collect();
    return Ok(result);
}

#[cfg(test)]
mod test {
    #[test]
    pub fn test_get_migrations() {
        let path = crate::map_to_crate_root(Some("examples/migrations"));
        let result = crate::get_migrations(&path);
        match result {
            Ok(migrations) => {
                assert_eq!(migrations.len(), 2, "Two migrations have been successfully loaded.");
            }
            Err(err) => {
                assert!(false, "Migration loading failed: {}", err);
            }
        }
    }
}