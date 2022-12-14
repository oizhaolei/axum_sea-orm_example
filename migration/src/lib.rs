pub use sea_orm_migration::prelude::*;

mod m20220120_000001_create_post_table;
mod m20220819_220330_create_cake;
mod m20220820_000001_alter_post_table;
mod m20220902_151527_create_user_table;
mod m20220902_153021_seeding_user_table_data;

pub struct Migrator;

#[async_trait::async_trait]
impl MigratorTrait for Migrator {
    fn migrations() -> Vec<Box<dyn MigrationTrait>> {
        vec![
            Box::new(m20220120_000001_create_post_table::Migration),
            Box::new(m20220819_220330_create_cake::Migration),
            Box::new(m20220820_000001_alter_post_table::Migration),
            Box::new(m20220902_151527_create_user_table::Migration),
            Box::new(m20220902_153021_seeding_user_table_data::Migration),
        ]
    }
}
