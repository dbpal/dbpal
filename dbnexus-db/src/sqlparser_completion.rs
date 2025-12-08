use crate::db::BackendKind;
use crate::helper::try_remove_one_at;
use crate::language_server::{AstCompletionHelper, CompletionItem, CompletionKind, ErrAndLoc};
use sqlparser::ast::*;
use sqlparser::tokenizer::{Location, Span};
use std::collections::HashSet;
use std::ops::DerefMut;
use std::sync::{Arc, Mutex};

pub struct CompletionStatement<'a> {
    pub(crate) loc: Location,
    pub(crate) result: Option<Vec<CompletionItem>>,
    pub(crate) helper: &'a dyn AstCompletionHelper,
    pub(crate) scopes: &'a Scopes,
    pub(crate) on_def: bool,
    pub(crate) select_item_columns: HashSet<TempTableColumn>,
}

#[derive(Clone)]
pub struct Scopes {
    pub(crate) scopes: Arc<Mutex<Vec<Scope>>>,
}

pub(crate) struct ScopesDropGuard {
    scopes: Arc<Mutex<Vec<Scope>>>,
}

impl Drop for ScopesDropGuard {
    fn drop(&mut self) {
        let mut m = self.scopes.lock().expect("lock failed");
        let m = m.deref_mut();
        m.pop();
    }
}

impl Scopes {
    #[allow(unused)]
    pub(crate) fn debug(&self) {
        for (index, s) in self
            .scopes
            .lock()
            .expect("lock failed")
            .iter()
            .rev()
            .enumerate()
        {
            println!("scope {:?}-----------------", index);
            for t in s.vars.iter() {
                println!("var:{:?}", t);
            }
            for t in s.tables.iter() {
                println!("tbl:{:?}", t)
            }
        }
    }
    pub(crate) fn new() -> Self {
        Self {
            scopes: Arc::new(Mutex::new(vec![Scope::default()])),
        }
    }

    pub(crate) fn current_scope<R>(&self, f: impl FnOnce(&mut Scope) -> R) -> R {
        f(self
            .scopes
            .lock()
            .expect("lock failed")
            .last_mut()
            .expect("we have at least one scope"))
    }

    pub(crate) fn tables_cloned(&self) -> Vec<TableEntered> {
        let mut ret = vec![];
        let lock = self.scopes.lock().expect("lock failed");
        for s in lock.iter().rev() {
            ret.extend(s.tables.clone().into_iter());
        }
        ret
    }

    pub(crate) fn vars_cloned(&self) -> Vec<Var> {
        let mut ret = vec![];
        let lock = self.scopes.lock().expect("lock failed");
        for s in lock.iter().rev() {
            ret.extend(s.vars.iter().cloned());
        }
        ret
    }

    pub(crate) fn enter(&self, scope: Scope) -> ScopesDropGuard {
        self.scopes.lock().expect("lock failed").push(scope);
        ScopesDropGuard {
            scopes: self.scopes.clone(),
        }
    }
    pub(crate) fn enter_var(&self, var: Var) {
        self.current_scope(|s| {
            s.enter_var(var);
        });
    }

    pub(crate) fn inner_first<R>(&self, mut f: impl FnMut(&Scope) -> Option<R>) -> Option<R> {
        for s in self.scopes.lock().expect("lock failed").iter().rev() {
            let x = f(s);
            if x.is_some() {
                return x;
            }
        }
        None
    }

    pub(crate) fn nearest_table_named(&self, name: &Ident) -> Option<TableEntered> {
        self.inner_first(|scope| -> Option<TableEntered> {
            for t in scope.tables.iter().rev() {
                if t.equal_name(name) {
                    return Some(t.clone());
                }
            }
            None
        })
    }
}

#[derive(Clone, Debug)]
pub struct TempTableColumn {
    pub(crate) name: String,
    pub(crate) data_type: Option<String>,
}
impl std::hash::Hash for TempTableColumn {
    fn hash<H: std::hash::Hasher>(&self, state: &mut H) {
        self.name.hash(state);
    }
}

impl PartialEq for TempTableColumn {
    fn eq(&self, other: &Self) -> bool {
        self.name == other.name
    }
}
impl Eq for TempTableColumn {}

impl From<TempTableColumn> for CompletionItem {
    fn from(value: TempTableColumn) -> Self {
        let TempTableColumn { name, data_type } = value;
        Self {
            item: name.into(),
            kind: CompletionKind::Column,
            desc: data_type.map(|x| x.into()),
        }
    }
}
impl From<&TempTableColumn> for CompletionItem {
    fn from(value: &TempTableColumn) -> Self {
        Self {
            item: value.name.clone().into(),
            kind: CompletionKind::Column,
            desc: value.data_type.as_ref().map(|x| x.clone().into()),
        }
    }
}

impl From<String> for TempTableColumn {
    fn from(value: String) -> Self {
        Self {
            name: value,
            data_type: None,
        }
    }
}

#[derive(Clone, Debug, Default)]
pub struct TempTable {
    pub(crate) columns: Arc<Vec<TempTableColumn>>,
}

#[derive(Clone, Default, Debug)]
pub struct Scope {
    tables: Vec<TableEntered>,
    vars: Vec<Var>,
}

#[derive(Clone, Debug)]
pub(crate) enum TableEntered {
    ObjectName {
        table: ObjectName,
        alias: Option<Ident>,
    },
    Temp {
        table: TempTable,
        alias: Option<Ident>,
    },
}

impl TableEntered {
    pub(crate) fn get_name(&self) -> Option<&Ident> {
        match self {
            TableEntered::ObjectName { alias, table } => {
                if let Some(alias) = alias {
                    Some(alias)
                } else {
                    match table.0.last().expect("table have at least one part") {
                        ObjectNamePart::Identifier(ident) => Some(ident),
                        ObjectNamePart::Function(object_name_part_function) => {
                            // TODO
                            Some(&object_name_part_function.name)
                        }
                    }
                }
            }
            TableEntered::Temp { alias, .. } => alias.as_ref(),
        }
    }

    fn equal_name(&self, name: &Ident) -> bool {
        self.get_name()
            .map(
                |x: &Ident| match (x.quote_style.is_some(), name.quote_style.is_some()) {
                    (false, false) => name.value.eq_ignore_ascii_case(&x.value),
                    _ => x.value == name.value,
                },
            )
            .unwrap_or(false)
    }
}

#[derive(Clone, Debug)]
pub struct Var {
    pub(crate) data_type: Option<DataType>,
    pub(crate) name: Ident,
}

impl From<String> for Var {
    fn from(value: String) -> Self {
        Self {
            data_type: None,
            name: Ident::new(value),
        }
    }
}

impl Scope {
    pub(crate) fn enter_table(&mut self, alias: Option<Ident>, table_name: ObjectName) {
        log::info!("enter table {:?} {:?}", alias, table_name);
        self.tables.push(TableEntered::ObjectName {
            table: table_name,
            alias,
        });
    }

    pub(crate) fn enter_temp_table(&mut self, alias: Option<Ident>, temp: TempTable) {
        if temp.columns.is_empty() {
            log::error!("enter temp table {:?} {:?}", alias, temp);
        } else {
            log::info!("enter temp table {:?} {:?}", alias, temp);
        }

        self.tables.push(TableEntered::Temp { table: temp, alias });
    }
    pub(crate) fn enter_temp_cols(&mut self, cols: Vec<TempTableColumn>) {
        self.enter_temp_table(
            None,
            TempTable {
                columns: Arc::new(cols),
            },
        );
    }

    pub(crate) fn enter_var(&mut self, var: Var) {
        log::info!("enter var:{:?}", var);
        self.vars.push(var);
    }
}

impl<'a> CompletionStatement<'a> {
    pub(crate) fn new(
        loc: Location, helper: &'a dyn AstCompletionHelper, scopes: &'a Scopes,
    ) -> Self {
        Self {
            loc,
            helper,
            result: None,
            scopes,
            on_def: false,
            select_item_columns: Default::default(),
        }
    }

    pub(crate) fn set_on_def(&mut self) {
        self.on_def = true
    }

    pub(crate) fn finished(&self) -> bool {
        self.result.is_some() || self.on_def
    }

