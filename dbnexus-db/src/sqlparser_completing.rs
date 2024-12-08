#![allow(unused)]

use std::ops::Deref;
use std::{collections::HashMap, future::Future, ops::DerefMut, vec};

use oracle::sql_type::Object;
use sqlparser::ast::{
    AlterTableOperation, CommentObject, Expr, FromTable, Function, Ident, Join,
    MySQLColumnPosition, ObjectName, ObjectType, OrderByExpr, Partition, Query, ShowCreateObject,
    Spanned, Subscript, TableConstraint, TableFactor,
};
use sqlparser::ast::{Select, SetExpr};
use sqlx::any;

use crate::{
    completion::{CompletionItem, CompletionKind},
    sqlparser_completion::{CompletionStament, Scope, ScopesGuard, TempTable, TempTableColumn},
};

impl Into<CompletionKind> for ObjectType {
    fn into(self) -> CompletionKind {
        match self {
            ObjectType::Table => CompletionKind::Table,
            ObjectType::View => CompletionKind::View,
            ObjectType::Index => CompletionKind::Index,
            ObjectType::Schema => CompletionKind::Schema,
            ObjectType::Database => CompletionKind::Database,
            ObjectType::Role => CompletionKind::Database,
            ObjectType::Sequence => CompletionKind::Sequence,
            ObjectType::Stage => CompletionKind::Stage,
            ObjectType::Type => CompletionKind::Type,
        }
    }
}

impl Into<CompletionKind> for ShowCreateObject {
    fn into(self) -> CompletionKind {
        match self {
            ShowCreateObject::Event => CompletionKind::Event,
            ShowCreateObject::Function => CompletionKind::Function,
            ShowCreateObject::Procedure => CompletionKind::Procedure,
            ShowCreateObject::Table => CompletionKind::Table,
            ShowCreateObject::Trigger => CompletionKind::Trigger,
            ShowCreateObject::View => CompletionKind::View,
        }
    }
}

impl Into<CompletionKind> for CommentObject {
    fn into(self) -> CompletionKind {
        match self {
            CommentObject::Column => CompletionKind::Column,
            CommentObject::Table => CompletionKind::Table,
            CommentObject::Extension => CompletionKind::Extension,
            CommentObject::Schema => CompletionKind::Schema,
            CommentObject::Database => CompletionKind::Database,
            CommentObject::User => CompletionKind::User,
            CommentObject::Role => CompletionKind::Role,
        }
    }
}

impl CompletionStament {
    pub(crate) async fn c_on_column_mysql_position(
        &self, table_name: &ObjectName, column_position: Option<&MySQLColumnPosition>,
    ) {
        if let Some(column_position) = column_position {
            match column_position {
                MySQLColumnPosition::First => {}
                MySQLColumnPosition::After(col) => self.c_on_column(table_name, col).await,
            }
        }
    }

    pub(crate) async fn c_on_prepare(&self, name: &ObjectName) {
        match name.0.len() {
            1 => {
                *self.result.write().unwrap() = self
                    .helper
                    .list_prepare_name()
                    .await
                    .map_err(|e| {
                        log::error!("list_prepare_name failed,err:{:?}", e);
                        self.set_has_err();
                    })
                    .ok()
                    .unwrap_or_default()
                    .into_iter()
                    .map(|x| CompletionItem {
                        item: x.into(),
                        kind: CompletionKind::PrepareStatement,
                    })
                    .collect::<Vec<_>>()
            }
            _ => {
                log::error!("handled name:{:?}", name);
            }
        }
    }
    pub(crate) async fn c_on_partition_expr(&self, table_name: &ObjectName, p: &Expr) {
        match p {
            Expr::Identifier(x) => {
                let (db, schema, table) = self.extract_db_schme_item(table_name);

                if self.in_pos(&x.span) {
                    *self.result.write().unwrap() = self
                        .helper
                        .list_partition(db, schema, table)
                        .await
                        .map_err(|e| {
                            log::error!("list partition failed,err:{:?}", e);
                            self.set_has_err();
                        })
                        .ok()
                        .unwrap_or_default()
                        .into_iter()
                        .map(|x| CompletionItem {
                            item: x.into(),
                            kind: CompletionKind::Partition,
                        })
                        .collect::<Vec<_>>();
                }
            }
            _ => {
                log::error!("unhandled partation type:{:?}", p);
            }
        }
    }
    pub(crate) async fn c_on_index(&self, name: &ObjectName) {
        if self.in_pos(&name.span()) {
            *self.result.write().unwrap() = self
                .helper
                .list_index()
                .await
                .map_err(|err| {
                    log::error!("list index failed,err:{}", err);
                    self.set_has_err();
                })
                .ok()
                .unwrap_or_default()
                .into_iter()
                .map(|x| CompletionItem {
                    item: x.into(),
                    kind: CompletionKind::Index,
                })
                .collect::<Vec<_>>();
        }
    }

