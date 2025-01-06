use std::{
    collections::{HashMap, HashSet},
    ops::{Deref, DerefMut},
    sync::{Arc, Mutex},
};

use sqlparser::{
    ast::{
        AlterIndexOperation, AlterRoleOperation, AssignmentTarget, CopySource, CreateFunctionBody,
        DataType, Declare, DeclareAssignment, GrantObjects, Ident, ObjectName, Privileges,
        SchemaName, SelectItem, SequenceOptions, Spanned, Statement, TableConstraint, TableFactor,
        TriggerEvent, UserDefinedTypeRepresentation,
    },
    tokenizer::{Location, Span},
};

use crate::completion::{AstCompletionHelper, CompletionItem, CompletionKind};

pub struct CompletionStament {
    pub(crate) loc: Location,
    pub(crate) result: Option<Vec<CompletionItem>>,
    pub(crate) helper: Arc<dyn AstCompletionHelper>,
    pub(crate) scopes: Scopes,
    pub(crate) on_def: bool,
    pub(crate) has_err: bool,
    pub(crate) query_id: uuid::Uuid,
}

#[derive(Clone)]
pub struct Scopes {
    ss: Arc<Mutex<Vec<Scope>>>,
}

pub(crate) struct ScopesGuard {
    ss: Arc<Mutex<Vec<Scope>>>,
    n: usize,
}

impl Drop for ScopesGuard {
    fn drop(&mut self) {
        let n = self.n;
        let mut m = self.ss.lock().unwrap();
        let m = m.deref_mut();
        for _ in 0..n {
            m.pop();
        }
    }
}

impl Scopes {
    pub(crate) fn nearest_from_tables(&self) -> Vec<FromTableOrTemp> {
        let x = self.ss.lock().unwrap();
        for s in x.deref().iter().rev() {
            if s.from_tables.len() > 0 {
                return s.from_tables.clone();
            }
        }
        vec![]
    }

    pub(crate) fn drop_n(&self, n: usize) -> ScopesGuard {
        ScopesGuard {
            ss: self.ss.clone(),
            n,
        }
    }
    pub(crate) fn enter_declare(&self, d: Arc<Declare>) {
        self.current_scope(|x| x.enter_declare(d));
    }
    pub(crate) fn current_scope<R>(&self, f: impl FnOnce(&mut Scope) -> R) -> R {
        f(self.ss.lock().unwrap().last_mut().unwrap())
    }
    pub(crate) fn new() -> Self {
        Self {
            // at lease one scope.
            ss: Arc::new(Mutex::new(vec![Scope::default()])),
        }
    }

    pub(crate) fn tables(&self) -> Vec<String> {
        let names: HashSet<String> = HashSet::new();
        let mut ret = Vec::new();
        self.inner_first(|scope| {
            if let Some(table) = &scope.this_table {
                match table {
                    ThisTableEnum::Table { name } => {
                        let name = name.0.last().unwrap();
                        let name_str = name.to_string();
                        if !names.contains(&name_str) {
                            ret.push(name_str);
                        }
                    }
                    ThisTableEnum::Temp { temp: _ } => {
                        // nothing to here.
                    }
                }
            }
            for name in scope.table_entered.keys() {
                let name_str = name.to_string();
                if !names.contains(&name_str) {
                    ret.push(name_str);
                }
            }
            for temp in scope.temp_tables.keys() {
                let name_str = temp.to_string();
                if !names.contains(&name_str) {
                    ret.push(name_str);
                }
            }
            // this clouse need return a option.
            return None as Option<()>;
        });
        ret
    }

    pub(crate) fn push(&self, scope: Scope) -> ScopesGuard {
        self.ss.lock().unwrap().push(scope);
        ScopesGuard {
            ss: self.ss.clone(),
            n: 1,
        }
    }
    pub(crate) fn push_without_guard(&self, s: Scope) {
        self.ss.lock().unwrap().push(s);
    }

