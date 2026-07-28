use crate::language_server::{remove_duplicate, CompletionItem, CompletionKind};
use crate::sqlparser_completion::{
    CompletionStatement, Scope, TableEntered, TempTable, TempTableColumn, Var,
};
use sqlparser::ast::*;
use std::sync::Arc;
impl From<ObjectType> for CompletionKind {
    fn from(val: ObjectType) -> Self {
        match val {
            ObjectType::Table => CompletionKind::Table,
            ObjectType::View => CompletionKind::View,
            ObjectType::Index => CompletionKind::Index,
            ObjectType::Schema => CompletionKind::Schema,
            ObjectType::Database => CompletionKind::Database,
            ObjectType::Role => CompletionKind::Database,
            ObjectType::Sequence => CompletionKind::Sequence,
            ObjectType::Stage => CompletionKind::Stage,
            ObjectType::Type => CompletionKind::Type,
            ObjectType::MaterializedView => CompletionKind::View,
            ObjectType::User => CompletionKind::Role,
            ObjectType::Stream => CompletionKind::Stream,
            ObjectType::Collation => CompletionKind::Text,
        }
    }
}

impl From<ShowCreateObject> for CompletionKind {
    fn from(val: ShowCreateObject) -> Self {
        match val {
            ShowCreateObject::Event => CompletionKind::Event,
            ShowCreateObject::Function => CompletionKind::Function,
            ShowCreateObject::Procedure => CompletionKind::Procedure,
            ShowCreateObject::Table => CompletionKind::Table,
            ShowCreateObject::Trigger => CompletionKind::Trigger,
            ShowCreateObject::View => CompletionKind::View,
        }
    }
}

impl From<CommentObject> for CompletionKind {
    fn from(val: CommentObject) -> Self {
        match val {
            CommentObject::Column => CompletionKind::Column,
            CommentObject::Table => CompletionKind::Table,
            CommentObject::Extension => CompletionKind::Extension,
            CommentObject::Schema => CompletionKind::Schema,
            CommentObject::Database => CompletionKind::Database,
            CommentObject::User => CompletionKind::User,
            CommentObject::Role => CompletionKind::Role,
            CommentObject::Collation => CompletionKind::Text,
            CommentObject::Domain => CompletionKind::Domain,
            CommentObject::Function => CompletionKind::Function,
            CommentObject::Index => CompletionKind::Index,
            CommentObject::MaterializedView => CompletionKind::View,
            CommentObject::Procedure => CompletionKind::Procedure,
            CommentObject::Sequence => CompletionKind::Sequence,
            CommentObject::Type => CompletionKind::Type,
            CommentObject::View => CompletionKind::View,
        }
    }
}

impl<'a> CompletionStatement<'a> {
    pub(crate) async fn c_on_create_table_options(&mut self, op: &CreateTableOptions) {
        match op {
            CreateTableOptions::None => {}
            CreateTableOptions::With(sql_options)
            | CreateTableOptions::Options(sql_options)
            | CreateTableOptions::Plain(sql_options)
            | CreateTableOptions::TableProperties(sql_options) => {
                for op in sql_options.iter() {
                    self.c_on_sql_option(op).await;
                    if self.finished() {
                        return;
                    }
                }
            }
        }
    }
    pub(crate) async fn c_on_view_column_def(&mut self, def: &ViewColumnDef) {
        if self.in_pos(&def.name.span) {
            self.set_on_def();
            return;
        }
        if let Some(x) = def.data_type.as_ref() {
            self.c_on_data_type(x).await;
        }
        if let Some(ops) = def.options.as_ref() {
            for op in ops.as_slice().iter() {
                self.c_on_column_option(op).await;
                if self.finished() {
                    return;
                }
            }
        }
    }
    pub(crate) async fn c_on_output_clause(&mut self, output: &OutputClause) {
        match output {
            OutputClause::Output {
                select_items,
                into_table,
                ..
            } => {
                self.c_on_select_items(&select_items[..]).await;
                if let Some(into_table) = into_table {
                    self.c_on_select_into(into_table).await;
                }
            }
            OutputClause::Returning { select_items, .. } => {
                self.c_on_select_items(&select_items[..]).await;
            }
        }
    }
    pub(crate) async fn c_on_select_into(&mut self, select_into: &SelectInto) {
        self.c_on_table(&select_into.name).await;
    }
    pub(crate) async fn c_on_show_statement_filter(&mut self, filter: &ShowStatementFilter) {
        match filter {
            ShowStatementFilter::Like(..) => {}
            ShowStatementFilter::ILike(..) => {}
            ShowStatementFilter::Where(expr) => self.c_on_expr(expr).await,
            ShowStatementFilter::NoKeyword(..) => {}
        }
    }
    pub(crate) async fn c_on_declare_assignment(&mut self, expr: &DeclareAssignment) {
        match expr {
            DeclareAssignment::Expr(expr)
            | DeclareAssignment::Default(expr)
            | DeclareAssignment::DuckAssignment(expr)
            | DeclareAssignment::For(expr)
            | DeclareAssignment::MsSqlAssignment(expr) => {
                self.c_on_expr(expr).await;
            }
        }
    }
    pub(crate) async fn c_on_postgres_extension(&mut self, name: &Ident, installed: bool) {
        if !self.in_pos(&name.span) {
            return;
        }
        log::info!("hit postgres extension:{:?}", name);
        self.result = Some(
            self.helper
                .postgres_list_extension(installed)
                .await
                .map_err(|e| log::error!("list postgres extension failed,err:{}", e))
                .unwrap_or_default()
                .into_iter()
                .map(CompletionItem::from)
                .collect(),
        )
    }
    pub(crate) async fn c_on_type(&mut self, name: &ObjectName) {
        self.c_on_schema(name).await;
        if self.finished() {
            return;
        }
        if !self.last_name_in_pos(name) {
            return;
        }
        log::info!("hit type:{:?}", name);
        let (schema, _) = self.extract_schema_and_item(name);
        self.result = Some(
            self.helper
                .list_created_type(schema.as_deref())
                .await
                .map_err(|e| log::error!("list type failed,err:{}", e))
                .unwrap_or_default()
                .into_iter()
                .map(CompletionItem::from)
                .collect(),
        );
    }
    pub(crate) async fn c_on_data_type(&mut self, ast: &DataType) {
        match ast {
            DataType::Table(column_defs) => {
                let empty = vec![];
                let column_defs = match column_defs {
                    Some(x) => x,
                    None => &empty,
                };
                let cols = column_defs
                    .iter()
                    .map(|x| TempTableColumn {
                        name: x.name.value.clone(),
                        data_type: Some(x.data_type.to_string()),
                    })
                    .collect::<Vec<_>>();

                self.scopes
                    .current_scope(|scope| scope.enter_temp_cols(cols));
                for col in column_defs.iter() {
                    if self.in_pos(&col.name.span) {
                        self.set_on_def();
                        return;
                    }
                    for def in col.options.iter() {
                        self.c_on_column_option_def(def).await;
                        if self.finished() {
                            return;
                        }
                    }
                }
            }
            DataType::NamedTable { name, columns } => {
                let temp_table = TempTable {
                    columns: Arc::new(
                        columns
                            .iter()
                            .map(|x| TempTableColumn {
                                name: x.name.value.clone(),
                                data_type: Some(x.data_type.to_string()),
                            })
                            .collect::<Vec<_>>(),
                    ),
                };
                let alias = match name.0.last() {
                    Some(x) => match x {
                        ObjectNamePart::Identifier(ident) => Some(ident.clone()),
                        ObjectNamePart::Function(..) => None,
                    },
                    None => None,
                };
                self.scopes
                    .current_scope(|scope| scope.enter_temp_table(alias, temp_table.clone()));
                for col in columns.iter() {
                    if self.in_pos(&col.name.span) {
                        self.set_on_def();
                        return;
                    }
                    for def in col.options.iter() {
                        self.c_on_column_option_def(def).await;
                        if self.finished() {
                            return;
                        }
                    }
                }
            }

            DataType::Custom(..) => {}
            DataType::Array(array_elem_type_def) => match array_elem_type_def {
                ArrayElemTypeDef::None => {}
                ArrayElemTypeDef::AngleBracket(data_type)
                | ArrayElemTypeDef::SquareBracket(data_type, _)
                | ArrayElemTypeDef::Parenthesis(data_type) => {
                    Box::pin(self.c_on_data_type(data_type.as_ref())).await
                }
            },
            DataType::Map(data_type, data_type1) => {
                Box::pin(self.c_on_data_type(data_type.as_ref())).await;
                Box::pin(self.c_on_data_type(data_type1.as_ref())).await;
            }
            DataType::Tuple(struct_fields) => {
                for field in struct_fields.iter() {
                    if let Some(name) = field.field_name.as_ref() {
                        if self.in_pos(&name.span) {
                            self.set_on_def();
                            return;
                        }
                    }
                    Box::pin(self.c_on_data_type(&field.field_type)).await;
                    if self.finished() {
                        return;
                    }
                }
            }
            DataType::Nested(column_defs) => {
                let cols = column_defs
                    .iter()
                    .map(|x| TempTableColumn {
                        name: x.name.value.clone(),
                        data_type: Some(x.data_type.to_string()),
                    })
                    .collect::<Vec<_>>();
                self.scopes
                    .current_scope(|scope| scope.enter_temp_cols(cols));
                for col in column_defs.iter() {
                    if self.in_pos(&col.name.span) {
                        self.set_on_def();
                        return;
                    }
                    for def in col.options.iter() {
                        self.c_on_column_option_def(def).await;
                        if self.finished() {
                            return;
                        }
                    }
                }
            }
            DataType::Enum(enum_members, _) => {
                for e in enum_members.iter() {
                    match e {
                        EnumMember::Name(..) => {}
                        EnumMember::NamedValue(_, expr) => {
                            self.c_on_expr(expr).await;
                            if self.finished() {
                                return;
                            }
                        }
                    }
                }
            }
            DataType::Struct(struct_fields, ..) => {
                let cols: Vec<_> = struct_fields
                    .iter()
                    .filter(|x| x.field_name.is_some())
                    .map(|x| TempTableColumn {
                        // we filtered before
                        name: x
                            .field_name
                            .as_ref()
                            .map(|x| x.value.clone())
                            .unwrap_or_default(),
                        data_type: Some(x.field_type.to_string()),
                    })
                    .collect();
                self.scopes.current_scope(|s| s.enter_temp_cols(cols));
                for f in struct_fields.iter() {
                    if let Some(name) = f.field_name.as_ref() {
                        if self.in_pos(&name.span) {
                            self.set_on_def();
                            return;
                        }
                    }
                    Box::pin(self.c_on_data_type(&f.field_type)).await;
                    if self.finished() {
                        return;
                    }
                    if let Some(ops) = f.options.as_ref() {
                        for op in ops.iter() {
                            self.c_on_sql_option(op).await;
                            if self.finished() {
                                return;
                            }
                        }
                    }
                }
            }
            DataType::Union(union_fields) => {
                for f in union_fields.iter() {
                    if self.in_pos(&f.field_name.span) {
                        self.set_on_def();
                        return;
                    }
                    Box::pin(self.c_on_data_type(&f.field_type)).await;
                    if self.finished() {
                        return;
                    }
                }
            }
            DataType::Nullable(data_type) => {
                Box::pin(self.c_on_data_type(data_type.as_ref())).await;
            }
            DataType::LowCardinality(data_type) => {
                Box::pin(self.c_on_data_type(data_type.as_ref())).await;
            }
            _ => {}
        }
    }
    pub(crate) async fn c_on_sqlite_module_name(&mut self, name: &Ident) {
        if !self.in_pos(&name.span) {
            return;
        }
        self.result = Some(
            self.helper
                .sqlite_module_name()
                .await
                .map_err(|e| log::error!("list sqlite module name failed:{}", e))
                .unwrap_or_default()
                .into_iter()
                .map(|name| CompletionItem {
                    item: name.into(),
                    desc: None,
                    kind: CompletionKind::Module,
                })
                .collect(),
        )
    }
    pub(crate) async fn c_on_duck_extension(&mut self, name: &Ident) {
        if !self.in_pos(&name.span) {
            return;
        }
        self.result = Some(
            self.helper
                .duckdb_extensions()
                .await
                .map_err(|err| {
                    log::error!("duckdb_extensions failed:{}", err);
                })
                .unwrap_or_default(),
        )
    }

    pub(crate) async fn c_on_operate_function_arg(&mut self, arg: &OperateFunctionArg) {
        let OperateFunctionArg {
            mode: _,
            name,
            data_type,
            default_expr,
        } = arg;
        if let Some(name) = name {
            if self.in_pos(&name.span) {
                self.set_on_def();
                return;
            }
        }
        self.c_on_data_type(data_type).await;
        if let Some(d) = default_expr {
            self.c_on_expr(d).await;
        }
    }
    pub(crate) async fn c_on_trigger_exec_body(&mut self, body: &TriggerExecBody) {
        let FunctionDesc { name: _, args } = &body.func_desc;

        if let Some(args) = args {
            for arg in args.iter() {
                self.c_on_operate_function_arg(arg).await;
                if self.finished() {
                    return;
                }
            }
        }
    }

    pub(crate) async fn c_on_role_objectname(&mut self, name: &ObjectName) {
        match name.0.len() {
            1 => {
                self.c_on_role(
                    match name.0.first().expect("we have checked the name length") {
                        ObjectNamePart::Identifier(ident) => ident,
                        ObjectNamePart::Function(object_name_part_function) => {
                            // TODO snowflack not support
                            &object_name_part_function.name
                        }
                    },
                )
                .await
            }
            _ => {
                log::error!("role name is too long:{:?}", name)
            }
        }
    }
    pub(crate) async fn c_on_role(&mut self, role: &Ident) {
        if !self.in_pos(&role.span) {
            return;
        }
        self.result = Some(self.helper.list_role_completion().await);
    }

    pub(crate) async fn c_on_cluster(&mut self, c: &Ident) {
        if !self.in_pos(&c.span) {
            return;
        }
        self.result = Some(
            self.helper
                .list_cluster()
                .await
                .map_err(|x| log::error!("list cluster failed,err:{}", x))
                .unwrap_or_default()
                .into_iter()
                .map(|name| CompletionItem {
                    item: name.into(),
                    kind: CompletionKind::Cluster,
                    desc: None,
                })
                .collect(),
        );
    }

