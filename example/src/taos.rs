#[macro_use]
extern crate rbatis;

use std::sync::Arc;
use rbatis::Rbatis;
use rbdc_tdengine::driver::TaosDriver;
use rbdc_tdengine::options::TaosConnectOptions;
use flyway::{MigrationRunner, MigrationsError};
use flyway::migrations;
use flyway_rbatis::*;

#[migrations("migrations/taos/")]
pub struct Migrations {
}

async fn run(rbatis: Arc<Rbatis>) -> Result<(), MigrationsError> {
    let migration_driver = Arc::new(RbatisMigrationDriver::new(rbatis.clone(), None));
    let migration_runner = MigrationRunner::new(Migrations {}, migration_driver.clone(), migration_driver.clone(),true);
    migration_runner.migrate().await?;
    Ok(())
}

#[tokio::main]
pub async fn main() {
    fast_log::init(
        fast_log::Config::new()
            .console()
            .level(log::LevelFilter::Debug),
    )
        .expect("rbatis init fail");

    let rb = Rbatis::new();
    // ------------choose database driver------------
    // rb.init(rbdc_mysql::driver::MysqlDriver {}, "mysql://root:123456@localhost:3306/test").unwrap();
    // rb.init(rbdc_pg::driver::PgDriver {}, "postgres://postgres:123456@localhost:5432/postgres").unwrap();
    // rb.init(rbdc_mssql::driver::MssqlDriver {}, "mssql://SA:TestPass!123456@localhost:1433/test").unwrap();
    rb.init_opt(
        TaosDriver {},
        TaosConnectOptions{
            dsn: "taos+ws://localhost:6041/test".to_string()
        }

    )
        .unwrap();

    run(Arc::new(rb)).await.expect("TODO: panic message");

}