    pub(crate) fn in_pos(&self, span: &Span) -> bool {
        (self.loc.line >= span.start.line && self.loc.line <= span.end.line)
            && (self.loc.column >= span.start.column && self.loc.column <= span.end.column)
    }
}
impl<'a> CompletionStatement<'a> {
    pub(crate) async fn completion_on_statement(&mut self, statement: &Statement) {
        let _guard = match statement {
            // only `Declare` enter var will enter var in outer scope
            Statement::Declare { .. } | Statement::Set(..) => None,
            _ => Some(self.scopes.enter(Scope::default())),
        };
        match statement {
            Statement::Analyze(Analyze {
                table_name,
                columns,
                partitions,
                ..
            }) => {
                self.c_on_table(table_name).await;
                if self.finished() {
                    return;
                }
                for c in columns.iter() {
                    self.c_on_column(table_name, c, false).await;
                    if self.finished() {
                        return;
                    }
                }
                if let Some(partitions) = partitions {
                    for p in partitions.iter() {
                        self.c_on_partition_expr(table_name, p).await;
                        if self.finished() {
                            return;
                        }
                    }
                }
            }
            Statement::Truncate(Truncate {
                table_names,
                on_cluster,
                partitions,
                ..
            }) => {
                for t in table_names.iter() {
                    self.c_on_table(&t.name).await;
                    if self.finished() {
                        return;
                    }
                }
                if let Some(on_cluster) = on_cluster.as_ref() {
                    self.c_on_cluster(on_cluster).await;
                }
                if let Some(partitions) = partitions.as_ref() {
                    for t in table_names.iter() {
                        self.scopes.current_scope(|scope| {
                            scope.enter_table(None, t.name.clone());
                        });
                    }
                    for p in partitions.iter() {
                        self.c_on_expr(p).await;
                        if self.finished() {
                            return;
                        }
                    }
                }
            }
            Statement::Msck { .. } => {
                // hive not support right now
            }
            Statement::Query(query) => {
                let _ = self.c_on_query(query).await;
            }
            Statement::Insert(insert) => {
                match &insert.table {
                    TableObject::TableName(table_name) => {
                        self.c_on_table(table_name).await;
                        if self.finished() {
                            return;
                        }
                        for c in insert.columns.iter().chain(insert.after_columns.iter()) {
                            self.c_on_column(table_name, c, false).await;
                            if self.finished() {
                                return;
                            }
                        }
                        self.scopes
                            .current_scope(|s| s.enter_table(None, table_name.clone()));
                    }
                    // table function in include in function.
                    TableObject::TableFunction(f) => {
                        self.c_on_function_call(f).await;
                    }
                }
                //
                if let Some(query) = &insert.source {
                    let _ = self.c_on_query(query).await;
                    if self.finished() {
                        return;
                    }
                }

                if let Some(alias) = &insert.table_alias {
                    if self.in_pos(&alias.span) {
                        self.set_on_def();
                        return;
                    }
                }
                if let Some(returning) = &insert.returning {
                    // 在 SQL 中，特别是在 PostgreSQL 中，RETURNING 关键字用于在 INSERT、UPDATE
                    // 或 DELETE 语句后返回被操作的行的特定列。
                    // 这使得你可以在执行数据修改后立即获取相关信息，例如新插入的记录的 ID。
                    self.c_on_select_items(&returning[..]).await;
                    if self.finished() {
                        return;
                    }
                }
                for ass in insert.assignments.iter() {
                    self.c_on_assignment(ass).await;
                    if self.finished() {
                        return;
                    }
                }
                if let Some(ss) = &insert.settings.as_ref() {
                    for s in ss.iter() {
                        self.c_on_setting(s).await;
                        if self.finished() {
                            return;
                        }
                    }
                }
                if let Some(c) = insert.format_clause.as_ref() {
                    self.c_on_input_format_clause(c).await;
                }
            }
            Statement::Install { extension_name } => {
                self.c_on_duck_extension(extension_name, Some(false)).await;
            }
            Statement::Load { extension_name } => {
                self.c_on_duck_extension(extension_name, Some(true)).await;
            }
            Statement::Directory { source, .. } => {
                let _ = self.c_on_query(source).await;
            }
            Statement::Call(function) => {
                Box::pin(self.c_on_function_call(function)).await;
            }
            Statement::Copy {
                source, options, ..
            } => {
                match source {
                    CopySource::Table {
                        table_name,
                        columns,
                    } => {
                        self.c_on_table_or_view(table_name).await;
                        if self.finished() {
                            return;
                        }
                        for c in columns.iter() {
                            self.c_on_column(table_name, c, false).await;
                            if self.finished() {
                                return;
                            }
                        }
                        for op in options.iter() {
                            match op {
                                CopyOption::ForceQuote(idents)
                                | CopyOption::ForceNotNull(idents)
                                | CopyOption::ForceNull(idents) => {
                                    for col in idents.iter() {
                                        self.c_on_column(table_name, col, false).await;
                                        if self.finished() {
                                            return;
                                        }
                                    }
                                }
                                _ => {}
                            }
                        }
                    }
                    CopySource::Query(query) => {
                        let columns = self.c_on_query(query).await.ok().unwrap_or_default();
                        for op in options.iter() {
                            match op {
                                CopyOption::ForceQuote(idents)
                                | CopyOption::ForceNotNull(idents)
                                | CopyOption::ForceNull(idents) => {
                                    for col in idents.iter() {
                                        if self.in_pos(&col.span) {
                                            self.result = Some(
                                                columns
                                                    .iter()
                                                    .map(CompletionItem::from)
                                                    .collect::<Vec<_>>(),
                                            );
                                        }
                                    }
                                }
                                _ => {}
                            }
                        }
                    }
                };
            }
            Statement::CopyIntoSnowflake { .. } => {
                // snowflake not support at all
            }
            Statement::Close { .. } => {
                // fallback to lexer
            }
            Statement::Update(Update {
                table,
                assignments,
                from,
                selection,
                returning,
                or: _,
                limit,
                ..
            }) => {
                let mut joins = vec![table.clone()];
                if let Some(kind) = from.as_ref() {
                    joins.extend(
                        match kind {
                            UpdateTableFromKind::BeforeSet(items) => items.clone(),
                            UpdateTableFromKind::AfterSet(items) => items.clone(),
                        }
                        .into_iter(),
                    );
                }
                self.c_on_table_with_joins(&joins[..]).await;
                if let Some(selection) = selection {
                    Box::pin(self.c_on_expr(selection)).await;
                    if self.finished() {
                        return;
                    }
                }
                for ass in assignments.iter() {
                    self.c_on_assignment(ass).await;
                    if self.finished() {
                        return;
                    }
                }
                if let Some(p) = returning {
                    self.c_on_select_items(p).await;
                }
                if let Some(limit) = limit {
                    self.c_on_expr(limit).await;
                }
            }
            Statement::Delete(delete) => {
                self.c_on_from_table(&delete.from).await;
                if self.finished() {
                    return;
                }
                if let Some(other) = &delete.using {
                    self.c_on_table_with_joins(other).await;
                    if self.finished() {
                        return;
                    }
                }
                for table in delete.tables.iter() {
                    self.c_on_table(table).await;
                    if self.finished() {
                        return;
                    }
                    self.scopes
                        .current_scope(|s| s.enter_table(None, table.clone()));
                }
                if let Some(s) = &delete.selection {
                    Box::pin(self.c_on_expr(s)).await;
                    if self.finished() {
                        return;
                    }
                }
                for orderby in delete.order_by.iter() {
                    Box::pin(self.c_on_order_by_expr(orderby)).await;
                    if self.finished() {
                        return;
                    }
                }
                if let Some(r) = &delete.returning {
                    self.c_on_select_items(&r[..]).await;
                }
            }
            Statement::CreateView(CreateView {
                query,
                name,
                columns,
                cluster_by,
                options,
                to,
                ..
            }) => {
                // name is user defined no need completion.
                let cols = self.c_on_query(query).await.unwrap_or_default();
                self.scopes.current_scope(|s| s.enter_temp_cols(cols));
                if self.in_pos(&name.span()) {
                    self.set_on_def();
                    return;
                }
                let cols: Vec<_> = columns
                    .iter()
                    .map(|x| TempTableColumn {
                        name: x.name.value.clone(),
                        data_type: x.data_type.as_ref().map(|x| x.to_string()),
                    })
                    .collect();

                self.scopes.current_scope(|s| {
                    s.enter_temp_table(
                        None,
                        TempTable {
                            columns: Arc::new(cols),
                        },
                    )
                });
                for col in columns.iter() {
                    self.c_on_view_column_def(col).await;
                    if self.finished() {
                        return;
                    }
                }

                for c in cluster_by.iter() {
                    self.c_on_cluster(c).await;
                    if self.finished() {
                        return;
                    }
                }
                self.c_on_create_table_options(options).await;
                if self.finished() {
                    return;
                }
                if let Some(to) = to {
                    self.c_on_table(to).await;
                }
            }
            Statement::CreateTable(create_table) => {
                self.c_on_create_table(create_table).await;
            }
            Statement::CreateVirtualTable {
                name, module_name, ..
            } => {
                self.c_on_schema(name).await;
                if self.finished() {
                    return;
                }
                if self.in_pos(&name.span()) {
                    self.set_on_def();
                }
                self.c_on_sqlite_module_name(module_name).await;
            }
            Statement::CreateIndex(create_index) => {
                if let Some(name) = &create_index.name {
                    if self.in_pos(&name.span()) {
                        self.set_on_def();
                        return;
                    }
                }
                self.c_on_table(&create_index.table_name).await;
                if self.finished() {
                    return;
                }
                self.scopes
                    .current_scope(|s| s.enter_table(None, create_index.table_name.clone()));

                // using 在 PostgreSQL 中，USING 关键字用于指定索引的类型，例如
                // B-tree、HASH、GIN 等。示例如下： using 用lexer的自动完成即可

                for c in create_index.columns.iter() {
                    self.c_on_order_by_expr(&c.column).await;
                    if self.finished() {
                        return;
                    }
                }
                // 在 PostgreSQL 中，CREATE INDEX 语句的 INCLUDE
                // 关键字用于在创建索引时指定额外的列，这些列不会用于索引的排序或查找，
                // 但会被包含在索引的叶节点中。这种功能主要用于提高查询性能，
                // 特别是在需要返回额外列的场景中。
                for i in create_index.include.iter() {
                    self.c_on_column(&create_index.table_name, i, false).await;
                    if self.finished() {
                        return;
                    }
                }
                if let Some(expr) = &create_index.predicate {
                    self.c_on_expr(expr).await;
                    if self.finished() {
                        return;
                    }
                }
                for w in create_index.with.iter() {
                    self.c_on_expr(w).await;
                    if self.finished() {
                        return;
                    }
                }
            }
            Statement::CreateRole(CreateRole {
                names,
                in_role,
                in_group,
                role,
                user,
                admin,
                authorization_owner,
                ..
            }) => {
                for name in names.iter() {
                    if self.in_pos(&name.span()) {
                        self.set_on_def();
                        return;
                    }
                }
                for role in in_role
                    .iter()
                    .chain(in_group.iter())
                    .chain(role.iter())
                    .chain(user.iter())
                    .chain(admin.iter())
                {
                    self.c_on_role(role).await;
                    if self.finished() {
                        return;
                    }
                }
                if let Some(auth) = authorization_owner {
                    match auth.0.len() {
                        1 => match auth.0.last().expect("we have checked the length") {
                            ObjectNamePart::Identifier(ident) => self.c_on_role(ident).await,
                            ObjectNamePart::Function(..) => {
                                // TODO
                            }
                        },
                        _ => {
                            log::error!("name length not right:{:?}", auth)
                        }
                    }
                }
            }
            Statement::CreateSecret { .. } => {
                // duckdb syntax
            }
            Statement::CreatePolicy {
                table_name,
                name,
                using,
                with_check,
                to,
                ..
            } => {
                if self.in_pos(&name.span) {
                    self.set_on_def();
                    return;
                }
                // CREATE POLICY 是 PostgreSQL 中用于定义行级安全策略的命令，
                // 主要适用于表（tables）和视图（views）。通过行级安全，您可以控制用户在表或视图中可访问的行。
                self.c_on_table_or_view(table_name).await;
                if self.finished() {
                    return;
                }
                if let Some(to) = to.as_ref() {
                    if to.iter().any(|x| match x {
                        Owner::Ident(ident) => self.in_pos(&ident.span),
                        _ => false,
                    }) {
                        // this postgres specific statement
                        // hit the position
                        // 是的，CREATE POLICY 语句中的 TO 子句用于指定角色（role）。这允许您定义特定角色对表或视图的访问权限。
                        let mut results: Vec<_> = self.helper.list_role_completion().await;
                        // https://www.postgresql.org/docs/current/sql-createpolicy.html
                        results.extend(
                            ["PUBLIC", "CURRENT_ROLE", "CURRENT_USER", "SESSION_USER"]
                                .iter()
                                .map(|x| CompletionItem {
                                    item: (*x).into(),
                                    kind: CompletionKind::Role,
                                    desc: None,
                                }),
                        );
                        self.result = Some(results);
                        return;
                    }
                }
                self.scopes
                    .current_scope(|s| s.enter_table(None, table_name.clone()));

                if let Some(x) = using {
                    self.c_on_expr(x).await;
                    if self.finished() {
                        return;
                    }
                }
                if let Some(x) = with_check {
                    self.c_on_expr(x).await;
                    if self.finished() {}
                }
            }
            Statement::AlterTable(AlterTable {
                name,
                operations,
                on_cluster,
                ..
            }) => {
                self.c_on_table(name).await;
                if self.finished() {
                    return;
                }
                for op in operations.iter() {
                    self.c_on_alert_table_operation(name, op).await;
                    if self.finished() {
                        return;
                    }
                }
                if let Some(on_cluster) = on_cluster.as_ref() {
                    self.c_on_cluster(on_cluster).await;
                }
            }
            Statement::AlterIndex { name, operation } => {
                self.c_on_index(name).await;
                if self.finished() {
                    return;
                }
                match operation {
                    AlterIndexOperation::RenameIndex { index_name } => {
                        self.c_on_schema(index_name).await;
                        if self.in_pos(&index_name.span()) {
                            self.set_on_def();
                        }
                    }
                }
            }
            Statement::AlterView {
                name,
                query,
                columns: _,
                with_options,
            } => {
                self.c_on_view(name).await;
                if self.finished() {
                    return;
                }
                let cols = self.c_on_query(query).await.unwrap_or_default();
                self.scopes.current_scope(|s| s.enter_temp_cols(cols));
                for w in with_options.iter() {
                    self.c_on_sql_option(w).await;
                    if self.finished() {
                        return;
                    }
                }
            }
            Statement::AlterRole { name, operation } => {
                self.c_on_role(name).await;
                if self.finished() {
                    return;
                }
                match operation {
                    AlterRoleOperation::RenameRole { role_name } => {
                        if self.in_pos(&role_name.span) {
                            self.set_on_def();
                        }
                    }
                    AlterRoleOperation::AddMember { member_name } => {
                        self.c_on_role(member_name).await;
                    }
                    AlterRoleOperation::DropMember { member_name } => {
                        self.c_on_role(member_name).await;
                    }
                    AlterRoleOperation::WithOptions { options } => {
                        for op in options.iter() {
                            self.c_on_role_option(op).await;
                            if self.finished() {
                                return;
                            }
                        }
                    }
                    AlterRoleOperation::Set { config_value, .. } => match config_value {
                        SetConfigValue::Default => {}
                        SetConfigValue::FromCurrent => {}
                        SetConfigValue::Value(expr) => self.c_on_expr(expr).await,
                    },
                    AlterRoleOperation::Reset { .. } => {
                        // no completion on this.
                    }
                }
            }
            Statement::AlterPolicy {
                table_name, name, ..
            } => {
                self.c_on_table_or_view(table_name).await;
                if self.finished() {
                    return;
                }
                self.c_on_table_policy(table_name, name).await;
            }
            Statement::AttachDatabase {
                schema_name,
                database_file_name,
                ..
            } => {
                if self.in_pos(&schema_name.span) {
                    self.set_on_def();
                    return;
                }
                self.c_on_expr(database_file_name).await;
            }
            Statement::AttachDuckDBDatabase { .. } => {
                // no support on this
            }
            Statement::DetachDuckDBDatabase { .. } => {
                // no support on this
            }
            Statement::Drop {
                object_type,
                names,
                table,
                ..
            } => {
                if let Some(table) = table {
                    self.c_on_table(table).await;
                }

                for name in names.iter() {
                    self.c_on_object_type_drop(*object_type, name).await;
                    if self.finished() {
                        return;
                    }
                }
            }
            Statement::DropFunction(DropFunction { func_desc, .. }) => {
                for d in func_desc.iter() {
                    self.c_on_function_drop(&d.name).await;
                    if self.finished() {
                        return;
                    }
                }
            }
            Statement::DropProcedure { proc_desc, .. } => {
                for d in proc_desc.iter() {
                    self.c_on_procedure(&d.name).await;
                    if self.finished() {
                        return;
                    }
                }
            }
            Statement::DropSecret { .. } => {
                // DROP SECRET 并不是 SQL 标准语句，而是部分现代数据库为管理“外部访问密钥”引入的扩展命令，
                // 用于删除事先通过 CREATE SECRET 注册的凭证对象。目前可见于以下系统
            }
            Statement::DropPolicy {
                table_name, name, ..
            } => {
                self.c_on_table(table_name).await;
                if self.finished() {
                    return;
                }
                self.c_on_table_policy(table_name, name).await;
            }
            Statement::Declare { stmts } => {
                for declare in stmts.iter() {
                    for name in declare.names.iter() {
                        if self.in_pos(&name.span) {
                            self.set_on_def();
                            return;
                        }
                        self.scopes.enter_var(Var {
                            name: name.clone(),
                            data_type: declare.data_type.clone(),
                        });
                    }
                    if let Some(d) = declare.data_type.as_ref() {
                        self.c_on_data_type(d).await;
                        if self.finished() {
                            return;
                        }
                    }
                    if let Some(expr) = &declare.assignment {
                        self.c_on_declare_assignment(expr).await;
                    }
                    if let Some(for_query) = &declare.for_query {
                        let _ = self.c_on_query(for_query).await;
                    }
                }
            }
            Statement::Fetch { into, .. } => {
                if let Some(into) = into {
                    self.c_on_table(into).await;
                }
                // other fallback to lexer
            }
            Statement::Flush { tables, .. } => {
                for table in tables.iter() {
                    self.c_on_table(table).await;
                    if self.finished() {
                        return;
                    }
                }
                // other fallback to lexer
            }
            Statement::Discard { .. } => {
                // fallback to lexer
                // no completable elements
            }
            Statement::ShowFunctions { filter } => {
                // Presto-specific
                // no one can write a sql like this???
                if let Some(filter) = filter {
                    self.c_on_show_statement_filter(filter).await;
                }
            }
            Statement::ShowVariable { variable } => {
                let kind = self.helper.backend_name();
                if kind.postgres_like() {
                    for var in variable {
                        self.c_on_variable(var).await;
                        if self.finished() {
                            return;
                        }
                    }
                }
            }
            Statement::ShowStatus { filter, .. } => {
                // nothing to show here
                // SHOW STATUS LIKE 'Connections';
                // fallback to lexer
                if let Some(filter) = filter {
                    self.c_on_show_statement_filter(filter).await;
                }
            }
            Statement::ShowVariables { filter, .. } => {
                // 给我的语法结构不好做自动完成
                // SHOW VARIABLES LIKE 'version';
                // fallback to lexer
                if let Some(filter) = filter {
                    self.c_on_show_statement_filter(filter).await;
                }
            }
            Statement::ShowCreate { obj_type, obj_name } => {
                self.c_on_mysql_show_create(*obj_type, obj_name).await;
            }
            Statement::ShowColumns { .. } => {
                // fallback to lexer
                // lexer completion will suggest tables.
            }
            Statement::ShowDatabases { .. } => {
                // fallback to lexer
            }
            Statement::ShowSchemas { .. } => {
                //  fallback to lexer
            }
            Statement::ShowTables { .. } => {
                // SHOW TABLES LIKE 'emp%';
                // fallback to lexer
            }
            Statement::ShowViews { .. } => {
                // SHOW VIEWS LIKE 'emp%';
            }
            Statement::ShowCollation { .. } => {
                // fallback to lexer
            }
            Statement::Use(u) => {
                match u {
                    Use::Catalog(object_name)
                    | Use::Schema(object_name)
                    | Use::Database(object_name) => {
                        if self.in_pos(&object_name.span()) {
                            let results = self.helper.list_database_schema_completion().await;
                            self.result = Some(results)
                        }
                    }
                    Use::Warehouse(..) => {
                        // TODO
                    }
                    Use::Role(object_name) => self.c_on_role_objectname(object_name).await,
                    Use::SecondaryRoles(..) => {
                        // USE SECONDARY ROLES ALL 是 Snowflake 数据库的 SQL 语句
                    }
                    Use::Object(..) => {
                        // TODO use a table???
                    }
                    Use::Default => {}
                }
            }
            Statement::StartTransaction {
                statements,
                exception,
                ..
            } => {
                for s in statements.iter() {
                    Box::pin(self.completion_on_statement(s)).await;
                    if self.finished() {
                        return;
                    }
                }

                if let Some(exception) = exception {
                    for when in exception.iter() {
                        if when.idents.iter().any(|s| self.in_pos(&s.span)) {
                            // those a expection names suppose to be defintion
                            self.set_on_def();
                            return;
                        }

                        for stmt in when.statements.iter() {
                            Box::pin(self.completion_on_statement(stmt)).await;
                            if self.finished() {
                                return;
                            }
                        }
                    }
                }
            }
            Statement::Comment {
                object_type,
                object_name,
                ..
            } => {
                //
                self.c_on_postgres_comment_object(*object_type, object_name)
                    .await;
            }
            Statement::Commit { .. } => {
                // fallback to lexer
            }
            Statement::Rollback { .. } => {
                // fallback to lexer
            }
            Statement::CreateSchema {
                schema_name,
                with,
                options,
                default_collate_spec,
                ..
            } => {
                // no need to completion.
                match schema_name {
                    SchemaName::Simple(object_name) => {
                        if self.in_pos(&object_name.span()) {
                            self.set_on_def();
                            return;
                        }
                    }
                    SchemaName::UnnamedAuthorization(ident) => {
                        if self.in_pos(&ident.span) {
                            self.set_on_def();
                            return;
                        }
                    }
                    SchemaName::NamedAuthorization(_object_name, ident) => {
                        if self.in_pos(&ident.span) {
                            self.set_on_def();
                            return;
                        }
                    }
                }
                if let Some(with) = with {
                    for w in with.iter() {
                        self.c_on_sql_option(w).await;
                        if self.finished() {
                            return;
                        }
                    }
                }
                if let Some(ops) = options {
                    for op in ops.iter() {
                        self.c_on_sql_option(op).await;
                        if self.finished() {
                            return;
                        }
                    }
                }
                if let Some(d) = default_collate_spec {
                    self.c_on_expr(d).await;
                }
            }
            Statement::CreateDatabase { db_name, .. } => {
                //
                if self.in_pos(&db_name.span()) {
                    self.set_on_def();
                }
            }
            Statement::CreateFunction(create_f) => {
                if self.in_pos(&create_f.name.span()) {
                    self.set_on_def();
                    return;
                }
                if let Some(args) = &create_f.args {
                    for a in args.iter() {
                        if let Some(name) = &a.name {
                            if self.in_pos(&name.span) {
                                self.set_on_def();
                                return;
                            }
                        }
                        self.c_on_data_type(&a.data_type).await;
                        if self.finished() {
                            return;
                        }
                        if let Some(expr) = &a.default_expr {
                            self.c_on_expr(expr).await;
                            if self.finished() {
                                return;
                            }
                        }
                        if let Some(name) = a.name.as_ref() {
                            self.scopes.current_scope(|s| {
                                s.enter_var(Var {
                                    data_type: Some(a.data_type.clone()),
                                    name: name.clone(),
                                })
                            });
                        }
                    }
                }
                if let Some(r) = create_f.return_type.as_ref() {
                    self.c_on_data_type(r).await;
                    if self.finished() {
                        return;
                    }
                }
                if let Some(x) = create_f.options.as_ref() {
                    for op in x.iter() {
                        self.c_on_sql_option(op).await;
                        if self.finished() {
                            return;
                        }
                    }
                }
                // due to this https://github.com/apache/datafusion-sqlparser-rs/issues/1356
                if let Some(x) = &create_f.function_body {
                    match x {
                        CreateFunctionBody::AsBeforeOptions {
                            body: expr,
                            link_symbol,
                        } => {
                            self.c_on_expr(expr).await;
                            if let Some(expr) = link_symbol {
                                self.c_on_expr(expr).await;
                            }
                        }
                        CreateFunctionBody::AsAfterOptions(expr)
                        | CreateFunctionBody::Return(expr)
                        | CreateFunctionBody::AsReturnExpr(expr) => {
                            self.c_on_expr(expr).await;
                        }
                        CreateFunctionBody::AsBeginEnd(begin_end_statements) => {
                            self.c_on_begin_end_statements(begin_end_statements).await;
                        }
                        CreateFunctionBody::AsReturnSelect(select) => {
                            let _ = self.c_on_select(select).await;
                        }
                    }
                }
            }
            Statement::CreateTrigger(create_trigger) => {
                if self.in_pos(&create_trigger.name.span()) {
                    self.set_on_def();
                    return;
                }
                self.c_on_table(&create_trigger.table_name).await;
                if self.finished() {
                    return;
                }
                for e in create_trigger.events.iter() {
                    if let TriggerEvent::Update(fields) = e {
                        for field in fields.iter() {
                            self.c_on_column(&create_trigger.table_name, field, false)
                                .await;
                            if self.finished() {
                                return;
                            }
                        }
                    }
                }
                if let Some(r) = create_trigger.referenced_table_name.as_ref() {
                    self.c_on_table(r).await;
                    if self.finished() {
                        return;
                    }
                }
                for r in create_trigger.referencing.iter() {
                    self.c_on_table(&r.transition_relation_name).await;
                    self.scopes
                        .current_scope(|s| s.enter_table(None, r.transition_relation_name.clone()));
                }
                let mut scope = Scope::default();
                /*
                    AI提示词 没有 NEW/OLD，而是使用 inserted.col 和 deleted.col 两张“伪表”
                */
                if self.helper.backend_name() == BackendKind::SqlServer {
                    // 	没有 NEW/OLD，而是使用 inserted.col 和 deleted.col 两张“伪表”
                    scope.enter_table(
                        Some(Ident::new("inserted")),
                        create_trigger.table_name.clone(),
                    );
                    scope.enter_table(
                        Some(Ident::new("deleted")),
                        create_trigger.table_name.clone(),
                    );
                } else {
                    scope.enter_table(Some(Ident::new("NEW")), create_trigger.table_name.clone());
                    scope.enter_table(Some(Ident::new("OLD")), create_trigger.table_name.clone());
                }
                let _guard = self.scopes.enter(scope);
                if let Some(c) = create_trigger.condition.as_ref() {
                    self.c_on_expr(c).await;
                    if self.finished() {
                        return;
                    }
                }
                if let Some(exec_body) = &create_trigger.exec_body {
                    self.c_on_trigger_exec_body(exec_body).await;
                }
                if let Some(ss) = &create_trigger.statements.as_ref() {
                    self.c_on_conditional_statements(ss).await;
                }
            }
            Statement::DropTrigger(drop_trigger) => {
                if let Some(table_name) = drop_trigger.table_name.as_ref() {
                    if drop_trigger.trigger_name.0.len() > 1 {
                        log::error!("multi part trigger name:{:?}", drop_trigger.trigger_name);
                    }
                    self.c_on_table(table_name).await;
                    if self.finished() {
                        return;
                    }
                    self.c_on_table_trigger(
                        table_name,
                        match &drop_trigger.trigger_name.0[0] {
                            ObjectNamePart::Identifier(ident) => ident,
                            ObjectNamePart::Function(object_name_part_function) => {
                                &object_name_part_function.name
                            }
                        },
                    )
                    .await;
                }
            }
            Statement::CreateProcedure {
                name, params, body, ..
            } => {
                if self.in_pos(&name.span()) {
                    self.set_on_def();
                    return;
                }
                if let Some(params) = params {
                    for p in params.iter() {
                        if self.in_pos(&p.name.span) {
                            self.set_on_def();
                            return;
                        }
                        self.c_on_data_type(&p.data_type).await;
                        if self.finished() {
                            return;
                        }
                        self.scopes.current_scope(|s| {
                            s.enter_var(Var {
                                name: p.name.clone(),
                                data_type: p.data_type.clone().into(),
                            })
                        });
                    }
                }
                self.c_on_conditional_statements(body).await;
            }
            Statement::CreateMacro { .. } => {
                // DuckDB not support.
            }
            Statement::CreateStage { .. } => {
                // snowflake not support
            }
            Statement::Assert { condition, message } => {
                self.c_on_expr(condition).await;
                if self.finished() {
                    return;
                }
                if let Some(msg) = message {
                    self.c_on_expr(msg).await;
                }
            }
            Statement::Grant {
                grantees,
                granted_by,
                objects,
                privileges,
                ..
            }
            | Statement::Revoke {
                privileges,
                objects,
                grantees,
                granted_by,
                ..
            } => {
                for g in grantees.iter() {
                    if let Some(g) = &g.name {
                        match g {
                            GranteeName::ObjectName(object_name) => {
                                if let Some(name) = object_name.0.first() {
                                    match name {
                                        ObjectNamePart::Identifier(ident) => {
                                            self.c_on_role(ident).await
                                        }
                                        ObjectNamePart::Function(..) => {}
                                    }
                                    if self.finished() {
                                        return;
                                    }
                                }
                            }
                            GranteeName::UserHost { user, .. } => {
                                self.c_on_role(user).await;
                                if self.finished() {
                                    return;
                                }
                            }
                        }
                    }
                }
                if let Some(granted_by) = granted_by {
                    self.c_on_role(granted_by).await;
                    if self.finished() {
                        return;
                    }
                }
                let mut tables_or_views = None as Option<Vec<ObjectName>>;
                if let Some(objects) = objects {
                    self.c_on_grant_objects(objects, &mut tables_or_views).await;
                }
                if let Some(tables_or_views) = tables_or_views {
                    match privileges {
                        Privileges::All { .. } => {
                            // nothing here
                        }
                        Privileges::Actions(vec) => {
                            for a in vec.iter() {
                                self.c_on_privileges_action(&tables_or_views[..], a).await;
                                if self.finished() {
                                    return;
                                }
                            }
                        }
                    }
                }
            }
            Statement::Deallocate { name, .. } => {
                self.c_on_prepare_ident(name).await;
            }
            Statement::Execute {
                parameters,
                using,
                name,
                ..
            } => {
                for p in parameters.iter() {
                    self.c_on_expr(p).await;
                    if self.finished() {
                        return;
                    }
                }
                for u in using.iter() {
                    self.c_on_expr(&u.expr).await;
                    if self.finished() {
                        return;
                    }
                    if let Some(name) = u.alias.as_ref() {
                        if self.in_pos(&name.span) {
                            self.set_on_def();
                            return;
                        }
                    }
                }
                if let Some(name) = name.as_ref() {
                    self.c_on_execute(name).await;
                }
            }
            Statement::Prepare {
                name,
                statement,
                data_types,
            } => {
                if self.in_pos(&name.span) {
                    self.set_on_def();
                    return;
                }
                for data_type in data_types.iter() {
                    self.c_on_data_type(data_type).await;
                    if self.finished() {
                        return;
                    }
                }
                Box::pin(self.completion_on_statement(statement)).await;
            }
            Statement::Kill { .. } => {}
            Statement::ExplainTable { table_name, .. } => {
                if self.helper.backend_name().mysql_like() {
                    //
                    self.c_on_table(table_name).await;
                    if self.last_name_in_pos(table_name) && table_name.0.len() == 1 {
                        let mut results = self.result.take().unwrap_or_default();
                        let kw = self.helper.key_words_completion().await;
                        // possible
                        results.extend(kw.into_iter());
                        self.result = Some(results);
                    }
                }
            }
            Statement::Explain {
                statement, options, ..
            } => {
                Box::pin(self.completion_on_statement(statement)).await;
                if self.finished() {
                    return;
                }
                if let Some(ops) = options {
                    for op in ops.iter() {
                        if let Some(arg) = op.arg.as_ref() {
                            self.c_on_expr(arg).await;
                            if self.finished() {
                                return;
                            }
                        }
                    }
                }
            }
            Statement::Savepoint { name } => {
                if self.in_pos(&name.span) {
                    self.set_on_def();
                }
            }
            Statement::ReleaseSavepoint { .. } => {
                // 我在postgres和mysql都没找到可以查询savepoint的语句
                // 所以这里不能做
            }
            Statement::Merge {
                table,
                source,
                on,
                output,
                clauses,
                ..
            } => {
                self.c_on_table_factor(table).await;
                if self.finished() {
                    return;
                }
                self.c_on_table_factor(source).await;
                if self.finished() {
                    return;
                }
                self.c_on_expr(on).await;
                for cl in clauses.iter() {
                    self.c_on_merge_clause(cl).await;
                    if self.finished() {
                        return;
                    }
                }
                if let Some(output) = output {
                    self.c_on_output_clause(output).await;
                }
            }
            Statement::Cache { .. } => {
                // Spark SQL
            }
            Statement::UNCache { .. } => {
                // Spark SQL
            }
            Statement::CreateSequence {
                name,
                sequence_options,
                data_type,
                ..
            } => {
                if let Some(d) = data_type {
                    self.c_on_data_type(d).await;
                }
                self.c_on_schema(name).await;
                if self.finished() {
                    return;
                }
                if self.in_pos(&name.span()) {
                    self.set_on_def();
                    return;
                }
                for op in sequence_options.iter() {
                    self.c_on_sequence_options(op).await;
                    if self.finished() {
                        return;
                    }
                }
            }
            Statement::CreateType {
                name,
                representation,
            } => {
                if self.in_pos(&name.span()) {
                    self.set_on_def();
                    return;
                }
                if let Some(rep) = representation {
                    match rep {
                        UserDefinedTypeRepresentation::Composite { attributes } => {
                            for a in attributes.iter() {
                                if self.in_pos(&a.name.span) {
                                    self.set_on_def();
                                    return;
                                }
                                self.c_on_data_type(&a.data_type).await;
                                if self.finished() {
                                    return;
                                }
                            }
                        }
                        UserDefinedTypeRepresentation::Enum { labels } => {
                            if labels.iter().any(|i| self.in_pos(&i.span)) {
                                self.set_on_def();
                            }
                        }
                        UserDefinedTypeRepresentation::Range { .. } => {}
                        UserDefinedTypeRepresentation::SqlDefinition { .. } => {}
                    }
                }
            }
            Statement::Pragma { name, .. } => {
                self.c_on_sqlite_pragma(name).await;
            }
            Statement::LockTables { tables } => {
                for t in tables.iter() {
                    self.c_on_table(&ObjectName(vec![ObjectNamePart::Identifier(
                        t.table.clone(),
                    )]))
                    .await;
                    if self.finished() {
                        return;
                    }
                    if let Some(alias) = t.alias.as_ref() {
                        if self.in_pos(&alias.span) {
                            self.set_on_def();
                            return;
                        }
                    }
                }
            }
            Statement::UnlockTables => {}
            Statement::Unload { query, with, .. } => {
                if let Some(q) = query.as_ref() {
                    let _ = self.c_on_query(q).await;
                }
                for w in with.iter() {
                    self.c_on_sql_option(w).await;
                    if self.finished() {
                        return;
                    }
                }
            }
            Statement::OptimizeTable {
                name,
                on_cluster,
                deduplicate,
                ..
            } => {
                self.c_on_table(name).await;
                if let Some(on_cluster) = on_cluster {
                    self.c_on_cluster(on_cluster).await;
                }
                if let Some(deduplicate) = deduplicate {
                    match deduplicate {
                        Deduplicate::All => {}
                        Deduplicate::ByExpression(expr) => {
                            self.c_on_expr(expr).await;
                        }
                    }
                }
            }
            Statement::LISTEN { .. } => {
                // 非常奇怪的命令像是 消息中间件
            }
            Statement::UNLISTEN { .. } => {}
            Statement::NOTIFY { .. } => {}
            Statement::LoadData {
                table_name,
                partitioned,
                ..
            } => {
                self.c_on_table(table_name).await;
                if let Some(partitioned) = partitioned {
                    for p in partitioned.iter() {
                        self.c_on_partition_expr(table_name, p).await;
                        if self.finished() {
                            return;
                        }
                    }
                }
            }
            Statement::CreateConnector(..) => {
                // hive not support
            }
            Statement::AlterType(at) => {
                self.c_on_type(&at.name).await;
                match &at.operation {
                    AlterTypeOperation::Rename(alter_type_rename) => {
                        if self.in_pos(&alter_type_rename.new_name.span) {
                            self.set_on_def();
                        }
                    }
                    AlterTypeOperation::AddValue(..) => {
                        // fallback to lexer
                    }
                    AlterTypeOperation::RenameValue(..) => {
                        // fallback to lexer
                    }
                }
            }
            Statement::AlterConnector { .. } => {
                // hive not support
            }
            Statement::AlterSession { .. } => {
                // snowflake not support
            }
            Statement::DropConnector { .. } => {
                // hive not support
            }
            Statement::CreateExtension(CreateExtension { name, schema, .. }) => {
                self.c_on_postgres_extension(name, false).await;
                if self.finished() {
                    return;
                }
                if let Some(schema) = schema {
                    if self.in_pos(&schema.span) {
                        self.result = Some(self.helper.list_schema_completion().await);
                    }
                }
            }
            Statement::DropExtension(DropExtension { names, .. }) => {
                for name in names.iter() {
                    self.c_on_postgres_extension(name, true).await;
                    if self.finished() {
                        return;
                    }
                }
            }
            Statement::ShowObjects(..) => {
                // Snowflake  not support
            }
            Statement::RenameTable(rename_tables) => {
                for rename in rename_tables.iter() {
                    self.c_on_table(&rename.old_name).await;
                    self.c_on_schema(&rename.new_name).await;
                    if self.last_name_in_pos(&rename.new_name) {
                        self.set_on_def();
                        return;
                    }
                }
            }
            Statement::List(..) => {
                // Snowflake not support
            }
            Statement::Remove(..) => {
                // Snowflake not support
            }
            Statement::RaisError {
                message,
                severity,
                state,
                arguments,
                ..
            } => {
                self.c_on_expr(message).await;
                self.c_on_expr(severity).await;
                self.c_on_expr(state).await;
                for ar in arguments.iter() {
                    self.c_on_expr(ar).await;
                    if self.finished() {
                        return;
                    }
                }
            }
            Statement::Set(set) => self.c_on_set(set).await,
            Statement::Case(case_statement) => self.c_on_case_statement(case_statement).await,
            Statement::If(if_statement) => self.c_on_if_statement(if_statement).await,
            Statement::While(while_statement) => self.c_on_while_statement(while_statement).await,
            Statement::Raise(raise_statement) => self.c_on_raise_statement(raise_statement).await,
            Statement::Open(open_statement) => self.c_on_open_statement(open_statement).await,
            Statement::DropDomain(drop_domain) => self.c_on_drop_domain(drop_domain).await,
            Statement::Deny(deny_statement) => self.c_on_deny_statement(deny_statement).await,
            Statement::CreateDomain(create_domain) => self.c_on_create_domain(create_domain).await,
            Statement::Print(print_statement) => self.c_on_print_statement(print_statement).await,
            Statement::Return(return_statement) => {
                self.c_on_return_statement(return_statement).await
            }
            Statement::CreateServer(create_server_statement) => {
                self.c_on_create_server_statement(create_server_statement)
                    .await;
            }
            Statement::AlterSchema(alter_schema) => self.c_on_alter_schema(alter_schema).await,
            Statement::ShowCharset(show_charset) => {
                if let Some(filter) = show_charset.filter.as_ref() {
                    self.c_on_show_statement_filter(filter).await;
                }
            }
            Statement::ExportData(export_data) => self.c_on_export_data(export_data).await,
            Statement::CreateUser(..) => {
                // fallback to lexer
            }
            Statement::Vacuum(..) => {
                // redshift not support right now
            }
            Statement::CreateOperator(..) => {}
            Statement::CreateOperatorFamily(..) => {}
            Statement::CreateOperatorClass(..) => {}
            Statement::DropOperator(..) => {}
            Statement::DropOperatorFamily(..) => {}
            Statement::DropOperatorClass(..) => {}
            Statement::AlterUser(..) => {}
            Statement::Reset(..) => {}
        }
    }

