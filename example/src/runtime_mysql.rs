/// 运行时迁移加载示例 - 演示如何使用 RuntimeMigrationStore
///
/// 此示例展示了如何从文件系统动态加载迁移脚本，而不是在编译期嵌入。
/// 这种方式允许在不重新编译应用程序的情况下更新迁移脚本。
///
/// 运行方式:
/// ```bash
/// cargo run --bin runtime_mysql
/// ```

use std::sync::Arc;
use rbatis::RBatis;
use rbdc_mysql::driver::MysqlDriver;
use flyway::{MigrationRunner, RuntimeMigrationStore};
use flyway_rbatis::*;

#[tokio::main]
pub async fn main() {
    // 初始化日志系统
    fast_log::init(
        fast_log::Config::new()
            .console()
            .level(log::LevelFilter::Debug),
    )
    .expect("日志初始化失败");

    println!("=== 运行时迁移加载模式 ===");
    println!("此示例演示如何使用 RuntimeMigrationStore 从文件系统加载迁移脚本\n");

    // 创建数据库连接
    let rb = RBatis::new();
    
    // 初始化 MySQL 驱动和连接字符串
    // 注意: 请根据实际情况修改连接字符串
    rb.init(
        MysqlDriver {},
        "mysql://root:123456@localhost:3306/test",
    )
    .expect("数据库连接初始化失败");
    
    let rb = Arc::new(rb);

    // 方式1: 基本使用 - 从目录加载迁移脚本
    println!("步骤1: 创建 RuntimeMigrationStore");
    let store = RuntimeMigrationStore::new("migrations/mysql");
    
    // 可选: 验证迁移目录是否存在
    println!("步骤2: 验证迁移目录");
    match store.validate() {
        Ok(()) => {
            println!("✓ 迁移目录验证通过: {:?}", store.migration_dir());
        }
        Err(e) => {
            eprintln!("⚠ 警告: {}", e);
            eprintln!("提示: 确保 migrations/mysql 目录存在且包含有效的 SQL 迁移文件");
        }
    }
    
    // 创建迁移驱动
    println!("\n步骤3: 创建迁移驱动");
    let migration_driver = Arc::new(RbatisMigrationDriver::new(rb.clone(), None));
    
    // 创建迁移运行器
    println!("步骤4: 创建 MigrationRunner");
    let migration_runner = MigrationRunner::new(
        store,
        migration_driver.clone(),
        migration_driver.clone(),
        true, // fail_continue: 遇到错误时是否继续执行后续迁移
    );
    
    // 执行迁移
    println!("\n步骤5: 执行迁移...");
    match migration_runner.migrate().await {
        Ok(version) => {
            println!("\n✓ 迁移成功!");
            if let Some(v) = version {
                println!("最高版本: {}", v);
            } else {
                println!("所有迁移已应用或无需迁移");
            }
        }
        Err(e) => {
            eprintln!("\n✗ 迁移失败: {}", e);
            return;
        }
    }
    
    println!("\n=== 运行时加载模式演示完成 ===");
}
