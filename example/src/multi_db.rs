//! # 统一多数据库迁移入口示例
//!
//! 本示例展示了"方案一"的完整实现模式：
//! - **按数据库类型分目录**：每种数据库方言有独立的迁移脚本目录
//! - **多 MigrationStore**：为每种数据库定义独立的迁移存储结构体
//! - **运行时自动派发**：根据连接字符串自动检测数据库类型，选择对应的迁移脚本执行
//!
//! ## 使用方法
//!
//! 1. 设置环境变量 `DATABASE_URL`，例如：
//!    - MySQL: `DATABASE_URL=mysql://user:password@localhost:3306/database`
//!    - TDengine: `DATABASE_URL=taos+ws://localhost:6041/database`
//!
//! 2. 运行迁移：
//!    ```bash
//!    cargo run --bin multi-db
//!    ```
//!
//! ## 设计思路
//!
//! 方案一的核心思想是"编译时分离，运行时派发"：
//! - 编译时：使用 `#[migrations]` 宏为每种数据库类型生成独立的迁移元数据
//! - 运行时：通过 `driver_type()` 检测数据库类型，选择对应的迁移执行器
//!
//! 这种方式的优点：
//! - 迁移脚本按数据库方言组织，便于维护
//! - 每种数据库可以有完全不同的表结构和 SQL 语法
//! - 支持动态切换数据库，无需重新编译

extern crate rbatis;

use std::env;
use std::sync::Arc;
use flyway::{MigrationRunner, MigrationsError};
use flyway::migrations;
use flyway_rbatis::{RbatisMigrationDriver, RbatisDbDriverType};
use rbatis::RBatis;
use rbdc_mysql::driver::MysqlDriver;
use rbdc_pg::driver::PgDriver;
use rbdc_tdengine::driver::TaosDriver;

// ============================================================================
// 方案一：为每种数据库定义独立的 MigrationStore
// ============================================================================

/// MySQL 数据库的迁移配置
///
/// 使用 `#[migrations]` 宏指向 MySQL 专用的迁移脚本目录。
/// 该目录下的 SQL 文件应使用 MySQL 方言编写。
#[migrations("migrations/mysql/")]
pub struct MysqlMigrations {}

/// TDengine (TaOS) 数据库的迁移配置
///
/// 使用 `#[migrations]` 宏指向 TDengine 专用的迁移脚本目录。
/// 该目录下的 SQL 文件应使用 TDengine 方言编写。
#[migrations("migrations/taos/")]
pub struct TaosMigrations {}

/// PostgreSQL 数据库的迁移配置
///
/// 使用 `#[migrations]` 宏指向 PostgreSQL 专用的迁移脚本目录。
/// 该目录下的 SQL 文件应使用 PostgreSQL 方言编写。
#[migrations("migrations/postgres/")]
pub struct PgMigrations {}

// ============================================================================
// 核心迁移函数：运行时自动派发
// ============================================================================