    pub(crate) async fn c_on_export_data(&mut self, _export_data: &ExportData) {
        // bigquery not support
    }

    pub(crate) async fn c_on_alter_schema(&mut self, alter_schema: &AlterSchema) {
        if self.in_pos(&alter_schema.name.span()) {
            self.result = Some(self.helper.list_database_schema_completion().await);
            return;
        }
        for op in alter_schema.operations.iter() {
            self.c_on_alter_schema_operation(op).await;
            if self.finished() {
                return;
            }
        }
    }
    pub(crate) async fn c_on_alter_schema_operation(&mut self, op: &AlterSchemaOperation) {
        match op {
            AlterSchemaOperation::SetDefaultCollate { collate } => {
                self.c_on_expr(collate).await;
            }
            AlterSchemaOperation::AddReplica { options, .. } => {
                if let Some(options) = options {
                    for option in options.iter() {
                        self.c_on_sql_option(option).await;
                        if self.finished() {
                            return;
                        }
                    }
                }
            }
            AlterSchemaOperation::DropReplica { .. } => {}
            AlterSchemaOperation::SetOptionsParens { options } => {
                for option in options.iter() {
                    self.c_on_sql_option(option).await;
                    if self.finished() {
                        return;
                    }
                }
            }
            AlterSchemaOperation::Rename { name } => {
                if self.in_pos(&name.span()) {
                    self.set_on_def();
                }
            }
            AlterSchemaOperation::OwnerTo { owner } => {
                self.c_on_owner(owner).await;
            }
        }
    }
    pub(crate) async fn c_on_sequence_options(&mut self, op: &SequenceOptions) {
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
            SequenceOptions::Cycle(_) => {}
        }
    }
}

