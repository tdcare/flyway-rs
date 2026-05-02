# flyway-rs

[![Crates.io](https://img.shields.io/crates/v/flyway.svg)](https://crates.io/crates/flyway)
[![License: MIT](https://img.shields.io/badge/license-MIT-blue.svg)](LICENSE)

[English](README.md)

`flyway-rs` 是一组用于加载和执行数据库迁移的 Rust crate 集合，灵感来源于 Java 生态中的 [Flyway](https://flywaydb.org/)。

本项目作为 [refinery](https://github.com/rust-db/refinery) 的替代方案而创建。`refinery` 的驱动架构较为封闭 —— `refinery::Migration::applied(...)` 方法不是公开的，导致外部 crate 无法实现 `refinery::AsyncMigrate` trait。详见 [此 issue](https://github.com/rust-db/refinery/issues/248)。

## 特性

- **多数据库支持**：MySQL、PostgreSQL、SQLite、MSSql、TDengine
- **编译期迁移嵌入**：通过过程宏在编译期解析 SQL 文件并嵌入二进制文件，运行时无需文件 I/O
- **版本化迁移管理**：自动跟踪已应用的迁移版本，支持版本状态管理
- **事务支持**：每个迁移在独立的数据库事务中执行，失败时自动回滚
- **失败继续模式**：可选配置，当单个迁移失败时继续执行后续迁移
- **SQL 语句注解**：支持 `--! may_fail: true` 注解，允许单条 SQL 语句失败而不中断整个迁移
- **多数据库运行时派发**：在运行时根据检测到的数据库类型动态选择迁移脚本
- **可插拔驱动架构**：通过 `MigrationStateManager` 和 `MigrationExecutor` trait 轻松实现自定义数据库驱动

## Crate 结构

| Crate | 说明 |
|---|---|
| [`flyway`](https://crates.io/crates/flyway) | 核心 crate。包含迁移运行器、核心 trait（`MigrationStore`、`MigrationStateManager`、`MigrationExecutor`），并重导出子 crate 的宏。 |
| [`flyway-rbatis`](https://crates.io/crates/flyway-rbatis) | 基于 [Rbatis](https://github.com/rbatis/rbatis) 的数据库驱动实现。支持 MySQL、PostgreSQL、SQLite、MSSql 和 TDengine。 |
| [`flyway-codegen`](https://crates.io/crates/flyway-codegen) | 过程宏 crate。提供 `#[migrations(...)]` 属性宏，用于编译期加载迁移文件。 |
| [`flyway-sql-changelog`](https://crates.io/crates/flyway-sql-changelog) | SQL 文件解析库。负责 SQL 语句分割、校验和计算及语句注解解析。 |

## 快速开始

### 1. 添加依赖

```toml
[dependencies]
flyway = "0.6.0"
flyway-rbatis = "0.6.0"
rbatis = "4.9"
rbdc-mysql = "4.9"    # 或 rbdc-pg 等
tokio = { version = "1", features = ["full"] }
```

### 2. 创建迁移文件

将 SQL 文件放置在指定目录中，遵循命名规范 `V<版本号>_<描述>.sql`：

```
migrations/
├── V1_Create_DeviceData.sql
├── V2_Create_PatientUseDevice.sql
├── V3__Create_VitalSign.sql
└── V6__Add_PatientNo.sql
```

每个文件可以包含多条以分号分隔的 SQL 语句。

### 3. 编写迁移代码

```rust
use std::sync::Arc;
use rbatis::RBatis;
use rbdc_mysql::driver::MysqlDriver;
use flyway::{MigrationRunner, MigrationsError, migrations};
use flyway_rbatis::RbatisMigrationDriver;

// 在编译期加载迁移 SQL 文件
#[migrations("migrations/mysql/")]
pub struct Migrations {}

async fn run(rb: Arc<RBatis>) -> Result<(), MigrationsError> {
    let driver = Arc::new(RbatisMigrationDriver::new(rb.clone(), None));
    let runner = MigrationRunner::new(
        Migrations {},
        driver.clone(),
        driver.clone(),
        true, // fail_continue：迁移失败时继续执行
    );
    runner.migrate().await?;
    Ok(())
}

#[tokio::main]
async fn main() {
    let rb = RBatis::new();
    rb.init(MysqlDriver {}, "mysql://root:123456@localhost:3306/test").unwrap();
    run(Arc::new(rb)).await.expect("迁移失败");
}
```

## 多数据库支持

`flyway-rs` 支持运行时数据库类型检测和迁移脚本自动派发。按数据库方言组织迁移脚本目录：

```
migrations/
├── mysql/
│   ├── V1_Create_DeviceData.sql
│   └── V2_Create_PatientUseDevice.sql
├── postgres/
│   ├── V1_Create_DeviceData.sql
│   └── V2_Create_PatientUseDevice.sql
└── taos/
    ├── V1_Create_DeviceData.sql
    └── V2_Create_PatientUseDevice.sql
```

为每种数据库类型定义独立的迁移存储：

```rust
use flyway::migrations;

#[migrations("migrations/mysql/")]
pub struct MysqlMigrations {}

#[migrations("migrations/postgres/")]
pub struct PgMigrations {}

#[migrations("migrations/taos/")]
pub struct TaosMigrations {}
```

然后使用 `RbatisMigrationDriver::driver_type()` 在运行时检测数据库类型，选择对应的迁移存储执行。完整实现请参考 [multi_db 示例](example/src/multi_db.rs)。

## SQL 注解

可以使用特殊注释对 SQL 语句进行注解，以控制错误处理行为：

```sql
--! may_fail: true
CREATE INDEX idx_patient_no ON VitalSign(patient_no);
```

当设置 `may_fail: true` 时，即使该语句执行失败，迁移也会继续进行。

## 迁移状态表

`flyway-rs` 会自动在数据库中创建迁移跟踪表（默认名称：`flyway_migrations`），用于记录已应用的迁移版本。表结构会根据不同数据库方言自动适配。

## 运行测试

```sh
cd flyway
cargo test
```

## 示例

参见 [`example`](example/) 目录获取完整的示例代码：

- [`mysql.rs`](example/src/mysql.rs) — MySQL 迁移示例
- [`taos.rs`](example/src/taos.rs) — TDengine 迁移示例
- [`multi_db.rs`](example/src/multi_db.rs) — 多数据库运行时派发示例

## 许可证

本项目基于 [MIT 许可证](LICENSE) 发布。
