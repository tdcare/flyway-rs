#[macro_use]
extern crate rbatis;

use std::sync::Arc;
use rbatis::Rbatis;
use rbdc_mysql::driver::MysqlDriver;
use flyway::{MigrationRunner, MigrationsError};
use flyway::migrations;
use flyway_rbatis::*;

#[migrations("migrations/mysql/")]
pub struct Migrations {
}

async fn run(rbatis: Arc<Rbatis>) -> Result<(), MigrationsError> {
    let migration_driver = Arc::new(RbatisMigrationDriver::new(rbatis.clone(), None));
    let migration_runner = MigrationRunner::new(
        Migrations {},
        migration_driver.clone(),
        migration_driver.clone()
    );
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
    rb.init(
        MysqlDriver {},
        "mysql://tdnis:Tdcare123for$@mysql-service:6006/tdbox_service?serverTimezone=Asia/Shanghai&useUnicode=true&characterEncoding=utf8",
    )
        .unwrap();

 run(Arc::new(rb)).await.expect("TODO: panic message");

}