    pub(crate) async fn c_on_mysql_trigger(&mut self, object_name: &ObjectName) {
        self.c_on_schema(object_name).await;
        if self.finished() {
            return;
        }
        if !self.last_name_in_pos(object_name) {
            return;
        }
        // 在 MySQL 中，SHOW CREATE TRIGGER 语句用于显示创建指定触发器的 CREATE TRIGGER
        // 语句 。其基本语法为：
        // sql复制
        // SHOW CREATE TRIGGER trigger_name;
        // 其中，trigger_name 是触发器的名称。需要注意的是，SHOW CREATE TRIGGER
        // 语句不支持直接指定数据库名称。如果要查看特定数据库中的触发器定义，
        // 需要先切换到该数据库，或者在触发器名称中明确指定数据库名称 。
        // 例如，如果要查看数据库 mydb 中名为 my_trigger
        // 的触发器定义，可以先切换到该数据库：
        let (schema, _) = self.extract_schema_and_item(object_name);
        self.result = Some(
            self.helper
                .mysql_show_create_trigger(schema.as_deref())
                .await
                .map_err(|x| log::error!("mysql_show_create_trigger failed,err:{}", x))
                .unwrap_or_default(),
        );
    }

    pub(crate) async fn c_on_mysql_show_create(
        &mut self,
        show_create: ShowCreateObject,
        name: &ObjectName,
    ) {
        match show_create {
            ShowCreateObject::Event => {
                self.c_on_event(name).await;
            }
            ShowCreateObject::Function => self.c_on_function(name).await,
            ShowCreateObject::Procedure => self.c_on_procedure(name).await,
            ShowCreateObject::Table => self.c_on_table(name).await,
            ShowCreateObject::Trigger => self.c_on_mysql_trigger(name).await,
            ShowCreateObject::View => self.c_on_view(name).await,
        }
    }

    pub(crate) async fn c_on_event(&mut self, name: &ObjectName) {
        self.c_on_schema(name).await;
        if self.finished() {
            return;
        }
        if !self.last_name_in_pos(name) {
            return;
        }
        let (schema, _) = self.extract_schema_and_item(name);
        self.result = Some(self.helper.list_event_completion(schema.as_deref()).await);
    }
    pub(crate) async fn c_on_variable(&mut self, var: &Ident) {
        if !self.in_pos(&var.span) {
            return;
        }
        let mut results: Vec<_> = self.helper.list_variable_completion().await;
        for var in self.scopes.vars_cloned() {
            results.push(CompletionItem {
                item: var.name.to_string().into(),
                kind: CompletionKind::Variable,
                desc: None,
            });
        }
        self.result = Some(results);
    }

    pub(crate) async fn c_on_execute(&mut self, name: &ObjectName) {
        if !self.last_name_in_pos(name) {
            return;
        }
        let (schema, _) = self.extract_schema_and_item(name);
        let mut results: Vec<CompletionItem> =
            self.helper.list_execute_completion(schema.as_deref()).await;
        if name.0.len() == 1 {
            // 也许在完成schema 如果要执行一个 sys.xxx
            results.extend(self.helper.list_schema_completion().await);
        }
        self.result = Some(results);
    }

    pub(crate) async fn c_on_prepare_ident(&mut self, ident: &Ident) {
        if !self.in_pos(&ident.span) {
            return;
        }
        self.result = Some(self.helper.list_execute_completion(None).await);
    }
    #[allow(dead_code)]
    pub(crate) async fn c_on_table_policy_object(
        &mut self,
        table_name: &ObjectName,
        name: &ObjectName,
    ) {
        match name.0.len() {
            1 => match name.0.last().expect("we have checked above") {
                ObjectNamePart::Identifier(ident) => {
                    self.c_on_table_policy(table_name, ident).await
                }
                ObjectNamePart::Function(..) => {
                    // not support right now.
                }
            },

            _ => {
                log::error!("unhandled table policy object :{}", name)
            }
        }
    }

    pub(crate) async fn c_on_table_policy(&mut self, table_name: &ObjectName, name: &Ident) {
        if !self.in_pos(&name.span) {
            return;
        }
        let (schema, table) = self.extract_schema_and_item(table_name);
        self.result = Some(
            self.helper
                .table_policy_completion(schema.as_deref(), &table)
                .await,
        );
    }

    pub(crate) async fn c_on_object_type_drop(&mut self, typ: ObjectType, name: &ObjectName) {
        self.c_on_schema(name).await;
        if self.finished() {
            return;
        }
        if !self.in_pos(&name.span()) {
            return;
        }
        log::info!("hit drop:{:?}", name);
        match typ {
            ObjectType::Table => self.c_on_table(name).await,
            ObjectType::View | ObjectType::MaterializedView => self.c_on_view(name).await,
            ObjectType::Index => self.c_on_index(name).await,
            ObjectType::Schema | ObjectType::Database => {
                //  我不知道这里指的是schema还是database所以返回全部的结果
                let results = self.helper.list_database_schema_completion().await;
                self.result = Some(results);
            }
            ObjectType::Role => self.c_on_role_objectname(name).await,
            ObjectType::Sequence => {
                self.c_on_sequence(name).await;
            }
            ObjectType::Stage => {
                // not support
            }
            ObjectType::Type => {
                self.c_on_type(name).await;
            }
            ObjectType::User => {
                if let Some(x) = name.0.last() {
                    match x {
                        ObjectNamePart::Identifier(ident) => self.c_on_user(ident).await,
                        ObjectNamePart::Function(..) => {}
                    }
                }
            }
            ObjectType::Stream => {}
            ObjectType::Collation => todo!(),
        }
    }
    pub(crate) async fn c_on_sequence(&mut self, name: &ObjectName) {
        self.c_on_schema(name).await;
        if self.finished() {
            return;
        }
        if !self.last_name_in_pos(name) {
            return;
        }
        log::info!("hit on sequence:{:?}", name);
        let (schema, _) = self.extract_schema_and_item(name);
        self.result = Some(
            self.helper
                .list_sequence_completion(schema.as_deref())
                .await,
        )
    }
    pub(crate) async fn c_on_user(&mut self, user: &Ident) {
        if !self.in_pos(&user.span) {
            return;
        }
        self.result = Some(self.helper.list_user_completion().await)
    }

    pub(crate) async fn c_on_privileges_action(&mut self, tables: &[ObjectName], a: &Action) {
        match a {
            Action::Insert { columns }
            | Action::References { columns }
            | Action::Select { columns }
            | Action::Update { columns } => {
                let empty = vec![];
                for col in match columns {
                    Some(x) => x,
                    None => &empty,
                } {
                    if !self.in_pos(&col.span) {
                        continue;
                    }
                    for table in tables.iter() {
                        let (schema, table) = self.extract_schema_and_item(table);
                        let cs = self
                            .helper
                            .list_column_oked(schema.as_deref(), &table, None)
                            .await
                            .into_iter()
                            .map(CompletionItem::from)
                            .collect::<Vec<_>>();
                        if !cs.is_empty() {
                            self.result = Some(cs);
                            return;
                        }
                    }
                }
            }
            Action::DatabaseRole { role } => self.c_on_role_objectname(role).await,

            Action::Role { role } => {
                self.c_on_role_objectname(role).await;
            }
            _ => {}
        }
    }

    pub(crate) async fn c_on_postgres_comment_object(
        &mut self,
        object_type: CommentObject,
        name: &ObjectName,
    ) {
        if !self.in_pos(&name.span()) {
            return;
        }
        match object_type {
            CommentObject::Table => self.c_on_table(name).await,
            CommentObject::Schema | CommentObject::Database => {
                let results = self.helper.list_database_schema_completion().await;
                self.result = Some(results);
            }
            CommentObject::Role => self.c_on_role_objectname(name).await,
            CommentObject::Column => {
                if name.0.len() >= 2 {
                    let table = name.0[..name.0.len() - 1].to_vec();
                    let table = ObjectName(table);
                    self.c_on_table(&table).await;
                    if self.finished() {
                        return;
                    }
                    match name.0.last().expect("we have checked the length") {
                        ObjectNamePart::Identifier(ident) => {
                            self.c_on_column(&table, ident, false).await
                        }
                        ObjectNamePart::Function(..) => {}
                    }
                }
            }
            CommentObject::Extension => match name.0.len() {
                1 => match name.0.last().expect("we have checked the length") {
                    ObjectNamePart::Identifier(ident) => {
                        self.c_on_postgres_extension(ident, false).await
                    }
                    ObjectNamePart::Function(..) => {
                        // ..
                    }
                },
                _ => {
                    log::error!("extension name not right:{:?}", name);
                }
            },
            CommentObject::User => match name.0.len() {
                // 在 PostgreSQL 里，“用户”本身就是 可登录的 role，所以给 user 加注释仍然用：
                // TODO
                1 => match name.0.last().expect("we have checked the length") {
                    ObjectNamePart::Identifier(ident) => {
                        self.c_on_user(ident).await;
                    }
                    ObjectNamePart::Function(..) => {}
                },
                _ => {
                    log::error!("name not right:{:?}", name)
                }
            },
            CommentObject::Collation => {}
            CommentObject::Domain => {}
            CommentObject::Function => {}
            CommentObject::Index => todo!(),
            CommentObject::MaterializedView => {}
            CommentObject::Procedure => {}
            CommentObject::Sequence => {}
            CommentObject::Type => {}
            CommentObject::View => {}
        }
    }
    pub(crate) async fn c_on_column_mysql_position(
        &mut self,
        table_name: &ObjectName,
        column_position: Option<&MySQLColumnPosition>,
    ) {
        if let Some(column_position) = column_position {
            match column_position {
                MySQLColumnPosition::First => {}
                MySQLColumnPosition::After(col) => self.c_on_column(table_name, col, false).await,
            }
        }
    }