    pub(crate) fn inner_first<R>(&self, mut f: impl FnMut(&Scope) -> Option<R>) -> Option<R> {
        for s in self.ss.lock().unwrap().iter().rev() {
            let x = f(s);
            if x.is_some() {
                return x;
            }
        }
        None
    }
    pub(crate) fn nearest_table_named(&self, name: &Ident) -> Option<NearNestTableNamed> {
        let name = name.to_owned();
        self.inner_first(|scope| -> Option<NearNestTableNamed> {
            let alias = scope.table_entered.get(&name).map(|x| x.clone());
            if alias.is_some() {
                return alias.map(|alias| NearNestTableNamed::Alias { table: alias });
            }

            let temp_table = scope.temp_tables.get(&name).map(|x| x.clone());
            if temp_table.is_some() {
                return temp_table.map(|temp_table| NearNestTableNamed::Temp { table: temp_table });
            }
            None
        })
    }

    pub(crate) fn this_table(&self) -> Option<ThisTableEnum> {
        let m = self.ss.lock().unwrap();
        let m = m.deref();
        for x in m.iter().rev() {
            if x.this_table.is_some() {
                return x.this_table.clone();
            }
        }
        None
    }
}

pub(crate) enum NearNestTableNamed {
    Alias { table: ObjectName },
    Temp { table: TempTable },
}

#[derive(Clone, Debug)]
pub struct TempTableColumn {
    pub(crate) name: String,
    // typ: String,
}

#[derive(Clone, Debug)]
pub struct TempTable {
    pub(crate) columns: Arc<Vec<TempTableColumn>>,
}

#[derive(Debug, Clone)]
pub enum ThisTableEnum {
    Table { name: ObjectName },
    Temp { temp: TempTable },
}

impl From<ObjectName> for ThisTableEnum {
    fn from(value: ObjectName) -> Self {
        Self::Table { name: value }
    }
}

#[derive(Clone, Default, Debug)]
pub struct Scope {
    table_entered: HashMap<Ident, ObjectName>,
    this_table: Option<ThisTableEnum>,
    temp_tables: HashMap<Ident, TempTable>,
    // https://www.postgresql.org/docs/current/plpgsql-declarations.html

    // right sqlparse have problem to parse function or precedure body
    // `declares` and `vars` not used.
    #[allow(unused)]
    declares: HashMap<String, Arc<Declare>>,
    //
    #[allow(unused)]
    vars: HashMap<String, Var>,

    pub(crate) from_tables: Vec<FromTableOrTemp>,
}

#[derive(Clone, Debug)]
pub(crate) enum FromTableOrTemp {
    FromTable { table: ObjectName },
    Temp { table: TempTable },
    Unnamed { table: TempTable },
}

impl From<ObjectName> for FromTableOrTemp {
    fn from(value: ObjectName) -> Self {
        Self::FromTable { table: value }
    }
}

impl From<TempTable> for FromTableOrTemp {
    fn from(value: TempTable) -> Self {
        Self::Temp { table: value }
    }
}

#[derive(Clone, Debug)]
pub struct Var {
    #[allow(dead_code)]
    datatype: DataType,
}

impl Scope {
    pub(crate) fn enter_table(&mut self, alias: Ident, table_name: ObjectName) {
        self.table_entered.insert(alias, table_name);
    }

    pub(crate) fn enter_temp_table(&mut self, name: Ident, table_name: TempTable) {
        self.temp_tables.insert(name, table_name);
    }

    pub(crate) fn enter_declare(&mut self, d: Arc<Declare>) {
        for name in d.names.iter() {
            self.declares.insert(name.value.clone(), d.clone());
        }
    }

    pub(crate) fn from_temp_tables(temp_tables: HashMap<Ident, TempTable>) -> Self {
        Self {
            temp_tables,
            ..Default::default()
        }
    }

    pub(crate) fn from_this_table(this_table: ObjectName) -> Self {
        Self {
            this_table: Some(ThisTableEnum::from(this_table)),
            ..Default::default()
        }
    }

    pub(crate) fn from_vars(vars: HashMap<String, Var>) -> Self {
        Self {
            vars,
            ..Default::default()
        }
    }
}

impl CompletionStament {
    pub(crate) fn set_on_def(&mut self) {
        self.on_def = true
    }
    pub(crate) fn set_has_err(&mut self) {
        self.has_err = true
    }
    pub(crate) fn finished(&self) -> bool {
        self.result.is_some() || self.on_def || self.has_err
    }
    pub(crate) fn new(
        loc: Location, helper: Arc<dyn AstCompletionHelper>, query_id: uuid::Uuid,
    ) -> Self {
        Self {
            loc,
            helper,
            result: None,
            scopes: Scopes::new(),
            on_def: false,
            has_err: false,
            query_id,
        }
    }

