use sqlparser::ast::ColumnOption;

use sqlparser::ast::*;

use crate::sqlparser_completion::{FromTableOrTemp, NearNestTableNamed, ThisTableEnum};
use crate::{
    language_server::{CompletionItem, CompletionKind},
    sqlparser_completion::{CompletionStament, Scope, ScopesGuard, TempTable, TempTableColumn},
};
use std::collections::HashMap;
use std::ops::Deref;
use std::sync::Arc;

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
    pub(crate) async fn c_on_trigger_exec_body(&mut self, _body: &TriggerExecBody) {
        // sqlparser don't support parse trigger body??
    }
    pub(crate) async fn c_on_role(&mut self, role: &Ident) {
        if !self.in_pos(&role.span) {
            return;
        }
        self.result = Some(
            self.helper
                .list_role()
                .await
                .map_err(|e| {
                    log::error!("list_role failed,err:{:?}", e);
                    self.set_has_err();
                })
                .unwrap_or_default()
                .into_iter()
                .map(|x| CompletionItem {
                    item: x.into(),
                    desc: None,
                    kind: CompletionKind::Role,
                })
                .collect(),
        );
    }

    pub(crate) async fn c_on_mysql_trigger(&mut self, object_name: &ObjectName) {
        self.c_on_db_schema(object_name).await;
        if self.finished() {
            return;
        }
        if !object_name
            .0
            .last()
            .map(|x| self.in_pos(&x.span))
            .unwrap_or(false)
        {
            return;
        }

        /*
           在 MySQL 中，SHOW CREATE TRIGGER 语句用于显示创建指定触发器的 CREATE TRIGGER 语句
           。其基本语法为：
           sql复制
           SHOW CREATE TRIGGER trigger_name;
           其中，trigger_name 是触发器的名称。需要注意的是，SHOW CREATE TRIGGER 语句不支持直接指定数据库名称。如果要查看特定数据库中的触发器定义，需要先切换到该数据库，或者在触发器名称中明确指定数据库名称
           。
           例如，如果要查看数据库 mydb 中名为 my_trigger 的触发器定义，可以先切换到该数据库：
        */

        self.result = Some(
            self.helper
                .mysql_show_create_trigger()
                .await
                .map_err(|x| log::error!("mysql_show_create_trigger failed,err:{}", x))
                .unwrap_or_default(),
        );
    }

    pub(crate) async fn c_on_mysql_show_create(
        &mut self, show_create: ShowCreateObject, name: &ObjectName,
    ) {
        match show_create {
            ShowCreateObject::Event => {
                // not support rigth now.
            }
            ShowCreateObject::Function => self.c_on_function(&name).await,
            ShowCreateObject::Procedure => self.c_on_procedure(&name).await,
            ShowCreateObject::Table => self.c_on_table(&name).await,
            ShowCreateObject::Trigger => self.c_on_mysql_trigger(&name).await,
            ShowCreateObject::View => self.c_on_view(&name).await,
        }
    }
    pub(crate) async fn c_on_variable(&mut self, var: &Ident) {
        if !self.in_pos(&var.span) {
            return;
        }
        self.result = Some(
            self.helper
                .list_variable()
                .await
                .map_err(|e| {
                    log::error!("list_variable failed,err:{:?}", e);
                    self.set_has_err();
                })
                .unwrap_or_default()
                .into_iter()
                .map(|x| CompletionItem {
                    item: x.into(),
                    kind: CompletionKind::Variable,
                    desc: None,
                })
                .collect(),
        );
    }
    pub(crate) async fn c_on_policy(&mut self, table_name: &ObjectName, name: &Ident) {
        if !self.in_pos(&name.span) {
            return;
        }
        let (db, schema, table) = self.extract_db_schme_item(table_name);
        self.result = Some(
            self.helper
                .postgres_table_policy(db, schema, table)
                .await
                .map_err(|e| log::error!("postgres_table_policy failed,err:{:?}", e))
                .unwrap_or_default()
                .into_iter()
                .map(|x| CompletionItem {
                    item: x.into(),
                    kind: CompletionKind::Policy,
                    desc: None,
                })
                .collect(),
        );
    }

    pub(crate) async fn c_on_type_drop(&mut self, typ: ObjectType, name: &ObjectName) {
        if !self.in_pos(&name.span()) {
            return;
        }
        match typ {
            ObjectType::Table => self.c_on_table(name).await,
            ObjectType::View => self.c_on_view(name).await,
            ObjectType::Index => self.c_on_index(name).await,
            ObjectType::Schema | ObjectType::Database => {
                // c_on_db_schema 假设形式是 database.schema.table
                // 但是在这种情况下 ident肯定没这么长
                // 所以加一个假的
                let mut name = name.0.clone();
                name.push(Ident::new("fake"));
                self.c_on_db_schema(&ObjectName(name)).await;
            }
            ObjectType::Role => {
                // rule not support
            }
            ObjectType::Sequence => {
                // rule not support
            }
            ObjectType::Stage => {
                // rule not support
            }
            ObjectType::Type => {
                // rule not support
            }
        }
    }
    pub(crate) async fn c_on_privileges_action(&mut self, tables: &[ObjectName], a: &Action) {
        match a {
            Action::Insert { columns }
            | Action::References { columns }
            | Action::Select { columns }
            | Action::Update { columns } => {
                if let Some(columns) = columns {
                    if columns.iter().any(|x| self.in_pos(&x.span)) {
                        // we simple suggest all column lies in talbe.
                        let mut cols = Vec::new();
                        for table in tables.iter() {
                            let (db, schema, table) = self.extract_db_schme_item(table);
                            let cs = self
                                .helper
                                .list_column(db, schema, table)
                                .await
                                .map_err(|e| {
                                    log::error!("list_column failed,err:{:?}", e);
                                    self.set_has_err();
                                })
                                .ok()
                                .unwrap_or_default();
                            cols.extend(cs.into_iter());
                        }
                        self.result = Some(cols);
                    }
                }
            }
            _ => {}
        }
    }
    pub(crate) async fn c_on_postgres_comment_object(
        &mut self, object_type: CommentObject, name: &ObjectName,
    ) {
        if !self.in_pos(&name.span()) {
            return;
        }
        match object_type {
            CommentObject::Table => self.c_on_table(name).await,
            CommentObject::Schema | CommentObject::Database => {
                // c_on_db_schema 假设形式是 database.schema.table
                // 但是在这种情况下 ident肯定没这么长
                // 所以加一个假的
                let mut name = name.0.clone();
                name.push(Ident::new("fake"));
                self.c_on_db_schema(&ObjectName(name)).await;
            }
            CommentObject::Role => {
                // not support right now
            }
            CommentObject::Column => {
                if name.0.len() >= 2 {
                    let table = (&name.0[..name.0.len() - 1]).to_vec();
                    self.c_on_column(&ObjectName(table), name.0.last().unwrap())
                        .await;
                }
            }
            CommentObject::Extension => {
                // not support right now
            }
            CommentObject::User => {
                // not support right now
            }
        }
    }

    pub(crate) async fn c_on_column_mysql_position(
        &mut self, table_name: &ObjectName, column_position: Option<&MySQLColumnPosition>,
    ) {
        if let Some(column_position) = column_position {
            match column_position {
                MySQLColumnPosition::First => {}
                MySQLColumnPosition::After(col) => self.c_on_column(table_name, col).await,
            }
        }
    }

    pub(crate) async fn c_on_prepare(&mut self, name: &ObjectName) {
        match name.0.len() {
            1 => {
                self.result = Some(
                    self.helper
                        .list_prepare_name(self.query_id.clone())
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
                            desc: None,
                        })
                        .collect::<Vec<_>>(),
                )
            }

            _ => {
                //TODO https://learn.microsoft.com/en-us/sql/relational-databases/stored-procedures/execute-a-stored-procedure
                // mssql 可以指定数据库
                log::error!("handled name:{:?}", name);
            }
        }
    }
    pub(crate) async fn c_on_partition_expr(&mut self, table_name: &ObjectName, p: &Expr) {
        match p {
            Expr::Identifier(x) => {
                let (db, schema, table) = self.extract_db_schme_item(table_name);

                if self.in_pos(&x.span) {
                    self.result = Some(
                        self.helper
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
                                desc: None,
                            })
                            .collect::<Vec<_>>(),
                    );
                }
            }
            _ => {
                log::error!("unhandled partation type:{:?}", p);
            }
        }
    }
    pub(crate) async fn c_on_index(&mut self, name: &ObjectName) {
        if !self.in_pos(&name.span()) {
            return;
        }
        let mut schema = None;
        match name.0.len() {
            2 => {
                schema = Some(name.0[1].value.as_str());
            }
            1 => {}
            _ => {
                log::error!("should have this kind of index name:{:?}", name);
            }
        }
        self.result = Some(
            self.helper
                .list_index(schema)
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
                    desc: None,
                })
                .collect::<Vec<_>>(),
        );
    }

    pub(crate) fn extract_db_schme_item<'a>(
        &mut self, item_name: &'a ObjectName,
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

    pub(crate) async fn c_on_postgres_rule(&mut self, table_name: &ObjectName, rule: &Ident) {
        if !self.in_pos(&rule.span) {
            return;
        }
        let (db, schema, table) = self.extract_db_schme_item(table_name);
        self.result = Some(
            self.helper
                .list_postgres_rule(db, schema, &table)
                .await
                .map_err(|err| {
                    log::error!("list_postgres_rule,err:{}", err);
                    self.set_has_err();
                })
                .ok()
                .unwrap_or_default()
                .into_iter()
                .map(|x| CompletionItem {
                    item: x.into(),
                    kind: CompletionKind::Rule,
                    desc: None,
                })
                .collect::<Vec<_>>(),
        );
    }
    pub(crate) async fn c_on_constraint(&mut self, table_name: &ObjectName, name: &Ident) {
        if self.in_pos(&name.span) {
            let (db, schema, table_name) = self.extract_db_schme_item(table_name);
            self.result = Some(
                self.helper
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
                        desc: None,
                    })
                    .collect::<Vec<_>>(),
            );
        }
    }

    pub(crate) async fn c_on_alert_table(
        &mut self, table_name: &ObjectName, operations: &Vec<AlterTableOperation>,
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
                        if let Some(name) = name {
                            if self.in_pos(&name.span) {
                                self.set_on_def();
                                return;
                            }
                        }
                        self.c_on_expr(expr).await;
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
                    self.c_on_table_trigger(table_name, name).await;
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
                        match p {
                            Partition::Identifier(ident) => {
                                if self.in_pos(&ident.span) {
                                    self.set_on_def();
                                    return;
                                }
                            }
                            Partition::Expr(expr) => {
                                self.c_on_expr(expr).await;
                            }
                            Partition::Part(expr) => {
                                self.c_on_expr(expr).await;
                            }
                            Partition::Partitions(vec) => {
                                for expr in vec.iter() {
                                    self.c_on_expr(expr).await;
                                    if self.finished() {
                                        return;
                                    }
                                }
                            }
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
                    self.c_on_column(table_name, col_name).await;
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
                AlterTableOperation::ClusterBy { exprs: _ } => {
                    //  Snowflake
                }
                AlterTableOperation::DropClusteringKey => {}
                AlterTableOperation::SuspendRecluster => {}
                AlterTableOperation::ResumeRecluster => {}
            }
        }
    }
    pub(crate) async fn c_on_view(&mut self, name: &ObjectName) {
        self.c_on_db_schema(name).await;
        if self.finished() {
            return;
        }
        if !name.0.last().map(|x| self.in_pos(&x.span)).unwrap_or(false) {
            return;
        }
        // here match
        let (db, schema, _name) = self.extract_db_schme_item(name);
        self.result = Some(
            self.helper
                .list_view(db, schema)
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
                    desc: None,
                })
                .collect::<Vec<_>>(),
        );
    }

    pub(crate) async fn c_on_expr_compund_identifier(&mut self, idents: &[Ident]) {
        // we must access table's column???
        match idents.len() {
            1 => {
                // maybe never execute to here???
                self.c_on_expr_ident(&idents[0]).await;
            }
            2 => {
                let table = &idents[0];
                if self.in_pos(&table.span) {
                    // handle name
                    self.c_on_table_or_view(&ObjectName(vec![table.clone()]))
                        .await;
                    return;
                }
                if let Some(table) = self.scopes.nearest_table_named(&table) {
                    match table {
                        NearNestTableNamed::Alias { table } => {
                            self.c_on_column(&table, &idents[1]).await;
                        }
                        NearNestTableNamed::Temp { table } => {
                            self.result =
                                Some(table.columns.iter().map(CompletionItem::from).collect());
                        }
                    }
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

    pub(crate) async fn c_on_expr_ident(&mut self, ident: &Ident) {
        if !self.in_pos(&ident.span) {
            return;
        }
        log::trace!("hit idenfier:{:?}", ident);
        if let Some(this_table) = self.scopes.this_table() {
            log::trace!("this table found:{:?}", this_table);
            match this_table {
                ThisTableEnum::Table { name } => {
                    self.c_on_column(&name, ident).await;
                }
                ThisTableEnum::Temp { temp } => {
                    self.result = Some(temp.columns.iter().map(CompletionItem::from).collect());
                }
            }
        } else {
            // another completion
            let tables = self.scopes.nearest_from_tables();
            log::trace!("from tables:{:?}", tables);
            for from in tables.iter() {
                let mut old = self.result.take().unwrap_or_default();
                match from {
                    // we only looking for unnamed
                    // because named need access by `table.column`
                    FromTableOrTemp::Unnamed { table } => {
                        old.extend(
                            table
                                .columns
                                .to_vec()
                                .into_iter()
                                .map(CompletionItem::from)
                                .collect::<Vec<_>>()
                                .into_iter(),
                        );
                        self.result = Some(old);
                    }
                    FromTableOrTemp::FromTable { table } => {
                        let (db, schema, table) = self.extract_db_schme_item(table);
                        let m = self
                            .helper
                            .list_column(db, schema, table)
                            .await
                            .map_err(|e| {
                                log::error!("list_column failed,err:{}", e);
                                self.set_has_err();
                            })
                            .ok()
                            .unwrap_or_default();
                        old.extend(m.into_iter());
                        self.result = Some(old);
                    }
                    FromTableOrTemp::Temp { table } => {
                        old.extend(
                            table
                                .columns
                                .to_vec()
                                .into_iter()
                                .map(CompletionItem::from)
                                .collect::<Vec<_>>()
                                .into_iter(),
                        );
                        self.result = Some(old);
                    }
                }
            }
        }
    }

    pub(crate) async fn c_on_column_option_def(&mut self, def: &ColumnOptionDef) {
        if let Some(name) = &def.name {
            if self.in_pos(&name.span) {
                self.set_on_def();
                return;
            }
        }
        match &def.option {
            ColumnOption::Null => {}
            ColumnOption::NotNull => {}
            ColumnOption::Default(expr) => {
                Box::pin(self.c_on_expr(expr)).await;
            }
            ColumnOption::Materialized(_expr) => {
                // ClickHouse
            }
            ColumnOption::Ephemeral(expr) => {
                if let Some(expr) = expr {
                    Box::pin(self.c_on_expr(expr)).await;
                }
            }
            ColumnOption::Alias(expr) => {
                Box::pin(self.c_on_expr(expr)).await;
            }
            ColumnOption::Unique { .. } => {}
            ColumnOption::ForeignKey {
                foreign_table,
                referred_columns,
                ..
            } => {
                for r in referred_columns.iter() {
                    self.c_on_column(foreign_table, r).await;
                    if self.finished() {}
                }
            }
            ColumnOption::Check(expr) => {
                self.c_on_expr(expr).await;
            }
            ColumnOption::DialectSpecific(_vec) => {}
            ColumnOption::CharacterSet(_object_name) => {}
            ColumnOption::Comment(_) => {}
            ColumnOption::OnUpdate(expr) => {
                self.c_on_expr(expr).await;
            }
            ColumnOption::Generated { .. } => {}
            ColumnOption::Options(_vec) => {}
            ColumnOption::Identity(_identity_property_kind) => {}
            ColumnOption::OnConflict(_keyword) => {}
            ColumnOption::Policy(_column_policy) => {}
            ColumnOption::Tags(_tags_column_option) => {}
        }
    }

    pub(crate) async fn c_on_expr(&mut self, expr: &Expr) {
        log::trace!("completion on:{:?}", expr);
        match expr {
            Expr::Identifier(ident) => self.c_on_expr_ident(ident).await,

            Expr::CompoundIdentifier(vec) => {
                self.c_on_expr_compund_identifier(&vec).await;
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
                let _ = Box::pin(self.c_on_query(&subquery))
                    .await
                    .map(|e| log::error!("a error:{:?}", e))
                    .ok();
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
            Expr::Function(function) => Box::pin(self.c_on_function_call(function)).await,

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
                let _ = Box::pin(self.c_on_query(&subquery))
                    .await
                    .map_err(|e| log::error!("a error:{:?}", e))
                    .ok();
            }
            Expr::Subquery(query) => {
                let _ = Box::pin(self.c_on_query(&query))
                    .await
                    .map_err(|e| log::error!("a error:{:?}", e))
                    .ok();
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
            Expr::Map(..) => {
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
                        match &this_table {
                            ThisTableEnum::Table { name } => {
                                self.c_on_column(name, c).await;
                                if self.finished() {
                                    return;
                                }
                            }
                            ThisTableEnum::Temp { temp: _temp } => {
                                // temp this table only use in table create
                                // I think I should never enter here.
                                log::error!(
                                    "temp this table only use in table create , I think I should never enter here."
                                );
                            }
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
            Expr::Lambda(..) => {
                // databricks no support
            }
        }
    }
    pub(crate) async fn c_on_order_by_expr(&mut self, orderby: &OrderByExpr) {
        self.c_on_expr(&orderby.expr).await;
    }

    pub(crate) async fn c_on_column(&mut self, table_name: &ObjectName, col: &Ident) {
        if self.in_pos(&col.span) {
            let (db, schema, table_name) = self.extract_db_schme_item(table_name);
            self.result = Some(
                self.helper
                    .list_column(db, schema, table_name)
                    .await
                    .map_err(|e| {
                        log::error!("list column error:{:?}", e);
                        self.set_has_err();
                    })
                    .ok()
                    .unwrap_or_default(),
            );
        }
    }

    pub(crate) async fn c_on_table_trigger(
        &mut self, table_name: &ObjectName, trigger_name: &Ident,
    ) {
        if self.in_pos(&trigger_name.span) {
            let (db, schema, table_name) = self.extract_db_schme_item(table_name);
            self.result = Some(
                self.helper
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
                        desc: None,
                    })
                    .collect(),
            );
        }
    }

    // completion 假设  database.schema
    pub(crate) async fn c_on_db_schema(&mut self, name: &ObjectName) {
        match name.0.len() {
            2 => {
                let schema = &name.0[0];
                if self.in_pos(&schema.span) {
                    self.result = Some(
                        self.helper
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
                                desc: None,
                            })
                            .collect::<Vec<_>>(),
                    );
                }
            }
            3 => {
                let db = &name.0[0];
                if self.in_pos(&db.span) {
                    self.result = Some(
                        self.helper
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
                                desc: None,
                            })
                            .collect::<Vec<_>>(),
                    );
                }
                let schema = &name.0[1];
                if self.in_pos(&schema.span) {
                    self.result = Some(
                        self.helper
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
                                desc: None,
                            })
                            .collect::<Vec<_>>(),
                    );
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
    pub(crate) async fn c_on_table_or_view(&mut self, name: &ObjectName) {
        self.c_on_view(name).await;
        let mut x1 = self.result.take().unwrap_or_default();
        self.c_on_table(name).await;
        x1.extend(self.result.take().unwrap_or_default().into_iter());
        self.result = Some(x1);
    }

    pub(crate) async fn c_on_table(&mut self, table_name: &ObjectName) {
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
        let (db, schema, _table_name) = self.extract_db_schme_item(table_name);
        let mut tables = self
            .helper
            .list_table(db, schema)
            .await
            .map_err(|e| {
                log::error!("list table failed,err:{:?}", e);
                self.set_has_err();
            })
            .ok()
            .unwrap_or_default();
        tables.extend(
            self.helper
                .list_view(db, schema)
                .await
                .map_err(|e| {
                    log::error!("list view failed,err:{:?}", e);
                    self.set_has_err();
                })
                .ok()
                .unwrap_or_default(),
        );
        tables.extend(self.scopes.tables().into_iter());
        // here maybe we are suggrest database all schemas.
        tables.extend(self.helper.list_database().await.unwrap_or_default());
        tables.extend(self.helper.list_schema(None).await.unwrap_or_default());

        let tables = tables
            .into_iter()
            .map(|x| CompletionItem {
                item: x.into(),
                kind: CompletionKind::Table,
                desc: None,
            })
            .collect::<Vec<_>>();

        self.result = Some(tables);
    }
    pub(crate) async fn c_from_table(&mut self, from_table: &FromTable) -> ScopesGuard {
        let tables = match from_table {
            FromTable::WithFromKeyword(vec) => vec,
            FromTable::WithoutKeyword(vec) => vec,
        };
        self.c_on_table_with_joins(&tables[..]).await
    }

    pub(crate) async fn c_on_table_with_joins(&mut self, joins: &[TableWithJoins]) -> ScopesGuard {
        for (index, join_tables) in joins.iter().enumerate() {
            let scope = self.c_on_table_with_join(join_tables).await;
            if self.finished() {
                return self.scopes.drop_n(index);
            }
            self.scopes.push_without_guard(scope);
        }

        self.scopes.drop_n(joins.len())
    }

    pub(crate) async fn c_on_table_with_join(&mut self, t: &TableWithJoins) -> Scope {
        let mut scope = Scope::default();
        self.c_on_table_factor(&t.relation, &mut scope).await;
        if self.finished() {
            return scope;
        }
        for join in t.joins.iter() {
            self.c_on_table_factor(&join.relation, &mut scope).await;
            if self.finished() {
                return scope;
            }
        }
        let _guard = self.scopes.push(scope.clone());
        for j in t.joins.iter() {
            match &j.join_operator {
                JoinOperator::Inner(join_constraint) => {
                    self.c_on_join_constraint(join_constraint).await
                }
                JoinOperator::LeftOuter(join_constraint) => {
                    self.c_on_join_constraint(join_constraint).await
                }
                JoinOperator::RightOuter(join_constraint) => {
                    self.c_on_join_constraint(join_constraint).await
                }
                JoinOperator::FullOuter(join_constraint) => {
                    self.c_on_join_constraint(join_constraint).await
                }
                JoinOperator::Semi(join_constraint) => {
                    self.c_on_join_constraint(join_constraint).await
                }
                JoinOperator::LeftSemi(join_constraint) => {
                    self.c_on_join_constraint(join_constraint).await
                }
                JoinOperator::RightSemi(join_constraint) => {
                    self.c_on_join_constraint(join_constraint).await
                }
                JoinOperator::Anti(join_constraint) => {
                    self.c_on_join_constraint(join_constraint).await
                }
                JoinOperator::LeftAnti(join_constraint) => {
                    self.c_on_join_constraint(join_constraint).await
                }
                JoinOperator::RightAnti(join_constraint) => {
                    self.c_on_join_constraint(join_constraint).await
                }
                JoinOperator::CrossApply => {}
                JoinOperator::OuterApply => {}
                JoinOperator::AsOf { .. } => {
                    // snowflake
                }
                JoinOperator::CrossJoin => {}
            }
        }
        return scope;
    }

    pub(crate) async fn c_on_join_constraint(&mut self, c: &JoinConstraint) {
        match c {
            JoinConstraint::On(expr) => {
                self.c_on_expr(expr).await;
            }
            JoinConstraint::Using(vec) => {
                // TODO need handle this.
                let in_pos = vec.iter().any(|x| self.in_pos(&x.span));
                if in_pos {
                    let tables = self.scopes.nearest_from_tables();
                    let mut cs = vec![];
                    for table in tables.iter() {
                        match table {
                            FromTableOrTemp::FromTable { table } => {
                                let (db, schema, table) = self.extract_db_schme_item(table);

                                cs.extend(
                                    self.helper
                                        .list_column(db, schema, table)
                                        .await
                                        .map_err(|e| {
                                            log::error!("list_column failed,err:{}", e);
                                            self.set_has_err();
                                        })
                                        .unwrap_or_default()
                                        .into_iter(),
                                );
                            }
                            FromTableOrTemp::Temp { table } => {
                                cs.extend(table.columns.iter().map(CompletionItem::from));
                            }
                            FromTableOrTemp::Unnamed { table } => {
                                cs.extend(table.columns.iter().map(CompletionItem::from));
                            }
                        }
                    }
                    self.result = Some(cs);
                }
            }
            JoinConstraint::Natural => {}
            JoinConstraint::None => {}
        }
    }

    pub(crate) async fn c_on_select(
        &mut self, selection: &Select,
    ) -> anyhow::Result<Vec<TempTableColumn>> {
        let _gurad = self.c_on_table_with_joins(&selection.from[..]).await;
        if self.finished() {
            return anyhow::Ok(vec![]);
        }
        if let Some(d) = &selection.distinct {
            match d {
                Distinct::Distinct => {}
                Distinct::On(vec) => {
                    for e in vec.iter() {
                        self.c_on_expr(e).await;
                        if self.finished() {
                            return anyhow::Ok(vec![]);
                        }
                    }
                }
            }
        }
        if let Some(t) = &selection.top {
            if let Some(q) = &t.quantity {
                match q {
                    TopQuantity::Expr(expr) => {
                        self.c_on_expr(expr).await;
                    }
                    TopQuantity::Constant(_) => {}
                }
            }
        }

        if let Some(into) = &selection.into {
            if self.in_pos(&into.name.span()) {
                self.set_on_def();
                return anyhow::Ok(vec![]);
            }
        }
        if let Some(selection) = &selection.selection {
            self.c_on_expr(selection).await;
            if self.finished() {
                return anyhow::Ok(vec![]);
            }
        }
        match &selection.group_by {
            GroupByExpr::All(_vec) => {
                // Snowflake ... not supported
            }
            GroupByExpr::Expressions(vec, _vec1) => {
                for e in vec.iter() {
                    self.c_on_expr(e).await;
                    if self.finished() {
                        return anyhow::Ok(vec![]);
                    }
                }
            }
        }
        if let Some(having) = &selection.having {
            self.c_on_expr(having).await;
            if self.finished() {
                return anyhow::Ok(vec![]);
            }
        }
        self.c_on_select_item(&selection.projection).await
    }

    pub(crate) async fn c_on_select_item(
        &mut self, projection: &[SelectItem],
    ) -> anyhow::Result<Vec<TempTableColumn>> {
        let mut ret = Vec::new();
        for p in projection.iter() {
            match p {
                SelectItem::UnnamedExpr(expr) => {
                    self.c_on_expr(expr).await;
                    if self.finished() {
                        return anyhow::Ok(vec![]);
                    }
                    ret.push(TempTableColumn {
                        name: expr.to_string(),
                        typ: None,
                    });
                }
                SelectItem::ExprWithAlias { expr, alias } => {
                    self.c_on_expr(expr).await;
                    if self.finished() {
                        return anyhow::Ok(vec![]);
                    }
                    ret.push(TempTableColumn {
                        name: alias.to_string(),
                        typ: None,
                    });
                }
                SelectItem::QualifiedWildcard(object_name, _wildcard_additional_options) => {
                    self.c_on_table(object_name).await;
                    if self.finished() {
                        return anyhow::Ok(vec![]);
                    }
                    match object_name.0.len() {
                        1 => {
                            let name = self.scopes.nearest_table_named(&object_name.0[0]);
                            match name {
                                Some(x) => match x {
                                    NearNestTableNamed::Alias { table } => {
                                        let (db, schema, table) =
                                            self.extract_db_schme_item(&table);
                                        let f = self
                                            .helper
                                            .list_column(db, schema, table)
                                            .await
                                            .map_err(|e| {
                                                log::error!("list_column failed,err:{:?}", e)
                                            })
                                            .ok()
                                            .unwrap_or_default();
                                        ret.extend(f.iter().map(TempTableColumn::from));
                                    }
                                    NearNestTableNamed::Temp { table } => {
                                        ret.extend(table.columns.iter().map(|x| TempTableColumn {
                                            name: x.name.clone(),
                                            typ: None,
                                        }));
                                    }
                                },
                                None => {
                                    // can't find maybe a error by user
                                }
                            }
                        }
                        2 | 3 => {
                            let (db, schema, table) = self.extract_db_schme_item(&object_name);
                            let f = self
                                .helper
                                .list_column(db, schema, table)
                                .await
                                .map_err(|e| log::error!("list_column failed,err:{:?}", e))
                                .ok()
                                .unwrap_or_default();
                            ret.extend(f.iter().map(TempTableColumn::from));
                        }
                        _ => {
                            log::error!(
                                "SelectItem QualifiedWildcard object name too long,I can't handle:{:?}",
                                object_name
                            )
                        }
                    }
                }
                SelectItem::Wildcard(_) => {
                    let tables = self.scopes.nearest_from_tables();
                    for table in tables.iter() {
                        match table {
                            FromTableOrTemp::FromTable { table } => {
                                let (db, schema, table) = self.extract_db_schme_item(table);

                                let cols = self
                                    .helper
                                    .list_column(db, schema, table)
                                    .await
                                    .map_err(|e| {
                                        log::error!("list_column failed,err:{:?}", e);
                                        self.set_on_def();
                                    })
                                    .ok()
                                    .unwrap_or_default()
                                    .into_iter()
                                    .map(TempTableColumn::from)
                                    .collect::<Vec<_>>();
                                ret.extend(cols.into_iter());
                            }
                            FromTableOrTemp::Temp { table } => {
                                ret.extend(table.columns.to_vec());
                            }
                            FromTableOrTemp::Unnamed { table } => {
                                ret.extend(table.columns.to_vec());
                            }
                        }
                    }
                }
            }
        }

        anyhow::Ok(ret)
    }

    pub(crate) async fn c_on_query(
        &mut self, query: &Query,
    ) -> anyhow::Result<Vec<TempTableColumn>> {
        let mut with_temp_tables = HashMap::new();
        if let Some(with) = query.with.as_ref() {
            for c in with.cte_tables.iter() {
                let cols = Box::pin(self.c_on_query(&c.query)).await?;
                with_temp_tables.insert(c.alias.name.clone(), TempTable {
                    columns: Arc::new(cols),
                });
            }
        }
        let s = Scope::from_temp_tables(with_temp_tables);
        let _with_temp_table_guard = self.scopes.push(s);
        let ret = vec![];

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
        &mut self, set: &SetExpr,
    ) -> anyhow::Result<Vec<TempTableColumn>> {
        match set {
            SetExpr::Select(select) => {
                return self.c_on_select(select).await;
            }
            SetExpr::Query(query) => {
                return Box::pin(self.c_on_query(&query)).await;
            }
            SetExpr::SetOperation { left, right, .. } => {
                // left and right must have same structure so we can i ignore left

                let _ = Box::pin(self.c_on_set_expr(&left)).await?;
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
                                    typ: None,
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
                    .map(TempTableColumn::from)
                    .collect();
                return anyhow::Ok(cols);
            }
        }
        anyhow::Ok(vec![])
    }

    pub(crate) async fn c_on_function_arg_expr(&mut self, arg: &FunctionArgExpr) {
        match arg {
            FunctionArgExpr::Expr(expr) => {
                self.c_on_expr(expr).await;
            }
            FunctionArgExpr::QualifiedWildcard(_object_name) => {}
            FunctionArgExpr::Wildcard => {}
        }
    }
    pub(crate) async fn c_on_table_factor(&mut self, f: &TableFactor, s: &mut Scope) {
        #[allow(unused)]
        match f {
            TableFactor::Table {
                name,
                alias,
                args,
                with_hints,
                partitions,
                ..
            } => {
                self.c_on_table(name).await;
                if self.finished() {
                    return;
                }
                s.from_tables.push(FromTableOrTemp::from(name.clone()));
                if let Some(alias) = alias {
                    // this is a alias but we often end in situation like this
                    // `select * from test ...`
                    // here we need completion on where order etc lexer elements
                    // if self.in_pos(&alias.name.span) {
                    //     self.set_on_def();
                    //     return;
                    // }
                    //TODO alias have column definition.
                    s.enter_table(alias.name.clone(), name.clone());
                } else {
                    s.enter_table(name.0.last().unwrap().clone(), name.clone());
                }
                for p in partitions.iter() {
                    self.c_on_partition_expr(name, &Expr::Identifier(p.clone()))
                        .await;
                    if self.finished() {
                        return;
                    }
                }
                if let Some(args) = args {
                    for a in args.args.iter() {
                        match a {
                            FunctionArg::Named {
                                name,
                                arg,
                                operator,
                            } => {
                                if self.in_pos(&name.span) {
                                    self.set_on_def();
                                    return;
                                }
                                self.c_on_function_arg_expr(arg).await;
                            }
                            FunctionArg::ExprNamed { arg, .. } => {
                                // TODO how to handle name expr.
                                self.c_on_function_arg_expr(arg).await;
                            }
                            FunctionArg::Unnamed(function_arg_expr) => {
                                self.c_on_function_arg_expr(function_arg_expr).await;
                            }
                        }
                    }
                }
                for h in with_hints.iter() {
                    self.c_on_expr(h).await;
                }
            }
            TableFactor::Derived {
                lateral,
                subquery,
                alias,
            } => {
                let cols = self
                    .c_on_query(&subquery)
                    .await
                    .map_err(|e| log::error!("self.c_on_query failed,err:{:?}", e))
                    .unwrap_or_default();

                if let Some(alias) = alias {
                    s.enter_temp_table(alias.name.clone(), TempTable {
                        columns: Arc::new(cols.clone()),
                    });
                    s.from_tables.push(FromTableOrTemp::Unnamed {
                        table: TempTable {
                            columns: Arc::new(cols),
                        },
                    });
                } else {
                    s.from_tables.push(FromTableOrTemp::Unnamed {
                        table: TempTable {
                            columns: Arc::new(cols),
                        },
                    });
                }
            }
            TableFactor::TableFunction { expr, alias } => {
                if let Some(alias) = alias {
                    s.enter_temp_table(alias.name.clone(), TempTable {
                        columns: Arc::new(vec![]),
                    });
                } else {
                    // TODO how to handle this case
                }
            }
            TableFactor::Function { alias, .. } => {
                if let Some(alias) = alias {
                    s.enter_temp_table(alias.name.clone(), TempTable {
                        columns: Arc::new(vec![]),
                    });
                } else {
                    // TODO how to handle this case
                }
            }
            TableFactor::UNNEST { .. } => {}
            TableFactor::JsonTable { .. } => {}
            TableFactor::OpenJsonTable { .. } => {}
            TableFactor::NestedJoin { .. } => {}
            TableFactor::Pivot { .. } => {}
            TableFactor::Unpivot { .. } => {}
            TableFactor::MatchRecognize { .. } => {}
        }
    }
    pub(crate) async fn c_on_procedure(&mut self, name: &ObjectName) {
        self.c_on_db_schema(name).await;
        if self.finished() {
            return;
        }
        if !self.last_name_in_pos(name) {
            return;
        }
        let (db, schema, _) = self.extract_db_schme_item(name);
        self.result = Some(
            self.helper
                .list_procedure(db, schema)
                .await
                .map_err(|e| {
                    log::error!("list_procedure error:{:?}", e);
                    self.set_has_err();
                })
                .ok()
                .unwrap_or_default()
                .into_iter()
                .map(|x| CompletionItem {
                    kind: CompletionKind::Function,
                    item: x.into(),
                    desc: None,
                })
                .collect::<Vec<_>>(),
        );
    }
    pub(crate) fn last_name_in_pos(&mut self, name: &ObjectName) -> bool {
        name.0.last().map(|x| self.in_pos(&x.span)).unwrap_or(false)
    }

    // sqlite pragma

    pub(crate) async fn c_on_pragma(&mut self, name: &ObjectName) {
        if !self.in_pos(&name.span()) {
            return;
        }
        self.result = Some(self.helper.sqlite_pragma());
    }

    pub(crate) async fn c_on_function_call(&mut self, function: &Function) {
        self.c_on_function(&function.name).await;
        // handle args
        match &function.args {
            FunctionArguments::None => {}
            FunctionArguments::Subquery(query) => {
                let _ = self.c_on_query(&query).await;
            }
            FunctionArguments::List(function_argument_list) => {
                for arg in function_argument_list.args.iter() {
                    match arg {
                        FunctionArg::Named { name, arg, .. } => {
                            if self.in_pos(&name.span) {
                                return;
                            }
                            match arg {
                                FunctionArgExpr::Expr(expr) => {
                                    self.c_on_expr(expr).await;
                                }
                                _ => {}
                            };
                        }
                        FunctionArg::ExprNamed { name, arg, .. } => {
                            if self.in_pos(&name.span()) {
                                // make suggest nothing.
                                return;
                            }
                            match arg {
                                FunctionArgExpr::Expr(expr) => {
                                    self.c_on_expr(expr).await;
                                }
                                _ => {}
                            };
                        }
                        FunctionArg::Unnamed(function_arg_expr) => match function_arg_expr {
                            FunctionArgExpr::Expr(expr) => {
                                self.c_on_expr(expr).await;
                            }
                            _ => {}
                        },
                    }
                }
            }
        }
    }

    pub(crate) async fn c_on_function_drop(&mut self, name: &ObjectName) {
        self.c_on_db_schema(name).await;
        if self.finished() {
            return;
        }
        if !self.last_name_in_pos(name) {
            return;
        }
        let (db, schema, _) = self.extract_db_schme_item(name);
        self.result = Some(
            self.helper
                .list_function(db, schema, false)
                .await
                .map_err(|e| {
                    log::error!("list function error:{:?}", e);
                    self.set_has_err();
                })
                .ok()
                .unwrap_or_default(),
        )
    }
    pub(crate) async fn c_on_function(&mut self, name: &ObjectName) {
        self.c_on_db_schema(name).await;
        if self.finished() {
            return;
        }
        if !self.last_name_in_pos(name) {
            return;
        }
        let (db, schema, _) = self.extract_db_schme_item(name);
        self.result = Some(
            self.helper
                .list_function(db, schema, true)
                .await
                .map_err(|e| {
                    log::error!("list function error:{:?}", e);
                    self.set_has_err();
                })
                .ok()
                .unwrap_or_default(),
        )
    }
}
