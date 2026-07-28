use sea_orm::entity::prelude::*;

#[derive(Clone, Debug, PartialEq, DeriveEntityModel)]
#[sea_orm(table_name = "machine_sessions")]
pub struct Model {
    #[sea_orm(primary_key, auto_increment = false)]
    pub id: String,
    pub machine_account_id: String,
    pub expires_at: i64,
    pub created_at: i64,
}

#[derive(Copy, Clone, Debug, EnumIter, DeriveRelation)]
pub enum Relation {
    #[sea_orm(
        belongs_to = "super::machine_account::Entity",
        from = "Column::MachineAccountId",
        to = "super::machine_account::Column::Id",
        on_update = "Cascade",
        on_delete = "Cascade"
    )]
    MachineAccount,
}

impl Related<super::machine_account::Entity> for Entity {
    fn to() -> RelationDef {
        Relation::MachineAccount.def()
    }
}

impl ActiveModelBehavior for ActiveModel {}