    pub(crate) fn in_pos(&self, span: &Span) -> bool {
        (self.loc.line >= span.start.line && self.loc.line <= span.end.line)
            && (self.loc.column >= span.start.column && self.loc.column <= span.end.column)
    }

    pub(crate) async fn completion_on_statement(&mut self, statement: &Statement) {
        match statement {
            Statement::Analyze {
                table_name,
                columns,
                partitions,
                ..
            } => {
                self.c_on_table(table_name).await;
                if self.finished() {
                    return;
                }
                for c in columns.iter() {
                    self.c_on_column(table_name, c).await;
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
            Statement::Truncate { table_names, .. } => {
                for t in table_names.iter() {
                    self.c_on_table(&t.name).await;
                    if self.finished() {
                        return;
                    }
                }
                // TODO how should I handle partitions
            }
            Statement::Msck { .. } => {
                // hive not support rigth now
            }
            Statement::Query(query) => {
                let _ = self.c_on_query(&query).await;
            }
            Statement::Insert(insert) => {
                self.c_on_table(&insert.table_name).await;
                if self.finished() {
                    return;
                }
                for c in insert.columns.iter() {
                    self.c_on_column(&insert.table_name, c).await;
                    if self.finished() {
                        return;
                    }
                }
                // I don't understand this,But we can still suggest table column
                for c in insert.after_columns.iter() {
                    self.c_on_column(&insert.table_name, c).await;
                    if self.finished() {
                        return;
                    }
                }
                //
                if let Some(query) = &insert.source {
                    let _ = self.c_on_query(&query).await;
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
                {
                    // 在 SQL 中，特别是在 PostgreSQL 中，RETURNING 关键字用于在 INSERT、UPDATE 或 DELETE 语句后返回被操作的行的特定列。这使得你可以在执行数据修改后立即获取相关信息，例如新插入的记录的 ID。
                    let _gurad = self
                        .scopes
                        .push(Scope::from_this_table(insert.table_name.clone()));
                    if let Some(returning) = &insert.returning {
                        for r in returning.iter() {
                            match r {
                                SelectItem::UnnamedExpr(expr) => {
                                    self.c_on_expr(expr).await;
                                }
                                SelectItem::ExprWithAlias { expr, alias } => {
                                    if self.in_pos(&alias.span) {
                                        self.set_on_def();
                                        return;
                                    }
                                    self.c_on_expr(expr).await;
                                }
                                SelectItem::QualifiedWildcard(..) => {
                                    // nothing here
                                }
                                SelectItem::Wildcard(..) => {
                                    // nothing here
                                }
                            }
                        }
                    }
                }
            }
            Statement::Install { .. } => {
                // duckdb no support at all.
            }
            Statement::Load { .. } => {
                // DuckDB no support at all.
            }
            Statement::Directory { .. } => {
                // TODO maybe dump some data to a file.
            }
            Statement::Call(function) => {
                Box::pin(self.c_on_function_call(function)).await;
            }
            Statement::Copy { source, .. } => {
                match source {
                    CopySource::Table {
                        table_name,
                        columns,
                    } => {
                        self.c_on_table(table_name).await;
                        if self.finished() {
                            return;
                        }
                        for c in columns.iter() {
                            self.c_on_column(table_name, &c).await;
                            if self.finished() {
                                return;
                            }
                        }
                    }
                    CopySource::Query(query) => {
                        let _ = self.c_on_query(&query).await;
                    }
                };
            }

            Statement::CopyIntoSnowflake { .. } => {
                // snow flake not support at all
            }
            Statement::Close { .. } => {
                //
            }
            Statement::Update {
                table,
                assignments,
                from,
                selection,
                returning,
                ..
            } => {
                let mut joins = vec![table.clone()];
                if let Some(x) = from.as_ref() {
                    joins.push(x.clone());
                }
                let _guard = self.c_on_table_with_joins(&joins[..]).await;
                let table = match &table.relation {
                    TableFactor::Table { name, .. } => Some(name.clone()),
                    _ => {
                        log::error!("update from table is not a Table but:{:?}", table);
                        None
                    }
                };
                if let Some(selection) = selection {
                    Box::pin(self.c_on_expr(selection)).await;
                    if self.finished() {
                        return;
                    }
                }
                for ass in assignments.iter() {
                    self.c_on_expr(&ass.value).await;
                    if self.finished() {
                        return;
                    }
                    if let Some(table) = &table {
                        match &ass.target {
                            AssignmentTarget::ColumnName(object_name) => {
                                match object_name.0.len() {
                                    1 => {
                                        Box::pin(
                                            self.c_on_column(table, object_name.0.last().unwrap()),
                                        )
                                        .await;
                                    }
                                    2 | 3 | 4 => {}
                                    _ => {
                                        log::error!("unhandled object name for column target")
                                    }
                                }
                            }

                            AssignmentTarget::Tuple(_vec) => todo!(),
                        }
                    }
                }
                if let Some(p) = returning {
                    let _ = self.c_on_select_item(&p).await;
                }
            }
            Statement::Delete(delete) => {
                let _guard = self.c_from_table(&delete.from).await;
                if self.finished() {
                    return;
                }
                let _guard = if let Some(other) = &delete.using {
                    let _guard = self.c_on_table_with_joins(&other).await;
                    if self.finished() {
                        return;
                    }
                    _guard
                } else {
                    // 0 means drop nothing.
                    self.scopes.drop_n(0)
                };

                for t in delete.tables.iter() {
                    self.c_on_table(t).await;
                    if self.finished() {
                        return;
                    }
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
                    let _ = self.c_on_select_item(&r[..]).await;
                }
            }
            Statement::CreateView { query, name, .. } => {
                if self.in_pos(&name.span()) {
                    self.set_on_def();
                    return;
                }
                // name is user defined no need completion.
                let _ = self.c_on_query(&query).await;
            }
            Statement::CreateTable(create_table) => {
                if self.in_pos(&create_table.name.span()) {
                    self.set_on_def();
                    return;
                }
                let mut scope = Scope::default();
                let this_table = ThisTableEnum::Temp {
                    temp: TempTable {
                        columns: Arc::new(
                            create_table
                                .columns
                                .iter()
                                .map(|c| TempTableColumn {
                                    name: c.name.value.clone(),
                                })
                                .collect(),
                        ),
                    },
                };
                scope.this_table = Some(this_table);
                let _guard = self.scopes.push(scope);
                for c in create_table.columns.iter() {
                    if self.in_pos(&c.name.span) {
                        self.set_on_def();
                        return;
                    }
                    for op in c.options.iter() {
                        Box::pin(self.c_on_column_option_def(op)).await;
                        if self.finished() {
                            return;
                        }
                    }
                }
                for c in create_table.constraints.iter() {
                    match c {
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
                            }
                            if let Some(name) = index_name {
                                if self.in_pos(&name.span) {
                                    self.set_on_def();
                                    return;
                                }
                            }
                            if columns.iter().any(|x| self.in_pos(&x.span)) {
                                // hit column
                                self.result = Some(
                                    columns
                                        .iter()
                                        .map(|x| CompletionItem {
                                            item: x.value.clone().into(),
                                            kind: CompletionKind::Column,
                                            desc: None,
                                        })
                                        .collect(),
                                );
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
                                    return;
                                }
                            }
                            if columns.iter().any(|x| self.in_pos(&x.span)) {
                                // hit column
                                self.result = Some(
                                    columns
                                        .iter()
                                        .map(|x| CompletionItem {
                                            item: x.value.clone().into(),
                                            kind: CompletionKind::Column,
                                            desc: None,
                                        })
                                        .collect(),
                                );
                                return;
                            }
                            self.c_on_table(foreign_table).await;
                            for r in referred_columns.iter() {
                                self.c_on_column(foreign_table, r).await;
                                if self.finished() {
                                    return;
                                }
                            }
                        }
                        TableConstraint::Check { name, expr } => {
                            if let Some(name) = name {
                                if self.in_pos(&name.span) {
                                    return;
                                }
                            }

                            Box::pin(self.c_on_expr(&expr)).await;
                        }
                        TableConstraint::Index { name, columns, .. } => {
                            if let Some(name) = name {
                                if self.in_pos(&name.span) {
                                    return;
                                }
                            }

                            if columns.iter().any(|x| self.in_pos(&x.span)) {
                                // hit column
                                self.result = Some(
                                    columns
                                        .iter()
                                        .map(|x| CompletionItem {
                                            item: x.value.clone().into(),
                                            kind: CompletionKind::Column,
                                            desc: None,
                                        })
                                        .collect(),
                                );
                            }
                        }
                        TableConstraint::FulltextOrSpatial {
                            opt_index_name,
                            columns,
                            ..
                        } => {
                            if let Some(name) = opt_index_name {
                                if self.in_pos(&name.span) {
                                    return;
                                }
                            }

                            if columns.iter().any(|x| self.in_pos(&x.span)) {
                                // hit column
                                self.result = Some(
                                    columns
                                        .iter()
                                        .map(|x| CompletionItem {
                                            item: x.value.clone().into(),
                                            kind: CompletionKind::Column,
                                            desc: None,
                                        })
                                        .collect(),
                                );
                            }
                        }
                    }
                }
                if let Some(query) = &create_table.query {
                    let _ = self.c_on_query(query).await;
                }

                // 其他都可以fallback 到lexer的实现
            }
            Statement::CreateVirtualTable { name, .. } => {
                if self.in_pos(&name.span()) {
                    self.set_on_def();
                    return;
                };
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
                let _guard = self
                    .scopes
                    .push(Scope::from_this_table(create_index.table_name.clone()));
                // using 在 PostgreSQL 中，USING 关键字用于指定索引的类型，例如 B-tree、HASH、GIN 等。示例如下：
                // using 用lexer的自动完成即可

                for c in create_index.columns.iter() {
                    self.c_on_order_by_expr(c).await;
                    if self.finished() {
                        return;
                    }
                }
                // 在 PostgreSQL 中，CREATE INDEX 语句的 INCLUDE 关键字用于在创建索引时指定额外的列，这些列不会用于索引的排序或查找，但会被包含在索引的叶节点中。这种功能主要用于提高查询性能，特别是在需要返回额外列的场景中。
                for i in create_index.include.iter() {
                    self.c_on_column(&create_index.table_name, i).await;
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
            }
            Statement::CreateRole {
                names,
                in_role,
                in_group,
                role,
                user,
                admin,
                ..
            } => {
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
            }
            Statement::CreateSecret { .. } => {
                // duckdb syntax
            }
            Statement::CreatePolicy {
                table_name,
                name,
                using,
                with_check,
                ..
            } => {
                if self.in_pos(&name.span) {
                    self.set_on_def();
                    return;
                }
                self.c_on_table(table_name).await;
                if self.finished() {
                    return;
                }
                let _guard = self.scopes.push(Scope::from_this_table(table_name.clone()));
                if let Some(x) = using {
                    self.c_on_expr(x).await;
                    if self.finished() {
                        return;
                    }
                }
                if let Some(x) = with_check {
                    self.c_on_expr(x).await;
                    if self.finished() {
                        return;
                    }
                }
            }
            Statement::AlterTable {
                name, operations, ..
            } => {
                self.c_on_alert_table(name, operations).await;
            }

            Statement::AlterIndex { name, operation } => {
                self.c_on_index(name).await;
                if self.finished() {
                    return;
                }
                match operation {
                    AlterIndexOperation::RenameIndex { index_name } => {
                        if self.in_pos(&index_name.span()) {
                            self.set_on_def();
                            return;
                        }
                    }
                }
            }
            Statement::AlterView { name, query, .. } => {
                self.c_on_view(name).await;
                if self.finished() {
                    return;
                }
                let _ = self.c_on_query(&query).await;
            }
            Statement::AlterRole { name, operation } => {
                self.c_on_role(name).await;
                match operation {
                    AlterRoleOperation::RenameRole { role_name } => {
                        if self.in_pos(&role_name.span) {
                            self.set_on_def();
                            return;
                        }
                    }
                    AlterRoleOperation::AddMember { member_name } => {
                        self.c_on_role(member_name).await;
                    }
                    AlterRoleOperation::DropMember { member_name } => {
                        self.c_on_role(member_name).await;
                    }
                    AlterRoleOperation::WithOptions { options: _ } => {
                        // even through have expr
                        // I think they are literal.
                    }
                    AlterRoleOperation::Set { .. } => {
                        // TODO what is this.
                    }
                    AlterRoleOperation::Reset { .. } => {
                        // TODO what is this.
                    }
                }
            }
            Statement::AlterPolicy {
                table_name, name, ..
            } => {
                self.c_on_table(table_name).await;
                if self.finished() {
                    return;
                }
                self.c_on_policy(table_name, name).await;
            }
            Statement::AttachDatabase { .. } => {
                // fallback to lexer
                // this SQLite-specific
            }
            Statement::AttachDuckDBDatabase { .. } => {
                // duckdb not supported
            }
            Statement::DetachDuckDBDatabase { .. } => {
                // duckdb not supported
            }
            Statement::Drop {
                object_type, names, ..
            } => {
                for name in names.iter() {
                    self.c_on_type_drop(object_type.clone(), name).await;
                    if self.finished() {
                        return;
                    }
                }
            }

            Statement::DropFunction { func_desc, .. } => {
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
                // fallback to lexer
            }
            Statement::DropPolicy {
                table_name, name, ..
            } => {
                self.c_on_table(table_name).await;
                if self.finished() {
                    return;
                }
                self.c_on_policy(table_name, name).await;
            }
            Statement::Declare { stmts } => {
                for d in stmts.iter() {
                    for name in d.names.iter() {
                        if self.in_pos(&name.span) {
                            self.set_on_def();
                            return;
                        }
                    }
                    if let Some(expr) = &d.assignment {
                        match expr {
                            DeclareAssignment::Expr(expr) => {
                                self.c_on_expr(&expr).await;
                            }
                            DeclareAssignment::Default(expr) => {
                                self.c_on_expr(&expr).await;
                            }
                            DeclareAssignment::DuckAssignment(expr) => {
                                self.c_on_expr(&expr).await;
                            }
                            DeclareAssignment::For(expr) => {
                                self.c_on_expr(&expr).await;
                            }
                            DeclareAssignment::MsSqlAssignment(expr) => {
                                self.c_on_expr(&expr).await;
                            }
                        }
                    }
                    if let Some(for_query) = &d.for_query {
                        let _ = self.c_on_query(&for_query).await;
                    }
                    self.scopes.enter_declare(Arc::new(d.clone()));
                }
            }

            Statement::CreateExtension { .. } => {
                // fallback to lexer
            }
            Statement::Fetch { .. } => {
                // fallback to lexer
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
            }
            Statement::SetRole { role_name, .. } => {
                // fallback to lexer
                if let Some(name) = role_name {
                    self.c_on_role(name).await;
                }
            }
            Statement::SetVariable { variables, .. } => {
                // fallback to lexer

                for v in variables.iter() {
                    match v.0.len() {
                        1 => self.c_on_variable(&v.0[0]).await,
                        _ => {
                            log::error!("not handled variable name length:{:?}", v);
                        }
                    }
                }
            }
            Statement::SetTimeZone { .. } => {
                // fallback to lexer
            }
            Statement::SetNames { .. } => {
                // this ast have no span info.
                // so we can't do
            }
            Statement::SetNamesDefault {} => {
                // fallback to lexer
            }
            Statement::ShowFunctions { .. } => {
                // Presto-specific
                // no one can write a sql like this???
            }
            Statement::ShowVariable { variable } => {
                for var in variable {
                    self.c_on_variable(var).await;
                    if self.finished() {
                        return;
                    }
                }
            }
            Statement::ShowStatus { .. } => {
                //
            }
            Statement::ShowVariables { .. } => {
                // this is show all variable
                // no need to completion.
            }
            Statement::ShowCreate { obj_type, obj_name } => {
                self.c_on_mysql_show_create(obj_type.clone(), obj_name)
                    .await;
            }
            // a list of show command 应该主要是用在模式匹配的地方呢
            Statement::ShowColumns { .. } => {}
            Statement::ShowDatabases { .. } => {}
            Statement::ShowSchemas { .. } => {}
            Statement::ShowTables { .. } => {}
            Statement::ShowViews { .. } => {}
            Statement::ShowCollation { .. } => {}
            Statement::Use(_x) => {
                // TODO maybe can auto completion.
                // wo perfer use don't use this statment.
            }
            Statement::StartTransaction { .. } => {}
            Statement::SetTransaction { .. } => {}
            Statement::Comment {
                object_type,
                object_name,
                ..
            } => {
                //
                self.c_on_postgres_comment_object(object_type.clone(), object_name)
                    .await;
            }
            Statement::Commit { .. } => {
                //
            }
            Statement::Rollback { .. } => {
                //
            }
            Statement::CreateSchema { schema_name, .. } => {
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
            }
            Statement::CreateDatabase { db_name, .. } => {
                //
                if self.in_pos(&db_name.span()) {
                    self.set_on_def();
                    return;
                }
            }
            Statement::CreateFunction {
                name,
                args,
                function_body,
                ..
            } => {
                if self.in_pos(&name.span()) {
                    self.set_on_def();
                    return;
                }
                let mut vars = HashMap::new();
                if let Some(args) = args {
                    for a in args.iter() {
                        if let Some(name) = &a.name {
                            if self.in_pos(&name.span) {
                                self.set_on_def();
                                return;
                            }
                            vars.insert(name.value.clone(), Var {
                                datatype: a.data_type.clone(),
                            });
                        }

                        if let Some(expr) = &a.default_expr {
                            self.c_on_expr(expr).await;
                            if self.finished() {
                                return;
                            }
                        }
                    }
                }
                let _gurad = self.scopes.push(Scope::from_vars(vars));
                // TODO why just a expr.
                // due to this https://github.com/apache/datafusion-sqlparser-rs/issues/1356
                if let Some(x) = function_body {
                    match x {
                        CreateFunctionBody::AsBeforeOptions(expr) => {
                            self.c_on_expr(expr).await;
                        }
                        CreateFunctionBody::AsAfterOptions(expr) => {
                            self.c_on_expr(expr).await;
                        }
                        CreateFunctionBody::Return(expr) => {
                            self.c_on_expr(expr).await;
                        }
                    }
                }
            }
            Statement::CreateTrigger {
                name,
                table_name,
                events,
                referenced_table_name,
                referencing,
                condition,
                exec_body,
                ..
            } => {
                // fallback to lexer
                if self.in_pos(&name.span()) {
                    self.set_on_def();
                    return;
                }
                self.c_on_table(table_name).await;
                if self.finished() {
                    return;
                }
                for e in events.iter() {
                    match e {
                        TriggerEvent::Update(fields) => {
                            for field in fields.iter() {
                                self.c_on_column(table_name, field).await;
                                if self.finished() {
                                    return;
                                }
                            }
                        }
                        _ => {}
                    }
                }
                if let Some(r) = referenced_table_name {
                    self.c_on_table(r).await;
                    if self.finished() {
                        return;
                    }
                }
                for r in referencing.iter() {
                    // TODO is this actual a table name.
                    self.c_on_table(&r.transition_relation_name).await;
                }
                let mut scope = Scope::default();
                // TODO `NEW` and `OLD` is suitable for all database we
                // support right now????
                scope.enter_table(Ident::new("NEW"), table_name.clone());
                scope.enter_table(Ident::new("OLD"), table_name.clone());
                let _gurad = self.scopes.push(scope);
                if let Some(c) = condition {
                    self.c_on_expr(c).await;
                    if self.finished() {
                        return;
                    }
                }
                self.c_on_trigger_exec_body(exec_body).await;
            }

            Statement::DropTrigger {
                trigger_name,
                table_name,
                ..
            } => {
                self.c_on_table(table_name).await;
                if self.finished() {
                    return;
                }
                if trigger_name.0.len() > 1 {
                    log::error!("multi part trigger name:{:?}", trigger_name);
                }
                self.c_on_table_trigger(table_name, &trigger_name.0[0])
                    .await;
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
                    }
                }
                for s in body.iter() {
                    Box::pin(self.completion_on_statement(s)).await;
                    if self.finished() {
                        return;
                    }
                }
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
                    self.c_on_role(g).await;
                    if self.finished() {
                        return;
                    }
                }
                if let Some(granted_by) = granted_by {
                    self.c_on_role(granted_by).await;
                    if self.finished() {
                        return;
                    }
                }
                let mut tables = None;
                match objects {
                    GrantObjects::AllSequencesInSchema { schemas: _schemas } => {}
                    GrantObjects::AllTablesInSchema { schemas: _schemas } => {}
                    GrantObjects::Schemas(_vec) => {}
                    GrantObjects::Sequences(_vec) => {}
                    GrantObjects::Tables(vec) => {
                        for table in vec.iter() {
                            self.c_on_table(table).await;
                            if self.finished() {
                                return;
                            }
                        }
                        tables = Some(vec);
                    }
                }
                if let Some(tables) = tables {
                    match privileges {
                        Privileges::All { .. } => {
                            // nothing here
                        }
                        Privileges::Actions(vec) => {
                            for a in vec.iter() {
                                self.c_on_privileges_action(&tables[..], a).await;
                                if self.finished() {
                                    return;
                                }
                            }
                        }
                    }
                }
            }