    pub(crate) async fn c_on_partition_expr(&mut self, table_name: &ObjectName, p: &Expr) {
        match p {
            Expr::Identifier(x) => {
                let (schema, table) = self.extract_schema_and_item(table_name);
                if self.in_pos(&x.span) {
                    self.result = Some(
                        self.helper
                            .list_partition(schema.as_deref(), &table)
                            .await
                            .map_err(|e| {
                                log::error!("list partition failed,err:{}", e);
                            })
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
                /*
                在大多数主流数据库管理系统中，
                ANALYZE 命令的 PARTITION 子句通常不支持使用表达式，
                要求明确指定分区名称。然而，
                某些数据库系统可能允许在不同上下文中使用表达式来选择分区。以下是一些示例：*/
                log::error!("unhandled partition type:{:?}", p);
            }
        }
    }

    pub(crate) async fn c_on_index(&mut self, name: &ObjectName) {
        self.c_on_schema(name).await;
        if !self.last_name_in_pos(name) {
            return;
        }
        let (schema, _) = self.extract_schema_and_item(name);
        self.result = Some(self.helper.list_index_completion(schema.as_deref()).await);
    }

    pub(crate) async fn c_on_postgres_rule(&mut self, table_name: &ObjectName, rule: &Ident) {
        if !self.in_pos(&rule.span) {
            return;
        }
        let (schema, table) = self.extract_schema_and_item(table_name);
        self.result = Some(
            self.helper
                .list_postgres_rule(schema.as_deref(), &table)
                .await
                .map_err(|err| {
                    log::error!("list_postgres_rule,err:{}", err);
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
        if !self.in_pos(&name.span) {
            return;
        }
        let (schema, table_name) = self.extract_schema_and_item(table_name);
        self.result = Some(
            self.helper
                .list_constraint(schema.as_deref(), &table_name)
                .await
                .map_err(|err| {
                    log::error!("list constraint failed,err:{}", err);
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

    pub(crate) async fn c_on_clickhouse_partition(
        &mut self,
        table_name: &ObjectName,
        p: &Partition,
    ) {
        match p {
            Partition::Identifier(ident) => self.c_on_column(table_name, ident, false).await,
            Partition::Expr(expr) => {
                self.c_on_expr(expr).await;
            }
            Partition::Part(expr) => {
                self.c_on_expr(expr).await;
            }
            Partition::Partitions(exprs) => {
                for expr in exprs.iter() {
                    self.c_on_expr(expr).await;
                    if self.finished() {
                        return;
                    }
                }
            }
        }
    }

    pub(crate) async fn c_on_clickhouse_table_projection(
        &mut self,
        table_name: &ObjectName,
        name: &Ident,
    ) {
        if !self.in_pos(&name.span) {
            return;
        }
        // hit pos
        let (schema, table) = self.extract_schema_and_item(table_name);
        self.result = Some(
            self.helper
                .list_clickhouse_table_projects(schema.as_deref(), &table)
                .await
                .map_err(|err| log::error!("list clickhouse table_projects failed ,err:{}", err))
                .unwrap_or_default()
                .into_iter()
                .map(|name| CompletionItem {
                    item: name.into(),
                    kind: CompletionKind::ClickHouseTableProjection,
                    desc: None,
                })
                .collect(),
        );
    }

    pub(crate) async fn c_on_table_constraint(
        &mut self,
        constraint: &TableConstraint,
        table_name: Option<&ObjectName>,
    ) {
        match constraint {
            TableConstraint::Unique(UniqueConstraint {
                name,
                index_name,
                columns,
                ..
            })
            | TableConstraint::PrimaryKey(PrimaryKeyConstraint {
                name,
                index_name,
                columns,
                ..
            }) => {
                if let Some(name) = name {
                    if let Some(table_name) = table_name {
                        self.c_on_constraint(table_name, name).await;
                        if self.finished() {
                            return;
                        }
                    }
                };
                if let Some(index_name) = index_name {
                    if self.in_pos(&index_name.span) {
                        self.set_on_def();
                        return;
                    }
                };
                for c in columns.iter() {
                    self.c_on_index_column(c).await;
                    if self.finished() {
                        return;
                    }
                }
            }
            TableConstraint::ForeignKey(ForeignKeyConstraint {
                name,
                columns,
                foreign_table,
                referred_columns,
                ..
            }) => {
                if let Some(name) = name {
                    if self.in_pos(&name.span) {
                        self.set_on_def();
                        return;
                    }
                }
                for c in columns.iter() {
                    self.c_on_expr_ident(c).await;
                }
                self.c_on_table(foreign_table).await;
                if self.finished() {
                    return;
                }
                for c in referred_columns.iter() {
                    self.c_on_column(foreign_table, c, false).await;
                    if self.finished() {
                        return;
                    }
                }
            }
            TableConstraint::Check(CheckConstraint { name, expr, .. }) => {
                //  https://www.w3schools.com/sql/sql_check.asp
                if let Some(name) = name {
                    if self.in_pos(&name.span) {
                        self.set_on_def();
                        return;
                    }
                }
                self.c_on_expr(expr).await;
            }
            TableConstraint::Index(IndexConstraint { name, columns, .. }) => {
                // MySQLs [index definition][1] for index creation
                if let Some(name) = name {
                    if self.in_pos(&name.span) {
                        self.set_on_def();
                        return;
                    }
                }
                for c in columns.iter() {
                    self.c_on_index_column(c).await;
                    if self.finished() {
                        return;
                    }
                }
            }
            TableConstraint::FulltextOrSpatial(FullTextOrSpatialConstraint {
                opt_index_name,
                columns,
                ..
            }) => {
                if let Some(opt_index_name) = opt_index_name {
                    if self.in_pos(&opt_index_name.span) {
                        self.set_on_def();
                        return;
                    }
                };
                for c in columns.iter() {
                    self.c_on_index_column(c).await;
                    if self.finished() {
                        return;
                    }
                }
            }
            TableConstraint::PrimaryKeyUsingIndex(..) => {}
            TableConstraint::UniqueUsingIndex(..) => {}
        }
    }

    pub(crate) async fn c_on_alert_table_operation(
        &mut self,
        table_name: &ObjectName,
        ast: &AlterTableOperation,
    ) {
        self.scopes
            .current_scope(|s| s.enter_table(None, table_name.clone()));
        match ast {
            AlterTableOperation::AddConstraint { constraint, .. } => {
                self.c_on_table_constraint(constraint, Some(table_name))
                    .await;
            }
            AlterTableOperation::AddColumn {
                column_def,
                column_position,
                ..
            } => {
                if self.in_pos(&column_def.name.span) {
                    self.set_on_def();
                    return;
                }
                let (schema, table) = self.extract_schema_and_item(table_name);
                let cols = {
                    let mut cols = self
                        .helper
                        .list_column_oked(schema.as_deref(), &table, None)
                        .await;
                    // add the column that we are adding
                    cols.push(TempTableColumn {
                        name: column_def.name.value.clone(),
                        data_type: Some(column_def.data_type.to_string()),
                    });
                    cols
                };
                self.scopes
                    .current_scope(|scope| scope.enter_temp_cols(cols));
                for o in column_def.options.iter() {
                    self.c_on_column_option_def(o).await;
                }
                self.c_on_column_mysql_position(table_name, column_position.as_ref())
                    .await;
            }
            AlterTableOperation::AddProjection { name, select, .. } => {
                if self.in_pos(&name.span) {
                    self.set_on_def();
                    return;
                }
                self.c_on_projection_select(select).await;
            }
            AlterTableOperation::DropProjection { name, .. } => {
                self.c_on_clickhouse_table_projection(table_name, name)
                    .await;
            }
            AlterTableOperation::MaterializeProjection { name, .. } => {
                self.c_on_clickhouse_table_projection(table_name, name)
                    .await;
            }
            AlterTableOperation::ClearProjection { name, .. } => {
                self.c_on_clickhouse_table_projection(table_name, name)
                    .await;
            }
            AlterTableOperation::DisableRowLevelSecurity => {}
            AlterTableOperation::DisableRule { name } => {
                self.c_on_postgres_rule(table_name, name).await
            }
            AlterTableOperation::DisableTrigger { name } => {
                self.c_on_table_trigger(table_name, name).await;
            }
            AlterTableOperation::DropConstraint { name, .. } => {
                self.c_on_constraint(table_name, name).await;
            }
            AlterTableOperation::DropColumn { column_names, .. } => {
                for c in column_names.iter() {
                    self.c_on_column(table_name, c, false).await;
                    if self.finished() {
                        return;
                    }
                }
            }
            AlterTableOperation::AttachPartition { partition } => {
                self.c_on_clickhouse_partition(table_name, partition).await;
            }
            AlterTableOperation::DetachPartition { partition } => {
                self.c_on_clickhouse_partition(table_name, partition).await;
            }
            AlterTableOperation::FreezePartition { partition, .. } => {
                self.c_on_clickhouse_partition(table_name, partition).await;
            }
            AlterTableOperation::UnfreezePartition { partition, .. } => {
                self.c_on_clickhouse_partition(table_name, partition).await;
            }
            AlterTableOperation::DropPrimaryKey { .. } => {
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
                for p in old_partitions.iter() {
                    self.c_on_partition_expr(table_name, p).await;
                    if self.finished() {
                        return;
                    }
                }
                for p in new_partitions.iter() {
                    self.c_on_expr(p).await;
                }
            }
            AlterTableOperation::AddPartitions { new_partitions, .. } => {
                for p in new_partitions.iter() {
                    self.c_on_clickhouse_partition(table_name, p).await;
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
                self.c_on_column(table_name, old_column_name, false).await;
                if self.finished() {
                    return;
                }
                if self.in_pos(&new_column_name.span) {
                    self.set_on_def();
                }
            }
            AlterTableOperation::RenameTable { table_name } => {
                if self.in_pos(&table_name.span()) {
                    self.set_on_def();
                }
            }
            AlterTableOperation::ChangeColumn {
                old_name,
                new_name,
                column_position,
                options,
                ..
            } => {
                self.c_on_column(table_name, old_name, false).await;
                if self.finished() {
                    return;
                }
                if self.in_pos(&new_name.span) {
                    self.set_on_def();
                    return;
                }
                self.c_on_column_mysql_position(table_name, column_position.as_ref())
                    .await;
                if self.finished() {
                    return;
                }
                for op in options.iter() {
                    self.c_on_column_option(op).await;
                    if self.finished() {
                        return;
                    }
                }
            }
            AlterTableOperation::ModifyColumn {
                col_name,
                column_position,
                options,
                ..
            } => {
                self.c_on_column(table_name, col_name, false).await;
                if self.finished() {
                    return;
                }
                self.c_on_column_mysql_position(table_name, column_position.as_ref())
                    .await;
                for op in options.iter() {
                    self.c_on_column_option(op).await;
                    if self.finished() {
                        return;
                    }
                }
            }
            AlterTableOperation::RenameConstraint { old_name, new_name } => {
                if self.in_pos(&new_name.span) {
                    self.set_on_def();
                    return;
                }
                self.c_on_constraint(table_name, old_name).await;
            }
            AlterTableOperation::AlterColumn { column_name, op } => {
                self.c_on_column(table_name, column_name, false).await;
                self.c_on_alter_column_operation(op).await;
            }
            AlterTableOperation::SwapWith { table_name } => {
                self.c_on_table(table_name).await;
            }
            AlterTableOperation::SetTblProperties { table_properties } => {
                for op in table_properties.iter() {
                    self.c_on_sql_option(op).await;
                    if self.finished() {
                        return;
                    }
                }
            }
            AlterTableOperation::OwnerTo { new_owner } => {
                self.c_on_owner(new_owner).await;
            }
            AlterTableOperation::ClusterBy { exprs } => {
                for e in exprs.iter() {
                    self.c_on_expr(e).await;
                    if self.finished() {
                        return;
                    }
                }
            }
            AlterTableOperation::DropClusteringKey => {}
            AlterTableOperation::SuspendRecluster => {}
            AlterTableOperation::ResumeRecluster => {}
            AlterTableOperation::Algorithm { .. } => {}
            AlterTableOperation::AutoIncrement { .. } => {}
            AlterTableOperation::DropForeignKey { .. } => {}
            AlterTableOperation::DropIndex { name } => {
                // 这里没有限定表 不过列出一个数据库下所有的index应该也是没有问题的
                self.c_on_index(&ObjectName(vec![ObjectNamePart::Identifier(name.clone())]))
                    .await;
            }
            AlterTableOperation::ReplicaIdentity { .. } => {
                // fallback to lexer
            }
            AlterTableOperation::Lock { .. } => {
                // fallback to lexer
            }
            AlterTableOperation::ValidateConstraint { name } => {
                self.c_on_constraint(table_name, name).await;
            }
            AlterTableOperation::SetOptionsParens { options } => {
                for op in options.iter() {
                    self.c_on_sql_option(op).await;
                    if self.finished() {
                        return;
                    }
                }
            }
            AlterTableOperation::Refresh { .. } => {}
            AlterTableOperation::Suspend => {}
            AlterTableOperation::Resume => {}
            AlterTableOperation::ForceRowLevelSecurity => {}
            AlterTableOperation::NoForceRowLevelSecurity => {}
            AlterTableOperation::AlterSortKey { .. } => {}
        }
    }

    pub(crate) async fn c_on_alter_column_operation(&mut self, op: &AlterColumnOperation) {
        match op {
            AlterColumnOperation::SetNotNull => {}
            AlterColumnOperation::DropNotNull => {}
            AlterColumnOperation::SetDefault { value } => {
                self.c_on_expr(value).await;
            }
            AlterColumnOperation::DropDefault => {}
            AlterColumnOperation::SetDataType {
                data_type,
                using,
                had_set: _,
            } => {
                self.c_on_data_type(data_type).await;
                if let Some(u) = using {
                    self.c_on_expr(u).await;
                }
            }
            AlterColumnOperation::AddGenerated {
                generated_as: _,
                sequence_options,
            } => {
                if let Some(ops) = sequence_options {
                    for op in ops.iter() {
                        self.c_on_sequence_options(op).await;
                        if self.finished() {
                            return;
                        }
                    }
                }
            }
        }
    }
    pub(crate) async fn c_on_group_by_with_modifier(&mut self, g: &GroupByWithModifier) {
        match g {
            GroupByWithModifier::Rollup => {}
            GroupByWithModifier::Cube => {}
            GroupByWithModifier::Totals => {}
            GroupByWithModifier::GroupingSets(expr) => {
                self.c_on_expr(expr).await;
            }
        }
    }
    pub(crate) async fn c_on_projection_select(&mut self, select: &ProjectionSelect) {
        self.c_on_select_items(&select.projection[..]).await;
        if let Some(x) = select.group_by.as_ref() {
            self.c_on_group_by_expr(x).await
        }
        if let Some(o) = select.order_by.as_ref() {
            self.c_on_order_by(o).await;
        }
    }

    pub(crate) async fn c_on_order_by(&mut self, o: &OrderBy) {
        match &o.kind {
            OrderByKind::All(..) => {}
            OrderByKind::Expressions(order_by_exprs) => {
                for o in order_by_exprs.iter() {
                    self.c_on_order_by_expr(o).await;
                    if self.finished() {
                        return;
                    }
                }
            }
        }
        if let Some(int) = o.interpolate.as_ref() {
            if let Some(exprs) = int.exprs.as_ref() {
                for expr in exprs.iter() {
                    if let Some(expr) = expr.expr.as_ref() {
                        self.c_on_expr(expr).await;
                        if self.finished() {
                            return;
                        }
                    }
                }
            }
        }
    }
    pub(crate) async fn c_on_owner(&mut self, o: &Owner) {
        match o {
            Owner::Ident(ident) => {
                if self.in_pos(&ident.span) {
                    self.result = Some(self.helper.user_and_role().await)
                }
            }
            Owner::CurrentRole => {}
            Owner::CurrentUser => {}
            Owner::SessionUser => {}
        }
    }

    pub(crate) async fn c_on_view(&mut self, name: &ObjectName) {
        self.c_on_schema(name).await;
        if self.finished() {
            return;
        }
        if !self.last_name_in_pos(name) {
            return;
        }
        // here match
        let (schema, _name) = self.extract_schema_and_item(name);
        self.result = Some(
            self.helper
                .list_view(schema.as_deref())
                .await
                .map_err(|e| {
                    log::error!("list view failed,err:{}", e);
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

    pub(crate) async fn object_name_to_idents(&self, name: &ObjectName) -> Vec<Ident> {
        name.0
            .iter()
            .map(|x| match x {
                ObjectNamePart::Identifier(ident) => ident.clone(),
                ObjectNamePart::Function(object_name_part_function) => {
                    // TODO  snowflack not support
                    object_name_part_function.name.clone()
                }
            })
            .collect()
    }

    pub(crate) async fn c_on_expr_compound_identifier(&mut self, idents: &[Ident]) {
        if !idents.iter().any(|x| self.in_pos(&x.span)) {
            return;
        }
        log::info!("hit compound identifier:{:?}", idents);
        match idents.len() {
            1 => {
                // maybe never execute to here???
                self.c_on_expr_ident(&idents[0]).await;
            }
            2 => {
                let table = &idents[0];
                // try match for schema
                if self.helper.hit_schema(&table.value).await
                    && self.in_pos(&idents.last().expect("we have checked the length").span)
                {
                    // matched schema
                    // this is very likely a `select * from sys.xxx` on mysql or other db
                    // treat this expr as table
                    self.c_on_select_target(&ObjectName(
                        idents
                            .iter()
                            .map(|x| ObjectNamePart::Identifier(x.clone()))
                            .collect(),
                    ))
                    .await;
                    return;
                }

                if self.in_pos(&table.span) {
                    // handle name
                    self.c_on_select_target(&ObjectName(vec![ObjectNamePart::Identifier(
                        table.clone(),
                    )]))
                    .await;
                    return;
                }
                if let Some(table) = self.scopes.nearest_table_named(table) {
                    // we are dealing a table entered
                    match &table {
                        TableEntered::ObjectName { table, .. } => {
                            self.c_on_column(table, &idents[1], false).await;
                        }
                        TableEntered::Temp { table, .. } => {
                            self.result =
                                Some(table.columns.iter().map(CompletionItem::from).collect());
                        }
                    }
                } else {
                    self.c_on_column(
                        &ObjectName(vec![ObjectNamePart::Identifier(table.clone())]),
                        &idents[1],
                        true,
                    )
                    .await;
                }
            }
            3 => {
                let table: ObjectName = ObjectName(
                    idents[0..idents.len() - 1]
                        .iter()
                        .cloned()
                        .map(ObjectNamePart::Identifier)
                        .collect::<Vec<_>>(),
                );
                self.c_on_table(&table).await;
                if self.finished() {
                    return;
                }
                self.c_on_column(&table, &idents[2], false).await;
            }
            _ => {
                log::error!("identifiers too long:{:?}", idents)
            }
        }
    }

    pub(crate) async fn c_on_expr_ident(&mut self, ident: &Ident) {
        if !self.in_pos(&ident.span) {
            return;
        }
        log::info!("hit identifier:{:?}", ident);
        if ident.value.starts_with("@@") && self.helper.backend_kind().support_at()
        // sql server的情况和mysql差不多
        {
            /*
            在 MySQL 中，@@ 是用于访问系统变量的语法。系统变量存储了数据库服务器的各种配置信息和状态。以下是一些关键点：
            访问系统变量：
            使用 @@ 前缀可以访问全局或会话级别的系统变量。
            例如，SELECT @@version; 查询当前 MySQL 服务器的版本。
            */
            self.result = Some(self.helper.list_variable_completion().await);
            return;
        }
        if ident.value.starts_with("@") && self.helper.backend_kind().support_at()
        // sql server的情况和mysql差不多
        {
            /*
            在 MySQL 中，用户自定义变量（以 @ 开头的变量）不会在任何系统视图或命令输出中列出，因此无法直接列出所有用户定义的变量。用户变量的作用域仅限于当前会话，并且它们不会被系统视图记录。

            查看用户自定义变量的方法
            虽然没有直接的方法列出所有用户自定义变量，但你可以通过以下方式管理和查看它们：

            手动记录：

            在使用用户变量时，最好手动记录它们的名称和用途，以便在需要时查找。
            查询特定变量：

            如果你知道某个用户变量的名称，可以直接查询其值：
            */

            // 最好什么都不做 也不表示我处理到了这个语法结果让词法分析来做完成
            // 我也不能通过数据操作查询到
            return;
        }

        let mut results = vec![];
        let tables = self.scopes.tables_cloned();
        for t in tables.iter() {
            println!("!!!!!!!!!:{:?}", t);
            match t {
                TableEntered::ObjectName { table, .. } => {
                    let (schema, table) = self.extract_schema_and_item(table);
                    let items = self
                        .helper
                        .list_column_oked(schema.as_deref(), &table, None)
                        .await;
                    results.extend(items.into_iter().map(CompletionItem::from));
                }
                TableEntered::Temp { table, .. } => {
                    results.extend(
                        table
                            .columns
                            .iter()
                            .map(CompletionItem::from)
                            .collect::<Vec<_>>(),
                    );
                }
            }
        }
        // all vars
        for var in self.scopes.vars_cloned() {
            results.push(CompletionItem {
                item: var.name.value.into(),
                kind: CompletionKind::Variable,
                desc: var.data_type.map(|s| s.to_string().into()),
            });
        }

        // this line will rewrite the self.results.
        self.c_on_select_target(&ObjectName(vec![ObjectNamePart::Identifier(ident.clone())]))
            .await;

        results.extend(self.result.take().unwrap_or_default());

        if ident.quote_style.is_none() {
            // 我们也要函数
            let fs = self
                .helper
                .list_function(None, true)
                .await
                .unwrap_or_default();
            results.extend(fs);
        }
        //也要schema
        let schemas = self.helper.list_database_completion().await;
        results.extend(schemas);
        /*
            ai提示词 sql查询里面 有哪些关键字 可以放在select的列的前面 给我完成的列表
        */
        results.extend(
            ["ALL", "DISTINCT", "CASE", "WHEN", "THEN", "ELSE"]
                .iter()
                .map(|x| CompletionItem {
                    item: (*x).into(),
                    kind: CompletionKind::KeyWord,
                    desc: None,
                }),
        );
        results = remove_duplicate(results);
        self.result = Some(results);
    }

    pub(crate) async fn c_on_column_option(&mut self, op: &ColumnOption) {
        match op {
            ColumnOption::Null => {}
            ColumnOption::NotNull => {}
            ColumnOption::Default(expr) => {
                Box::pin(self.c_on_expr(expr)).await;
            }
            ColumnOption::Materialized(expr) => {
                Box::pin(self.c_on_expr(expr)).await;
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
            ColumnOption::ForeignKey(ForeignKeyConstraint {
                foreign_table,
                referred_columns,
                ..
            }) => {
                self.c_on_table(foreign_table).await;
                if self.finished() {
                    return;
                }
                for r in referred_columns.iter() {
                    self.c_on_column(foreign_table, r, false).await;
                    if self.finished() {}
                }
            }
            ColumnOption::Check(CheckConstraint { expr, .. }) => {
                self.c_on_expr(expr).await;
            }
            ColumnOption::DialectSpecific(..) => {
                // fallback to lexer
            }
            ColumnOption::CharacterSet(..) => {
                // fallback to lexer
            }
            ColumnOption::Comment(_) => {
                // no position
            }
            ColumnOption::OnUpdate(expr) => {
                self.c_on_expr(expr).await;
            }
            ColumnOption::Generated {
                sequence_options,
                generation_expr,
                ..
            } => {
                if let Some(options) = sequence_options {
                    for op in options.iter() {
                        self.c_on_sequence_options(op).await;
                        if self.finished() {
                            return;
                        }
                    }
                }
                if let Some(expr) = generation_expr {
                    self.c_on_expr(expr).await;
                }
            }
            ColumnOption::Options(options) => {
                for op in options.iter() {
                    self.c_on_sql_option(op).await;
                    if self.finished() {
                        return;
                    }
                }
            }
            ColumnOption::Identity(..) => {
                // fallback to lexer
            }
            ColumnOption::OnConflict(..) => {}
            ColumnOption::Policy(column_policy) => {
                let ast = match column_policy {
                    ColumnPolicy::MaskingPolicy(column_policy_property) => column_policy_property,
                    ColumnPolicy::ProjectionPolicy(column_policy_property) => {
                        column_policy_property
                    }
                };
                if self.finished() {
                    return;
                }
                if let Some(xs) = ast.using_columns.as_ref() {
                    for x in xs.iter() {
                        self.c_on_expr_ident(x).await;
                        if self.finished() {
                            return;
                        }
                    }
                }
            }
            ColumnOption::Tags(..) => {
                //fallback to lexer
            }
            ColumnOption::Collation(..) => {
                // 这里是字符集设置吗
            }
            ColumnOption::Srid(expr) => {
                self.c_on_expr(expr).await;
            }
            ColumnOption::PrimaryKey(..) => {}
            ColumnOption::Invisible => {}
        }
    }

    /// on `ColumnOptionDef` require table been entered
    ///
    pub(crate) async fn c_on_column_option_def(&mut self, def: &ColumnOptionDef) {
        if let Some(name) = &def.name {
            if self.in_pos(&name.span) {
                self.set_on_def();
                return;
            }
        }
        self.c_on_column_option(&def.option).await;
    }

    pub(crate) async fn c_on_sql_option(&mut self, ast: &SqlOption) {
        match ast {
            SqlOption::Clustered(..) => {}
            SqlOption::Ident(..) => {
                // fallback to lexer
            }
            SqlOption::KeyValue { value, .. } => {
                self.c_on_expr(value).await;
            }
            SqlOption::Partition {
                column_name,
                for_values,
                ..
            } => {
                if self.in_pos(&column_name.span) {
                    self.c_on_expr_ident(column_name).await;
                    return;
                }
                for f in for_values.iter() {
                    self.c_on_expr(f).await;
                    if self.finished() {
                        return;
                    }
                }
            }
            SqlOption::Comment(..) => {}
            SqlOption::TableSpace(..) => {}
            SqlOption::NamedParenthesizedList(..) => {}
        }
    }
    pub(crate) async fn c_on_expr(&mut self, expr: &Expr) {
        log::trace!("completion on:{:?}", expr);
        match expr {
            Expr::Identifier(ident) => self.c_on_expr_ident(ident).await,
            Expr::CompoundIdentifier(vec) => {
                self.c_on_expr_compound_identifier(vec).await;
            }
            Expr::JsonAccess { value, .. } => {
                Box::pin(self.c_on_expr(value.as_ref())).await;
            }
            Expr::IsFalse(expr) => {
                Box::pin(self.c_on_expr(expr)).await;
            }
            Expr::IsNotFalse(expr) => {
                Box::pin(self.c_on_expr(expr)).await;
            }
            Expr::IsTrue(expr) => {
                Box::pin(self.c_on_expr(expr)).await;
            }
            Expr::IsNotTrue(expr) => {
                Box::pin(self.c_on_expr(expr)).await;
            }
            Expr::IsNull(expr) => {
                Box::pin(self.c_on_expr(expr)).await;
            }
            Expr::IsNotNull(expr) => {
                Box::pin(self.c_on_expr(expr)).await;
            }
            Expr::IsUnknown(expr) => {
                Box::pin(self.c_on_expr(expr)).await;
            }
            Expr::IsNotUnknown(expr) => {
                Box::pin(self.c_on_expr(expr)).await;
            }
            Expr::IsDistinctFrom(expr, expr1) => {
                Box::pin(self.c_on_expr(expr)).await;
                if self.finished() {
                    return;
                }
                Box::pin(self.c_on_expr(expr1)).await;
            }
            Expr::IsNotDistinctFrom(expr, expr1) => {
                Box::pin(self.c_on_expr(expr)).await;
                if self.finished() {
                    return;
                }
                Box::pin(self.c_on_expr(expr1)).await;
            }
            Expr::InList {
                expr,
                list,
                negated: _,
            } => {
                Box::pin(self.c_on_expr(expr)).await;
                if self.finished() {
                    return;
                }
                for expr in list.iter() {
                    Box::pin(self.c_on_expr(expr)).await;
                    if self.finished() {
                        return;
                    }
                }
            }
            Expr::InSubquery {
                expr,
                subquery,
                negated: _,
            } => {
                let cols = Box::pin(self.c_on_query(subquery.as_ref()))
                    .await
                    .map_err(|e| log::error!("a error:{:?}", e))
                    .unwrap_or_default();
                self.scopes.current_scope(|s| s.enter_temp_cols(cols));
                Box::pin(self.c_on_expr(expr)).await;
            }
            Expr::InUnnest {
                expr,
                array_expr,
                negated: _,
            } => {
                Box::pin(self.c_on_expr(expr)).await;
                if self.finished() {
                    return;
                }
                Box::pin(self.c_on_expr(array_expr)).await;
            }
            Expr::Between {
                expr,
                low,
                high,
                negated: _,
            } => {
                Box::pin(self.c_on_expr(expr)).await;
                if self.finished() {
                    return;
                }
                Box::pin(self.c_on_expr(low)).await;
                if self.finished() {
                    return;
                }
                Box::pin(self.c_on_expr(high)).await;
            }
            Expr::BinaryOp { left, right, op: _ } => {
                Box::pin(self.c_on_expr(left)).await;
                if self.finished() {
                    return;
                }
                Box::pin(self.c_on_expr(right)).await;
            }
            Expr::Like { expr, pattern, .. } => {
                Box::pin(self.c_on_expr(expr)).await;
                if self.finished() {
                    return;
                }
                Box::pin(self.c_on_expr(pattern)).await;
            }
            Expr::ILike { expr, pattern, .. } => {
                Box::pin(self.c_on_expr(expr)).await;
                if self.finished() {
                    return;
                }
                Box::pin(self.c_on_expr(pattern)).await;
            }
            Expr::SimilarTo { expr, pattern, .. } => {
                Box::pin(self.c_on_expr(expr)).await;
                if self.finished() {
                    return;
                }
                Box::pin(self.c_on_expr(pattern)).await;
            }
            Expr::RLike { expr, pattern, .. } => {
                Box::pin(self.c_on_expr(expr)).await;
                if self.finished() {
                    return;
                }
                Box::pin(self.c_on_expr(pattern)).await;
            }
            Expr::AnyOp { left, right, .. } => {
                Box::pin(self.c_on_expr(left)).await;
                if self.finished() {
                    return;
                }
                Box::pin(self.c_on_expr(right)).await;
            }
            Expr::AllOp { left, right, .. } => {
                Box::pin(self.c_on_expr(left)).await;
                if self.finished() {
                    return;
                }
                Box::pin(self.c_on_expr(right)).await;
            }
            Expr::UnaryOp { expr, .. } => {
                Box::pin(self.c_on_expr(expr)).await;
                if self.finished() {}
            }
            Expr::Convert {
                expr,
                styles,
                data_type,
                ..
            } => {
                Box::pin(self.c_on_expr(expr)).await;
                for style in styles.iter() {
                    Box::pin(self.c_on_expr(style)).await;
                    if self.finished() {
                        return;
                    }
                }
                if let Some(data_type) = data_type {
                    Box::pin(self.c_on_data_type(data_type)).await;
                }
            }
            Expr::Cast {
                expr, data_type, ..
            } => {
                Box::pin(self.c_on_expr(expr)).await;
                Box::pin(self.c_on_data_type(data_type)).await;
            }
            Expr::AtTimeZone {
                timestamp,
                time_zone,
            } => {
                Box::pin(self.c_on_expr(timestamp)).await;
                if self.finished() {
                    return;
                }
                Box::pin(self.c_on_expr(time_zone)).await;
            }
            Expr::Extract { expr, .. } => {
                Box::pin(self.c_on_expr(expr)).await;
            }
            Expr::Ceil { expr, .. } => {
                Box::pin(self.c_on_expr(expr)).await;
            }
            Expr::Floor { expr, .. } => {
                Box::pin(self.c_on_expr(expr)).await;
            }
            Expr::Position { expr, r#in } => {
                Box::pin(self.c_on_expr(expr)).await;
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
                Box::pin(self.c_on_expr(expr)).await;
                if self.finished() {
                    return;
                }
                if let Some(from) = substring_from {
                    Box::pin(self.c_on_expr(from)).await;
                }
                if let Some(f) = substring_for {
                    Box::pin(self.c_on_expr(f)).await;
                }
            }
            Expr::Trim {
                expr,
                trim_what,
                trim_characters,
                ..
            } => {
                Box::pin(self.c_on_expr(expr)).await;
                if self.finished() {
                    return;
                }
                if let Some(what) = trim_what {
                    Box::pin(self.c_on_expr(what)).await;
                    if self.finished() {
                        return;
                    }
                }
                if let Some(cs) = trim_characters {
                    for c in cs.iter() {
                        Box::pin(self.c_on_expr(c)).await;
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
                Box::pin(self.c_on_expr(expr)).await;
                if self.finished() {
                    return;
                }
                Box::pin(self.c_on_expr(overlay_what)).await;
                if self.finished() {
                    return;
                }
                Box::pin(self.c_on_expr(overlay_from)).await;
                if self.finished() {
                    return;
                }
                if let Some(overlay_for) = overlay_for {
                    Box::pin(self.c_on_expr(overlay_for)).await;
                }
            }
            Expr::Collate { expr, .. } => {
                Box::pin(self.c_on_expr(expr)).await;
            }
            Expr::Nested(expr) => {
                Box::pin(self.c_on_expr(expr)).await;
            }
            Expr::Value(..) => {
                // literal value
                // nothing to completion
            }
            Expr::TypedString(ty) => {
                // literal value
                Box::pin(self.c_on_data_type(&ty.data_type)).await;
            }
            Expr::Function(function) => Box::pin(self.c_on_function_call(function)).await,
            Expr::Case {
                operand,
                conditions,
                else_result,
                ..
            } => {
                if let Some(operand) = operand {
                    Box::pin(self.c_on_expr(operand)).await;
                    if self.finished() {
                        return;
                    }
                }
                for c in conditions.iter() {
                    Box::pin(self.c_on_expr(&c.condition)).await;
                    if self.finished() {
                        return;
                    }
                    Box::pin(self.c_on_expr(&c.result)).await;
                    if self.finished() {
                        return;
                    }
                }
                if let Some(e) = else_result {
                    Box::pin(self.c_on_expr(e)).await;
                }
            }
            Expr::Exists { subquery, .. } => {
                let _ = Box::pin(self.c_on_query(subquery))
                    .await
                    .map_err(|e| log::error!("a error:{}", e));
            }
            Expr::Subquery(query) => {
                let _ = Box::pin(self.c_on_query(query))
                    .await
                    .map_err(|e| log::error!("a error:{}", e));
            }
            Expr::GroupingSets(vec) => {
                for exprs in vec.iter() {
                    for expr in exprs.iter() {
                        Box::pin(self.c_on_expr(expr)).await;
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
            Expr::Struct { values, .. } => {
                for v in values.iter() {
                    Box::pin(self.c_on_expr(v)).await;
                    if self.finished() {
                        return;
                    }
                }
            }
            Expr::Named { expr, .. } => {
                Box::pin(self.c_on_expr(expr.as_ref())).await;
            }
            Expr::Dictionary(_) => {
                // literal value
            }
            Expr::Map(_) => {
                // literal value
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
                for c in columns.iter() {
                    let idents = self.object_name_to_idents(c).await;
                    self.c_on_expr_compound_identifier(&idents[..]).await;
                    if self.finished() {
                        return;
                    }
                }
            }
            Expr::Wildcard(..) => {
                // nothing here
            }
            Expr::QualifiedWildcard(table, _) => {
                self.c_on_table_or_view(table).await;
            }
            Expr::OuterJoin(expr) => {
                Box::pin(self.c_on_expr(expr)).await;
            }
            Expr::Prior(expr) => {
                Box::pin(self.c_on_expr(expr)).await;
            }
            Expr::Lambda(f) => {
                Box::pin(self.c_on_lambda(f)).await;
            }
            Expr::CompoundFieldAccess { root, access_chain } => {
                Box::pin(self.c_on_expr(root)).await;
                for acc in access_chain.iter() {
                    self.c_on_access_expr(acc).await;
                    if self.finished() {
                        return;
                    }
                }
            }
            Expr::IsNormalized { expr, .. } => {
                Box::pin(self.c_on_expr(expr)).await;
            }
            Expr::Prefixed { value, .. } => {
                Box::pin(self.c_on_expr(value)).await;
            }
            Expr::MemberOf(member_of) => {
                Box::pin(self.c_on_expr(&member_of.array)).await;
                Box::pin(self.c_on_expr(&member_of.value)).await;
            }
        }
    }
    pub(crate) async fn c_on_access_expr(&mut self, acc: &AccessExpr) {
        match acc {
            AccessExpr::Dot(expr) => {
                Box::pin(self.c_on_expr(expr)).await;
            }
            AccessExpr::Subscript(subscript) => match subscript {
                Subscript::Index { index } => {
                    Box::pin(self.c_on_expr(index)).await;
                }
                Subscript::Slice {
                    lower_bound,
                    upper_bound,
                    stride,
                } => {
                    if let Some(lower_bound) = lower_bound {
                        Box::pin(self.c_on_expr(lower_bound)).await;
                    }
                    if let Some(upper_bound) = upper_bound {
                        Box::pin(self.c_on_expr(upper_bound)).await;
                    }
                    if let Some(stride) = stride {
                        Box::pin(self.c_on_expr(stride)).await;
                    }
                }
            },
        }
    }
    pub(crate) async fn c_on_lambda(&mut self, lambda: &LambdaFunction) {
        // lambda consider a function
        let _guard = self.scopes.enter(Scope::default());
        for name in lambda.params.iter() {
            if self.in_pos(&name.name.span) {
                self.set_on_def();
                return;
            }
            self.scopes.current_scope(|scope| {
                scope.enter_var(Var {
                    name: name.name.clone(),
                    data_type: None,
                })
            });
        }
        self.c_on_expr(&lambda.body).await;
    }

    pub(crate) async fn c_on_order_by_expr(&mut self, orderby: &OrderByExpr) {
        self.c_on_expr(&orderby.expr).await;
        if self.finished() {
            return;
        }
        if let Some(fill) = orderby.with_fill.as_ref() {
            self.c_on_with_fill(fill).await;
        }
    }

    pub(crate) async fn c_on_with_fill(&mut self, fill: &WithFill) {
        if let Some(from) = fill.from.as_ref() {
            self.c_on_expr(from).await;
            if self.finished() {
                return;
            }
        }
        if let Some(to) = fill.to.as_ref() {
            self.c_on_expr(to).await;
            if self.finished() {
                return;
            }
        }
        if let Some(step) = fill.step.as_ref() {
            self.c_on_expr(step).await;
        }
    }

    pub(crate) async fn c_on_index_column(&mut self, col: &IndexColumn) {
        self.c_on_order_by_expr(&col.column).await;
    }

    pub(crate) async fn c_on_column(
        &mut self,
        table_name: &ObjectName,
        col: &Ident,
        suggest_table: bool,
    ) {
        if !self.in_pos(&col.span) {
            return;
        }
        log::info!("hit column:{:?}", col);
        if suggest_table {
            self.c_on_table_or_view(table_name).await;
        }
        let mut result = self.result.take().unwrap_or_default();
        let (schema, table_name) = self.extract_schema_and_item(table_name);
        let items = self
            .helper
            .list_column_oked(schema.as_deref(), &table_name, None)
            .await
            .into_iter()
            .map(CompletionItem::from)
            .collect::<Vec<_>>();
        result.extend(items);
        self.result = Some(result);
    }

    pub(crate) async fn c_on_table_trigger(
        &mut self,
        table_name: &ObjectName,
        trigger_name: &Ident,
    ) {
        if !self.in_pos(&trigger_name.span) {
            return;
        }
        let (schema, table_name) = self.extract_schema_and_item(table_name);
        self.result = Some(
            self.helper
                .list_table_trigger(schema.as_deref(), &table_name)
                .await
                .map_err(|e| {
                    log::error!("list_table_trigger failed,err:{:?}", e);
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

    // completion 假设  schema.table
    pub(crate) async fn c_on_schema(&mut self, name: &ObjectName) {
        match name.0.len() {
            2 => {
                let schema = &name.0[0];
                if self.in_pos(&schema.span()) {
                    self.result = Some(self.helper.list_schema_completion().await);
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
        self.c_on_schema(name).await;
        if self.finished() {
            return;
        }
        if !self.last_name_in_pos(name) {
            return;
        }
        self.c_on_view(name).await;
        let mut items = self.result.take().unwrap_or_default();
        self.c_on_table(name).await;
        items.extend(self.result.take().unwrap_or_default());
        self.result = Some(items);
    }

    pub(crate) async fn c_on_select_target(&mut self, name: &ObjectName) {
        if !self.last_name_in_pos(name) {
            return;
        }
        log::info!("hit select target:{:?}", name);
        self.c_on_table_or_view(name).await;
        let (schema, _) = self.extract_schema_and_item(name);
        let mut items = self.result.take().unwrap_or_default();
        let f = self
            .helper
            .list_function(schema.as_deref(), schema.is_none())
            .await
            .map_err(|e| log::error!("list function failed , e:{}", e))
            .unwrap_or_default();
        items.extend(f);
        self.result = Some(items);
    }

    pub(crate) async fn c_on_table(&mut self, table_name: &ObjectName) {
        self.c_on_schema(table_name).await;
        if self.finished() {
            return;
        }
        if !self.last_name_in_pos(table_name) {
            return;
        }
        log::info!("hit table:{:?}", table_name);
        let mut items = vec![];
        for x in self.scopes.tables_cloned() {
            if let Some(name) = x.get_name() {
                items.push(name.value.to_string());
            }
        }
        let (schema, _) = self.extract_schema_and_item(table_name);
        items.extend(
            self.helper
                .list_table(schema.as_deref())
                .await
                .map_err(|e| {
                    log::error!("list table failed,err:{:?}", e);
                })
                .ok()
                .unwrap_or_default(),
        );
        if table_name.0.len() == 1 {
            items.extend(self.helper.list_schema().await.unwrap_or_default());
        }
        let items = items
            .into_iter()
            .map(|x| CompletionItem {
                item: x.into(),
                kind: CompletionKind::Table,
                desc: None,
            })
            .collect::<Vec<_>>();
        // remove duplicate we have entered table and tables in database
        let items = remove_duplicate(items);

        self.result = Some(items);
    }

    pub(crate) async fn c_on_from_table(&mut self, from_table: &FromTable) {
        let tables = match from_table {
            FromTable::WithFromKeyword(vec) => vec,
            FromTable::WithoutKeyword(vec) => vec,
        };
        self.c_on_table_with_joins(&tables[..]).await
    }

    pub(crate) async fn c_on_table_with_joins(&mut self, joins: &[TableWithJoins]) {
        for join_tables in joins.iter() {
            self.c_on_table_with_join(join_tables).await;
            if self.finished() {
                return;
            }
        }
    }
    pub(crate) async fn c_on_join_operator(&mut self, ast: &JoinOperator) {
        match ast {
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
            JoinOperator::Semi(join_constraint) => self.c_on_join_constraint(join_constraint).await,
            JoinOperator::LeftSemi(join_constraint) => {
                self.c_on_join_constraint(join_constraint).await
            }
            JoinOperator::RightSemi(join_constraint) => {
                self.c_on_join_constraint(join_constraint).await
            }
            JoinOperator::Anti(join_constraint) => self.c_on_join_constraint(join_constraint).await,
            JoinOperator::LeftAnti(join_constraint) => {
                self.c_on_join_constraint(join_constraint).await
            }
            JoinOperator::RightAnti(join_constraint) => {
                self.c_on_join_constraint(join_constraint).await
            }
            JoinOperator::CrossApply => {}
            JoinOperator::OuterApply => {}
            JoinOperator::AsOf {
                match_condition,
                constraint,
            } => {
                self.c_on_join_constraint(constraint).await;
                if self.finished() {
                    return;
                }
                self.c_on_expr(match_condition).await;
            }
            JoinOperator::CrossJoin(join_constraint) => {
                self.c_on_join_constraint(join_constraint).await
            }
            JoinOperator::Join(join_constraint) => self.c_on_join_constraint(join_constraint).await,
            JoinOperator::Left(join_constraint) => self.c_on_join_constraint(join_constraint).await,
            JoinOperator::Right(join_constraint) => {
                self.c_on_join_constraint(join_constraint).await
            }
            JoinOperator::StraightJoin(join_constraint) => {
                self.c_on_join_constraint(join_constraint).await
            }
            JoinOperator::ArrayJoin => {}
            JoinOperator::LeftArrayJoin => {}
            JoinOperator::InnerArrayJoin => {}
        }
    }
    pub(crate) async fn c_on_table_with_join(&mut self, t: &TableWithJoins) {
        self.c_on_table_factor(&t.relation).await;
        for join in t.joins.iter() {
            self.c_on_table_factor(&join.relation).await;
            if self.finished() {
                return;
            }
        }
        for j in t.joins.iter() {
            self.c_on_join_operator(&j.join_operator).await;
        }
    }

    pub(crate) async fn c_on_join_constraint(&mut self, c: &JoinConstraint) {
        match c {
            JoinConstraint::On(expr) => {
                self.c_on_expr(expr).await;
            }
            JoinConstraint::Using(vec) => {
                /*
                mysql数据库里面的join using是怎么用的呢
                编辑
                复制
                JOIN … USING (col) 是 ANSI-SQL 的一种简写形式，MySQL 完全支持。
                它做的事可以用 ON 写出来，只是更短、不会重复输出关联列。
                一、语法模板
                sql
                复制
                SELECT …
                FROM 表1
                [INNER | LEFT | RIGHT] JOIN 表2
                USING (关联列名);   -- 多列就逗号分隔：USING (id, name)
                */
                for v in vec.iter() {
                    let idents = self.object_name_to_idents(v).await;
                    self.c_on_expr_compound_identifier(&idents[..]).await;
                    if self.finished() {
                        return;
                    }
                }
            }
            JoinConstraint::Natural => {}
            JoinConstraint::None => {}
        }
    }

    pub(crate) async fn c_on_distinct(&mut self, d: &Distinct) {
        match d {
            Distinct::Distinct => {}
            Distinct::On(vec) => {
                for e in vec.iter() {
                    self.c_on_expr(e).await;
                }
            }
            Distinct::All => {}
        }
    }
    pub(crate) async fn c_on_top(&mut self, top: &Top) {
        if let Some(q) = &top.quantity {
            match q {
                TopQuantity::Expr(expr) => {
                    self.c_on_expr(expr).await;
                }
                TopQuantity::Constant(_) => {}
            }
        }
    }
    pub(crate) async fn c_on_group_by_expr(&mut self, ast: &GroupByExpr) {
        match ast {
            GroupByExpr::All(vec) => {
                for g in vec.iter() {
                    self.c_on_group_by_with_modifier(g).await;
                }
            }
            GroupByExpr::Expressions(vec, gs) => {
                for e in vec.iter() {
                    self.c_on_expr(e).await;
                    if self.finished() {
                        return;
                    }
                }
                for g in gs.iter() {
                    self.c_on_group_by_with_modifier(g).await;
                    if self.finished() {
                        return;
                    }
                }
            }
        }
    }
    pub(crate) async fn c_on_connect_by(&mut self, cb: &ConnectByKind) {
        match cb {
            ConnectByKind::ConnectBy { relationships, .. } => {
                for r in relationships.iter() {
                    self.c_on_expr(r).await;
                    if self.finished() {
                        return;
                    }
                }
            }
            ConnectByKind::StartWith { condition, .. } => {
                self.c_on_expr(condition.as_ref()).await;
            }
        }
    }
    pub(crate) async fn c_on_select(
        &mut self,
        select: &Select,
    ) -> anyhow::Result<Vec<TempTableColumn>> {
        // we enter those table in outer scope so outer can have tables definitions
        self.c_on_table_with_joins(&select.from).await;
        let _guard = self.scopes.enter(Scope::default());
        //  in case we are dealing with a sql `select a as b from c`
        //  a been rename to b
        //  we use the the database system to get the columns
        let cols = {
            let mut select = select.clone();
            select.selection = None; // make sure no selection maybe selection have error
            self.helper
                .get_query_columns(&select.to_string())
                .await
                .map_err(|e| log::error!("get query columns error: {}", e))
                .unwrap_or_default()
        };

        self.scopes
            .current_scope(|s| s.enter_temp_cols(cols.clone()));

        let ret = anyhow::Ok(cols);
        if let Some(d) = &select.distinct {
            self.c_on_distinct(d).await;
        }
        if let Some(t) = &select.top {
            self.c_on_top(t).await
        }
        if let Some(into) = &select.into {
            self.c_on_table(&into.name).await;
        }
        if let Some(p) = &select.prewhere {
            self.c_on_expr(p).await;
        }
        if let Some(selection) = &select.selection {
            self.c_on_expr(selection).await;
            if self.finished() {
                return ret;
            }
        }
        self.c_on_group_by_expr(&select.group_by).await;
        if let Some(having) = &select.having {
            self.c_on_expr(having).await;
            if self.finished() {
                return ret;
            }
        }
        if let Some(q) = &select.qualify {
            self.c_on_expr(q).await;
            if self.finished() {
                return ret;
            }
        }

        for cb in select.connect_by.iter() {
            self.c_on_connect_by(cb).await;
            if self.finished() {
                return ret;
            }
        }

        for w in select.named_window.iter() {
            self.c_on_named_window_definition(w).await;
            if self.finished() {
                return ret;
            }
        }
        self.c_on_select_items(&select.projection).await;
        ret
    }

    pub(crate) async fn c_on_named_window_definition(&mut self, ast: &NamedWindowDefinition) {
        if self.in_pos(&ast.0.span) {
            self.set_on_def();
            return;
        }
        self.c_on_named_window_expr(&ast.1).await;
    }

    pub(crate) async fn c_on_named_window_expr(&mut self, ast: &NamedWindowExpr) {
        match ast {
            NamedWindowExpr::NamedWindow(..) => {}
            NamedWindowExpr::WindowSpec(window_spec) => self.c_on_window_spec(window_spec).await,
        }
    }
    pub(crate) async fn c_on_input_format_clause(&mut self, ast: &InputFormatClause) {
        /*
        在 ClickHouse 里，FORMAT 并不是 SQL 标准里的 “FORMAT 子句”（像 TO_CHAR(dt,'YYYY-MM-DD') 那种），而是放在整条语句 最后 的一个 结果输出格式指示器。
        语法只有一句话：
        sql
        复制
        SELECT …
        [FROM …]
        [WHERE …]
        [GROUP BY …]
        [ORDER BY …]
        [LIMIT …]
        FORMAT <format_name>;
        */
        log::trace!("input clause:{:?}", ast);
        self.c_on_on_clickhouse_format(&ast.ident).await;
        for v in ast.values.iter() {
            self.c_on_expr(v).await;
            if self.finished() {
                return;
            }
        }
    }

    pub(crate) async fn c_on_on_clickhouse_format(&mut self, ast: &Ident) {
        if !self.in_pos(&ast.span) {
            return;
        }
        log::info!("hit clickhouse format:{:?}", ast);

        self.result = Some(
            self.helper
                .clickhouse_format_clause()
                .into_iter()
                .map(|x| CompletionItem {
                    item: x.into(),
                    kind: CompletionKind::KeyWord,
                    desc: None,
                })
                .collect(),
        )
    }
    pub(crate) async fn c_on_setting(&mut self, ast: &Setting) {
        if self.in_pos(&ast.key.span) {
            self.set_on_def();
            return;
        }
        self.c_on_expr(&ast.value).await;
    }
    pub(crate) async fn c_on_select_items(&mut self, projection: &[SelectItem]) {
        self.select_item_columns.clear();
        log::trace!("projection:{:?}", projection);
        for p in projection.iter() {
            match p {
                SelectItem::UnnamedExpr(expr) => {
                    self.c_on_expr(expr).await;
                    if self.finished() {
                        return;
                    }
                    self.select_item_columns.insert(expr.to_string().into());
                }
                SelectItem::ExprWithAlias { expr, alias } => {
                    self.c_on_expr(expr).await;
                    if self.finished() {
                        return;
                    }
                    if self.in_pos(&alias.span) {
                        self.set_on_def();
                    }
                    self.select_item_columns.insert(alias.value.clone().into());
                }
                SelectItem::QualifiedWildcard(select, wildcard_additional_options) => {
                    self.c_on_select_item_qualified_wildcard_kind(select).await;
                    self.c_on_wildcard_additional_options(wildcard_additional_options)
                        .await;
                    match select {
                        SelectItemQualifiedWildcardKind::ObjectName(object_name) => {
                            match object_name.0.len() {
                                1 => {
                                    match object_name.0.first().expect("we have checked the Length")
                                    {
                                        ObjectNamePart::Identifier(ident) => {
                                            match self.scopes.nearest_table_named(ident) {
                                                Some(x) => match x {
                                                    TableEntered::ObjectName { table, .. } => {
                                                        let (schema, table) =
                                                            self.extract_schema_and_item(&table);
                                                        let cols = self
                                                            .helper
                                                            .list_column_oked(
                                                                schema.as_deref(),
                                                                &table,
                                                                None,
                                                            )
                                                            .await;
                                                        for col in cols {
                                                            self.select_item_columns.insert(col);
                                                        }
                                                    }
                                                    TableEntered::Temp { table, .. } => {
                                                        for col in table.columns.iter() {
                                                            self.select_item_columns
                                                                .insert(col.clone());
                                                        }
                                                    }
                                                },
                                                None => {
                                                    let cols = self
                                                        .helper
                                                        .list_column_oked(None, &ident.value, None)
                                                        .await;
                                                    for col in cols {
                                                        self.select_item_columns.insert(col);
                                                    }
                                                }
                                            }
                                        }
                                        ObjectNamePart::Function(..) => {
                                            // TODO
                                        }
                                    }
                                }
                                2 => {
                                    // schema.table
                                    let (schema, table) = self.extract_schema_and_item(object_name);
                                    let cols = self
                                        .helper
                                        .list_column_oked(schema.as_deref(), &table, None)
                                        .await;
                                    for col in cols {
                                        self.select_item_columns.insert(col);
                                    }
                                }
                                _ => {
                                    log::error!("name length is not right:{:?}", object_name)
                                }
                            }
                        }
                        SelectItemQualifiedWildcardKind::Expr(expr) => {
                            self.select_item_columns.insert(expr.to_string().into());
                        }
                    }
                }
                SelectItem::Wildcard(add) => {
                    self.c_on_wildcard_additional_options(add).await;
                    let tables = self.scopes.tables_cloned();
                    for table in tables {
                        match table {
                            TableEntered::ObjectName { table, .. } => {
                                let (schema, table) = self.extract_schema_and_item(&table);
                                let cols = self
                                    .helper
                                    .list_column_oked(schema.as_deref(), &table, None)
                                    .await;
                                for col in cols {
                                    self.select_item_columns.insert(col);
                                }
                            }
                            TableEntered::Temp { table, .. } => {
                                for col in table.columns.iter() {
                                    self.select_item_columns.insert(col.clone());
                                }
                            }
                        }
                    }
                }
                SelectItem::ExprWithAliases { .. } => {
                    // spack not support
                }
            }
        }
    }

    pub(crate) async fn c_on_select_item_qualified_wildcard_kind(
        &mut self,
        ast: &SelectItemQualifiedWildcardKind,
    ) {
        match ast {
            SelectItemQualifiedWildcardKind::ObjectName(object_name) => {
                self.c_on_table(object_name).await
            }
            SelectItemQualifiedWildcardKind::Expr(expr) => {
                self.c_on_expr(expr).await;
            }
        }
    }
    pub(crate) async fn c_on_wildcard_additional_options(
        &mut self,
        ast: &WildcardAdditionalOptions,
    ) {
        if let Some(ex) = ast.opt_except.as_ref() {
            for col in ex
                .additional_elements
                .iter()
                .chain(std::iter::once(&ex.first_element))
            {
                self.c_on_expr_ident(col).await;
                if self.finished() {
                    return;
                }
            }
        }
        if let Some(x) = ast.opt_replace.as_ref() {
            for item in x.items.iter() {
                self.c_on_expr_ident(&item.column_name).await;
                if self.finished() {
                    return;
                }
                self.c_on_expr(&item.expr).await;
            }
        }
        if let Some(x) = ast.opt_rename.as_ref() {
            match x {
                RenameSelectItem::Single(ident_with_alias) => {
                    self.c_on_ident_with_alias(ident_with_alias).await;
                }
                RenameSelectItem::Multiple(items) => {
                    for item in items.iter() {
                        self.c_on_ident_with_alias(item).await;
                        if self.finished() {
                            return;
                        }
                    }
                }
            }
        }
    }

    pub(crate) async fn c_on_ident_with_alias(&mut self, ast: &IdentWithAlias) {
        self.c_on_expr_ident(&ast.ident).await;
        if self.finished() {
            return;
        }
        if self.in_pos(&ast.alias.span) {
            self.set_on_def();
        }
    }
    pub(crate) async fn c_on_cte(&mut self, c: &Cte) {
        let cols = Box::pin(self.c_on_query(&c.query))
            .await
            .map_err(|e| log::error!("query failed,err:{}", e))
            .unwrap_or_default();

        let cols = if let Some(cols) = self.get_table_alias_columns(&Some(c.alias.clone())) {
            cols
        } else {
            cols
        };
        self.scopes.current_scope(|s| {
            s.enter_temp_table(
                Some(c.alias.name.clone()),
                TempTable {
                    columns: Arc::new(cols),
                },
            )
        })
    }
    pub(crate) async fn c_on_query(
        &mut self,
        query: &Query,
    ) -> anyhow::Result<Vec<TempTableColumn>> {
        let _guard = self.scopes.enter(Scope::default());
        if let Some(with) = query.with.as_ref() {
            for c in with.cte_tables.iter() {
                self.c_on_cte(c).await;
            }
        }
        let results = Box::pin(self.c_on_set_expr(&query.body))
            .await
            .map_err(|e| log::error!("set expr failed,err:{}", e))
            .unwrap_or_default();

        // enter table here if this is subquery or something else
        self.scopes
            .current_scope(|scope| scope.enter_temp_cols(results.clone()));

        if let Some(orderby) = query.order_by.as_ref() {
            self.c_on_order_by(orderby).await;
        }
        if let Some(limit) = query.limit_clause.as_ref() {
            self.c_on_limit_clause(limit).await;
        }
        if let Some(f) = query.fetch.as_ref() {
            if let Some(q) = f.quantity.as_ref() {
                self.c_on_expr(q).await;
            }
        }
        if let Some(ss) = query.settings.as_ref() {
            for s in ss.iter() {
                self.c_on_setting(s).await;
            }
        }
        for op in query.pipe_operators.iter() {
            self.c_on_pipe_operator(op).await;
        }
        anyhow::Ok(results)
    }

    pub(crate) async fn c_on_limit_clause(&mut self, ast: &LimitClause) {
        match ast {
            LimitClause::LimitOffset {
                limit,
                offset,
                limit_by,
            } => {
                if let Some(limit) = limit {
                    self.c_on_expr(limit).await;
                }
                if let Some(offset) = offset {
                    self.c_on_expr(&offset.value).await;
                }
                for l in limit_by.iter() {
                    self.c_on_expr(l).await;
                    if self.finished() {
                        break;
                    }
                }
            }
            LimitClause::OffsetCommaLimit { offset, limit } => {
                self.c_on_expr(offset).await;
                self.c_on_expr(limit).await;
            }
        }
    }
    pub(crate) async fn c_on_assignment(&mut self, ass: &Assignment) {
        self.c_on_expr(&ass.value).await;
        if self.finished() {
            return;
        }
        match &ass.target {
            AssignmentTarget::ColumnName(object_name) => {
                let idents = self.object_name_to_idents(object_name).await;
                self.c_on_expr_compound_identifier(&idents[..]).await;
            }
            AssignmentTarget::Tuple(object_names) => {
                for name in object_names.iter() {
                    let idents = self.object_name_to_idents(name).await;
                    self.c_on_expr_compound_identifier(&idents[..]).await;
                    if self.finished() {
                        return;
                    }
                }
            }
        }
    }

    pub(crate) async fn c_on_expr_with_alias_and_order_by(
        &mut self,
        ast: &ExprWithAliasAndOrderBy,
    ) {
        if let Some(alias) = ast.expr.alias.as_ref() {
            if self.in_pos(&alias.span) {
                self.set_on_def();
                return;
            }
        }
        self.c_on_expr(&ast.expr.expr).await;
    }
    pub(crate) async fn c_on_pipe_operator(&mut self, ast: &PipeOperator) {
        match ast {
            PipeOperator::Limit { expr, offset } => {
                self.c_on_expr(expr).await;
                if self.finished() {
                    return;
                }
                if let Some(offset) = offset {
                    self.c_on_expr(offset).await;
                }
            }
            PipeOperator::Where { expr } => {
                self.c_on_expr(expr).await;
            }
            PipeOperator::OrderBy { exprs } => {
                for expr in exprs.iter() {
                    self.c_on_order_by_expr(expr).await;
                    if self.finished() {
                        return;
                    }
                }
            }
            PipeOperator::Select { exprs } => {
                self.c_on_select_items(&exprs[..]).await;
            }
            PipeOperator::Extend { exprs } => {
                self.c_on_select_items(&exprs[..]).await;
            }
            PipeOperator::Set { assignments } => {
                for ass in assignments.iter() {
                    self.c_on_assignment(ass).await;
                    if self.finished() {
                        return;
                    }
                }
            }
            PipeOperator::Drop { columns } => {
                for col in columns.iter() {
                    self.c_on_expr_ident(col).await;
                    if self.finished() {
                        return;
                    }
                }
            }

            PipeOperator::As { alias } => {
                if self.in_pos(&alias.span) {
                    self.set_on_def();
                }
            }
            PipeOperator::Aggregate {
                full_table_exprs,
                group_by_expr,
            } => {
                for x in full_table_exprs.iter().chain(group_by_expr.iter()) {
                    self.c_on_expr_with_alias_and_order_by(x).await;
                    if self.finished() {
                        return;
                    }
                }
            }
            PipeOperator::TableSample { sample } => {
                self.c_on_table_sample(sample).await;
            }
            PipeOperator::Rename { mappings } => {
                for map in mappings.iter() {
                    if self.in_pos(&map.alias.span) {
                        self.set_on_def();
                        return;
                    }
                    self.c_on_expr_ident(&map.ident).await;
                    if self.finished() {
                        return;
                    }
                }
            }
            PipeOperator::Union { queries, .. }
            | PipeOperator::Intersect { queries, .. }
            | PipeOperator::Except { queries, .. } => {
                for q in queries.iter() {
                    let _ = Box::pin(self.c_on_query(q)).await;
                    if self.finished() {
                        return;
                    }
                }
            }
            PipeOperator::Call { function, alias } => {
                if let Some(alias) = alias {
                    if self.in_pos(&alias.span) {
                        self.set_on_def();
                        return;
                    }
                }
                Box::pin(self.c_on_function_call(function)).await;
            }

            PipeOperator::Pivot { .. } => {
                // bigquery not support
            }
            PipeOperator::Unpivot { .. } => {
                // big query not support
            }
            PipeOperator::Join(join) => {
                self.c_on_join(join).await;
            }
        }
    }
    pub(crate) async fn c_on_table_sample(&mut self, sample: &TableSample) {
        if let Some(q) = sample.quantity.as_ref() {
            self.c_on_expr(&q.value).await;
        }
        if let Some(bucket) = sample.bucket.as_ref() {
            if let Some(on) = bucket.on.as_ref() {
                self.c_on_expr(on).await;
            }
        }
        if let Some(offset) = sample.offset.as_ref() {
            self.c_on_expr(offset).await;
        }
    }
    pub(crate) async fn c_on_join(&mut self, join: &Join) {
        self.c_on_table_factor(&join.relation).await;
        self.c_on_join_operator(&join.join_operator).await;
    }

    pub(crate) async fn c_on_set_expr(
        &mut self,
        set: &SetExpr,
    ) -> anyhow::Result<Vec<TempTableColumn>> {
        match set {
            SetExpr::Select(select) => self.c_on_select(select).await,
            SetExpr::Query(query) => Box::pin(self.c_on_query(query)).await,
            SetExpr::SetOperation { left, right, .. } => {
                let l = Box::pin(self.c_on_set_expr(left)).await;
                let r = Box::pin(self.c_on_set_expr(right)).await;
                if l.is_ok() {
                    l
                } else {
                    r
                }
            }
            SetExpr::Values(values) => {
                for v in values.rows.iter() {
                    for v in v.iter() {
                        self.c_on_expr(v).await;
                        if self.finished() {
                            return anyhow::Ok(vec![]);
                        }
                    }
                }
                /*
                PostgreSQL：明确支持VALUES作为表达式表，例如VALUES (1, 2), (3, 4)可独立使用，等效于SELECT * FROM (VALUES (1, 2), (3, 4)) AS t(c1, c2)。
                貌似只有postgres支持这种语法
                */
                self.helper
                    .get_query_columns(&format!("select * from ({})", values))
                    .await
            }
            SetExpr::Insert(statement)
            | SetExpr::Update(statement)
            | SetExpr::Delete(statement)
            | SetExpr::Merge(statement) => {
                self.completion_on_statement(statement).await;
                anyhow::Ok(
                    std::mem::take(&mut self.select_item_columns)
                        .into_iter()
                        .collect(),
                )
            }
            SetExpr::Table(table) => {
                if let Some(t) = &table.table_name {
                    self.helper
                        .list_column(table.schema_name.as_deref(), t, None)
                        .await
                } else {
                    anyhow::Ok(vec![])
                }
            }
        }
    }

    pub(crate) async fn c_on_function_arg_expr(&mut self, arg: &FunctionArgExpr) {
        match arg {
            FunctionArgExpr::Expr(expr) => {
                self.c_on_expr(expr).await;
            }
            FunctionArgExpr::QualifiedWildcard(object_name) => {
                let idents = self.object_name_to_idents(object_name).await;
                self.c_on_expr_compound_identifier(&idents[..]).await;
            }
            FunctionArgExpr::Wildcard => {}
            FunctionArgExpr::WildcardWithOptions(..) => {}
        }
    }

    pub(crate) async fn c_on_function_arg(&mut self, ast: &FunctionArg) {
        match ast {
            FunctionArg::Named {
                name,
                arg,
                operator: _,
            } => {
                if self.in_pos(&name.span) {
                    self.set_on_def();
                    return;
                }
                self.c_on_function_arg_expr(arg).await;
            }
            FunctionArg::ExprNamed {
                name,
                arg,
                operator: _,
            } => {
                self.c_on_expr(name).await;
                self.c_on_function_arg_expr(arg).await;
            }
            FunctionArg::Unnamed(function_arg_expr) => {
                self.c_on_function_arg_expr(function_arg_expr).await;
            }
        }
    }

    pub(crate) fn get_table_alias_columns(
        &self,
        t: &Option<TableAlias>,
    ) -> Option<Vec<TempTableColumn>> {
        match t {
            // function_call [WITH ORDINALITY] [[AS] table_alias [(column_alias [, ... ])]]
            // ROWS FROM( function_call [, ... ] ) [WITH ORDINALITY] [[AS] table_alias [(column_alias [, ... ])]]这里面的column_alias是列的定义吗
            // 编辑
            // 复制
            // 是的，column_alias 就是给 列起的别名，也就是 列的定义。
            // 你在问题中提到的这段语法：
            Some(alias) if !alias.columns.is_empty() => Some(
                alias
                    .columns
                    .iter()
                    .map(|x| TempTableColumn {
                        name: x.name.value.clone(),
                        data_type: x.data_type.as_ref().map(|x| x.to_string()),
                    })
                    .collect::<Vec<_>>(),
            ),
            _ => None,
        }
    }

    pub(crate) async fn c_on_table_alias(&mut self, alias: &TableAlias) {
        if self.in_pos(&alias.name.span) {
            self.set_on_def();
            return;
        }
        for col in alias.columns.iter() {
            self.c_on_table_alias_column_def(col).await;
            if self.finished() {
                return;
            }
        }
    }
    pub(crate) async fn c_on_table_alias_column_def(&mut self, col: &TableAliasColumnDef) {
        if self.in_pos(&col.name.span) {
            self.set_on_def();
            return;
        }
        if let Some(data_type) = col.data_type.as_ref() {
            self.c_on_data_type(data_type).await;
        }
    }
    pub(crate) async fn c_on_json_table_column(&mut self, ast: &JsonTableColumn) {
        match ast {
            JsonTableColumn::Named(json_table_named_column) => {
                if self.in_pos(&json_table_named_column.name.span) {
                    self.set_on_def();
                    return;
                }
                self.c_on_data_type(&json_table_named_column.r#type).await;
            }
            JsonTableColumn::ForOrdinality(name) => {
                if self.in_pos(&name.span) {
                    self.set_on_def();
                }
            }
            JsonTableColumn::Nested(json_table_nested_column) => {
                for col in json_table_nested_column.columns.iter() {
                    Box::pin(self.c_on_json_table_column(col)).await;
                    if self.finished() {
                        return;
                    }
                }
            }
        }
    }
    pub(crate) async fn c_on_open_json_table_column(&mut self, ast: &OpenJsonTableColumn) {
        if self.in_pos(&ast.name.span) {
            self.set_on_def();
            return;
        }
        self.c_on_data_type(&ast.r#type).await;
    }

    pub(crate) async fn c_on_table_version(&mut self, ast: &TableVersion) {
        match ast {
            TableVersion::ForSystemTimeAsOf(expr) => self.c_on_expr(expr).await,
            TableVersion::Function(expr) => self.c_on_expr(expr).await,
            TableVersion::TimestampAsOf(expr) => self.c_on_expr(expr).await,
            TableVersion::VersionAsOf(expr) => {
                self.c_on_expr(expr).await;
            }
            TableVersion::Changes { changes, at, end } => {
                self.c_on_expr(changes).await;
                if self.finished() {
                    return;
                }
                self.c_on_expr(at).await;
                if self.finished() {
                    return;
                }
                if let Some(end) = end {
                    self.c_on_expr(end).await;
                }
            }
        }
    }
    pub(crate) async fn c_on_table_factor(&mut self, ast: &TableFactor) {
        match ast {
            TableFactor::Table {
                name,
                alias,
                args,
                with_hints,
                partitions,
                version,
                ..
            } => {
                if let Some(alias) = alias {
                    self.c_on_table_alias(alias).await;
                    if self.finished() {
                        return;
                    }
                }
                self.c_on_schema(name).await;
                if self.finished() {
                    return;
                }
                log::info!("table name:{:?} {}", name, self.last_name_in_pos(name));
                if self.last_name_in_pos(name) {
                    self.c_on_select_target(name).await;
                    return;
                }
                let (schema, table_name) = self.extract_schema_and_item(name);
                let columns = if let Some(cols) = self.get_table_alias_columns(alias) {
                    cols
                } else {
                    self.helper
                        .list_column_oked(
                            schema.as_deref(),
                            &table_name,
                            args.as_ref()
                                .map(|x| x.args.iter().map(|x| x.to_string()).collect()),
                        )
                        .await
                };
                self.scopes.current_scope(|s| {
                    if let Some(alias) = alias {
                        s.enter_temp_table(
                            Some(alias.name.clone()),
                            TempTable {
                                columns: Arc::new(columns),
                            },
                        );
                    }
                    s.enter_table(None, name.clone());
                });
                if let Some(version) = version {
                    self.c_on_table_version(version).await;
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
                        self.c_on_function_arg(a).await;
                        if self.finished() {
                            return;
                        }
                    }
                }
                for h in with_hints.iter() {
                    self.c_on_expr(h).await;
                }
            }
            TableFactor::Derived {
                subquery,
                alias,
                lateral: _,
                sample: _,
            } => {
                if let Some(alias) = alias {
                    self.c_on_table_alias(alias).await;
                    if self.finished() {
                        return;
                    }
                }
                let query_cols = Box::pin(self.c_on_query(subquery))
                    .await
                    .unwrap_or_default();
                let cols = if let Some(cols) = self.get_table_alias_columns(alias) {
                    cols
                } else {
                    query_cols
                };

                self.scopes.current_scope(|s| {
                    s.enter_temp_table(
                        alias.as_ref().map(|x| x.name.clone()),
                        TempTable {
                            columns: Arc::new(cols),
                        },
                    )
                });
            }
            TableFactor::TableFunction { expr, alias } => {
                if let Some(alias) = alias {
                    self.c_on_table_alias(alias).await;
                    if self.finished() {
                        return;
                    }
                }
                let cols = self.get_table_alias_columns(alias).unwrap_or_default();
                self.c_on_expr(expr).await;
                if let Some(alias) = alias.as_ref() {
                    self.scopes.current_scope(|s| {
                        s.enter_temp_table(
                            Some(alias.name.clone()),
                            TempTable {
                                columns: Arc::new(cols),
                            },
                        )
                    });
                }
            }
            TableFactor::Function {
                alias, name, args, ..
            } => {
                if let Some(alias) = alias {
                    self.c_on_table_alias(alias).await;
                    if self.finished() {
                        return;
                    }
                }
                for arg in args.iter() {
                    self.c_on_function_arg(arg).await;
                    if self.finished() {
                        return;
                    }
                }
                self.c_on_schema(name).await;
                if self.finished() {
                    return;
                }
                let (schema, name) = self.extract_schema_and_item(name);

                let columns = if let Some(cols) = self.get_table_alias_columns(alias) {
                    cols
                } else {
                    self.helper
                        .list_column_oked(
                            schema.as_deref(),
                            &name,
                            Some(args.iter().map(|s| s.to_string()).collect()),
                        )
                        .await
                };
                self.scopes.current_scope(|s| {
                    s.enter_temp_table(
                        alias.as_ref().map(|x| x.name.clone()),
                        TempTable {
                            columns: Arc::new(columns),
                        },
                    )
                });
            }
            TableFactor::UNNEST {
                alias, array_exprs, ..
            } => {
                if let Some(alias) = alias {
                    self.c_on_table_alias(alias).await;
                    if self.finished() {
                        return;
                    }
                }
                for expr in array_exprs.iter() {
                    self.c_on_expr(expr).await;
                    if self.finished() {
                        return;
                    }
                }
                let query_cols = self
                    .helper
                    .get_query_columns(&format!("select * from {}", ast))
                    .await
                    .map_err(|e| log::error!("get query columns error: {}", e))
                    .unwrap_or_default();
                let cols = if let Some(cols) = self.get_table_alias_columns(alias) {
                    cols
                } else {
                    query_cols
                };
                self.scopes.current_scope(|s| {
                    s.enter_temp_table(
                        alias.as_ref().map(|x| x.name.clone()),
                        TempTable {
                            columns: Arc::new(cols),
                        },
                    )
                });
            }
            TableFactor::JsonTable {
                alias,
                json_expr,
                columns,
                ..
            } => {
                for col in columns.iter() {
                    self.c_on_json_table_column(col).await;
                    if self.finished() {
                        return;
                    }
                }
                if let Some(alias) = alias {
                    self.c_on_table_alias(alias).await;
                    if self.finished() {
                        return;
                    }
                }
                self.c_on_expr(json_expr).await;
                if self.finished() {
                    return;
                }
                let query_cols = self
                    .helper
                    .get_query_columns(&format!("select * from {}", ast))
                    .await
                    .map_err(|e| log::error!("get query columns error: {}", e))
                    .unwrap_or_default();

                let cols = if let Some(cols) = self.get_table_alias_columns(alias) {
                    cols
                } else if !columns.is_empty() {
                    fn collect_json_columns(ret: &mut Vec<TempTableColumn>, ast: &JsonTableColumn) {
                        match ast {
                            JsonTableColumn::Named(json_table_named_column) => {
                                ret.push(TempTableColumn {
                                    name: json_table_named_column.name.value.clone(),
                                    data_type: Some(json_table_named_column.r#type.to_string()),
                                });
                            }
                            JsonTableColumn::ForOrdinality(ident) => {
                                ret.push(TempTableColumn {
                                    name: ident.value.clone(),
                                    data_type: None,
                                });
                            }
                            JsonTableColumn::Nested(json_table_nested_column) => {
                                for col in json_table_nested_column.columns.iter() {
                                    collect_json_columns(ret, col)
                                }
                            }
                        }
                    }
                    let mut ret = vec![];
                    columns
                        .iter()
                        .for_each(|x| collect_json_columns(&mut ret, x));
                    ret
                } else {
                    query_cols
                };
                self.scopes.current_scope(|s| {
                    s.enter_temp_table(
                        alias.as_ref().map(|x| x.name.clone()),
                        TempTable {
                            columns: Arc::new(cols),
                        },
                    );
                });
            }
            TableFactor::OpenJsonTable {
                alias,
                json_expr,
                columns,
                ..
            } => {
                for col in columns.iter() {
                    self.c_on_open_json_table_column(col).await;
                    if self.finished() {
                        return;
                    }
                }
                if let Some(alias) = alias {
                    self.c_on_table_alias(alias).await;
                    if self.finished() {
                        return;
                    }
                }
                self.c_on_expr(json_expr).await;
                if self.finished() {
                    return;
                }
                let query_cols = self
                    .helper
                    .get_query_columns(&format!("select * from {}", ast))
                    .await
                    .map_err(|e| log::error!("get query columns error: {}", e))
                    .unwrap_or_default();

                let cols = if let Some(cols) = self.get_table_alias_columns(alias) {
                    cols
                } else if !columns.is_empty() {
                    columns
                        .iter()
                        .map(|c| TempTableColumn {
                            name: c.name.value.clone(),
                            data_type: Some(c.r#type.to_string()),
                        })
                        .collect()
                } else {
                    query_cols
                };
                self.scopes.current_scope(|s| {
                    s.enter_temp_table(
                        alias.as_ref().map(|x| x.name.clone()),
                        TempTable {
                            columns: Arc::new(cols),
                        },
                    );
                });
            }
            TableFactor::NestedJoin {
                alias,
                table_with_joins,
            } => {
                if let Some(alias2) = alias {
                    self.c_on_table_alias(alias2).await;
                    if self.finished() {
                        return;
                    }
                    self.scopes.current_scope(|s| {
                        s.enter_temp_table(
                            Some(alias2.name.clone()),
                            TempTable {
                                columns: Arc::new(
                                    self.get_table_alias_columns(alias).unwrap_or_default(),
                                ),
                            },
                        )
                    });
                }
                Box::pin(self.c_on_table_with_join(table_with_joins)).await;
            }
            TableFactor::Pivot { .. } => {
                // bigquery Snowflake not support
            }
            TableFactor::Unpivot { .. } => {
                // snowflake not support
            }
            TableFactor::MatchRecognize { .. } => {
                // snowflake not support
            }
            TableFactor::XmlTable {
                alias,
                namespaces,
                passing,
                columns,
                row_expression,
            } => {
                for c in columns.iter() {
                    self.c_on_xml_table_column(c).await;
                    if self.finished() {
                        return;
                    }
                }
                self.c_on_expr(row_expression).await;
                if self.finished() {
                    return;
                }
                if let Some(alias) = alias {
                    self.c_on_table_alias(alias).await;
                    if self.finished() {
                        return;
                    }
                }
                for p in passing.arguments.iter() {
                    self.c_on_expr(&p.expr).await;
                    if self.finished() {
                        return;
                    }
                    if let Some(name) = p.alias.as_ref() {
                        if self.in_pos(&name.span) {
                            self.set_on_def();
                            return;
                        }
                    }
                }
                for def in namespaces.iter() {
                    if self.in_pos(&def.name.span) {
                        self.set_on_def();
                        return;
                    }
                    self.c_on_expr(&def.uri).await;
                    if self.finished() {
                        return;
                    }
                }
                let cols = if let Some(cols) = self.get_table_alias_columns(alias) {
                    cols
                } else {
                    columns
                        .iter()
                        .map(|c| TempTableColumn {
                            name: c.name.value.clone(),
                            data_type: None,
                        })
                        .collect()
                };
                self.scopes.current_scope(|s| {
                    s.enter_temp_table(
                        alias.as_ref().map(|x| x.name.clone()),
                        TempTable {
                            columns: Arc::new(cols),
                        },
                    )
                });
            }
            TableFactor::SemanticView { .. } => {
                // snowflake not support
            }
        }
    }
    pub(crate) async fn c_on_xml_table_column(&mut self, col: &XmlTableColumn) {
        if self.in_pos(&col.name.span) {
            self.set_on_def();
            return;
        }
        match &col.option {
            XmlTableColumnOption::NamedInfo {
                r#type,
                path,
                default,
                nullable: _,
            } => {
                self.c_on_data_type(r#type).await;
                if self.finished() {
                    return;
                }
                if let Some(path) = path {
                    self.c_on_expr(path).await;
                    if self.finished() {
                        return;
                    }
                }
                if let Some(d) = default {
                    self.c_on_expr(d).await;
                    if self.finished() {}
                }
            }
            XmlTableColumnOption::ForOrdinality => {}
        }
    }

    pub(crate) async fn c_on_merge_clause(&mut self, cl: &MergeClause) {
        if let Some(p) = cl.predicate.as_ref() {
            self.c_on_expr(p).await;
        }
        match &cl.action {
            MergeAction::Insert(merge_insert_expr) => {
                match &merge_insert_expr.kind {
                    MergeInsertKind::Values(values) => {
                        for exprs in values.rows.iter() {
                            for expr in exprs.iter() {
                                self.c_on_expr(expr).await;
                                if self.finished() {
                                    return;
                                }
                            }
                        }
                    }
                    MergeInsertKind::Row => {}
                }
                // nothing to do here we can fallback to the lexer
                // for col in merge_insert_expr.columns.iter() {
                //
                // }
            }
            MergeAction::Update(assignments) => {
                for ass in assignments.assignments.iter() {
                    self.c_on_assignment(ass).await;
                    if self.finished() {
                        return;
                    }
                }
                if let Some(e) = assignments.update_predicate.as_ref() {
                    self.c_on_expr(e).await;
                }
                if let Some(e) = assignments.delete_predicate.as_ref() {
                    self.c_on_expr(e).await;
                }
            }
            MergeAction::Delete { .. } => {}
        }
    }
    pub(crate) async fn c_on_procedure(&mut self, name: &ObjectName) {
        self.c_on_schema(name).await;
        if self.finished() {
            return;
        }
        if !self.last_name_in_pos(name) {
            return;
        }
        log::info!("hit procedure:{:?}", name);
        let (schema, _) = self.extract_schema_and_item(name);
        self.result = Some(
            self.helper
                .list_procedure_completion(schema.as_deref())
                .await,
        );
    }

    pub(crate) fn last_name_in_pos(&mut self, name: &ObjectName) -> bool {
        name.0
            .last()
            .map(|x| self.in_pos(&x.span()))
            .unwrap_or_default()
    }

    // sqlite pragma
    pub(crate) async fn c_on_sqlite_pragma(&mut self, name: &ObjectName) {
        if !self.in_pos(&name.span()) {
            return;
        }
        self.result = Some(
            self.helper
                .sqlite_pragma()
                .await
                .map_err(|e| log::error!("get sqlite pragma failed,err:{}", e))
                .unwrap_or_default(),
        );
    }

    pub(crate) async fn c_on_function_call(&mut self, function: &Function) {
        self.c_on_function(&function.name).await;
        // handle args
        self.c_on_function_arguments(&function.args).await;
        self.c_on_function_arguments(&function.parameters).await;
        if let Some(expr) = function.filter.as_deref() {
            self.c_on_expr(expr).await;
        }
        if let Some(over) = function.over.as_ref() {
            self.c_on_window_type(over).await;
        }
        for w in function.within_group.iter() {
            self.c_on_order_by_expr(w).await;
            if self.finished() {
                return;
            }
        }
    }
    pub(crate) async fn c_on_window_spec(&mut self, ast: &WindowSpec) {
        // window_name  是 PostgreSQL 在  WINDOW  子句里给窗口定义起的“名字”。
        // 作用：当一条查询里多个窗口函数都要用 同一套分区、排序、范围 时，先给这段窗口定义一个名字，后面所有  OVER  子句直接引用这个名字即可，避免重复书写，也更容易维护。
        if let Some(window_name) = ast.window_name.as_ref() {
            if self.in_pos(&window_name.span) {
                self.set_on_def();
                return;
            }
        }
        for o in ast.order_by.iter() {
            self.c_on_order_by_expr(o).await;
            if self.finished() {
                return;
            }
        }

        if let Some(wf) = ast.window_frame.as_ref() {
            self.c_on_window_frame(wf).await;
        }
    }

    pub(crate) async fn c_on_window_frame(&mut self, ast: &WindowFrame) {
        self.c_on_window_frame_bound(&ast.start_bound).await;
        if let Some(end) = ast.end_bound.as_ref() {
            self.c_on_window_frame_bound(end).await;
        }
    }

    pub(crate) async fn c_on_window_type(&mut self, ast: &WindowType) {
        match ast {
            WindowType::WindowSpec(window_spec) => {
                self.c_on_window_spec(window_spec).await;
            }
            WindowType::NamedWindow(..) => {
                // fallback to lexer
            }
        }
    }
    pub(crate) async fn c_on_window_frame_bound(&mut self, ast: &WindowFrameBound) {
        match ast {
            WindowFrameBound::CurrentRow => {}
            WindowFrameBound::Preceding(expr) | WindowFrameBound::Following(expr) => {
                if let Some(expr) = expr.as_ref() {
                    self.c_on_expr(expr).await;
                }
            }
        }
    }

    pub(crate) async fn c_on_function_arguments(&mut self, args: &FunctionArguments) {
        match args {
            FunctionArguments::None => {}
            FunctionArguments::Subquery(query) => {
                let _ = self.c_on_query(query).await;
            }
            FunctionArguments::List(function_argument_list) => {
                for arg in function_argument_list.args.iter() {
                    match arg {
                        FunctionArg::Named {
                            name,
                            arg,
                            operator: _,
                        } => {
                            if self.in_pos(&name.span) {
                                self.set_on_def();
                                return;
                            }
                            if let FunctionArgExpr::Expr(expr) = arg {
                                self.c_on_expr(expr).await;
                            };
                        }
                        FunctionArg::ExprNamed {
                            name,
                            arg,
                            operator: _,
                        } => {
                            if self.in_pos(&name.span()) {
                                // make suggest nothing.
                                self.set_on_def();
                                return;
                            }
                            self.c_on_function_arg_expr(arg).await;
                        }
                        FunctionArg::Unnamed(function_arg_expr) => {
                            self.c_on_function_arg_expr(function_arg_expr).await;
                        }
                    }
                }
            }
        }
    }

    pub(crate) async fn c_on_function_drop(&mut self, name: &ObjectName) {
        self.c_on_schema(name).await;
        if self.finished() {
            return;
        }
        if !self.last_name_in_pos(name) {
            return;
        }
        log::info!("hit function drop:{:?}", name);
        let (schema, _) = self.extract_schema_and_item(name);
        self.result = Some(
            self.helper
                .list_function(schema.as_deref(), false)
                .await
                .map_err(|e| {
                    log::error!("list function error:{:?}", e);
                })
                .unwrap_or_default(),
        )
    }

    pub(crate) async fn c_on_function(&mut self, name: &ObjectName) {
        self.c_on_schema(name).await;
        if self.finished() {
            return;
        }
        if !self.last_name_in_pos(name) {
            return;
        }
        let (schema, _) = self.extract_schema_and_item(name);
        let mut ret = self
            .helper
            .list_function(schema.as_deref(), schema.is_none())
            .await
            .map_err(|e| {
                log::error!("list function error:{:?}", e);
            })
            .unwrap_or_default();
        ret.extend(
            self.helper
                .list_procedure_completion(schema.as_deref())
                .await,
        );
        self.result = Some(ret)
    }

    pub(crate) async fn c_on_role_option(&mut self, ast: &RoleOption) {
        match ast {
            RoleOption::ConnectionLimit(expr) | RoleOption::ValidUntil(expr) => {
                self.c_on_expr(expr).await;
            }
            RoleOption::Password(x) => {
                self.c_on_password(x).await;
            }
            _ => {}
        }
    }
    pub(crate) async fn c_on_password(&mut self, ast: &Password) {
        match ast {
            Password::Password(expr) => self.c_on_expr(expr).await,
            Password::NullPassword => {}
        }
    }
    pub(crate) fn extract_schema_and_item(
        &self,
        item_name: &ObjectName,
    ) -> (Option<String>, String) {
        extract_schema_and_item(item_name)
    }
}

pub(crate) fn extract_schema_and_item(item_name: &ObjectName) -> (Option<String>, String) {
    let mut schema = None;
    let mut item = "".to_string();
    match item_name.0.len() {
        1 => {
            item = match &item_name.0[0] {
                ObjectNamePart::Identifier(ident) => ident.value.to_string(),
                ObjectNamePart::Function(..) => {
                    // TODO support
                    "".to_string()
                }
            };
        }
        2 => {
            schema = Some(match &item_name.0[0] {
                ObjectNamePart::Identifier(ident) => ident.value.to_string(),
                ObjectNamePart::Function(..) => "".to_string(),
            });
            item = match &item_name.0[1] {
                ObjectNamePart::Identifier(ident) => ident.value.to_string(),
                ObjectNamePart::Function(..) => "".to_string(),
            };
        }
        _ => {
            log::error!("wrong ObjectName:{:?},size not right", item_name);
        }
    };
    (schema, item)
}