impl<'a> CompletionStatement<'a> {
    async fn c_on_create_server_statement(
        &mut self, create_server_statement: &CreateServerStatement,
    ) {
        if self.in_pos(&create_server_statement.name.span()) {
            self.set_on_def();
        }
    }
    async fn c_on_conditional_statements(&mut self, ast: &ConditionalStatements) {
        match ast {
            ConditionalStatements::Sequence { statements } => {
                for stmt in statements.iter() {
                    Box::pin(self.completion_on_statement(stmt)).await;
                }
            }
            ConditionalStatements::BeginEnd(begin_end_statements) => {
                self.c_on_begin_end_statements(begin_end_statements).await;
            }
        }
    }
    async fn c_on_begin_end_statements(&mut self, ast: &BeginEndStatements) {
        // begin ane end introduce a new scope
        let _guard = self.scopes.enter(Scope::default());
        for s in ast.statements.iter() {
            Box::pin(self.completion_on_statement(s)).await;
            if self.finished() {
                return;
            }
        }
    }

    async fn c_on_create_table(&mut self, create_table: &CreateTable) {
        self.c_on_schema(&create_table.name).await;
        if self.finished() {
            return;
        }
        if self.in_pos(&create_table.name.span()) {
            self.set_on_def();
            return;
        }
        let this_table = TempTable {
            columns: Arc::new(
                create_table
                    .columns
                    .iter()
                    .map(|c| TempTableColumn {
                        name: c.name.value.clone(),
                        data_type: Some(c.data_type.to_string()),
                    })
                    .collect(),
            ),
        };
        self.scopes
            .current_scope(|scope| scope.enter_temp_table(None, this_table.clone()));
        for c in create_table.columns.iter() {
            if self.in_pos(&c.name.span) {
                self.set_on_def();
                return;
            }
            self.c_on_data_type(&c.data_type).await;
            if self.finished() {
                return;
            }
            for op in c.options.iter() {
                Box::pin(self.c_on_column_option_def(op)).await;
                if self.finished() {
                    return;
                }
            }
        }
        if let Some(on_cluster) = create_table.on_cluster.as_ref() {
            self.c_on_cluster(on_cluster).await;
            if self.finished() {
                return;
            }
        }
        if let Some(p) = create_table.primary_key.as_ref() {
            self.c_on_expr(p.as_ref()).await;
            if self.finished() {
                return;
            }
        }
        if let Some(p) = create_table.order_by.as_ref() {
            match p {
                OneOrManyWithParens::One(p) => self.c_on_expr(p).await,
                OneOrManyWithParens::Many(items) => {
                    for item in items.iter() {
                        self.c_on_expr(item).await;
                        if self.finished() {
                            return;
                        }
                    }
                }
            }
        }
        for c in create_table.constraints.iter() {
            self.c_on_table_constraint(c, None).await;
        }
        if let Some(query) = &create_table.query {
            let _ = self.c_on_query(query).await;
        }
        if let Some(i) = create_table.inherits.as_ref() {
            for t in i.iter() {
                self.c_on_table(t).await;
                if self.finished() {
                    return;
                }
            }
        }
        if let Some(like) = create_table.like.as_ref() {
            self.c_on_create_table_like_kind(like).await;
        }
        if let Some(clone) = create_table.clone.as_ref() {
            self.c_on_table(clone).await;
            if self.finished() {}
        }

        // 其他都可以fallback 到lexer的实现
    }

