#![allow(unused)]

use std::vec;

use oracle::sql_type::Object;
use sqlparser::ast::{
    AlterTableOperation, CommentObject, Expr, FromTable, Ident, Join, MySQLColumnPosition,
    ObjectName, ObjectType, OrderByExpr, Query, ShowCreateObject, Spanned, TableConstraint,
    TableFactor,
};

use crate::{
    completion::{CompletionItem, CompletionKind},
    sqlparser_completion::{CompletionStament, Scope, ScopesGuard},
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
    pub(crate) async fn c_on_index(&self, name: &ObjectName) {
        if self.in_pos(&name.span()) {
            *self.result.borrow_mut() = self
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
    pub(crate) async fn c_on_postgres_rule(&self, table_name: &ObjectName, rule: &Ident) {
        if !self.in_pos(&rule.span) {
            return;
        }
        let (db, schema, table) = self.extract_db_schme_item(table_name);
        *self.result.borrow_mut() = self
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
            *self.result.borrow_mut() = self
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
                    TableConstraint::PrimaryKey {
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
                        // TODO I don't understand this.
                        //  https://www.w3schools.com/sql/sql_check.asp
                        self.scopes.push(Scope::from_this_table(table_name.clone()));
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
                    self.c_on_trigger(table_name, &ObjectName(vec![name.clone()]))
                        .await;
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
                    self.c_on_trigger(table_name, &ObjectName(vec![name.clone()]))
                        .await;
                }
                AlterTableOperation::EnableRowLevelSecurity => {
                    // nothing to here
                }
                AlterTableOperation::EnableRule { name } => {
                    self.c_on_postgres_rule(table_name, name).await;
                }
                AlterTableOperation::EnableTrigger { name } => {
                    self.c_on_trigger(table_name, &ObjectName(vec![name.clone()]))
                        .await;
                }
                AlterTableOperation::RenamePartitions { .. } => {
                    // TODO
                }
                AlterTableOperation::AddPartitions { .. } => {
                    // TODO
                }
                AlterTableOperation::DropPartitions { .. } => {
                    // TODO
                }
                AlterTableOperation::RenameColumn {
                    old_column_name,
                    new_column_name,
                } => {
                    self.c_on_column(table_name, old_column_name).await;
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
                    data_type,
                    options,
                    column_position,
                } => {
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
                }
                AlterTableOperation::SwapWith { .. } => {
                    // no support
                }
                AlterTableOperation::SetTblProperties { .. } => {
                    // no special handle
                }
                AlterTableOperation::OwnerTo { new_owner } => {
                    // no need
                }
            }
        }
    }
    pub(crate) async fn c_on_view(&self, view: &ObjectName) {}
    pub(crate) async fn c_on_expr(&self, expr: &Expr) {}
    pub(crate) async fn c_on_order_by_expr(&self, orderby: &OrderByExpr) {}

    pub(crate) async fn c_on_column(&self, table_name: &ObjectName, col: &Ident) {
        if self.in_pos(&col.span) {
            let (db, schema, table_name) = self.extract_db_schme_item(table_name);

            *self.result.borrow_mut() = self
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

    pub(crate) async fn c_on_trigger(&self, table_name: &ObjectName, trigger_name: &ObjectName) {
        unimplemented!()
    }
    pub(crate) async fn c_on_db_schema(&self, name: &ObjectName) {
        match name.0.len() {
            2 => {
                let schema = &name.0[0];
                if self.in_pos(&schema.span) {
                    *self.result.borrow_mut() = self
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
                    *self.result.borrow_mut() = self
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
                    *self.result.borrow_mut() = self
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
    pub(crate) async fn c_on_table(&self, table_name: &ObjectName) {
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
        *self.result.borrow_mut() = self
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
    pub(crate) async fn c_on_query(&self, q: &Query) {}
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
            *self.result.borrow_mut() = self
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

    pub(crate) async fn c_on_function(&self, name: &ObjectName) {
        self.c_on_db_schema(name).await;
        if self.finished() {
            return;
        }

        if self.last_name_in_pos(name) {
            let (db, schema, _) = self.extract_db_schme_item(name);
            *self.result.borrow_mut() = self
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
