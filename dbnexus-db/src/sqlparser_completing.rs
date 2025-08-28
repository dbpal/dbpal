use crate::language_server::{CompletionItem, CompletionKind, remove_dupliate};
use crate::sqlparser_completion::{
    CompletionStatement, Scope, ScopesDropGuard, TableEntered, TempTable, TempTableColumn,
};
use core::str;
use sqlparser::ast::{ColumnOption, *};
use std::collections::HashMap;
use std::ops::Deref;
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
        }
    }
}

impl<'a> CompletionStatement<'a> {
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
                .list_type(schema)
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
                let temp_table = TempTable {
                    columns: Arc::new(
                        column_defs
                            .iter()
                            .map(|x| TempTableColumn {
                                name: x.name.value.clone(),
                                data_type: Some(x.data_type.to_string()),
                            })
                            .collect::<Vec<_>>(),
                    ),
                };

                let mut scope = Scope::default();
                scope.enter_temp_table(None, temp_table.clone());
                let _gurad = self.scopes.enter(scope);
                for col in column_defs.iter() {
                    if self.in_pos(&col.name.span) {
                        self.set_on_def();
                        return;
                    }
                    for def in col.options.iter() {
                        self.c_on_column_option_def(def, temp_table.columns.clone(), None)
                            .await;
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
                let mut scope = Scope::default();
                scope.enter_temp_table(alias, temp_table.clone());
                let _gurad = self.scopes.enter(scope);
                for col in columns.iter() {
                    if self.in_pos(&col.name.span) {
                        self.set_on_def();
                        return;
                    }
                    for def in col.options.iter() {
                        self.c_on_column_option_def(def, temp_table.columns.clone(), None)
                            .await;
                        if self.finished() {
                            return;
                        }
                    }
                }
            }
            DataType::Character(..) => {}
            DataType::Char(..) => {}
            DataType::CharacterVarying(..) => {}
            DataType::CharVarying(..) => {}
            DataType::Varchar(..) => {}
            DataType::Nvarchar(..) => {}
            DataType::Uuid => {}
            DataType::CharacterLargeObject(_) => {}
            DataType::CharLargeObject(_) => {}
            DataType::Clob(_) => {}
            DataType::Binary(_) => {}
            DataType::Varbinary(..) => {}
            DataType::Blob(_) => {}
            DataType::TinyBlob => {}
            DataType::MediumBlob => {}
            DataType::LongBlob => {}
            DataType::Bytes(_) => {}
            DataType::Numeric(..) => {}
            DataType::Decimal(..) => {}
            DataType::BigNumeric(..) => {}
            DataType::BigDecimal(..) => {}
            DataType::Dec(..) => {}
            DataType::Float(_) => {}
            DataType::TinyInt(_) => {}
            DataType::TinyIntUnsigned(_) => {}
            DataType::UTinyInt => {}
            DataType::Int2(_) => {}
            DataType::Int2Unsigned(_) => {}
            DataType::SmallInt(_) => {}
            DataType::SmallIntUnsigned(_) => {}
            DataType::USmallInt => {}
            DataType::MediumInt(_) => {}
            DataType::MediumIntUnsigned(_) => {}
            DataType::Int(_) => {}
            DataType::Int4(_) => {}
            DataType::Int8(_) => {}
            DataType::Int16 => {}
            DataType::Int32 => {}
            DataType::Int64 => {}
            DataType::Int128 => {}
            DataType::Int256 => {}
            DataType::Integer(_) => {}
            DataType::IntUnsigned(_) => {}
            DataType::Int4Unsigned(_) => {}
            DataType::IntegerUnsigned(_) => {}
            DataType::HugeInt => {}
            DataType::UHugeInt => {}
            DataType::UInt8 => {}
            DataType::UInt16 => {}
            DataType::UInt32 => {}
            DataType::UInt64 => {}
            DataType::UInt128 => {}
            DataType::UInt256 => {}
            DataType::BigInt(_) => {}
            DataType::BigIntUnsigned(_) => {}
            DataType::UBigInt => {}
            DataType::Int8Unsigned(_) => {}
            DataType::Signed => {}
            DataType::SignedInteger => {}
            DataType::Unsigned => {}
            DataType::UnsignedInteger => {}
            DataType::Float4 => {}
            DataType::Float32 => {}
            DataType::Float64 => {}
            DataType::Real => {}
            DataType::Float8 => {}
            DataType::Double(..) => {}
            DataType::DoublePrecision => {}
            DataType::Bool => {}
            DataType::Boolean => {}
            DataType::Date => {}
            DataType::Date32 => {}
            DataType::Time(..) => {}
            DataType::Datetime(_) => {}
            DataType::Datetime64(_, _) => {}
            DataType::Timestamp(..) => {}
            DataType::TimestampNtz => {}
            DataType::Interval => {}
            DataType::JSON => {}
            DataType::JSONB => {}
            DataType::Regclass => {}
            DataType::Text => {}
            DataType::TinyText => {}
            DataType::MediumText => {}
            DataType::LongText => {}
            DataType::String(_) => {}
            DataType::FixedString(_) => {}
            DataType::Bytea => {}
            DataType::Bit(_) => {}
            DataType::BitVarying(_) => {}
            DataType::VarBit(_) => {}
            DataType::Custom(_object_name, ..) => {
                // TODO list type???
            }
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
                }
            }
            DataType::Nested(column_defs) => {
                let temp_table = TempTable {
                    columns: Arc::new(
                        column_defs
                            .iter()
                            .map(|x| TempTableColumn {
                                name: x.name.value.clone(),
                                data_type: Some(x.data_type.to_string()),
                            })
                            .collect::<Vec<_>>(),
                    ),
                };
                let mut scope = Scope::default();
                scope.enter_temp_table(None, temp_table.clone());
                let _gurad = self.scopes.enter(scope);
                for col in column_defs.iter() {
                    if self.in_pos(&col.name.span) {
                        self.set_on_def();
                        return;
                    }
                    for def in col.options.iter() {
                        self.c_on_column_option_def(def, temp_table.columns.clone(), None)
                            .await;
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
                        }
                    }
                }
            }
            DataType::Set(..) => {
                // nothings
            }
            DataType::Struct(struct_fields, ..) => {
                let cols: Vec<_> = struct_fields
                    .iter()
                    .filter(|x| matches!(x.field_name, Some(_)))
                    .map(|x| TempTableColumn {
                        // we filterd before
                        name: x.field_name.as_ref().map(|x| x.value.clone()).unwrap(),
                        data_type: Some(x.field_type.to_string()),
                    })
                    .collect();
                let cols = Arc::new(cols);

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
                            self.c_on_sql_option(op, cols.clone()).await;
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
            DataType::Unspecified => {}
            DataType::Trigger => {}
            DataType::AnyType => {}
            DataType::GeometricType(..) => {}
            DataType::TsVector => {}
            DataType::TsQuery => {}
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
    pub(crate) async fn c_on_duck_extension(&mut self, name: &Ident, installed: Option<bool>) {
        if !self.in_pos(&name.span) {
            return;
        }
        self.result = Some(
            self.helper
                .duckdb_extensions(installed)
                .await
                .map_err(|err| {
                    log::error!("duckdb_extensions failed:{}", err);
                })
                .unwrap_or_default(),
        )
    }

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
                    log::error!("list role failed,err:{}", e);
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
            ShowCreateObject::Function => self.c_on_function(name).await,
            ShowCreateObject::Procedure => self.c_on_procedure(name).await,
            ShowCreateObject::Table => self.c_on_table(name).await,
            ShowCreateObject::Trigger => self.c_on_mysql_trigger(name).await,
            ShowCreateObject::View => self.c_on_view(name).await,
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
                    log::error!("list variable failed,err:{:?}", e);
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

    pub(crate) async fn c_on_execute(&mut self, name: &ObjectName) {
        if !self.last_name_in_pos(name) {
            return;
        }
        let (schema, _) = self.extract_schema_and_item(name);
        self.result = Some(
            self.helper
                .list_execute(schema)
                .await
                .map_err(|e| log::error!("list prepare failed,err:{}", e))
                .unwrap_or_default()
                .into_iter()
                .map(CompletionItem::from)
                .collect(),
        );
    }

    pub(crate) async fn c_on_prepare_ident(&mut self, ident: &Ident) {
        if !self.in_pos(&ident.span) {
            return;
        }
        self.result = Some(
            self.helper
                .list_execute(None)
                .await
                .map_err(|e| log::error!("list prepare failed,err:{}", e))
                .unwrap_or_default()
                .into_iter()
                .map(CompletionItem::from)
                .collect(),
        );
    }

    pub(crate) async fn c_on_table_policy_object(
        &mut self, table_name: &ObjectName, name: &ObjectName,
    ) {
        match name.0.len() {
            1 => match name.0.last().unwrap() {
                ObjectNamePart::Identifier(ident) => {
                    self.c_on_table_policy(table_name, ident).await
                }
                ObjectNamePart::Function(..) => {
                    // not support right now.
                }
            },

            _ => {
                log::error!("unhandle table policy object :{}", name)
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
                .table_policy(schema, table)
                .await
                .map_err(|e| log::error!("table_policy failed,err:{:?}", e))
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
        self.c_on_schema(name).await;
        if self.finished() {
            return;
        }
        if !self.in_pos(&name.span()) {
            return;
        }
        match typ {
            ObjectType::Table => self.c_on_table(name).await,
            ObjectType::View | ObjectType::MaterializedView => self.c_on_view(name).await,
            ObjectType::Index => self.c_on_index(name).await,
            ObjectType::Schema => {
                // c_on_db_schema 假设形式是 database.schema.table
                // 但是在这种情况下 ident肯定没这么长
                // 所以加一个假的
                let mut name = name.0.clone();
                name.push(ObjectNamePart::Identifier(Ident::new("fake")));
                self.c_on_schema(&ObjectName(name)).await;
            }
            ObjectType::Role => match name.0.len() {
                1 => {
                    self.c_on_role(match name.0.first().unwrap() {
                        ObjectNamePart::Identifier(ident) => ident,
                        ObjectNamePart::Function(object_name_part_function) => {
                            // TODO
                            &object_name_part_function.name
                        }
                    })
                    .await
                }
                _ => {
                    log::error!("role name is too long:{:?}", name)
                }
            },
            ObjectType::Sequence => {
                // not support
            }
            ObjectType::Stage => {
                // not support
            }
            ObjectType::Type => {
                // not support
            }
            ObjectType::User => match name.0.last() {
                Some(x) => match x {
                    ObjectNamePart::Identifier(ident) => self.c_on_user(ident).await,
                    ObjectNamePart::Function(..) => {}
                },
                None => {}
            },
            ObjectType::Database => {
                // not support
            }
        }
    }
    pub(crate) async fn c_on_user(&mut self, user: &Ident) {
        if self.in_pos(&user.span) {
            return;
        }
        self.result = Some(
            self.helper
                .list_user()
                .await
                .map_err(|e| log::error!("list user failed,err:{}", e))
                .unwrap_or_default()
                .into_iter()
                .map(|x| CompletionItem {
                    item: x.into(),
                    kind: CompletionKind::User,
                    desc: None,
                })
                .collect(),
        )
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
                            .list_column(schema, table, false)
                            .await
                            .map_err(|e| {
                                log::error!("list_column failed,err:{}", e);
                            })
                            .map(|x| x.into_iter().map(CompletionItem::from).collect::<Vec<_>>());
                        if let Ok(x) = cs {
                            self.result = Some(x);
                            return;
                        }
                    }
                }
            }
            Action::AddSearchOptimization => {}
            Action::Apply { .. } => {}
            Action::ApplyBudget => {}
            Action::AttachListing => {}
            Action::AttachPolicy => {}
            Action::Audit => {}
            Action::BindServiceEndpoint => {}
            Action::Connect => {}
            Action::Create { .. } => {}
            Action::DatabaseRole { role } => match role.0.len() {
                1 => match role.0.first().unwrap() {
                    ObjectNamePart::Identifier(ident) => self.c_on_role(ident).await,
                    ObjectNamePart::Function(..) => {}
                },
                _ => {
                    log::error!("not 1 length role name named {:?}", role)
                }
            },
            Action::Delete => {}
            Action::EvolveSchema => {}
            Action::Exec { .. } => {}
            Action::Execute { .. } => {}
            Action::Failover => {}
            Action::ImportedPrivileges => {}
            Action::ImportShare => {}
            Action::Manage { .. } => {}
            Action::ManageReleases => {}
            Action::ManageVersions => {}
            Action::Modify { .. } => {}
            Action::Monitor { .. } => {}
            Action::Operate => {}
            Action::OverrideShareRestrictions => {}
            Action::Ownership => {}
            Action::PurchaseDataExchangeListing => {}
            Action::Read => {}
            Action::ReadSession => {}
            Action::Replicate => {}
            Action::ResolveAll => {}
            Action::Role { role } => {
                self.c_on_role(role).await;
            }
            Action::Temporary => {}
            Action::Trigger => {}
            Action::Truncate => {}
            Action::Usage => {}
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
            CommentObject::Schema => {
                // c_on_db_schema 假设形式是 database.schema.table
                // 但是在这种情况下 ident肯定没这么长
                // 所以加一个假的
                let mut name = name.0.clone();
                name.push(ObjectNamePart::Identifier(Ident::new("fake")));
                self.c_on_schema(&ObjectName(name)).await;
            }
            CommentObject::Role => {
                // not support right now
            }
            CommentObject::Column => {
                if name.0.len() >= 2 {
                    let table = name.0[..name.0.len() - 1].to_vec();
                    self.c_on_column(
                        &ObjectName(table),
                        match name.0.last().unwrap() {
                            ObjectNamePart::Identifier(ident) => ident,
                            ObjectNamePart::Function(object_name_part_function) => {
                                // TODO snowflak not support
                                &object_name_part_function.name
                            }
                        },
                        false,
                    )
                    .await;
                }
            }
            CommentObject::Extension => {
                // not support right now
            }
            CommentObject::User => match name.0.len() {
                1 => match name.0.last().unwrap() {
                    ObjectNamePart::Identifier(ident) => {
                        self.c_on_user(ident).await;
                    }
                    ObjectNamePart::Function(..) => {}
                },
                _ => {
                    log::error!("name not right:{:?}", name)
                }
            },
            CommentObject::Database => {
                //not support right now s
            }
        }
    }

    pub(crate) async fn c_on_column_mysql_position(
        &mut self, table_name: &ObjectName, column_position: Option<&MySQLColumnPosition>,
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
                            .list_partition(schema, table)
                            .await
                            .map_err(|e| {
                                log::error!("list partition failed,err:{}", e);
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
                /*
                在大多数主流数据库管理系统中，
                ANALYZE 命令的 PARTITION 子句通常不支持使用表达式，
                要求明确指定分区名称。然而，
                某些数据库系统可能允许在不同上下文中使用表达式来选择分区。以下是一些示例：*/
                log::error!("unhandled partation type:{:?}", p);
            }
        }
    }

    pub(crate) async fn c_on_index(&mut self, name: &ObjectName) {
        self.c_on_schema(name).await;
        if self.finished() {
            return;
        }
        if !self.last_name_in_pos(name) {
            return;
        }
        let (schema, _) = self.extract_schema_and_item(name);
        self.result = Some(
            self.helper
                .list_index(schema)
                .await
                .map_err(|err| {
                    log::error!("list index failed,err:{}", err);
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

    pub(crate) fn extract_schema_and_item<'b>(
        &mut self, item_name: &'b ObjectName,
    ) -> (Option<&'b str>, &'b str) {
        let mut schema = None;
        let mut table = "";
        match item_name.0.len() {
            1 => {
                table = match &item_name.0[0] {
                    ObjectNamePart::Identifier(ident) => ident.value.as_str(),
                    ObjectNamePart::Function(..) => {
                        // TODO support
                        ""
                    }
                };
            }
            2 => {
                schema = Some(match &item_name.0[0] {
                    ObjectNamePart::Identifier(ident) => ident.value.as_str(),
                    ObjectNamePart::Function(..) => "",
                });
                table = match &item_name.0[1] {
                    ObjectNamePart::Identifier(ident) => ident.value.as_str(),
                    ObjectNamePart::Function(..) => "",
                };
            }
            _ => {
                log::error!("wrong ObjectName:{:?},size not right", item_name);
            }
        };
        (schema, table)
    }

    pub(crate) async fn c_on_postgres_rule(&mut self, table_name: &ObjectName, rule: &Ident) {
        if !self.in_pos(&rule.span) {
            return;
        }
        let (schema, table) = self.extract_schema_and_item(table_name);
        self.result = Some(
            self.helper
                .list_postgres_rule(schema, table)
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
        if self.in_pos(&name.span) {
            let (schema, table_name) = self.extract_schema_and_item(table_name);
            self.result = Some(
                self.helper
                    .list_constraint(schema, table_name)
                    .await
                    .map_err(|err| {
                        log::error!("list_constraint failed,err:{}", err);
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

    pub(crate) async fn c_on_clickhouse_partition(
        &mut self, table_name: &ObjectName, p: &Partition,
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
        &mut self, table_name: &ObjectName, name: &Ident,
    ) {
        if !self.in_pos(&name.span) {
            return;
        }
        // hit pos
        let (schema, table) = self.extract_schema_and_item(table_name);
        self.result = Some(
            self.helper
                .list_clickhouse_table_projects(schema, table)
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

    pub(crate) async fn c_on_alert_table(
        &mut self, table_name: &ObjectName, operations: &Vec<AlterTableOperation>,
    ) {
        self.c_on_table(table_name).await;
        if self.finished() {
            return;
        }
        for op in operations.iter() {
            match op {
                AlterTableOperation::AddConstraint { constraint, .. } => {
                    match constraint {
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
                                self.c_on_constraint(table_name, name).await;
                                if self.finished() {
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
                                self.c_on_index_column(table_name, c).await;
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
                                self.c_on_column(table_name, c, false).await;
                                if self.finished() {
                                    return;
                                }
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
                        TableConstraint::Check { name, expr, .. } => {
                            //  https://www.w3schools.com/sql/sql_check.asp
                            let _guard = self
                                .scopes
                                .enter(Scope::from_this_table(table_name.clone()));
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
                                self.c_on_index_column(table_name, c).await;
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
                                self.c_on_index_column(table_name, c).await;
                                if self.finished() {
                                    return;
                                }
                            }
                        }
                    }
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
                    let cols = Arc::new({
                        let mut cols = self
                            .helper
                            .list_column(schema, table, false)
                            .await
                            .map_err(|e| log::error!("list table colunm error: {}", e))
                            .unwrap_or_default();
                        // add the column that we are adding
                        cols.push(TempTableColumn {
                            name: column_def.name.value.clone(),
                            data_type: Some(column_def.data_type.to_string()),
                        });
                        cols
                    });
                    let mut scope = Scope::default();
                    scope.enter_temp_table(
                        None,
                        TempTable {
                            columns: cols.clone(),
                        },
                    );
                    let _gurad = self.scopes.enter(scope);
                    for o in column_def.options.iter() {
                        self.c_on_column_option_def(o, cols.clone(), Some(table_name))
                            .await;
                    }
                    self.c_on_column_mysql_position(table_name, column_position.as_ref())
                        .await;
                }
                AlterTableOperation::AddProjection { name, select, .. } => {
                    if self.in_pos(&name.span) {
                        self.set_on_def();
                        return;
                    }
                    self.c_on_select_item(&select.projection[..]).await;
                    let _gurad = self
                        .scopes
                        .enter(Scope::from_this_table(table_name.clone()));
                    if let Some(x) = select.group_by.as_ref() {
                        match x {
                            GroupByExpr::All(group_by_with_modifiers) => {
                                for g in group_by_with_modifiers.iter() {
                                    match g {
                                        GroupByWithModifier::Rollup => {}
                                        GroupByWithModifier::Cube => {}
                                        GroupByWithModifier::Totals => {}
                                        GroupByWithModifier::GroupingSets(expr) => {
                                            self.c_on_expr(expr).await;
                                            if self.finished() {
                                                return;
                                            }
                                        }
                                    }
                                }
                            }
                            GroupByExpr::Expressions(exprs, group_by_with_modifiers) => {
                                for g in group_by_with_modifiers.iter() {
                                    match g {
                                        GroupByWithModifier::Rollup => {}
                                        GroupByWithModifier::Cube => {}
                                        GroupByWithModifier::Totals => {}
                                        GroupByWithModifier::GroupingSets(expr) => {
                                            self.c_on_expr(expr).await;
                                            if self.finished() {
                                                return;
                                            }
                                        }
                                    }
                                }
                                for e in exprs.iter() {
                                    self.c_on_expr(e).await;
                                    if self.finished() {
                                        return;
                                    }
                                }
                            }
                        }
                    }
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
                    let _guard = self
                        .scopes
                        .enter(Scope::from_this_table(table_name.clone()));
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
                    self.c_on_column(table_name, old_column_name, false).await;
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
                }
                AlterTableOperation::ModifyColumn {
                    col_name,
                    column_position,
                    ..
                } => {
                    self.c_on_column(table_name, col_name, false).await;
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
                    self.c_on_column(table_name, column_name, false).await;
                    // other fallback to lexer.
                }
                AlterTableOperation::SwapWith { table_name } => {
                    self.c_on_table(table_name).await;
                }
                AlterTableOperation::SetTblProperties { .. } => {
                    // no special handle
                }
                AlterTableOperation::OwnerTo { new_owner } => {
                    // no need
                    self.c_on_owner(new_owner).await;
                }
                AlterTableOperation::ClusterBy { exprs: _ } => {
                    //  Snowflake
                }
                AlterTableOperation::DropClusteringKey => {}
                AlterTableOperation::SuspendRecluster => {}
                AlterTableOperation::ResumeRecluster => {}
                AlterTableOperation::Algorithm { .. } => {}
                AlterTableOperation::AutoIncrement { .. } => {}
                AlterTableOperation::DropForeignKey { .. } => {
                    //  TODO support this.
                }
                AlterTableOperation::DropIndex { name } => {
                    self.c_on_index(&ObjectName(vec![ObjectNamePart::Identifier(name.clone())]))
                        .await;
                }
                AlterTableOperation::ReplicaIdentity { .. } => {}
                AlterTableOperation::Lock { .. } => {}
                AlterTableOperation::ValidateConstraint { .. } => {
                    // TODO select containts
                }
            }
        }
    }
    pub(crate) async fn c_on_owner(&mut self, o: &Owner) {
        match o {
            Owner::Ident(x) => {
                if self.in_pos(&x.span) {
                    self.result = Some(
                        self.helper
                            .list_role()
                            .await
                            .unwrap_or_default()
                            .into_iter()
                            .chain(
                                self.helper
                                    .list_user()
                                    .await
                                    .unwrap_or_default()
                                    .into_iter(),
                            )
                            .map(|x| CompletionItem {
                                item: x.into(),
                                kind: CompletionKind::User,
                                desc: None,
                            })
                            .collect::<Vec<_>>(),
                    )
                }
            }
            _ => {}
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
                .list_view(schema)
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

    pub(crate) async fn c_on_expr_compund_identifier(&mut self, idents: &[Ident]) {
        // we must access table's column???
        match idents.len() {
            1 => {
                // maybe never execute to here???
                self.c_on_expr_ident(&idents[0]).await;
            }
            2 => {
                let table = &idents[0];

                // try match for schema
                if self.helper.hit_schema(&table.value).await {
                    // matched  schema
                    // this is very likely a `select * from sys.xxx`   on mysql
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
                    match &table {
                        TableEntered::FromTable { table, .. } => {
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
                        .to_vec()
                        .into_iter()
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
        log::trace!("hit idenfier:{:?}", ident);
        let mut results = vec![];
        let tables = self.scopes.tables_cloned();
        for t in tables.iter() {
            match t {
                TableEntered::FromTable { table, .. } => {
                    let (schema, table) = self.extract_schema_and_item(table);
                    let items = self
                        .helper
                        .list_column(schema, table, false)
                        .await
                        .map_err(|e| {
                            log::error!("list_column failed,err:{}", e);
                        })
                        .ok()
                        .unwrap_or_default();
                    results.extend(items.into_iter().map(CompletionItem::from));
                }
                TableEntered::Temp { table, .. } => {
                    results.extend(
                        table
                            .columns
                            .to_vec()
                            .into_iter()
                            .map(CompletionItem::from)
                            .collect::<Vec<_>>(),
                    );
                }
            }
        }
        // this line will rewrite the results.
        self.c_on_select_target(&ObjectName(vec![ObjectNamePart::Identifier(ident.clone())]))
            .await;

        results.extend(self.result.take().unwrap_or_default().into_iter());

        if ident.quote_style.is_none() {
            // 我们也要函数
            let fs = self
                .helper
                .list_function(None, true)
                .await
                .ok()
                .unwrap_or_default();
            results.extend(fs);
        }
        //也要schema
        let schemas: Vec<_> = self
            .helper
            .list_schema()
            .await
            .ok()
            .unwrap_or_default()
            .into_iter()
            .map(|x| CompletionItem {
                item: x.into(),
                kind: CompletionKind::Schema,
                desc: None,
            })
            .collect();
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
        results.extend(schemas.into_iter());
        results = remove_dupliate(results);
        self.result = Some(results);
    }

    /// on `ColumnOptionDef` require table been entered
    ///
    pub(crate) async fn c_on_column_option_def(
        &mut self, def: &ColumnOptionDef, cols: Arc<Vec<TempTableColumn>>,
        table_name: Option<&ObjectName>,
    ) {
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
            ColumnOption::ForeignKey {
                foreign_table,
                referred_columns,
                ..
            } => {
                self.c_on_table(foreign_table).await;
                if self.finished() {
                    return;
                }
                for r in referred_columns.iter() {
                    self.c_on_column(foreign_table, r, false).await;
                    if self.finished() {}
                }
            }
            ColumnOption::Check(expr) => {
                self.c_on_expr(expr).await;
            }
            ColumnOption::DialectSpecific(..) => {
                // fallback to lexer
            }
            ColumnOption::CharacterSet(..) => {
                // fallback to lexer
            }
            ColumnOption::Comment(_) => {
                // no postiion
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
                        match op {
                            SequenceOptions::IncrementBy(expr, _) => {
                                self.c_on_expr(expr).await;
                            }
                            SequenceOptions::MinValue(expr) => {
                                if let Some(expr) = expr {
                                    self.c_on_expr(expr).await;
                                }
                            }
                            SequenceOptions::MaxValue(expr) => {
                                if let Some(expr) = expr {
                                    self.c_on_expr(expr).await;
                                }
                            }
                            SequenceOptions::StartWith(expr, _) => {
                                self.c_on_expr(expr).await;
                            }
                            SequenceOptions::Cache(expr) => {
                                self.c_on_expr(expr).await;
                            }
                            SequenceOptions::Cycle(..) => {}
                        }
                    }
                }
                if let Some(expr) = generation_expr {
                    self.c_on_expr(expr).await;
                }
            }
            ColumnOption::Options(options) => {
                for op in options.iter() {
                    self.c_on_sql_option(op, cols.clone()).await;
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
                if let Some(table_name) = table_name {
                    self.c_on_table_policy_object(table_name, &ast.policy_name)
                        .await;
                }
                if self.finished() {
                    return;
                }
                if let Some(xs) = ast.using_columns.as_ref() {
                    if xs.iter().any(|col| self.in_pos(&col.span)) {
                        self.result = Some(cols.iter().map(CompletionItem::from).collect());
                        return;
                    }
                }
            }
            ColumnOption::Tags(..) => {
                //fallback to lexer
            }
            ColumnOption::Collation(..) => {
                // 我不知道这个是什么呢
            }
            ColumnOption::Srid(expr) => {
                self.c_on_expr(expr).await;
            }
        }
    }

    pub(crate) async fn c_on_sql_option(
        &mut self, ast: &SqlOption, cols: Arc<Vec<TempTableColumn>>,
    ) {
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
                    self.result = Some(cols.iter().map(CompletionItem::from).collect());

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
                self.c_on_expr_compund_identifier(vec).await;
            }
            Expr::JsonAccess { .. } => {
                // snowflake databricks not support
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
            Expr::InList { expr, list, .. } => {
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
            Expr::InSubquery { expr, subquery, .. } => {
                Box::pin(self.c_on_expr(expr)).await;
                if self.finished() {
                    return;
                }

                let _ = Box::pin(self.c_on_query(subquery.as_ref()))
                    .await
                    .map(|e| log::error!("a error:{:?}", e))
                    .ok();
            }
            Expr::InUnnest {
                expr, array_expr, ..
            } => {
                Box::pin(self.c_on_expr(expr)).await;
                if self.finished() {
                    return;
                }
                Box::pin(self.c_on_expr(array_expr)).await;
            }
            Expr::Between {
                expr, low, high, ..
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
            Expr::BinaryOp { left, right, .. } => {
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
            Expr::Convert { expr, styles, .. } => {
                Box::pin(self.c_on_expr(expr)).await;
                for style in styles.iter() {
                    Box::pin(self.c_on_expr(style)).await;
                    if self.finished() {
                        return;
                    }
                }
            }
            Expr::Cast { expr, .. } => {
                Box::pin(self.c_on_expr(expr)).await;
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
            Expr::TypedString { .. } => {
                // literal value
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
                    .map_err(|e| log::error!("a error:{}", e))
                    .ok();
            }
            Expr::Subquery(query) => {
                let _ = Box::pin(self.c_on_query(query))
                    .await
                    .map_err(|e| log::error!("a error:{}", e))
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
                // bigquery not support
            }
            Expr::Named { .. } => {
                // bigquery not support
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
                    match c.0.len() {
                        1 => match c.0.first().unwrap() {
                            ObjectNamePart::Identifier(ident) => self.c_on_expr_ident(ident).await,
                            ObjectNamePart::Function(..) => {
                                // TODO
                            }
                        },

                        _ => {
                            let idents =
                                c.0.iter()
                                    .filter(|x| match x {
                                        ObjectNamePart::Identifier(_) => true,
                                        ObjectNamePart::Function(..) => false,
                                    })
                                    .map(|x| match x {
                                        ObjectNamePart::Identifier(ident) => ident.clone(),
                                        ObjectNamePart::Function(object_name_part_function) => {
                                            object_name_part_function.name.clone()
                                        }
                                    })
                                    .collect::<Vec<_>>();

                            self.c_on_expr_compund_identifier(&idents[..]).await;
                        }
                    }
                    if self.finished() {
                        return;
                    }
                }
            }
            Expr::Wildcard(..) => {
                // nothing here
            }
            Expr::QualifiedWildcard(table, _) => {
                // nothing here
                self.c_on_table_or_view(table).await;
            }
            Expr::OuterJoin(expr) => {
                Box::pin(self.c_on_expr(expr)).await;
            }
            Expr::Prior(expr) => {
                Box::pin(self.c_on_expr(expr)).await;
            }
            Expr::Lambda(..) => {
                // databricks no support
            }
            Expr::CompoundFieldAccess { root, access_chain } => {
                Box::pin(self.c_on_expr(root)).await;

                for acc in access_chain.iter() {
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

    pub(crate) async fn c_on_order_by_expr(&mut self, orderby: &OrderByExpr) {
        self.c_on_expr(&orderby.expr).await;
        if self.finished() {
            return;
        }
        if let Some(full) = orderby.with_fill.as_ref() {
            if let Some(from) = full.from.as_ref() {
                self.c_on_expr(from).await;
                if self.finished() {
                    return;
                }
            }
            if let Some(to) = full.to.as_ref() {
                self.c_on_expr(to).await;
                if self.finished() {
                    return;
                }
            }
            if let Some(step) = full.step.as_ref() {
                self.c_on_expr(step).await;
                if self.finished() {}
            }
        }
    }

    pub(crate) async fn c_on_index_column(&mut self, table_name: &ObjectName, col: &IndexColumn) {
        let _gurad = self
            .scopes
            .enter(Scope::from_this_table(table_name.clone()));
        self.c_on_expr(&col.column.expr).await;
        if let Some(with_full) = col.column.with_fill.as_ref() {
            if let Some(x) = with_full.from.as_ref() {
                self.c_on_expr(x).await;
            }
            if let Some(x) = with_full.to.as_ref() {
                self.c_on_expr(x).await;
            }
            if let Some(x) = with_full.step.as_ref() {
                self.c_on_expr(x).await;
            }
        }
    }

    pub(crate) async fn c_on_column(
        &mut self, table_name: &ObjectName, col: &Ident, suggest_table: bool,
    ) {
        if !self.in_pos(&col.span) {
            return;
        }
        log::info!("hit column:{:?}", col);
        if suggest_table {
            self.c_on_table_or_view(table_name).await;
        }
        let (schema, table_name) = self.extract_schema_and_item(table_name);
        if self.result.is_none() {
            self.result = Some(vec![]);
        }
        let items = self
            .helper
            .list_column(schema, table_name, false)
            .await
            .map_err(|e| {
                log::error!("list column error:{:?}", e);
            })
            .ok()
            .unwrap_or_default()
            .into_iter()
            .map(CompletionItem::from)
            .collect::<Vec<_>>();
        if self.result.is_none() {
            self.result = Some(vec![]);
        }
        self.result.as_mut().unwrap().extend(items);
    }

    pub(crate) async fn c_on_table_trigger(
        &mut self, table_name: &ObjectName, trigger_name: &Ident,
    ) {
        if self.in_pos(&trigger_name.span) {
            let (schema, table_name) = self.extract_schema_and_item(table_name);
            self.result = Some(
                self.helper
                    .list_table_trigger(schema, table_name)
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
    }

    // completion 假设  schema.table
    pub(crate) async fn c_on_schema(&mut self, name: &ObjectName) {
        match name.0.len() {
            2 => {
                let schema = &name.0[0];
                if self.in_pos(&schema.span()) {
                    self.result = Some(
                        self.helper
                            .list_schema()
                            .await
                            .map_err(|e| {
                                log::error!("list schema error:{:?}", e);
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
        items.extend(self.result.take().unwrap_or_default().into_iter());
        self.result = Some(items);
    }

    pub(crate) async fn c_on_select_target(&mut self, name: &ObjectName) {
        self.c_on_table_or_view(name).await;
        let (schema, _) = self.extract_schema_and_item(name);
        let mut items = self.result.take().unwrap_or_default();
        let f = self
            .helper
            .list_function(schema, schema.is_none())
            .await
            .map_err(|e| log::error!("list function failed , e:{}", e))
            .ok()
            .unwrap_or_default();
        items.extend(f.into_iter());
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
        let mut items = vec![];
        for x in self.scopes.tables_cloned() {
            if let Some(name) = x.get_name() {
                items.push(name.value.to_string());
            }
        }
        // here match
        let (schema, _table_name) = self.extract_schema_and_item(table_name);
        items.extend(
            self.helper
                .list_table(schema)
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
        // remove dupliate we have entered table and tables in database
        let items = remove_dupliate(items);

        self.result = Some(items);
    }

    pub(crate) async fn c_from_table(&mut self, from_table: &FromTable) -> ScopesDropGuard {
        let tables = match from_table {
            FromTable::WithFromKeyword(vec) => vec,
            FromTable::WithoutKeyword(vec) => vec,
        };
        self.c_on_table_with_joins(&tables[..]).await
    }

    pub(crate) async fn c_on_table_with_joins(
        &mut self, joins: &[TableWithJoins],
    ) -> ScopesDropGuard {
        for (index, join_tables) in joins.iter().enumerate() {
            let scope = self.c_on_table_with_join(join_tables).await;
            if self.finished() {
                return self.scopes.drop_n(index);
            }
            self.scopes.enter_without_guard(scope);
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
        let _guard = self.scopes.enter(scope.clone());
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
                JoinOperator::Join(join_constraint) => {
                    self.c_on_join_constraint(join_constraint).await
                }
                JoinOperator::Left(join_constraint) => {
                    self.c_on_join_constraint(join_constraint).await
                }
                JoinOperator::Right(join_constraint) => {
                    self.c_on_join_constraint(join_constraint).await
                }
                JoinOperator::StraightJoin(join_constraint) => {
                    self.c_on_join_constraint(join_constraint).await
                }
            }
        }
        scope
    }

    pub(crate) async fn c_on_join_constraint(&mut self, c: &JoinConstraint) {
        match c {
            JoinConstraint::On(expr) => {
                self.c_on_expr(expr).await;
            }
            JoinConstraint::Using(vec) => {
                // TODO need handle this.
                let in_pos = vec.iter().any(|x| self.in_pos(&x.span()));
                if in_pos {
                    let tables = self.scopes.tables_cloned();
                    let mut cs = vec![];
                    for table in tables.iter() {
                        match table {
                            TableEntered::FromTable { table, .. } => {
                                let (schema, table) = self.extract_schema_and_item(table);

                                cs.extend(
                                    self.helper
                                        .list_column(schema, table, false)
                                        .await
                                        .map_err(|e| {
                                            log::error!("list_column failed,err:{}", e);
                                        })
                                        .unwrap_or_default()
                                        .into_iter()
                                        .map(CompletionItem::from),
                                );
                            }
                            TableEntered::Temp { table, .. } => {
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
        &mut self, selection: &Select, s: &mut Vec<Scope>,
    ) -> anyhow::Result<Vec<TempTableColumn>> {
        let _gurad = self.c_on_table_with_joins(&selection.from[..]).await;
        // 这个复制一下scope可以在上层的结构中使用
        _gurad.copy(s);
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
        self.c_on_select_item(&selection.projection).await;
        self.helper.get_query_columns(&selection.to_string()).await
    }

    pub(crate) async fn c_on_select_item(&mut self, projection: &[SelectItem]) {
        for p in projection.iter() {
            match p {
                SelectItem::UnnamedExpr(expr) => {
                    self.c_on_expr(expr).await;
                    if self.finished() {
                        return;
                    }
                }
                SelectItem::ExprWithAlias { expr, alias } => {
                    self.c_on_expr(expr).await;
                    if self.finished() {
                        return;
                    }
                    if self.in_pos(&alias.span) {
                        self.set_on_def();
                    }
                }
                SelectItem::QualifiedWildcard(select, wildcard_additional_options) => {
                    self.c_on_select_item_qualified_wildcardkind(select).await;
                    self.c_on_wildcard_additional_options(wildcard_additional_options)
                        .await;
                }
                SelectItem::Wildcard(add) => {
                    self.c_on_wildcard_additional_options(add).await;
                }
            }
        }
    }

    pub(crate) async fn c_on_select_item_qualified_wildcardkind(
        &mut self, ast: &SelectItemQualifiedWildcardKind,
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
        &mut self, ast: &WildcardAdditionalOptions,
    ) {
        if let Some(x) = ast.opt_except.as_ref() {
            for col in x
                .additional_elements
                .iter()
                .chain(std::iter::once(&x.first_element))
            {
                self.c_on_expr_ident(col).await;
                if self.finished() {
                    return;
                }
            }
        }
        if let Some(x) = ast.opt_except.as_ref() {
            for col in x
                .additional_elements
                .iter()
                .chain(std::iter::once(&x.first_element))
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
    pub(crate) async fn c_on_query(
        &mut self, query: &Query,
    ) -> anyhow::Result<Vec<TempTableColumn>> {
        let mut with_temp_tables = HashMap::new();
        if let Some(with) = query.with.as_ref() {
            for c in with.cte_tables.iter() {
                let cols = Box::pin(self.c_on_query(&c.query)).await?;
                with_temp_tables.insert(
                    c.alias.name.clone(),
                    TempTable {
                        columns: Arc::new(cols),
                    },
                );
            }
        }
        let s = Scope::from_temp_tables(with_temp_tables);
        let _with_temp_table_guard = self.scopes.enter(s);
        let mut s_on_set_expr: Vec<Scope> = vec![];
        let _ = Box::pin(self.c_on_set_expr(&query.body, &mut s_on_set_expr)).await?;
        let _gurad = self.scopes.enter_scopes(s_on_set_expr);

        if let Some(orderby) = query.order_by.as_ref() {
            match &orderby.kind {
                OrderByKind::All(..) => {}
                OrderByKind::Expressions(order_by_exprs) => {
                    for expr in order_by_exprs.iter() {
                        self.c_on_order_by_expr(expr).await;
                    }
                }
            }
            if let Some(interpolate) = &orderby.interpolate {
                if let Some(exprs) = &interpolate.exprs {
                    for expr in exprs.iter() {
                        if let Some(expr) = &expr.expr {
                            self.c_on_expr(expr).await;
                        }
                    }
                }
            }
        }
        if let Some(limit) = query.limit_clause.as_ref() {
            match limit {
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
        return self.helper.get_query_columns(&query.to_string()).await;
    }

    pub(crate) async fn c_on_set_expr(
        &mut self, set: &SetExpr, s: &mut Vec<Scope>,
    ) -> anyhow::Result<Vec<TempTableColumn>> {
        match set {
            SetExpr::Select(select) => {
                return self.c_on_select(select, s).await;
            }
            SetExpr::Query(query) => {
                return Box::pin(self.c_on_query(query)).await;
            }
            SetExpr::SetOperation { left, right, .. } => {
                let _ = Box::pin(self.c_on_set_expr(left, s)).await?;
                return Box::pin(self.c_on_set_expr(right, s)).await;
            }
            SetExpr::Values(values) => {
                for v in values.rows.iter() {
                    for v in v.iter() {
                        self.c_on_expr(v).await;
                        if self.finished() {
                            break;
                        }
                    }
                }
                return self.helper.get_query_columns(&values.to_string()).await;
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
                        table.schema_name.as_deref(),
                        table.table_name.as_deref().unwrap_or_default(),
                        false,
                    )
                    .await
                    .map_err(|e| {
                        log::error!("list_column failed,err:{:?}", e);
                    })
                    .ok()
                    .unwrap_or_default()
                    .into_iter()
                    .collect();
                return anyhow::Ok(cols);
            }
            SetExpr::Delete(statement) => {
                self.completion_on_statement(statement).await;
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
        match f {
            TableFactor::Table {
                name,
                alias,
                args,
                with_hints,
                partitions,
                ..
            } => {
                self.c_on_schema(name).await;
                if self.finished() {
                    return;
                }
                if self.last_name_in_pos(name) {
                    self.c_on_select_target(name).await;
                    if self.finished() {
                        return;
                    }
                }
                s.enter_table(None, name.clone());
                let columns = match args {
                    Some(_) => {
                        // this maybe a function get is a try
                        let (schema, name) = self.extract_schema_and_item(name);

                        // TODO how to handle this case
                        let columns = self
                            .helper
                            .list_column(schema, name, true)
                            .await
                            .map_err(|e| log::error!("list column failed,e:{}", e))
                            .ok()
                            .unwrap_or_default()
                            .into_iter()
                            .collect::<Vec<_>>();

                        if !columns.is_empty() {
                            Some(columns)
                        } else {
                            None
                        }
                    }
                    None => None,
                };

                match columns {
                    Some(columns) => {
                        if let Some(alias) = alias {
                            s.enter_temp_table(
                                Some(alias.name.clone()),
                                TempTable {
                                    columns: Arc::new(columns),
                                },
                            );
                        } else {
                            s.enter_temp_table(
                                None,
                                TempTable {
                                    columns: Arc::new(columns),
                                },
                            );
                        }
                    }
                    None => {
                        if let Some(alias) = alias {
                            // this is a alias but we often end in situation like this
                            // `select * from test ...`
                            // here we need completion on where order etc lexer elements
                            // if self.in_pos(&alias.name.span) {
                            //     self.set_on_def();
                            //     return;
                            // }
                            // TODO alias have column definition.
                            s.enter_table(Some(alias.name.clone()), name.clone());
                        } else {
                            s.enter_table(None, name.clone());
                        }
                    }
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
                            FunctionArg::Named { name, arg, .. } => {
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
                subquery, alias, ..
            } => {
                let cols = self
                    .c_on_query(subquery)
                    .await
                    .map_err(|e| log::error!("self.c_on_query failed,err:{:?}", e))
                    .unwrap_or_default();

                if let Some(alias) = alias {
                    s.enter_temp_table(
                        Some(alias.name.clone()),
                        TempTable {
                            columns: Arc::new(cols.clone()),
                        },
                    );
                    s.enter_temp_table(
                        None,
                        TempTable {
                            columns: Arc::new(cols),
                        },
                    );
                } else {
                    s.enter_temp_table(
                        None,
                        TempTable {
                            columns: Arc::new(cols),
                        },
                    );
                }
            }
            TableFactor::TableFunction { expr, alias } => {
                self.c_on_expr(expr).await;
                if let Some(alias) = alias.as_ref() {
                    s.enter_temp_table(
                        Some(alias.name.clone()),
                        TempTable {
                            // TODO get columns.
                            columns: Arc::new(vec![]),
                        },
                    );
                }
            }
            TableFactor::Function { alias, name, .. } => {
                let (schema, name) = self.extract_schema_and_item(name);
                // TODO how to handle this case
                let columns = self
                    .helper
                    .list_column(schema, name, true)
                    .await
                    .map_err(|e| log::error!("list column failed,e:{}", e))
                    .ok()
                    .unwrap_or_default();
                let columns: Vec<_> = columns.into_iter().collect();

                if let Some(alias) = alias {
                    s.enter_temp_table(
                        Some(alias.name.clone()),
                        TempTable {
                            columns: Arc::new(columns),
                        },
                    );
                } else {
                    s.enter_temp_table(
                        None,
                        TempTable {
                            columns: Arc::new(columns),
                        },
                    );
                }
            }
            TableFactor::UNNEST { .. } => {}
            TableFactor::JsonTable { .. } => {}
            TableFactor::OpenJsonTable { .. } => {}
            TableFactor::NestedJoin { .. } => {}
            TableFactor::Pivot { .. } => {}
            TableFactor::Unpivot { .. } => {}
            TableFactor::MatchRecognize { .. } => {}
            TableFactor::XmlTable { .. } => {}
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
        let (schema, _) = self.extract_schema_and_item(name);
        self.result = Some(
            self.helper
                .list_procedure(schema)
                .await
                .map_err(|e| {
                    log::error!("list_procedure error:{:?}", e);
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
        name.0
            .last()
            .map(|x| self.in_pos(&x.span()))
            .unwrap_or(false)
    }

    // sqlite pragma

    pub(crate) async fn c_on_pragma(&mut self, name: &ObjectName) {
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
            self.c_on_window_specification(over).await;
        }
        for w in function.within_group.iter() {
            self.c_on_order_by_expr(w).await;
            if self.finished() {
                return;
            }
        }
    }

    pub(crate) async fn c_on_window_specification(&mut self, ast: &WindowType) {
        match ast {
            WindowType::WindowSpec(window_spec) => {
                if let Some(window_name) = window_spec.window_name.as_ref() {
                    if self.in_pos(&window_name.span) {
                        self.set_on_def();
                        return;
                    }
                }
                for o in window_spec.order_by.iter() {
                    self.c_on_order_by_expr(o).await;
                    if self.finished() {
                        return;
                    }
                }
            }
            WindowType::NamedWindow(..) => {
                // fallback to lexer
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
                        FunctionArg::Named { name, arg, .. } => {
                            if self.in_pos(&name.span) {
                                return;
                            }
                            if let FunctionArgExpr::Expr(expr) = arg {
                                self.c_on_expr(expr).await;
                            };
                        }
                        FunctionArg::ExprNamed { name, arg, .. } => {
                            if self.in_pos(&name.span()) {
                                // make suggest nothing.
                                return;
                            }
                            if let FunctionArgExpr::Expr(expr) = arg {
                                self.c_on_expr(expr).await;
                            };
                        }
                        FunctionArg::Unnamed(function_arg_expr) => {
                            if let FunctionArgExpr::Expr(expr) = function_arg_expr {
                                self.c_on_expr(expr).await;
                            }
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
        let (schema, _) = self.extract_schema_and_item(name);
        self.result = Some(
            self.helper
                .list_function(schema, false)
                .await
                .map_err(|e| {
                    log::error!("list function error:{:?}", e);
                })
                .ok()
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
            .list_function(schema, schema.is_none())
            .await
            .map_err(|e| {
                log::error!("list function error:{:?}", e);
            })
            .ok()
            .unwrap_or_default();
        ret.extend(
            self.helper
                .list_procedure(schema)
                .await
                .map_err(|e| log::error!("list precedure error:{:?}", e))
                .unwrap_or_default()
                .into_iter()
                .map(|x| CompletionItem {
                    item: x.into(),
                    kind: CompletionKind::Procedure,
                    desc: None,
                }),
        );
        self.result = Some(ret)
    }
}
