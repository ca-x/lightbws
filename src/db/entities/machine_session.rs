use sea_orm::entity::prelude::*;

#[derive(Clone, Debug, PartialEq, DeriveEntityModel)]
#[sea_orm(table_name = "machine_sessions")]
pub struct Model {
    #[sea_orm(primary_key, auto_increment = false)]
    pub id: String,
    pub machine_access_token_id: String,
    pub expires_at: i64,
    pub created_at: i64,
}

#[derive(Copy, Clone, Debug, EnumIter, DeriveRelation)]
pub enum Relation {
    #[sea_orm(
        belongs_to = "super::machine_access_token::Entity",
        from = "Column::MachineAccessTokenId",
        to = "super::machine_access_token::Column::Id",
        on_update = "Cascade",
        on_delete = "Cascade"
    )]
    AccessToken,
}

impl Related<super::machine_access_token::Entity> for Entity {
    fn to() -> RelationDef {
        Relation::AccessToken.def()
    }
}

impl ActiveModelBehavior for ActiveModel {}