            Statement::Deallocate { name, .. } => {
                self.c_on_prepare(&ObjectName(vec![name.clone()])).await;
            }
            Statement::Execute {
                name,
                parameters,
                using,
                ..
            } => {
                //
                self.c_on_prepare(name).await;
                if self.finished() {
                    return;
                }
                for p in parameters.iter() {
                    self.c_on_expr(p).await;
                    if self.finished() {
                        return;
                    }
                }
                for u in using.iter() {
                    self.c_on_expr(u).await;
                    if self.finished() {
                        return;
                    }
                }
            }

            Statement::Prepare {
                name, statement, ..
            } => {
                if self.in_pos(&name.span) {
                    self.set_on_def();
                    return;
                }
                Box::pin(self.completion_on_statement(statement)).await;
            }
            Statement::Kill { .. } => {}
            Statement::ExplainTable { table_name, .. } => {
                self.c_on_table(table_name).await;
            }
            Statement::Explain { statement, .. } => {
                Box::pin(self.completion_on_statement(statement)).await;
            }
            Statement::Savepoint { name } => {
                if self.in_pos(&name.span) {
                    self.set_on_def();
                    return;
                }
            }
            Statement::ReleaseSavepoint { .. } => {
                // 我在postgres和mysql都没找到可以查询savepoint的语句
                // 所以这里不能做
            }
            Statement::Merge { .. } => {
                //Snowflake ... not support rightnow
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
                ..
            } => {
                if self.in_pos(&name.span()) {
                    self.set_on_def();
                    return;
                }
                for op in sequence_options.iter() {
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
            Statement::CreateType {
                name,
                representation,
            } => {
                if self.in_pos(&name.span()) {
                    self.set_on_def();
                    return;
                }
                match representation {
                    UserDefinedTypeRepresentation::Composite { attributes } => {
                        for a in attributes.iter() {
                            if self.in_pos(&a.name.span) {
                                self.set_on_def();
                                return;
                            }
                        }
                    }
                    UserDefinedTypeRepresentation::Enum { labels } => {
                        for a in labels.iter() {
                            if self.in_pos(&a.span) {
                                self.set_on_def();
                                return;
                            }
                        }
                    }
                }
            }
            Statement::Pragma { name, .. } => {
                self.c_on_pragma(name).await;
            }
            Statement::LockTables { tables } => {
                for t in tables.iter() {
                    self.c_on_table(&ObjectName(vec![t.table.clone()])).await;
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
            Statement::Unload { .. } => {
                // not support right now
            }
            Statement::OptimizeTable { .. } => {
                // ClickHouse spec
            }

            Statement::LISTEN { .. } => {}
            Statement::UNLISTEN { .. } => {}
            Statement::NOTIFY { .. } => {}
            Statement::LoadData { .. } => {}
        }
    }
}

#[cfg(test)]
mod tests {
    #[ignore = "this is just a test for me"]
    #[test]
    fn test_sql_statement() {
        let sql = r#"
 
DECLARE
    result INT := 1;
BEGIN
    IF n < 0 THEN
        RETURN NULL;  -- 负数的阶乘未定义
    ELSIF n = 0 THEN
        RETURN 1;  -- 0 的阶乘为 1
    ELSE
        FOR i IN 1..n LOOP
            result := result * i;
        END LOOP;
        RETURN result;
    END IF;
END;
 
        "#;

        let dialect = sqlparser::dialect::PostgreSqlDialect {};
        let p = sqlparser::parser::Parser::new(&dialect);
        let mut p = p.try_with_sql(sql).unwrap();
        let statement = p.parse_statement().unwrap();
        // let s = serde_json::to_string(match &statement {
        //     Statement::Delete(x) => &x.from,
        //     _ => {
        //         unreachable!()
        //     }
        // })
        // .unwrap();
        let s = serde_json::to_string(&statement).unwrap();

        println!("{}", s);
    }
}
