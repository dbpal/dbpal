use crate::language_server::{AstCompletionHelper, CompletionItem, CompletionKind};
use sqlparser::ast::CopyOption;
use sqlparser::ast::{
    AlterIndexOperation, AlterRoleOperation, AssignmentTarget, CaseStatement,
    ConditionalStatementBlock, CopySource, CreateDomain, CreateFunctionBody, DataType, Declare,
    DeclareAssignment, DenyStatement, DropDomain, GrantObjects, Grantee, GranteeName, Ident,
    IfStatement, ObjectName, ObjectNamePart, OneOrManyWithParens, OpenStatement, PrintStatement,
    Privileges, RaiseStatement, RaiseStatementValue, ReturnStatement, ReturnStatementValue,
    SchemaName, SelectItem, SequenceOptions, Set, SetSessionParamKind, Spanned, Statement,
    TableConstraint, TableFactor, TableObject, TriggerEvent, UpdateTableFromKind,
    UserDefinedTypeRepresentation, WhileStatement,
};
use sqlparser::tokenizer::{Location, Span};
use std::collections::HashMap;
use std::ops::DerefMut;
use std::sync::{Arc, Mutex};

pub struct CompletionStatement {
    pub(crate) loc: Location,
    pub(crate) result: Option<Vec<CompletionItem>>,
    pub(crate) helper: Arc<dyn AstCompletionHelper>,
    pub(crate) scopes: Scopes,
    pub(crate) on_def: bool,
}

#[derive(Clone)]
pub struct Scopes {
    ss: Arc<Mutex<Vec<Scope>>>,
}

pub(crate) struct ScopesDropGuard {
    ss: Arc<Mutex<Vec<Scope>>>,
    n: usize,
}

impl ScopesDropGuard {
    pub(crate) fn copy(&self, s: &mut Vec<Scope>) {
        let lock = self.ss.lock().unwrap();
        let length = lock.len();
        for index in (length - self.n)..length {
            s.push(lock.get(index).unwrap().clone());
        }
    }
}