/// 执行数据库迁移
///
/// 该函数会自动检测数据库类型，并选择对应的迁移脚本执行。
///
/// # 参数
/// - `rb`: 已初始化的 RBatis 实例
///
/// # 返回
/// - `Ok(())`: 迁移成功
/// - `Err(MigrationsError)`: 迁移失败
///
/// # 支持的数据库类型
/// - MySQL: 使用 `migrations/mysql/` 目录下的迁移脚本
/// - TDengine: 使用 `migrations/taos/` 目录下的迁移脚本
/// - PostgreSQL: 使用 `migrations/postgres/` 目录下的迁移脚本
///
/// # 扩展方法
/// 如需支持更多数据库，只需：
/// 1. 新增一个使用 `#[migrations]` 宏的结构体，指向新的迁移目录
/// 2. 在下面的 `match` 语句中添加新的分支
pub async fn migrate(rb: Arc<RBatis>) -> Result<(), MigrationsError> {
    // 创建迁移驱动（用于执行迁移和管理状态）
    let migration_driver = Arc::new(RbatisMigrationDriver::new(rb.clone(), None));

    // 检测数据库类型
    let db_type = migration_driver
        .driver_type()
        .map_err(|e| MigrationsError::migration_database_failed(None, Some(e.into())))?;

    // 根据数据库类型选择对应的迁移配置并执行
    match db_type {
        RbatisDbDriverType::MySql => {
            log::info!("检测到 MySQL 数据库，使用 migrations/mysql/ 目录下的迁移脚本");
            let runner = MigrationRunner::new(
                MysqlMigrations {},
                migration_driver.clone(),
                migration_driver.clone(),
                true, // 启用自动提交
            );
            runner.migrate().await.map(|_| ())
        }
        RbatisDbDriverType::TDengine => {
            log::info!("检测到 TDengine 数据库，使用 migrations/taos/ 目录下的迁移脚本");
            let runner = MigrationRunner::new(
                TaosMigrations {},
                migration_driver.clone(),
                migration_driver.clone(),
                true, // 启用自动提交
            );
            runner.migrate().await.map(|_| ())
        }
        RbatisDbDriverType::Pg => {
            log::info!("检测到 PostgreSQL 数据库，使用 migrations/postgres/ 目录下的迁移脚本");
            let runner = MigrationRunner::new(
                PgMigrations {},
                migration_driver.clone(),
                migration_driver.clone(),
                true, // 启用自动提交
            );
            runner.migrate().await.map(|_| ())
        }
        RbatisDbDriverType::Sqlite => {
            log::warn!("SQLite 数据库暂不支持，请添加对应的迁移目录和配置");
            Err(MigrationsError::migration_setup_failed(None))
        }
        RbatisDbDriverType::MsSql => {
            log::warn!("SQL Server 数据库暂不支持，请添加对应的迁移目录和配置");
            Err(MigrationsError::migration_setup_failed(None))
        }
        RbatisDbDriverType::Other(driver_name) => {
            log::error!("不支持的数据库类型: {}", driver_name);
            log::error!("请确保使用正确的数据库连接字符串，或为此数据库类型添加迁移配置");
            Err(MigrationsError::migration_setup_failed(None))
        }
    }
}

// ============================================================================
// 主函数：演示统一入口的使用方法
// ============================================================================

#[tokio::main]
pub async fn main() {
    // 初始化日志
    fast_log::init(
        fast_log::Config::new()
            .console()
            .level(log::LevelFilter::Debug),
    )
    .expect("日志初始化失败");

    // 从环境变量获取数据库连接字符串
    let database_url = env::var("DATABASE_URL").unwrap_or_else(|_| {
        log::warn!("未设置 DATABASE_URL 环境变量，使用默认的 MySQL 连接");
        "mysql://root:123456@localhost:3306/test".to_string()
    });

    log::info!("数据库连接: {}", mask_password(&database_url));

    // 初始化 RBatis
    let rb = RBatis::new();

    // 根据连接字符串前缀选择合适的驱动
    if database_url.starts_with("mysql://") {
        rb.init(MysqlDriver {}, &database_url)
            .expect("MySQL 数据库连接失败");
    } else if database_url.starts_with("taos") {
        rb.init(TaosDriver {}, &database_url)
            .expect("TDengine 数据库连接失败");
    } else if database_url.starts_with("postgres://") {
        rb.init(PgDriver {}, &database_url)
            .expect("PostgreSQL 数据库连接失败");
    } else {
        log::error!("不支持的数据库连接字符串格式: {}", database_url);
        log::error!("支持的格式:");
        log::error!("  - MySQL: mysql://user:password@host:port/database");
        log::error!("  - TDengine: taos+ws://host:port/database");
        log::error!("  - PostgreSQL: postgres://user:password@host:port/database");
        std::process::exit(1);
    }

    // 执行迁移
    match migrate(Arc::new(rb)).await {
        Ok(_) => {
            log::info!("数据库迁移成功完成！");
        }
        Err(e) => {
            log::error!("数据库迁移失败: {:?}", e);
            std::process::exit(1);
        }
    }
}

/// 隐藏连接字符串中的密码（用于日志输出）
fn mask_password(url: &str) -> String {
    // 简单的密码隐藏实现
    if let Some(at_pos) = url.find('@') {
        if let Some(colon_pos) = url[..at_pos].rfind(':') {
            if let Some(slash_pos) = url[..colon_pos].rfind('/') {
                let prefix = &url[..slash_pos + 1];
                let user_end = url[slash_pos + 1..colon_pos].len();
                let suffix = &url[at_pos..];
                return format!("{}{}:***{}", prefix, &url[slash_pos + 1..slash_pos + 1 + user_end], suffix);
            }
        }
    }
    url.to_string()
}
