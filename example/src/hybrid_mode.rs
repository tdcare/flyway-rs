/// 混合模式示例 - 演示如何在编译期和运行时模式之间切换
///
/// 此示例展示了如何根据环境变量动态选择使用编译期嵌入还是运行时加载的迁移方式。
/// 这种方式提供了最大的灵活性，可以在开发和生产环境中使用不同的策略。
///
/// 运行方式:
/// ```bash
/// # 使用编译期嵌入模式(默认)
/// cargo run --bin hybrid_mode
///
/// # 使用运行时加载模式
/// USE_RUNTIME_MIGRATIONS=true cargo run --bin hybrid_mode
/// ```

use std::sync::Arc;
use rbatis::RBatis;
use rbdc_mysql::driver::MysqlDriver;
use flyway::{MigrationRunner, MigrationStore, RuntimeMigrationStore};
use flyway_rbatis::*;
use flyway_codegen::migrations;

// 编译期嵌入的迁移定义
// 使用 #[migrations] 宏在编译时将 SQL 文件嵌入到二进制文件中
#[migrations("migrations/mysql/")]
pub struct CompileTimeMigrations {}

/// 根据环境变量选择迁移模式
///
/// 通过检查 `USE_RUNTIME_MIGRATIONS` 环境变量来决定使用哪种迁移加载方式:
/// - 如果设置为 "true" 或 "1": 使用运行时加载模式 (RuntimeMigrationStore)
/// - 其他情况(包括未设置): 使用编译期嵌入模式 (CompileTimeMigrations)
///
/// # 返回
///
/// 返回一个 trait object，可以是编译期或运行时迁移存储
fn create_migration_store() -> Box<dyn MigrationStore> {
    let use_runtime = std::env::var("USE_RUNTIME_MIGRATIONS")
        .map(|v| v == "true" || v == "1")
        .unwrap_or(false);
    
    if use_runtime {
        println!("📦 使用运行时加载模式");
        println!("   迁移脚本将从文件系统动态读取");
        println!("   目录: migrations/mysql/\n");
        Box::new(RuntimeMigrationStore::new("migrations/mysql"))
    } else {
        println!("🔧 使用编译期嵌入模式");
        println!("   迁移脚本已嵌入到二进制文件中\n");
        Box::new(CompileTimeMigrations {})
    }
}

#[tokio::main]
pub async fn main() {
    // 初始化日志系统
    fast_log::init(
        fast_log::Config::new()
            .console()
            .level(log::LevelFilter::Debug),
    )
    .expect("日志初始化失败");

    println!("=== 混合模式迁移示例 ===");
    println!("此示例演示如何根据环境变量在两种迁移模式之间切换\n");

    // 显示当前选择的模式
    println!("当前配置:");
    match std::env::var("USE_RUNTIME_MIGRATIONS") {
        Ok(val) => println!("  USE_RUNTIME_MIGRATIONS = {}", val),
        Err(_) => println!("  USE_RUNTIME_MIGRATIONS = (未设置，使用默认值)"),
    }
    println!();

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

    // 根据环境变量创建迁移存储
    println!("步骤1: 创建迁移存储");
    let store = create_migration_store();
    
    // 创建迁移驱动
    println!("\n步骤2: 创建迁移驱动");
    let migration_driver = Arc::new(RbatisMigrationDriver::new(rb.clone(), None));
    
    // 创建迁移运行器
    println!("步骤3: 创建 MigrationRunner");
    let migration_runner = MigrationRunner::new(
        store,
        migration_driver.clone(),
        migration_driver.clone(),
        true, // fail_continue: 遇到错误时是否继续执行后续迁移
    );
    
    // 执行迁移
    println!("\n步骤4: 执行迁移...");
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
    
    println!("\n=== 混合模式演示完成 ===");
    println!("\n提示:");
    println!("  - 设置 USE_RUNTIME_MIGRATIONS=true 可切换到运行时加载模式");
    println!("  - 编译期模式适合生产环境(更快的启动速度)");
    println!("  - 运行时模式适合开发环境(无需重新编译即可更新迁移)");
}
