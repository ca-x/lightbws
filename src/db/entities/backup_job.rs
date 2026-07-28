use sea_orm::entity::prelude::*;

#[derive(Clone, Debug, PartialEq, DeriveEntityModel)]
#[sea_orm(table_name = "backup_jobs")]
pub struct Model {
    #[sea_orm(primary_key, auto_increment = false)]
    pub id: String,
    pub target_id: String,
    pub trigger_kind: String,
    pub status: String,
    pub object_key: String,
    pub byte_size: Option<i64>,
    pub error_code: Option<String>,
    pub created_at: i64,
    pub completed_at: Option<i64>,
}

#[derive(Copy, Clone, Debug, EnumIter, DeriveRelation)]
pub enum Relation {
    #[sea_orm(
        belongs_to = "super::backup_target::Entity",
        from = "Column::TargetId",
        to = "super::backup_target::Column::Id",
        on_update = "Cascade",
        on_delete = "Cascade"
    )]
    Target,
}

impl Related<super::backup_target::Entity> for Entity {
    fn to() -> RelationDef {
        Relation::Target.def()
    }
}

impl ActiveModelBehavior for ActiveModel {}