    pub(crate) async fn c_on_create_table_like_kind(&mut self, ast: &CreateTableLikeKind) {
        match ast {
            CreateTableLikeKind::Parenthesized(create_table_like) => {
                self.c_on_create_table_like(create_table_like).await;
            }
            CreateTableLikeKind::Plain(create_table_like) => {
                self.c_on_create_table_like(create_table_like).await;
            }
        }
    }

    pub(crate) async fn c_on_create_table_like(&mut self, ast: &CreateTableLike) {
        self.c_on_table_or_view(&ast.name).await;
    }

    async fn c_on_grant_objects(
        &mut self, objects: &GrantObjects, tables_or_views: &mut Option<Vec<ObjectName>>,
    ) {
        match objects {
            GrantObjects::FutureExternalTablesInSchema { schemas }
            | GrantObjects::FutureMaterializedViewsInSchema { schemas }
            | GrantObjects::FutureSequencesInSchema { schemas }
            | GrantObjects::AllViewsInSchema { schemas }
            | GrantObjects::FutureTablesInSchema { schemas }
            | GrantObjects::FutureViewsInSchema { schemas }
            | GrantObjects::AllMaterializedViewsInSchema { schemas }
            | GrantObjects::AllExternalTablesInSchema { schemas }
            | GrantObjects::AllSequencesInSchema { schemas }
            | GrantObjects::AllTablesInSchema { schemas }
            | GrantObjects::AllFunctionsInSchema { schemas }
            | GrantObjects::Schemas(schemas) => {
                if schemas.iter().any(|x| self.in_pos(&x.span())) {
                    let result = self.helper.list_database_schema_completion().await;
                    self.result = Some(result);
                }
            }
            GrantObjects::Sequences(vec) => {
                for s in vec.iter() {
                    self.c_on_sequence(s).await;
                    if self.finished() {
                        return;
                    }
                }
            }
            GrantObjects::Tables(vec) => {
                for table in vec.iter() {
                    self.c_on_table(table).await;
                    if self.finished() {
                        return;
                    }
                }
                *tables_or_views = Some(vec.clone());
            }
            GrantObjects::Databases(databases)
            | GrantObjects::FutureSchemasInDatabase { databases } => {
                if databases.iter().any(|x| self.in_pos(&x.span())) {
                    self.result = Some(self.helper.list_database_schema_completion().await);
                }
            }
            GrantObjects::Views(views) => {
                for view in views.iter() {
                    self.c_on_view(view).await;
                    if self.finished() {
                        return;
                    }
                }
                *tables_or_views = Some(views.clone());
            }
            GrantObjects::Warehouses(..) => {
                // TODO
            }
            GrantObjects::Integrations(..) => {
                // I don't know what is it
            }
            GrantObjects::ResourceMonitors(..) => {
                // I don't know what is it
            }
            GrantObjects::Users(users) => {
                if users.iter().any(|x| self.in_pos(&x.span())) {
                    self.result = Some(self.helper.list_user_completion().await)
                }
            }
            GrantObjects::ComputePools(..) => {
                // I don't know what is it
            }
            GrantObjects::Connections(..) => {
                // I don't know what is it
            }
            GrantObjects::FailoverGroup(..) => {
                // I don't know what is it
            }
            GrantObjects::ReplicationGroup(..) => {
                // I don't know what is it
            }
            GrantObjects::ExternalVolumes(..) => {
                // I don't know what is it
            }
            GrantObjects::Procedure { name, arg_types } => {
                for t in arg_types.iter() {
                    self.c_on_data_type(t).await;
                    if self.finished() {
                        return;
                    }
                }
                self.c_on_procedure(name).await;
            }
            GrantObjects::Function { name, .. } => {
                self.c_on_function(name).await;
            }
        }
    }