    pub(crate) fn extract_db_schme_item<'a>(
        &self, item_name: &'a ObjectName,
    ) -> (Option<&'a str>, Option<&'a str>, &'a str) {
        let mut db = None;
        let mut schema = None;
        let mut table = "";
        match item_name.0.len() {
            1 => {
                table = item_name.0[0].value.as_str();
            }
            2 => {
                schema = Some(item_name.0[0].value.as_str());
                table = item_name.0[1].value.as_str();
            }
            3 => {
                db = Some(item_name.0[0].value.as_str());
                schema = Some(item_name.0[1].value.as_str());
                table = item_name.0[2].value.as_str();
            }
            _ => {
                self.set_has_err();
                log::error!("empty ObjectName:{:?}", item_name);
            }
        };
        return (db, schema, table);
    }
    pub(crate) async fn c_on_partition(&self, p: &Partition) {
        // TODO
    }

    pub(crate) async fn c_on_postgres_rule(&self, table_name: &ObjectName, rule: &Ident) {
        if !self.in_pos(&rule.span) {
            return;
        }
        let (db, schema, table) = self.extract_db_schme_item(table_name);
        *self.result.write().unwrap() = self
            .helper
            .list_postgres_rule(db, schema, &table)
            .await
            .map_err(|err| {
                log::error!("list index failed,err:{}", err);
                self.set_has_err();
            })
            .ok()
            .unwrap_or_default()
            .into_iter()
            .map(|x| CompletionItem {
                item: x.into(),
                kind: CompletionKind::Index,
            })
            .collect::<Vec<_>>();
    }
    pub(crate) async fn c_on_constraint(&self, table_name: &ObjectName, name: &Ident) {
        if self.in_pos(&name.span) {
            let (db, schema, table_name) = self.extract_db_schme_item(table_name);
            *self.result.write().unwrap() = self
                .helper
                .list_constraint(db, schema, table_name)
                .await
                .map_err(|err| {
                    log::error!("list_constraint failed,err:{:?}", err);
                    self.set_has_err()
                })
                .ok()
                .unwrap_or_default()
                .into_iter()
                .map(|x| CompletionItem {
                    item: x.into(),
                    kind: CompletionKind::Constraint,
                })
                .collect::<Vec<_>>();
        }
    }

    pub(crate) async fn c_on_alert_table(
        &self, table_name: &ObjectName, operations: &Vec<AlterTableOperation>,
    ) {
        self.c_on_table(table_name).await;
        if self.finished() {
            return;
        }
        for op in operations.iter() {
            match op {
                AlterTableOperation::AddConstraint(table_constraint) => match table_constraint {
                    TableConstraint::Unique {
                        name,
                        index_name,
                        columns,
                        ..
                    }
                    | TableConstraint::PrimaryKey {
                        name,
                        index_name,
                        columns,
                        ..
                    } => {
                        if let Some(name) = name {
                            if self.in_pos(&name.span) {
                                self.set_on_def();
                                return;
                            }
                        };
                        if let Some(index_name) = index_name {
                            if self.in_pos(&index_name.span) {
                                self.set_on_def();
                                return;
                            }
                        };
                        for c in columns.iter() {
                            self.c_on_column(table_name, c).await;
                            if self.finished() {
                                return;
                            }
                        }
                    }
                    TableConstraint::ForeignKey {
                        name,
                        columns,
                        foreign_table,
                        referred_columns,
                        ..
                    } => {
                        if let Some(name) = name {
                            if self.in_pos(&name.span) {
                                self.set_on_def();
                                return;
                            }
                        }
                        for c in columns.iter() {
                            self.c_on_column(table_name, c).await;
                            if self.finished() {
                                return;
                            }
                        }
                        self.c_on_table(foreign_table).await;
                        if self.finished() {
                            return;
                        }
                        for c in referred_columns.iter() {
                            self.c_on_column(foreign_table, c).await;
                            if self.finished() {
                                return;
                            }
                        }
                    }
                    TableConstraint::Check { name, expr } => {
                        //  https://www.w3schools.com/sql/sql_check.asp
                        let _guard = self.scopes.push(Scope::from_this_table(table_name.clone()));
                        self.c_on_expr(expr);
                    }
                    TableConstraint::Index { name, columns, .. } => {
                        // MySQLs [index definition][1] for index creation
                        if let Some(name) = name {
                            if self.in_pos(&name.span) {
                                self.set_on_def();
                                return;
                            }
                        }
                        for c in columns.iter() {
                            self.c_on_column(table_name, c).await;
                            if self.finished() {
                                return;
                            }
                        }
                    }
                    TableConstraint::FulltextOrSpatial {
                        opt_index_name,
                        columns,
                        ..
                    } => {
                        if let Some(opt_index_name) = opt_index_name {
                            if self.in_pos(&opt_index_name.span) {
                                self.set_on_def();
                                return;
                            }
                        };
                        for c in columns.iter() {
                            self.c_on_column(table_name, c).await;
                            if self.finished() {
                                return;
                            }
                        }
                    }
                },
                AlterTableOperation::AddColumn {
                    column_def,
                    column_position,
                    ..
                } => {
                    if self.in_pos(&column_def.name.span) {
                        self.set_on_def();
                        return;
                    }
                    self.c_on_column_mysql_position(table_name, column_position.as_ref())
                        .await;
                }
                AlterTableOperation::AddProjection { .. } => {
                    // this is a ClickHouse-specific operation.
                }
                AlterTableOperation::DropProjection { .. } => {
                    // this is a ClickHouse-specific operation.
                }
                AlterTableOperation::MaterializeProjection { .. } => {
                    // this is a ClickHouse-specific operation.
                }
                AlterTableOperation::ClearProjection { .. } => {
                    // this is a ClickHouse-specific operation.
                }
                AlterTableOperation::DisableRowLevelSecurity => {
                    // nothing to here
                }
                AlterTableOperation::DisableRule { name } => {
                    self.c_on_postgres_rule(table_name, name).await
                }
                AlterTableOperation::DisableTrigger { name } => {
                    self.c_on_table_trigger(table_name, name).await;
                }
                AlterTableOperation::DropConstraint { name, .. } => {
                    self.c_on_constraint(table_name, name).await;
                }
                AlterTableOperation::DropColumn { column_name, .. } => {
                    self.c_on_column(table_name, column_name).await;
                }

                AlterTableOperation::AttachPartition { .. } => {
                    // this is a ClickHouse-specific operation
                }
                AlterTableOperation::DetachPartition { .. } => {
                    //  this is a ClickHouse-specific operation
                }
                AlterTableOperation::FreezePartition { .. } => {
                    //  this is a ClickHouse-specific operation
                }
                AlterTableOperation::UnfreezePartition { .. } => {
                    // this is a ClickHouse-specific operation
                }
                AlterTableOperation::DropPrimaryKey => {
                    // nothing to here
                }
                AlterTableOperation::EnableAlwaysRule { name } => {
                    self.c_on_postgres_rule(table_name, name).await;
                }
                AlterTableOperation::EnableAlwaysTrigger { name } => {
                    self.c_on_postgres_rule(table_name, name).await;
                }
                AlterTableOperation::EnableReplicaRule { name } => {
                    self.c_on_postgres_rule(table_name, name).await;
                }
                AlterTableOperation::EnableReplicaTrigger { name } => {
                    self.c_on_table_trigger(table_name, name).await;
                }
                AlterTableOperation::EnableRowLevelSecurity => {
                    // nothing to here
                }
                AlterTableOperation::EnableRule { name } => {
                    self.c_on_postgres_rule(table_name, name).await;
                }
                AlterTableOperation::EnableTrigger { name } => {
                    self.c_on_table_trigger(table_name, name).await;
                }
                AlterTableOperation::RenamePartitions {
                    old_partitions,
                    new_partitions,
                } => {
                    // TODO
                    for p in old_partitions.iter() {
                        self.c_on_partition_expr(table_name, p).await;
                        if self.finished() {
                            return;
                        }
                    }
                    for p in new_partitions.iter() {
                        if self.in_pos(&p.span()) {
                            self.set_on_def();
                            return;
                        }
                    }
                }
                AlterTableOperation::AddPartitions { new_partitions, .. } => {
                    let _guard = self.scopes.push(Scope::from_this_table(table_name.clone()));
                    for p in new_partitions.iter() {
                        self.c_on_partition(p).await;
                        if self.finished() {
                            return;
                        }
                    }
                }
                AlterTableOperation::DropPartitions { partitions, .. } => {
                    for p in partitions.iter() {
                        self.c_on_partition_expr(table_name, p).await;
                        if self.finished() {
                            return;
                        }
                    }
                }
                AlterTableOperation::RenameColumn {
                    old_column_name,
                    new_column_name,
                } => {
                    self.c_on_column(table_name, old_column_name).await;
                    if self.finished() {
                        return;
                    }
                    if self.in_pos(&new_column_name.span) {
                        self.set_on_def();
                        return;
                    }
                }
                AlterTableOperation::RenameTable { table_name } => {
                    if self.in_pos(&table_name.span()) {
                        self.set_on_def();
                        return;
                    }
                }
                AlterTableOperation::ChangeColumn {
                    old_name,
                    new_name,
                    column_position,
                    ..
                } => {
                    self.c_on_column(table_name, old_name).await;
                    if self.finished() {
                        return;
                    }
                    if self.in_pos(&new_name.span) {
                        self.set_on_def();
                        return;
                    }
                    self.c_on_column_mysql_position(table_name, column_position.as_ref())
                        .await;
                }
                AlterTableOperation::ModifyColumn {
                    col_name,
                    column_position,
                    ..
                } => {
                    self.c_on_column(table_name, col_name);
                    if self.finished() {
                        return;
                    }
                    self.c_on_column_mysql_position(table_name, column_position.as_ref())
                        .await;
                }
                AlterTableOperation::RenameConstraint { old_name, new_name } => {
                    if self.in_pos(&new_name.span) {
                        self.set_on_def();
                        return;
                    }
                    self.c_on_constraint(table_name, old_name).await;
                }
                AlterTableOperation::AlterColumn { column_name, .. } => {
                    self.c_on_column(table_name, column_name).await;
                    // other fallback to lexer.
                }
                AlterTableOperation::SwapWith { .. } => {
                    // snowflake no support
                }
                AlterTableOperation::SetTblProperties { .. } => {
                    // no special handle
                }
                AlterTableOperation::OwnerTo { .. } => {
                    // no need
                }
            }
        }
    }
    pub(crate) async fn c_on_view(&self, name: &ObjectName) {
        self.c_on_db_schema(name).await;
        if self.finished() {
            return;
        }
        if !name.0.last().map(|x| self.in_pos(&x.span)).unwrap_or(false) {
            return;
        }
        // here match
        let (db, schema, name) = self.extract_db_schme_item(name);
        *self.result.write().unwrap() = self
            .helper
            .list_table(db, schema)
            .await
            .map_err(|e| {
                log::error!("list table failed,err:{:?}", e);
                self.set_has_err();
            })
            .ok()
            .unwrap_or_default()
            .into_iter()
            .map(|x| CompletionItem {
                item: x.into(),
                kind: CompletionKind::Table,
            })
            .collect::<Vec<_>>();
    }

    pub(crate) async fn c_on_expr_compund_identifier(&self, idents: &[Ident]) {
        // we must access table's column???
        match idents.len() {
            1 => {
                if let Some(table_name) = self.scopes.this_table() {
                    self.c_on_column(&table_name, &idents[0]).await
                }
            }
            2 => {
                let table = &idents[0];
                if self.in_pos(&table.span) {
                    // handle name
                    self.c_on_table_or_view(&ObjectName(vec![table.clone()]))
                        .await;
                    return;
                }
                if let Some(table) = self.scopes.find_table_alias(&table.value) {
                    self.c_on_column(&table, &idents[1]).await;
                } else {
                    self.c_on_column(&ObjectName(vec![table.clone()]), &idents[1])
                        .await;
                }
            }
            3 | 4 => {
                let table: ObjectName = ObjectName(idents[0..idents.len() - 1].to_vec());
                self.c_on_table(&table).await;
                if self.finished() {
                    return;
                }
                self.c_on_column(&table, &idents[2]).await;
            }
            _ => {
                log::error!("identifers too long:{:?}", idents)
            }
        }
    }
    pub(crate) async fn c_on_expr(&self, expr: &Expr) {
        match expr {
            Expr::Identifier(ident) => {
                //
                if let Some(this_table) = self.scopes.this_table() {
                    self.c_on_column(&this_table, ident).await;
                } else {
                    // another completion
                }
            }
            Expr::CompoundIdentifier(vec) => {
                self.c_on_expr_compund_identifier(&vec);
            }
            Expr::JsonAccess { .. } => {
                // snowflake databricks not support
            }
            Expr::CompositeAccess { expr, .. } => {
                // TODO
                Box::pin(self.c_on_expr(&expr)).await;
                if self.finished() {
                    return;
                }
            }
            Expr::IsFalse(expr) => {
                Box::pin(self.c_on_expr(&expr)).await;
            }
            Expr::IsNotFalse(expr) => {
                Box::pin(self.c_on_expr(&expr)).await;
            }
            Expr::IsTrue(expr) => {
                Box::pin(self.c_on_expr(&expr)).await;
            }
            Expr::IsNotTrue(expr) => {
                Box::pin(self.c_on_expr(&expr)).await;
            }
            Expr::IsNull(expr) => {
                Box::pin(self.c_on_expr(&expr)).await;
            }
            Expr::IsNotNull(expr) => {
                Box::pin(self.c_on_expr(&expr)).await;
            }
            Expr::IsUnknown(expr) => {
                Box::pin(self.c_on_expr(&expr)).await;
            }
            Expr::IsNotUnknown(expr) => {
                Box::pin(self.c_on_expr(&expr)).await;
            }
            Expr::IsDistinctFrom(expr, expr1) => {
                Box::pin(self.c_on_expr(&expr)).await;
                if self.finished() {
                    return;
                }
                Box::pin(self.c_on_expr(&expr1)).await;
            }
            Expr::IsNotDistinctFrom(expr, expr1) => {
                Box::pin(self.c_on_expr(&expr)).await;
                if self.finished() {
                    return;
                }
                Box::pin(self.c_on_expr(&expr1)).await;
            }
            Expr::InList { expr, list, .. } => {
                Box::pin(self.c_on_expr(&expr)).await;
                if self.finished() {
                    return;
                }
                for expr in list.iter() {
                    Box::pin(self.c_on_expr(&expr)).await;
                    if self.finished() {
                        return;
                    }
                }
            }
            Expr::InSubquery { expr, subquery, .. } => {
                Box::pin(self.c_on_expr(&expr)).await;
                if self.finished() {
                    return;
                }
                Box::pin(self.c_on_query(&subquery)).await;
            }
            Expr::InUnnest {
                expr, array_expr, ..
            } => {
                Box::pin(self.c_on_expr(&expr)).await;
                if self.finished() {
                    return;
                }
                Box::pin(self.c_on_expr(&array_expr)).await;
            }
            Expr::Between {
                expr, low, high, ..
            } => {
                Box::pin(self.c_on_expr(&expr)).await;
                if self.finished() {
                    return;
                }
                Box::pin(self.c_on_expr(&low)).await;
                if self.finished() {
                    return;
                }
                Box::pin(self.c_on_expr(&high)).await;
            }
            Expr::BinaryOp { left, right, .. } => {
                Box::pin(self.c_on_expr(&left)).await;
                if self.finished() {
                    return;
                }
                Box::pin(self.c_on_expr(&right)).await;
            }
            Expr::Like { expr, pattern, .. } => {
                Box::pin(self.c_on_expr(&expr)).await;
                if self.finished() {
                    return;
                }
                Box::pin(self.c_on_expr(&pattern)).await;
            }
            Expr::ILike { expr, pattern, .. } => {
                Box::pin(self.c_on_expr(&expr)).await;
                if self.finished() {
                    return;
                }
                Box::pin(self.c_on_expr(&pattern)).await;
            }
            Expr::SimilarTo { expr, pattern, .. } => {
                Box::pin(self.c_on_expr(&expr)).await;
                if self.finished() {
                    return;
                }
                Box::pin(self.c_on_expr(&pattern)).await;
            }
            Expr::RLike { expr, pattern, .. } => {
                Box::pin(self.c_on_expr(&expr)).await;
                if self.finished() {
                    return;
                }
                Box::pin(self.c_on_expr(&pattern)).await;
            }
            Expr::AnyOp { left, right, .. } => {
                Box::pin(self.c_on_expr(&left)).await;
                if self.finished() {
                    return;
                }
                Box::pin(self.c_on_expr(&right)).await;
            }
            Expr::AllOp { left, right, .. } => {
                Box::pin(self.c_on_expr(&left)).await;
                if self.finished() {
                    return;
                }
                Box::pin(self.c_on_expr(&right)).await;
            }
            Expr::UnaryOp { expr, .. } => {
                Box::pin(self.c_on_expr(&expr)).await;
                if self.finished() {
                    return;
                }
            }
            Expr::Convert { expr, styles, .. } => {
                Box::pin(self.c_on_expr(&expr)).await;
                for style in styles.iter() {
                    Box::pin(self.c_on_expr(&style)).await;
                    if self.finished() {
                        return;
                    }
                }
            }
            Expr::Cast { expr, .. } => {
                Box::pin(self.c_on_expr(&expr)).await;
            }
            Expr::AtTimeZone {
                timestamp,
                time_zone,
            } => {
                Box::pin(self.c_on_expr(&timestamp)).await;
                if self.finished() {
                    return;
                }
                Box::pin(self.c_on_expr(&time_zone)).await;
            }
            Expr::Extract { expr, .. } => {
                Box::pin(self.c_on_expr(&expr)).await;
            }
            Expr::Ceil { expr, .. } => {
                Box::pin(self.c_on_expr(&expr)).await;
            }
            Expr::Floor { expr, .. } => {
                Box::pin(self.c_on_expr(&expr)).await;
            }
            Expr::Position { expr, r#in } => {
                Box::pin(self.c_on_expr(&expr)).await;
                if self.finished() {
                    return;
                }
                Box::pin(self.c_on_expr(r#in)).await;
            }
            Expr::Substring {
                expr,
                substring_from,
                substring_for,
                ..
            } => {
                Box::pin(self.c_on_expr(&expr)).await;
                if self.finished() {
                    return;
                }
                if let Some(from) = substring_from {
                    Box::pin(self.c_on_expr(&from)).await;
                }
                if let Some(f) = substring_for {
                    Box::pin(self.c_on_expr(&f)).await;
                }
            }
            Expr::Trim {
                expr,

                trim_what,
                trim_characters,
                ..
            } => {
                Box::pin(self.c_on_expr(&expr)).await;
                if self.finished() {
                    return;
                }
                if let Some(what) = trim_what {
                    Box::pin(self.c_on_expr(&what)).await;
                    if self.finished() {
                        return;
                    }
                }
                if let Some(cs) = trim_characters {
                    for c in cs.iter() {
                        Box::pin(self.c_on_expr(&c)).await;
                        if self.finished() {
                            return;
                        }
                    }
                }
            }
            Expr::Overlay {
                expr,
                overlay_what,
                overlay_from,
                overlay_for,
            } => {
                Box::pin(self.c_on_expr(&expr)).await;
                if self.finished() {
                    return;
                }
                Box::pin(self.c_on_expr(&overlay_what)).await;
                if self.finished() {
                    return;
                }
                Box::pin(self.c_on_expr(&overlay_from)).await;
                if self.finished() {
                    return;
                }
                if let Some(overlay_for) = overlay_for {
                    Box::pin(self.c_on_expr(&overlay_for)).await;
                }
            }
            Expr::Collate { expr, .. } => {
                Box::pin(self.c_on_expr(&expr)).await;
            }
            Expr::Nested(expr) => {
                Box::pin(self.c_on_expr(&expr)).await;
            }
            Expr::Value(..) => {
                // literal value
                // nothing to completion
            }
            Expr::IntroducedString { .. } => {
                // literal value
            }
            Expr::TypedString { .. } => {
                // literal value
            }
            Expr::MapAccess { .. } => {
                // clickhouse
            }
            Expr::Function(function) => self.c_on_function_def(function).await,

            Expr::Method(_method) => {
                // TODO
            }
            Expr::Case {
                operand,
                conditions,
                results,
                else_result,
            } => {
                if let Some(operand) = operand {
                    Box::pin(self.c_on_expr(&operand)).await;
                    if self.finished() {
                        return;
                    }
                }
                for c in conditions.iter() {
                    Box::pin(self.c_on_expr(c)).await;
                    if self.finished() {
                        return;
                    }
                }
                for r in results.iter() {
                    Box::pin(self.c_on_expr(r)).await;
                    if self.finished() {
                        return;
                    }
                }
                if let Some(e) = else_result {
                    Box::pin(self.c_on_expr(e)).await;
                }
            }

            Expr::Exists { subquery, .. } => {
                Box::pin(self.c_on_query(&subquery)).await;
            }
            Expr::Subquery(query) => {
                Box::pin(self.c_on_query(&query)).await;
            }
            Expr::GroupingSets(vec) => {
                for v in vec.iter() {
                    for e in v.iter() {
                        Box::pin(self.c_on_expr(e)).await;
                        if self.finished() {
                            return;
                        }
                    }
                }
            }
            Expr::Cube(vec) => {
                for v in vec.iter() {
                    for e in v.iter() {
                        Box::pin(self.c_on_expr(e)).await;
                        if self.finished() {
                            return;
                        }
                    }
                }
            }
            Expr::Rollup(vec) => {
                for v in vec.iter() {
                    for e in v.iter() {
                        Box::pin(self.c_on_expr(e)).await;
                        if self.finished() {
                            return;
                        }
                    }
                }
            }
            Expr::Tuple(vec) => {
                for e in vec.iter() {
                    Box::pin(self.c_on_expr(e)).await;
                    if self.finished() {
                        return;
                    }
                }
            }
            Expr::Struct { .. } => {
                // bitquery not support
            }
            Expr::Named { .. } => {
                // bitquery not support
            }
            Expr::Dictionary(..) => {
                // duckdb not support
            }
            Expr::Map(map) => {
                // duckdb not support
            }
            Expr::Subscript { expr, subscript } => {
                Box::pin(self.c_on_expr(&expr)).await;
                if self.finished() {
                    return;
                }
                match subscript.as_ref() {
                    Subscript::Index { index } => {
                        Box::pin(self.c_on_expr(&index)).await;
                    }
                    Subscript::Slice {
                        lower_bound,
                        upper_bound,
                        stride,
                    } => {
                        if let Some(lower_bound) = lower_bound {
                            Box::pin(self.c_on_expr(&lower_bound)).await;
                            if self.finished() {
                                return;
                            }
                        }
                        if let Some(upper_bound) = upper_bound {
                            Box::pin(self.c_on_expr(&upper_bound)).await;
                            if self.finished() {
                                return;
                            }
                        }
                        if let Some(stride) = stride {
                            Box::pin(self.c_on_expr(&stride)).await;
                            if self.finished() {
                                return;
                            }
                        }
                    }
                }
            }
            Expr::Array(array) => {
                for e in array.elem.iter() {
                    Box::pin(self.c_on_expr(e)).await;
                    if self.finished() {
                        return;
                    }
                }
            }
            Expr::Interval(interval) => {
                Box::pin(self.c_on_expr(&interval.value)).await;
            }
            Expr::MatchAgainst { columns, .. } => {
                if let Some(this_table) = self.scopes.this_table() {
                    for c in columns.iter() {
                        self.c_on_column(&this_table, c).await;
                        if self.finished() {
                            return;
                        }
                    }
                }
            }
            Expr::Wildcard(..) => {
                // nothing here
            }
            Expr::QualifiedWildcard(..) => {
                // nothing here
            }
            Expr::OuterJoin(expr) => {
                Box::pin(self.c_on_expr(&expr)).await;
            }
            Expr::Prior(expr) => {
                Box::pin(self.c_on_expr(&expr)).await;
            }
            Expr::Lambda(lambda_function) => {
                // databricks no support
            }
        }
    }
    pub(crate) async fn c_on_order_by_expr(&self, orderby: &OrderByExpr) {
        self.c_on_expr(&orderby.expr);
    }

    pub(crate) async fn c_on_column(&self, table_name: &ObjectName, col: &Ident) {
        if self.in_pos(&col.span) {
            let (db, schema, table_name) = self.extract_db_schme_item(table_name);
            *self.result.write().unwrap() = self
                .helper
                .list_column(db, schema, table_name)
                .await
                .map_err(|e| {
                    log::error!("list column error:{:?}", e);
                    self.set_has_err();
                })
                .ok()
                .unwrap_or_default()
                .into_iter()
                .map(|x| CompletionItem {
                    kind: CompletionKind::Table,
                    item: x.field.into(),
                })
                .collect::<Vec<_>>();
        }
    }

    pub(crate) async fn c_on_table_trigger(&self, table_name: &ObjectName, trigger_name: &Ident) {
        if self.in_pos(&trigger_name.span) {
            let (db, schema, table_name) = self.extract_db_schme_item(table_name);
            *self.result.write().unwrap() = self
                .helper
                .list_table_trigger(db, schema, table_name)
                .await
                .map_err(|e| {
                    log::error!("list_table_trigger failed,err:{:?}", e);
                    self.set_has_err();
                })
                .ok()
                .unwrap_or_default()
                .into_iter()
                .map(|x| CompletionItem {
                    item: x.into(),
                    kind: CompletionKind::Trigger,
                })
                .collect();
        }
    }

    // completion 假设  database.schema
    pub(crate) async fn c_on_db_schema(&self, name: &ObjectName) {
        match name.0.len() {
            2 => {
                let schema = &name.0[0];
                if self.in_pos(&schema.span) {
                    *self.result.write().unwrap() = self
                        .helper
                        .list_schema(None)
                        .await
                        .map_err(|e| {
                            log::error!("list schema error:{:?}", e);
                            self.set_has_err();
                        })
                        .ok()
                        .unwrap_or_default()
                        .into_iter()
                        .map(|x| CompletionItem {
                            kind: CompletionKind::Schema,
                            item: x.into(),
                        })
                        .collect::<Vec<_>>();
                }
            }
            3 => {
                let db = &name.0[0];
                if self.in_pos(&db.span) {
                    *self.result.write().unwrap() = self
                        .helper
                        .list_database()
                        .await
                        .map_err(|e| {
                            log::error!("list db error:{:?}", e);
                            self.set_has_err();
                        })
                        .ok()
                        .unwrap_or_default()
                        .into_iter()
                        .map(|x| CompletionItem {
                            kind: CompletionKind::Database,
                            item: x.into(),
                        })
                        .collect::<Vec<_>>();
                }
                let schema = &name.0[1];
                if self.in_pos(&schema.span) {
                    *self.result.write().unwrap() = self
                        .helper
                        .list_schema(Some(&db.value))
                        .await
                        .map_err(|e| {
                            log::error!("list schema error:{:?}", e);
                            self.set_has_err();
                        })
                        .ok()
                        .unwrap_or_default()
                        .into_iter()
                        .map(|x| CompletionItem {
                            kind: CompletionKind::Schema,
                            item: x.into(),
                        })
                        .collect::<Vec<_>>();
                }
            }
            1 => {
                // nothing here
            }
            _ => {
                log::error!("object name too long,{:?}", name);
            }
        }
    }

    // completion on table or view.
    pub(crate) async fn c_on_table_or_view(&self, name: &ObjectName) {
        self.c_on_view(name).await;
        let x1 = std::mem::take(self.result.write().unwrap().deref_mut());
        self.c_on_table(name).await;
        self.result.write().unwrap().extend(x1.into_iter());
    }

    pub(crate) async fn c_on_table(&self, table_name: &ObjectName) {
        println!("!!!!!!!!!!!!!!!!!:{:?}", table_name);
        self.c_on_db_schema(table_name).await;
        if self.finished() {
            return;
        }
        if !table_name
            .0
            .last()
            .map(|x| self.in_pos(&x.span))
            .unwrap_or(false)
        {
            return;
        }

        // here match
        let (db, schema, table_name) = self.extract_db_schme_item(table_name);
        *self.result.write().unwrap() = self
            .helper
            .list_table(db, schema)
            .await
            .map_err(|e| {
                log::error!("list table failed,err:{:?}", e);
                self.set_has_err();
            })
            .ok()
            .unwrap_or_default()
            .into_iter()
            .map(|x| CompletionItem {
                item: x.into(),
                kind: CompletionKind::Table,
            })
            .collect::<Vec<_>>();
    }
    pub(crate) fn c_from_table(&self, from_table: &FromTable) -> ScopesGuard {
        let tables = match from_table {
            FromTable::WithFromKeyword(vec) => vec,
            FromTable::WithoutKeyword(vec) => vec,
        };
        // for t in tables.iter() {
        //     let m = HashMap::new();
        //     self.c_on_table_factor(&t.relation);
        //     if self.oked() {
        //         break;
        //     }
        //     for j in t.joins.iter() {
        //         self.c_on_join(j);
        //         if self.oked() {
        //             break;
        //         }
        //     }
        // }
        unimplemented!()
    }

    pub(crate) async fn c_on_select(&self, s: &Select) -> anyhow::Result<Vec<TempTableColumn>> {
        unimplemented!()
    }

    pub(crate) async fn c_on_query(&self, query: &Query) -> anyhow::Result<Vec<TempTableColumn>> {
        let mut temp_tables = HashMap::new();
        if let Some(with) = query.with.as_ref() {
            for c in with.cte_tables.iter() {
                let cols = Box::pin(self.c_on_query(&c.query)).await?;
                temp_tables.insert(c.alias.name.value.clone(), TempTable { columns: cols });
            }
        }
        let s = Scope::from_temp_tables(temp_tables);
        let _guard = self.scopes.push(s);
        let ret = vec![];
        // handle a lot of
        if let Some(orderby) = query.order_by.as_ref() {
            for e in orderby.exprs.iter() {
                self.c_on_order_by_expr(e).await;
                if self.finished() {
                    return anyhow::Ok(ret);
                }
            }
        }
        // limit and offset can't filter on column values
        // so we don't need handle limit and offset
        // althought it is a expr,But I think it is literal.

        // limit by is not support right now.
        let _ = &query.limit_by;

        return Box::pin(self.c_on_set_expr(&query.body)).await;
    }

    pub(crate) async fn c_on_set_expr(
        &self, set: &SetExpr,
    ) -> anyhow::Result<Vec<TempTableColumn>> {
        match set {
            SetExpr::Select(select) => {
                return self.c_on_select(select).await;
            }
            SetExpr::Query(query) => {
                return Box::pin(self.c_on_query(&query)).await;
            }
            SetExpr::SetOperation { left, right, .. } => {
                Box::pin(self.c_on_set_expr(&left)).await;
                return Box::pin(self.c_on_set_expr(&right)).await;
            }
            SetExpr::Values(values) => {
                if values.rows.len() > 0 {
                    let last = values.rows.len() - 1;
                    for (index, es) in values.rows.iter().enumerate() {
                        for e in es.iter() {
                            self.c_on_expr(e).await;
                            if self.finished() {
                                return anyhow::Ok(vec![]);
                            }
                        }
                        // last one we will return anyway
                        if index == last {
                            let cols = (0..es.len())
                                .map(|x| TempTableColumn {
                                    name: format!("column_{}", x),
                                })
                                .collect::<Vec<_>>();
                            return anyhow::Ok(cols);
                        }
                    }
                }
            }
            SetExpr::Insert(statement) => {
                self.completion_on_statement(statement).await;
            }
            SetExpr::Update(statement) => {
                self.completion_on_statement(statement).await;
            }
            SetExpr::Table(table) => {
                if table.table_name.is_none() {
                    log::error!("null table name for table command,:{:?}", table.deref());
                }
                let cols = self
                    .helper
                    .list_column(
                        None,
                        table.schema_name.as_deref(),
                        table
                            .table_name
                            .as_ref()
                            .map(|x| x.as_str())
                            .unwrap_or_default(),
                    )
                    .await
                    .map_err(|e| {
                        log::error!("list_column failed,err:{:?}", e);
                        self.set_has_err();
                    })
                    .ok()
                    .unwrap_or_default()
                    .into_iter()
                    .map(|x| TempTableColumn {
                        name: x.field.clone(),
                    })
                    .collect();
                return anyhow::Ok(cols);
            }
        }
        anyhow::Ok(vec![])
    }

    pub(crate) async fn c_on_join(&self, j: &Join) {}

    pub(crate) fn c_on_table_factor(&self, f: &TableFactor) {
        #[allow(unused)]
        match f {
            TableFactor::Table {
                name,
                alias,
                args,
                with_hints,
                version,
                with_ordinality,
                partitions,
                json_path,
            } => todo!(),
            TableFactor::Derived {
                lateral,
                subquery,
                alias,
            } => todo!(),
            TableFactor::TableFunction { expr, alias } => todo!(),
            TableFactor::Function {
                lateral,
                name,
                args,
                alias,
            } => todo!(),
            TableFactor::UNNEST {
                alias,
                array_exprs,
                with_offset,
                with_offset_alias,
                with_ordinality,
            } => todo!(),
            TableFactor::JsonTable {
                json_expr,
                json_path,
                columns,
                alias,
            } => todo!(),
            TableFactor::OpenJsonTable {
                json_expr,
                json_path,
                columns,
                alias,
            } => todo!(),
            TableFactor::NestedJoin {
                table_with_joins,
                alias,
            } => todo!(),
            TableFactor::Pivot {
                table,
                aggregate_functions,
                value_column,
                value_source,
                default_on_null,
                alias,
            } => todo!(),
            TableFactor::Unpivot {
                table,
                value,
                name,
                columns,
                alias,
            } => todo!(),
            TableFactor::MatchRecognize {
                table,
                partition_by,
                order_by,
                measures,
                rows_per_match,
                after_match_skip,
                pattern,
                symbols,
                alias,
            } => todo!(),
        }
    }
    pub(crate) async fn c_on_procedure(&self, name: &ObjectName) {
        self.c_on_db_schema(name).await;
        if self.finished() {
            return;
        }
        if self.last_name_in_pos(name) {
            let (db, schema, _) = self.extract_db_schme_item(name);
            *self.result.write().unwrap() = self
                .helper
                .list_procedure(db, schema)
                .await
                .map_err(|e| {
                    log::error!("list function error:{:?}", e);
                    self.set_has_err();
                })
                .ok()
                .unwrap_or_default()
                .into_iter()
                .map(|x| CompletionItem {
                    kind: CompletionKind::Function,
                    item: x.into(),
                })
                .collect::<Vec<_>>();
        }
    }
    pub(crate) fn last_name_in_pos(&self, name: &ObjectName) -> bool {
        name.0.last().map(|x| self.in_pos(&x.span)).unwrap_or(false)
    }

    pub(crate) async fn c_on_pragma(&self, name: &ObjectName) {}

    pub(crate) async fn c_on_function_def(&self, name: &Function) {}
    pub(crate) async fn c_on_function(&self, name: &ObjectName) {
        self.c_on_db_schema(name).await;
        if self.finished() {
            return;
        }
        if self.last_name_in_pos(name) {
            let (db, schema, _) = self.extract_db_schme_item(name);
            *self.result.write().unwrap() = self
                .helper
                .list_function(db, schema)
                .await
                .map_err(|e| {
                    log::error!("list function error:{:?}", e);
                    self.set_has_err();
                })
                .ok()
                .unwrap_or_default()
                .into_iter()
                .map(|x| CompletionItem {
                    kind: CompletionKind::Function,
                    item: x.into(),
                })
                .collect::<Vec<_>>();
        }
    }
}
