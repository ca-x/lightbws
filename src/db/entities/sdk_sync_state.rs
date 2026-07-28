use sea_orm::entity::prelude::*;

#[derive(Clone, Debug, PartialEq, DeriveEntityModel)]
#[sea_orm(table_name = "sdk_sync_state")]
pub struct Model {
    #[sea_orm(primary_key, auto_increment = false)]
    pub organization_id: String,
    pub revision_nanos: i64,
}

#[derive(Copy, Clone, Debug, EnumIter, DeriveRelation)]
pub enum Relation {}

impl ActiveModelBehavior for ActiveModel {}