    fn try_remove_one_at(&self, value: String) -> String {
        if self.helper.backend_name().support_at() {
            try_remove_one_at(value)
        } else {
            value
        }
    }

    async fn object_name_to_string(&self, name: &ObjectName) -> String {
        let x = self
            .object_name_to_idents(name)
            .await
            .into_iter()
            .map(|x| x.to_string())
            .collect::<Vec<_>>()
            .join(".");
        self.try_remove_one_at(x)
    }

    async fn c_on_set(&mut self, ast: &Set) {
        match ast {
            Set::SingleAssignment {
                variable, values, ..
            } => {
                for var in variable.0.iter() {
                    match var {
                        ObjectNamePart::Identifier(ident) => {
                            self.c_on_variable(ident).await;
                        }
                        ObjectNamePart::Function(..) => {}
                    }
                }
                for value in values.iter() {
                    self.c_on_expr(value).await;
                    if self.finished() {
                        return;
                    }
                }
                let name = self.object_name_to_string(variable).await;
                self.scopes.current_scope(|s| s.enter_var(name.into()))
            }
            Set::ParenthesizedAssignments { variables, values } => {
                for var in variables.iter() {
                    for var in var.0.iter() {
                        match var {
                            ObjectNamePart::Identifier(ident) => {
                                self.c_on_variable(ident).await;
                            }
                            ObjectNamePart::Function(..) => {}
                        }
                    }
                    let name = self.object_name_to_string(var).await;
                    self.scopes.current_scope(|s| s.enter_var(name.into()))
                }
                for value in values.iter() {
                    self.c_on_expr(value).await;
                    if self.finished() {
                        return;
                    }
                }
            }
            Set::MultipleAssignments { assignments } => {
                for ass in assignments.iter() {
                    for name in ass.name.0.iter() {
                        match name {
                            ObjectNamePart::Identifier(ident) => self.c_on_variable(ident).await,
                            ObjectNamePart::Function(..) => {}
                        }
                    }
                    self.c_on_expr(&ass.value).await;
                    if self.finished() {
                        return;
                    }
                    let name = self.object_name_to_string(&ass.name).await;
                    self.scopes.current_scope(|s| s.enter_var(name.into()))
                }
            }
            Set::SetSessionParam(..) => {
                // no completionable items
            }
            Set::SetRole { role_name, .. } => {
                if let Some(role_name) = role_name {
                    self.c_on_role(role_name).await;
                }
            }
            Set::SetTimeZone { value, .. } => {
                self.c_on_expr(value).await;
            }
            Set::SetNames { .. } => {}
            Set::SetNamesDefault {} => {}
            Set::SetTransaction { .. } => {
                // fallback to lexer
            }
            Set::SetSessionAuthorization(..) => {}
        }
    }

