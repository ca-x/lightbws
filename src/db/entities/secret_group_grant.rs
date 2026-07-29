use sea_orm::entity::prelude::*;

#[derive(Clone, Debug, PartialEq, serde::Serialize, serde::Deserialize, DeriveEntityModel)]
#[sea_orm(table_name = "secret_group_grants")]
pub struct Model {
    #[sea_orm(primary_key, auto_increment = false)]
    pub secret_id: String,
    #[sea_orm(primary_key, auto_increment = false)]
    pub group_id: String,
    pub can_read: bool,
    pub can_write: bool,
    pub created_at: i64,
    pub updated_at: i64,
}

#[derive(Copy, Clone, Debug, EnumIter, DeriveRelation)]
pub enum Relation {}

impl ActiveModelBehavior for ActiveModel {}