impl Drop for ScopesDropGuard {
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
    pub(crate) fn drop_n(&self, n: usize) -> ScopesDropGuard {
        ScopesDropGuard {
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

    pub(crate) fn tables(&self) -> Vec<TableEntered> {
        let mut ret = vec![];

        let lock = self.ss.lock().unwrap();
        for s in lock.iter().rev() {
            ret.extend(s.tables.clone().into_iter());
        }
        ret
    }

    pub(crate) fn push(&self, scope: Scope) -> ScopesDropGuard {
        self.ss.lock().unwrap().push(scope);
        ScopesDropGuard {
            ss: self.ss.clone(),
            n: 1,
        }
    }

    pub(crate) fn push_scopes(&self, scopes: Vec<Scope>) -> ScopesDropGuard {
        let length = scopes.len();

        self.ss.lock().unwrap().extend(scopes.into_iter());
        ScopesDropGuard {
            ss: self.ss.clone(),
            n: length,
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
    pub(crate) typ: Option<String>,
}

impl From<TempTableColumn> for CompletionItem {
    fn from(value: TempTableColumn) -> Self {
        Self::from(&value)
    }
}
impl From<&TempTableColumn> for CompletionItem {
    fn from(value: &TempTableColumn) -> Self {
        Self {
            item: value.name.to_string().into(),
            kind: CompletionKind::Column,
            desc: value.typ.as_ref().map(|x| x.clone().into()),
        }
    }
}

impl From<&CompletionItem> for TempTableColumn {
    fn from(value: &CompletionItem) -> Self {
        debug_assert!(value.kind == CompletionKind::Column);
        Self {
            name: value.item.to_string(),
            typ: value.desc.as_ref().map(|x| x.to_string()),
        }
    }
}
impl From<CompletionItem> for TempTableColumn {
    fn from(value: CompletionItem) -> Self {
        Self::from(&value)
    }
}

#[derive(Clone, Debug)]
pub struct TempTable {
    pub(crate) columns: Arc<Vec<TempTableColumn>>,
}

#[derive(Clone, Default, Debug)]
pub struct Scope {
    tables: Vec<TableEntered>,
    // https://www.postgresql.org/docs/current/plpgsql-declarations.html

    // right sqlparse have problem to parse function or precedure body
    // `declares` and `vars` not used.
    #[allow(unused)]
    declares: HashMap<String, Arc<Declare>>,
    //
    #[allow(unused)]
    vars: HashMap<String, Var>,
}

#[derive(Clone, Debug)]
pub(crate) enum TableEntered {
    FromTable {
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
            TableEntered::FromTable { alias, table } => {
                if let Some(alias) = alias {
                    Some(alias)
                } else {
                    match table.0.last().unwrap() {
                        ObjectNamePart::Identifier(ident) => Some(ident),
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
                    _ => &x.value == &name.value,
                },
            )
            .unwrap_or(false)
    }
}

#[derive(Clone, Debug)]
pub struct Var {
    #[allow(dead_code)]
    datatype: DataType,
}

impl Scope {
    pub(crate) fn enter_table(&mut self, alias: Option<Ident>, table_name: ObjectName) {
        self.tables.push(TableEntered::FromTable {
            table: table_name,
            alias,
        });
    }

    pub(crate) fn enter_temp_table(&mut self, alias: Option<Ident>, temp: TempTable) {
        self.tables.push(TableEntered::Temp { table: temp, alias });
    }

    pub(crate) fn enter_declare(&mut self, d: Arc<Declare>) {
        for name in d.names.iter() {
            self.declares.insert(name.value.clone(), d.clone());
        }
    }
    pub(crate) fn from_temp_tables(temp_tables: HashMap<Ident, TempTable>) -> Self {
        let mut tables = vec![];
        for (name, temp) in temp_tables {
            tables.push(TableEntered::Temp {
                table: temp,
                alias: Some(name),
            });
        }
        Self {
            tables,
            ..Default::default()
        }
    }
    // we know we are dealing with this table.
    pub(crate) fn from_this_table(this_table: ObjectName) -> Self {
        Self {
            tables: vec![TableEntered::FromTable {
                table: this_table,
                alias: None,
            }],
            ..Default::default()
        }
    }

    pub(crate) fn from_temp_table(this_table: TempTable) -> Self {
        Self {
            tables: vec![TableEntered::Temp {
                table: this_table,
                alias: None,
            }],
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

impl CompletionStatement {
    pub(crate) fn new(loc: Location, helper: Arc<dyn AstCompletionHelper>) -> Self {
        Self {
            loc,
            helper,
            result: None,
            scopes: Scopes::new(),
            on_def: false,
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
impl CompletionStatement {
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
            Statement::Truncate {
                table_names,
                on_cluster,
                ..
            } => {
                for t in table_names.iter() {
                    self.c_on_table(&t.name).await;
                    if self.finished() {
                        return;
                    }
                }
                if let Some(on_cluster) = on_cluster.as_ref() {
                    self.c_on_cluster(on_cluster).await;
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
                match &insert.table {
                    TableObject::TableName(object_name) => {
                        self.c_on_table(object_name).await;
                        if self.finished() {
                            return;
                        }
                        for c in insert.columns.iter() {
                            self.c_on_column(object_name, c, false).await;
                            if self.finished() {
                                return;
                            }
                        }
                        for c in insert.after_columns.iter() {
                            self.c_on_column(object_name, c, false).await;
                            if self.finished() {
                                return;
                            }
                        }
                    }
                    TableObject::TableFunction(..) => {}
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
                match &insert.table {
                    TableObject::TableName(object_name) => {
                        // 在 SQL 中，特别是在 PostgreSQL 中，RETURNING 关键字用于在 INSERT、UPDATE
                        // 或 DELETE 语句后返回被操作的行的特定列。
                        // 这使得你可以在执行数据修改后立即获取相关信息，例如新插入的记录的 ID。
                        let _gurad = self
                            .scopes
                            .push(Scope::from_this_table(object_name.clone()));
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
                        // 在 SQL 中，特别是在 PostgreSQL 中，RETURNING 关键字用于在 INSERT、UPDATE
                        // 或 DELETE 语句后返回被操作的行的特定列。
                        // 这使得你可以在执行数据修改后立即获取相关信息，例如新插入的记录的 ID。
                        let _gurad = self
                            .scopes
                            .push(Scope::from_this_table(object_name.clone()));
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
                    TableObject::TableFunction(..) => {}
                };
            }
            Statement::Install { extension_name } => {
                self.c_on_duck_extension(&extension_name, Some(false)).await;
            }
            Statement::Load { extension_name } => {
                self.c_on_duck_extension(&extension_name, Some(true)).await;
            }
            Statement::Directory { source, .. } => {
                let _ = self.c_on_query(&source).await;
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
                        self.c_on_table(table_name).await;
                        if self.finished() {
                            return;
                        }
                        for c in columns.iter() {
                            self.c_on_column(table_name, &c, false).await;
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
                        let columns = self.c_on_query(&query).await.ok().unwrap_or_default();
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
                                                    .map(|x| CompletionItem::from(x))
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
                    joins.extend(
                        match x {
                            UpdateTableFromKind::BeforeSet(items) => items.clone(),
                            UpdateTableFromKind::AfterSet(items) => items.clone(),
                        }
                        .into_iter(),
                    );
                }

                let _guard = self.c_on_table_with_joins(&joins[..]).await;
                let table = match &table.relation {
                    TableFactor::Table { name, .. } => Some(name.clone()),
                    _ => {
                        log::error!("update from table is not a Table but:{}", table);
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
                                        Box::pin(self.c_on_column(
                                            table,
                                            match object_name.0.last().unwrap() {
                                                ObjectNamePart::Identifier(ident) => ident,
                                            },
                                            false,
                                        ))
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
                let this_table = TempTable {
                    columns: Arc::new(
                        create_table
                            .columns
                            .iter()
                            .map(|c| TempTableColumn {
                                name: c.name.value.clone(),
                                typ: Some(c.data_type.to_string()),
                            })
                            .collect(),
                    ),
                };
                scope.enter_temp_table(None, this_table);
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
                                self.c_on_column(foreign_table, r, false).await;
                                if self.finished() {
                                    return;
                                }
                            }
                        }
                        TableConstraint::Check { name, expr, .. } => {
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
                name,
                operations,
                on_cluster,
                ..
            } => {
                self.c_on_alert_table(name, operations).await;
                if self.finished() {
                    return;
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
                        // no completion on this.
                    }
                    AlterRoleOperation::Reset { .. } => {
                        // no completion on this.
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
                // no support on this
            }
            Statement::DetachDuckDBDatabase { .. } => {
                // no support on this
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
            Statement::ShowFunctions { .. } => {
                // Presto-specific
                // no one can write a sql like this???
            }
            Statement::ShowVariable { variable } => {
                let name = self.helper.backend_name();
                if name.postgres_like() {
                    for var in variable {
                        self.c_on_variable(var).await;
                        if self.finished() {
                            return;
                        }
                    }
                }
            }
            Statement::ShowStatus { .. } => {
                //
            }
            Statement::ShowVariables { .. } => {
                // 给我的语法结构不好做自动完成
            }
            Statement::ShowCreate { obj_type, obj_name } => {
                self.c_on_mysql_show_create(obj_type.clone(), obj_name)
                    .await;
            }
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
            Statement::StartTransaction {
                statements,
                exception_statements,
                ..
            } => {
                for s in statements.iter() {
                    Box::pin(self.completion_on_statement(s)).await;
                    if self.finished() {
                        return;
                    }
                }
                if let Some(s) = exception_statements.as_ref() {
                    for s in s.iter() {
                        Box::pin(self.completion_on_statement(s)).await;
                        if self.finished() {
                            return;
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
                self.c_on_postgres_comment_object(object_type.clone(), object_name)
                    .await;
            }
            Statement::Commit { .. } => {
                // fallback to lexer
            }
            Statement::Rollback { .. } => {
                // fallback to lexer
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
            Statement::CreateFunction(create_f) => {
                if self.in_pos(&create_f.name.span()) {
                    self.set_on_def();
                    return;
                }
                let mut vars = HashMap::new();
                if let Some(args) = &create_f.args {
                    for a in args.iter() {
                        if let Some(name) = &a.name {
                            if self.in_pos(&name.span) {
                                self.set_on_def();
                                return;
                            }
                            vars.insert(
                                name.value.clone(),
                                Var {
                                    datatype: a.data_type.clone(),
                                },
                            );
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
                // due to this https://github.com/apache/datafusion-sqlparser-rs/issues/1356
                if let Some(x) = &create_f.function_body {
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
                        CreateFunctionBody::AsBeginEnd(begin_end_statements) => {
                            for s in begin_end_statements.statements.iter() {
                                Box::pin(self.completion_on_statement(s)).await;
                                if self.finished() {
                                    return;
                                }
                            }
                        }
                        CreateFunctionBody::AsReturnExpr(expr) => {
                            self.c_on_expr(expr).await;
                        }
                        CreateFunctionBody::AsReturnSelect(select) => {
                            let mut s = Vec::new();
                            let _ = self.c_on_select(select, &mut s).await;
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
                                self.c_on_column(table_name, field, false).await;
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
                scope.enter_table(Some(Ident::new("NEW")), table_name.clone());
                scope.enter_table(Some(Ident::new("OLD")), table_name.clone());
                let _gurad = self.scopes.push(scope);
                if let Some(c) = condition {
                    self.c_on_expr(c).await;
                    if self.finished() {
                        return;
                    }
                }
                if let Some(exec_body) = exec_body {
                    self.c_on_trigger_exec_body(exec_body).await;
                }
            }
            Statement::DropTrigger {
                trigger_name,
                table_name,
                ..
            } => {
                if let Some(table_name) = table_name {
                    if trigger_name.0.len() > 1 {
                        log::error!("multi part trigger name:{:?}", trigger_name);
                    }
                    self.c_on_table(table_name).await;
                    if self.finished() {
                        return;
                    }
                    self.c_on_table_trigger(
                        table_name,
                        match &trigger_name.0[0] {
                            ObjectNamePart::Identifier(ident) => ident,
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
                    }
                }

                for s in body.statements() {
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
                    if let Some(g) = &g.name {
                        match g {
                            GranteeName::ObjectName(object_name) => {
                                if let Some(name) = object_name.0.first() {
                                    self.c_on_role(match name {
                                        ObjectNamePart::Identifier(ident) => ident,
                                    })
                                    .await;
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
                let mut tables = None as Option<Vec<ObjectName>>;
                if let Some(objects) = objects {
                    self.c_on_grant_objects(objects, &mut tables).await;
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
            Statement::Deallocate { .. } => {}
            Statement::Execute {
                parameters, using, ..
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
                if self.helper.backend_name().mysql_like() {
                    //
                    self.c_on_table(table_name).await;
                    if self.last_name_in_pos(table_name) && table_name.0.len() == 1 {
                        let mut r = self.result.take().unwrap_or_default();
                        let words = self
                            .helper
                            .keywords()
                            .await
                            .map_err(|e| log::error!("get keywords failed,e:{}", e))
                            .unwrap_or_default();

                        r.extend(words.into_iter());

                        self.result = Some(r);
                    }
                }
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
                // Snowflake not support rightnow
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
            Statement::Unload { .. } => {
                // not support right now
            }
            Statement::OptimizeTable { name, .. } => {
                self.c_on_table(&name).await;
            }
            Statement::LISTEN { .. } => {}
            Statement::UNLISTEN { .. } => {}
            Statement::NOTIFY { .. } => {}
            Statement::LoadData { .. } => {}
            Statement::CreateConnector(..) => {}
            Statement::AlterType(..) => {
                // TODO list type for this.
            }
            Statement::AlterConnector { .. } => {}
            Statement::AlterSession { .. } => {}
            Statement::DropConnector { .. } => {}
            Statement::DropExtension { .. } => {}
            Statement::ShowObjects(..) => {}
            Statement::RenameTable(rename_tables) => {
                for rename in rename_tables.iter() {
                    self.c_on_table(&rename.old_name).await;
                    self.c_on_table(&rename.new_name).await;
                }
            }
            Statement::List(..) => {}
            Statement::Remove(..) => {}
            Statement::RaisError {
                message,
                severity,
                state,
                arguments,
                ..
            } => {
                self.c_on_expr(&message).await;
                self.c_on_expr(&severity).await;
                self.c_on_expr(&state).await;
                for ar in arguments.iter() {
                    self.c_on_expr(&ar).await;
                }
            }
            Statement::Set(set) => self.c_on_set(set).await,
            Statement::Case(case_statement) => self.c_on_case(case_statement).await,
            Statement::If(if_statement) => self.c_on_if(if_statement).await,
            Statement::While(while_statement) => self.c_on_while(while_statement).await,
            Statement::Raise(raise_statement) => self.c_on_raise(raise_statement).await,
            Statement::Open(open_statement) => self.c_on_open(open_statement).await,
            Statement::DropDomain(drop_domain) => self.c_on_drop_domain(drop_domain).await,
            Statement::Deny(deny_statement) => self.c_on_deny_statement(deny_statement).await,
            Statement::CreateDomain(create_domain) => self.c_on_create_domain(create_domain).await,
            Statement::Print(print_statement) => self.c_on_print_statement(print_statement).await,
            Statement::Return(return_statement) => {
                self.c_on_return_statement(return_statement).await
            }
        }
    }
}

impl CompletionStatement {
    async fn c_on_grant_objects(
        &mut self, objects: &GrantObjects, tables: &mut Option<Vec<ObjectName>>,
    ) {
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
                *tables = Some(vec.clone());
            }
            GrantObjects::Databases(..) => {}
            GrantObjects::Views(..) => {}
            GrantObjects::Warehouses(..) => {}
            GrantObjects::Integrations(..) => {}
            GrantObjects::ResourceMonitors(..) => {}
            GrantObjects::Users(..) => {}
            GrantObjects::ComputePools(..) => {}
            GrantObjects::Connections(..) => {}
            GrantObjects::FailoverGroup(..) => {}
            GrantObjects::ReplicationGroup(..) => {}
            GrantObjects::ExternalVolumes(..) => {}
        }
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
                    }
                }
                for value in values.iter() {
                    self.c_on_expr(value).await;
                    if self.finished() {
                        return;
                    }
                }
            }
            Set::ParenthesizedAssignments { variables, values } => {
                for var in variables.iter() {
                    for var in var.0.iter() {
                        match var {
                            ObjectNamePart::Identifier(ident) => {
                                self.c_on_variable(ident).await;
                            }
                        }
                    }
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
                        }
                    }

                    self.c_on_expr(&ass.value).await;
                    if self.finished() {
                        return;
                    }
                }
            }
            Set::SetSessionParam(set_session_param_kind) => match set_session_param_kind {
                SetSessionParamKind::Generic(..) => {}
                SetSessionParamKind::IdentityInsert(..) => {}
                SetSessionParamKind::Offsets(..) => {}
                SetSessionParamKind::Statistics(..) => {}
            },
            Set::SetRole { role_name, .. } => {
                if let Some(role_name) = role_name {
                    self.c_on_role(role_name).await;
                }
            }
            Set::SetTimeZone { .. } => {}
            Set::SetNames { .. } => {}
            Set::SetNamesDefault {} => {}
            Set::SetTransaction { .. } => {
                // fallback to lexer
            }
        }
    }

    async fn c_on_conditional_statement_block(&mut self, s: &ConditionalStatementBlock) {
        if let Some(condition) = s.condition.as_ref() {
            self.c_on_expr(condition).await;
        }
        for s in s.statements() {
            Box::pin(self.completion_on_statement(s)).await;
            if self.finished() {
                return;
            }
        }
    }

    async fn c_on_case(&mut self, ast: &CaseStatement) {
        if let Some(match_expr) = ast.match_expr.as_ref() {
            self.c_on_expr(match_expr).await;
        }
        for wb in ast.when_blocks.iter() {
            self.c_on_conditional_statement_block(wb).await;
        }
        if let Some(else_block) = ast.else_block.as_ref() {
            self.c_on_conditional_statement_block(else_block).await
        }
    }

    async fn c_on_if(&mut self, ast: &IfStatement) {
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

    async fn c_on_while(&mut self, ast: &WhileStatement) {
        self.c_on_conditional_statement_block(&ast.while_block)
            .await
    }
    async fn c_on_raise(&mut self, ast: &RaiseStatement) {
        if let Some(value) = ast.value.as_ref() {
            match value {
                RaiseStatementValue::UsingMessage(expr) => self.c_on_expr(expr).await,
                RaiseStatementValue::Expr(expr) => self.c_on_expr(expr).await,
            }
        }
    }

    async fn c_on_open(&mut self, _ast: &OpenStatement) {
        // nothing to do here
    }
    async fn c_on_drop_domain(&mut self, _ast: &DropDomain) {
        // nothing to do here
    }
    async fn c_on_grantee(&mut self, g: &Grantee) {
        if let Some(name) = g.name.as_ref() {
            match name {
                GranteeName::ObjectName(object_name) => {
                    for name in object_name.0.iter() {
                        match name {
                            ObjectNamePart::Identifier(ident) => self.c_on_role(ident).await,
                        }
                    }
                }

                GranteeName::UserHost { .. } => {}
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

    async fn c_on_create_domain(&mut self, _ast: &CreateDomain) {
        // TODO handle this.
    }

    async fn c_on_print_statement(&mut self, ast: &PrintStatement) {
        self.c_on_expr(&ast.message).await;
    }
    async fn c_on_return_statement(&mut self, ast: &ReturnStatement) {
        if let Some(r) = ast.value.as_ref() {
            match r {
                ReturnStatementValue::Expr(expr) => {
                    self.c_on_expr(expr).await;
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    #[ignore = "this is just a test for me"]
    #[test]
    fn test_sql_statement() {
        let sql = r#"
            explain sel
        "#;
        let dialect = sqlparser::dialect::GenericDialect {};
        let p = sqlparser::parser::Parser::new(&dialect);
        let mut p = p.try_with_sql(sql).unwrap();
        let statement = p.parse_statement().unwrap();

        let s = serde_json::to_string(&statement).unwrap();

        println!("{}", s);
    }
}