    async fn c_on_conditional_statement_block(&mut self, s: &ConditionalStatementBlock) {
        if let Some(condition) = s.condition.as_ref() {
            self.c_on_expr(condition).await;
        }
        self.c_on_conditional_statements(&s.conditional_statements)
            .await;
    }

    async fn c_on_case_statement(&mut self, ast: &CaseStatement) {
        if let Some(match_expr) = ast.match_expr.as_ref() {
            self.c_on_expr(match_expr).await;
        }
        for wb in ast.when_blocks.iter() {
            self.c_on_conditional_statement_block(wb).await;
            if self.finished() {
                return;
            }
        }
        if let Some(else_block) = ast.else_block.as_ref() {
            self.c_on_conditional_statement_block(else_block).await;
            if self.finished() {}
        }
    }

    async fn c_on_if_statement(&mut self, ast: &IfStatement) {
        self.c_on_conditional_statement_block(&ast.if_block).await;
        if self.finished() {
            return;
        }
        for e in ast.elseif_blocks.iter() {
            self.c_on_conditional_statement_block(e).await;
        }
        if let Some(e) = ast.else_block.as_ref() {
            self.c_on_conditional_statement_block(e).await
        }
    }

    async fn c_on_while_statement(&mut self, ast: &WhileStatement) {
        self.c_on_conditional_statement_block(&ast.while_block)
            .await
    }
    async fn c_on_raise_statement(&mut self, ast: &RaiseStatement) {
        if let Some(value) = ast.value.as_ref() {
            match value {
                RaiseStatementValue::UsingMessage(expr) => self.c_on_expr(expr).await,
                RaiseStatementValue::Expr(expr) => self.c_on_expr(expr).await,
            }
        }
    }

