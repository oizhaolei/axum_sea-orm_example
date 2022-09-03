use entity::user;
use sea_orm::DeleteResult;
use sea_orm_migration::prelude::*;
use sea_orm_migration::sea_orm::entity::*;

#[derive(DeriveMigrationName)]
pub struct Migration;

#[async_trait::async_trait]
impl MigrationTrait for Migration {
    async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        // Replace the sample below with your own migration scripts
        let db = manager.get_connection();
        user::ActiveModel {
            email: Set("account@example.com".to_owned()),
            hash: Set("cLVE7E3Y71+ng0/laMdt9fPPdbb93vE9eeJCjoda21s=".to_owned()), // "secret"
            ..Default::default()
        }
        .insert(db)
        .await?;

        Ok(())
    }

    async fn down(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        // Replace the sample below with your own migration scripts
        let db = manager.get_connection();
        let res: DeleteResult = entity::user::Entity::delete_many().exec(db).await?;
        assert!(res.rows_affected > 0);

        Ok(())
    }
}