    async fn c_on_open_statement(&mut self, ast: &OpenStatement) {
        if self.in_pos(&ast.cursor_name.span) {
            self.set_on_def();
        }
    }
    async fn c_on_drop_domain(&mut self, ast: &DropDomain) {
        self.c_on_schema(&ast.name).await;
        if !self.last_name_in_pos(&ast.name) {
            return;
        }
        log::info!("hit drop domain:{:?}", ast.name);
        let (schema, _) = self.extract_schema_and_item(&ast.name);
        self.result = Some(self.helper.list_domain_completion(schema.as_deref()).await);
    }
    async fn c_on_grantee(&mut self, g: &Grantee) {
        if let Some(name) = g.name.as_ref() {
            match name {
                GranteeName::ObjectName(object_name) => {
                    self.c_on_role_objectname(object_name).await;
                }
                GranteeName::UserHost { user, .. } => {
                    self.c_on_user(user).await;
                }
            }
        }
    }

    async fn c_on_deny_statement(&mut self, ast: &DenyStatement) {
        let mut tables = None;
        self.c_on_grant_objects(&ast.objects, &mut tables).await;
        for g in ast.grantees.iter() {
            self.c_on_grantee(g).await;
            if self.finished() {
                return;
            }
        }
        if let Some(g) = ast.granted_by.as_ref() {
            self.c_on_role(g).await;
        }
    }
    /*
    example:

    CREATE DOMAIN phone_cn AS text
    CHECK (
        VALUE ~ '^1[0-9]{10}$'   -- 11 位，1 开头
        AND VALUE IS NOT NULL        -- 显式拒绝 NULL
    );*/
    async fn c_on_create_domain(&mut self, ast: &CreateDomain) {
        self.c_on_schema(&ast.name).await;
        if self.last_name_in_pos(&ast.name) {
            self.set_on_def();
            return;
        }
        self.c_on_data_type(&ast.data_type).await;
        if let Some(d) = ast.default.as_ref() {
            self.c_on_expr(d).await;
        }
        //
        for _ in ast.constraints.iter() {
            // TODO here I don't the table is
            // looking at the syntax I think I want to fallback to lexer
        }
    }

    async fn c_on_print_statement(&mut self, ast: &PrintStatement) {
        self.c_on_expr(&ast.message).await;
    }
    async fn c_on_return_statement(&mut self, ast: &ReturnStatement) {
        if let Some(expr) = &ast.value {
            match expr {
                ReturnStatementValue::Expr(expr) => self.c_on_expr(expr).await,
            }
        }
    }
}

use regex::Regex;

/// 从错误字符串中提取 `Line: X, Column: Y`
pub fn sqlparser_err_extract_line_col(error: impl Into<String>) -> (Option<u32>, Option<u32>) {
    // 匹配两种常见格式：
    // 1. Line: 2, Column: 14
    // 2. line 2, column 14
    let error = error.into();
    let re = Regex::new(r"(?i)line\s*[:]?\s*(\d+).*column\s*[:]?\s*(\d+)")
        .expect("this regexp must compile");
    if let Some(caps) = re.captures(&error) {
        let line = caps.get(1).and_then(|s| s.as_str().parse().ok());
        let col = caps.get(2).and_then(|s| s.as_str().parse().ok());
        (line, col)
    } else {
        (None, None)
    }
}

impl From<sqlparser::parser::ParserError> for ErrAndLoc {
    fn from(value: sqlparser::parser::ParserError) -> Self {
        let msg = value.to_string();
        let (line, column) = sqlparser_err_extract_line_col(&msg);
        Self { line, column, msg }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_table_entered_equal_name() {
        assert!(
            TableEntered::Temp {
                table: Default::default(),
                alias: Some(Ident::new("abc")),
            }
            .equal_name(&Ident::new("ABC"))
        );
        assert!(
            TableEntered::Temp {
                table: Default::default(),
                alias: Some(Ident::with_quote('"', "abc")),
            }
            .equal_name(&Ident::new("abc"))
        );
        assert!(
            !TableEntered::Temp {
                table: Default::default(),
                alias: Some(Ident::with_quote('"', "abc")),
            }
            .equal_name(&Ident::new("ABC"))
        );
    }

    #[test]
    fn test_extract_line_col() {
        {
            let err = "Expected: SELECT, VALUES, or a subquery in the query body, found: from at Line: 2, Column: 14";
            let (line, col) = sqlparser_err_extract_line_col(err);
            assert_eq!(line, Some(2));
            assert_eq!(col, Some(14));
        }
        {
            let err = "Expected: an SQL statement, found: 3.14 at Line: 3, Column: 22";
            let (line, col) = sqlparser_err_extract_line_col(err);
            assert_eq!(line, Some(3));
            assert_eq!(col, Some(22));
        }
    }
    #[ignore = "this is just a test for me"]
    #[test]
    fn test_sql_statement() {
        let sql = r#"
            delete emp from emp where emp.id = 1
        "#;
        let dialect = sqlparser::dialect::MySqlDialect {};
        let p = sqlparser::parser::Parser::new(&dialect);
        let mut p = p.try_with_sql(sql).unwrap();
        let statement = p.parse_statement().unwrap();

        let s = serde_json::to_string(&statement).unwrap();

        println!("{:?}", s);
    }

    #[ignore = "this is just a test for me"]
    #[test]
    fn test_sql_statementxxxx() {
        let sql = r#"

SELECT *
FROM XMLTABLE(
        '/users/user'
        PASSING XMLPARSE(
            DOCUMENT $$
            <users>
                <user id="1" name="Alice" age="30"/>
                <user id="2" name="Bob"   age="25"/>
            </users>
            $$
        )
        COLUMNS
            id   int         PATH '@id',
            name text        PATH '@name',
            age  int         PATH '@age'
) AS t WHERE t.id > 0 ;

        "#;
        let dialect = sqlparser::dialect::PostgreSqlDialect {};
        let p = sqlparser::parser::Parser::new(&dialect);
        let mut p = p.try_with_sql(sql).unwrap();
        let statement = p.parse_statement().unwrap();

        let s = serde_json::to_string(&statement).unwrap();

        println!("{:?}", s);
    }

    #[ignore = "this is just a test for me"]
    #[test]
    fn test_sql_lexer() {
        let sql = r#"
            select 1
        "#;
        let dialect = sqlparser::dialect::MySqlDialect {};
        let tokens = sqlparser::tokenizer::Tokenizer::new(&dialect, sql)
            .tokenize_with_location()
            .unwrap_or_default();
        for t in tokens.iter() {
            println!("{:?}", t);
        }
    }

    #[ignore = "this is just a test for me"]
    #[test]
    fn test_sql_lexer111() {
        let sql = r#" abc. "#;
        let dialect = sqlparser::dialect::PostgreSqlDialect {};
        let tokens = sqlparser::tokenizer::Tokenizer::new(&dialect, sql)
            .tokenize_with_location()
            .unwrap_or_default();
        for t in tokens.iter() {
            println!("{:?}", t);
        }
    }
}
