use std::{
    collections::HashMap,
    sync::{
        atomic::{AtomicBool, Ordering},
        Arc, Mutex, RwLock,
    },
};

use sqlparser::{
    ast::{
        AlterIndexOperation, CopySource, CreateFunctionBody, DataType, Declare, DeclareAssignment,
        FunctionArg, FunctionArgExpr, FunctionArguments, GrantObjects, ObjectName, SchemaName,
        SelectItem, SequenceOptions, Spanned, Statement, UserDefinedTypeRepresentation,
    },
    tokenizer::{Location, Span},
};

use crate::completion::{CompletionItem, CompletionKind, CompletionOnSynTaxHelper};

pub struct CompletionStament {
    pub(crate) loc: Location,
    pub(crate) result: RwLock<Vec<CompletionItem>>,
    pub(crate) helper: Arc<dyn CompletionOnSynTaxHelper>,
    pub(crate) scopes: Scopes,
    pub(crate) on_def: AtomicBool,
    pub(crate) has_err: AtomicBool,
}

#[derive(Clone)]
pub struct Scopes {
    ss: Arc<Mutex<Vec<Scope>>>,
}

pub(crate) struct ScopesGuard {
    ss: Arc<Mutex<Vec<Scope>>>,
}

impl Drop for ScopesGuard {
    fn drop(&mut self) {
        self.ss.lock().unwrap().pop();
    }
}

impl Scopes {
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
    pub(crate) fn push(&self, s: Scope) -> ScopesGuard {
        self.ss.lock().unwrap().push(s);
        ScopesGuard {
            ss: self.ss.clone(),
        }
    }
    pub(crate) fn inner_first<R>(&self, f: impl Fn(&Scope) -> Option<R>) -> Option<R> {
        for s in self.ss.lock().unwrap().iter().rev() {
            let x = f(s);
            if x.is_some() {
                return x;
            }
        }
        None
    }
    pub(crate) fn find_table_alias(&self, name: &str) -> Option<ObjectName> {
        let name = name.to_owned();
        self.inner_first(|x| x.table_alias.get(&name).map(|x| x.clone()))
    }
    pub(crate) fn this_table(&self) -> Option<ObjectName> {
        self.inner_first(|x| x.this_table.clone())
    }
}

#[derive(Clone, Debug)]
pub struct TempTableColumn {
    pub(crate) name: String,
    // typ: String,
}

#[derive(Clone, Debug)]
pub struct TempTable {
    pub(crate) columns: Vec<TempTableColumn>,
}

#[derive(Clone, Default, Debug)]
pub struct Scope {
    table_alias: HashMap<String, ObjectName>,
    this_table: Option<ObjectName>,
    temp_tables: HashMap<String, TempTable>,
    // https://www.postgresql.org/docs/current/plpgsql-declarations.html
    // declares variables
    declares: HashMap<String, Arc<Declare>>,
    //
    vars: HashMap<String, Var>,
}

#[derive(Clone, Debug)]
pub struct Var {
    datatype: DataType,
}

impl Scope {
    pub(crate) fn enter_declare(&mut self, d: Arc<Declare>) {
        for name in d.names.iter() {
            self.declares.insert(name.value.clone(), d.clone());
        }
    }

    pub(crate) fn from_temp_tables(this_table: HashMap<String, TempTable>) -> Self {
        Self {
            temp_tables: this_table,
            ..Default::default()
        }
    }

    pub(crate) fn from_this_table(this_table: ObjectName) -> Self {
        Self {
            this_table: Some(this_table),
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
    pub(crate) fn set_on_def(&self) {
        self.on_def.store(true, Ordering::Relaxed);
    }
    pub(crate) fn set_has_err(&self) {
        self.has_err.store(true, Ordering::Relaxed);
    }

    pub(crate) fn finished(&self) -> bool {
        self.result.read().unwrap().len() > 0
            || self.on_def.load(Ordering::Relaxed)
            || self.has_err.load(Ordering::Relaxed)
    }

    pub(crate) fn new(loc: Location, helper: Arc<dyn CompletionOnSynTaxHelper>) -> Self {
        Self {
            loc,
            helper,
            result: RwLock::new(vec![]),
            scopes: Scopes::new(),
            on_def: AtomicBool::new(false),
            has_err: AtomicBool::new(false),
        }
    }

    pub(crate) fn in_pos(&self, span: &Span) -> bool {
        (self.loc.line >= span.start.line && self.loc.line <= span.end.line)
            && (self.loc.column >= span.start.column && self.loc.column <= span.end.column)
    }

    pub(crate) async fn completion_on_statement(&self, statement: &Statement) {
        #[allow(unused)]
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
                    self.c_on_query(&query).await;
                    if self.finished() {
                        return;
                    }
                }
                /*
                在 INSERT 语句中使用别名的实际应用场景较少，更多的是在 SELECT 查询中使用。表别名在 INSERT 的上下文中没有实际的功能，但可以在设计更复杂的查询时提高可读性。如果你有特定的用例或需要进一步的例子，请告诉我！ */
                #[allow(unused)]
                &insert.table_alias;

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
                // } no support at all.
            }
            Statement::Directory { .. } => {
                // TODO maybe dump some data to a file.
                //
            }
            Statement::Call(function) => {
                //
                self.c_on_function(&function.name).await;
                // handle args
                match &function.args {
                    FunctionArguments::None => {}
                    FunctionArguments::Subquery(query) => {
                        self.c_on_query(&query);
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
                                            self.c_on_expr(expr);
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
                                            self.c_on_expr(expr);
                                        }
                                        _ => {}
                                    };
                                }
                                FunctionArg::Unnamed(function_arg_expr) => {
                                    match function_arg_expr {
                                        FunctionArgExpr::Expr(expr) => {
                                            self.c_on_expr(expr);
                                        }
                                        _ => {}
                                    }
                                }
                            }
                        }
                    } //
                }
                if let Some(filter) = &function.filter {
                    self.c_on_expr(&filter);
                }
                for x in function.within_group.iter() {
                    self.c_on_order_by_expr(x);
                    if self.finished() {
                        return;
                    }
                }
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
                            self.c_on_column(table_name, &c);
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
            Statement::Close { cursor } => {
                //
            }
            Statement::Update {
                table,
                assignments,
                from,
                selection,
                returning,
                or,
            } => todo!(),
            Statement::Delete(delete) => {
                for t in delete.tables.iter() {
                    self.c_on_table(t).await;
                    if self.finished() {
                        return;
                    }
                }
                self.c_from_table(&delete.from);
                if self.finished() {
                    return;
                }
            }
            Statement::CreateView { query, name, .. } => {
                if self.in_pos(&name.span()) {
                    self.set_on_def();
                    return;
                }
                // name is user defined no need completion.
                self.c_on_query(&query).await;
            }
            Statement::CreateTable(create_table) => {
                if (self.in_pos(&create_table.name.span())) {
                    self.set_on_def();
                    return;
                }
                for c in create_table.columns.iter() {
                    if (self.in_pos(&c.name.span)) {
                        self.set_on_def();
                        return;
                    }
                }
                if let Some(query) = &create_table.query {
                    self.c_on_query(query).await;
                }

                // 大部分都可以fallback 到lexer的实现
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

                // using 在 PostgreSQL 中，USING 关键字用于指定索引的类型，例如 B-tree、HASH、GIN 等。示例如下：
                // using 用lexer的自动完成即可

                for c in create_index.columns.iter() {
                    self.c_on_order_by_expr(c);
                    if self.finished() {
                        return;
                    }
                }
                // 在 PostgreSQL 中，CREATE INDEX 语句的 INCLUDE 关键字用于在创建索引时指定额外的列，这些列不会用于索引的排序或查找，但会被包含在索引的叶节点中。这种功能主要用于提高查询性能，特别是在需要返回额外列的场景中。
                for i in create_index.include.iter() {
                    self.c_on_column(&create_index.table_name, i);
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
            Statement::CreateRole { .. } => {
                // fallback to lexer
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
                self.c_on_query(&query).await;
            }
            Statement::AlterRole { .. } => {
                // fallback to lexer
            }
            Statement::AlterPolicy { table_name, .. } => {
                self.c_on_table(table_name).await;
            }
            Statement::AttachDatabase { .. } => {
                // fallback to lexer
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
                let kind: CompletionKind = object_type.clone().into();
                for name in names.iter() {
                    self.c_on_db_schema(name).await;
                    if self.finished() {
                        return;
                    }
                    let (db, schema, item) = self.extract_db_schme_item(name);
                    if self.last_name_in_pos(name) {
                        let items = self
                            .helper
                            .list_drop_by_type(*object_type, db, schema)
                            .await
                            .map_err(|e| log::error!("list drop error:{:?}", e))
                            .ok()
                            .unwrap_or_default()
                            .into_iter()
                            .map(|x| CompletionItem {
                                item: x.into(),
                                kind: kind.clone(),
                            })
                            .collect::<Vec<_>>();
                        *self.result.write().unwrap() = items;
                    }
                }
            }
            Statement::DropFunction { func_desc, .. } => {
                for d in func_desc.iter() {
                    self.c_on_function(&d.name).await;
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
                // TODO I don't know this is
            }
            Statement::DropPolicy { .. } => {
                // TODO
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
                        self.c_on_query(&for_query).await;
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
            Statement::SetRole { .. } => {
                // fallback to lexer
            }
            Statement::SetVariable {
                local,
                hivevar,
                variables,
                value,
            } => {
                // fallback to lexer
            }
            Statement::SetTimeZone { .. } => {
                // fallback to lexer
            }
            Statement::SetNames { .. } => {
                // fallback to lexer
            }
            Statement::SetNamesDefault {} => {
                // fallback to lexer
            }
            Statement::ShowFunctions { .. } => {
                // fallback to lexer
            }
            Statement::ShowVariable { .. } => {
                // fallback to lexer
            }
            Statement::ShowStatus { .. } => {
                //
            }
            Statement::ShowVariables { .. } => {}
            Statement::ShowCreate { obj_type, obj_name } => {
                self.c_on_db_schema(obj_name).await;
                if self.finished() {
                    return;
                }
                if self.last_name_in_pos(obj_name) {
                    let kind = obj_type.clone().into();
                    // mysql can only have schema same as mysql database
                    let (_, schema, _) = self.extract_db_schme_item(obj_name);

                    let items = self
                        .helper
                        .mysql_list_show_create(obj_type.clone(), schema)
                        .await
                        .map_err(|e| log::error!("mysql list show create error:{:?}", e))
                        .ok()
                        .unwrap_or_default()
                        .into_iter()
                        .map(|x| CompletionItem {
                            item: x.into(),
                            kind,
                        })
                        .collect::<Vec<_>>();
                    *self.result.write().unwrap() = items;
                }
            }
            Statement::ShowColumns { .. } => {}
            Statement::ShowDatabases { .. } => {}
            Statement::ShowSchemas { .. } => {}
            Statement::ShowTables { .. } => {}
            Statement::ShowViews { .. } => {}
            Statement::ShowCollation { .. } => {}
            Statement::Use(_) => {
                // TODO maybe can auto completion.
            }
            Statement::StartTransaction { .. } => {}
            Statement::SetTransaction { .. } => {}
            Statement::Comment {
                object_type,
                object_name,
                ..
            } => {
                //
                let kind: CompletionKind = object_type.clone().into();
                let x = self
                    .helper
                    .postgres_list_comment_object(object_type.clone(), object_name)
                    .await
                    .map_err(|e| log::error!("list comment object error:{:?}", e))
                    .ok()
                    .unwrap_or_default()
                    .into_iter()
                    .map(|x| CompletionItem {
                        item: x.into(),
                        kind,
                    })
                    .collect::<Vec<_>>();
                *self.result.write().unwrap() = x;
            }

            Statement::Commit { chain } => {
                //
            }
            Statement::Rollback { chain, savepoint } => {
                //
            }
            Statement::CreateSchema {
                schema_name,
                if_not_exists,
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
            Statement::CreateTrigger { .. } => {
                // fallback to lexer
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
                self.c_on_table_trigger(table_name, &trigger_name.0[0]);
            }
            Statement::CreateProcedure { .. } => {
                // fallback lexer
            }
            Statement::CreateMacro { .. } => {
                // fallback lexer
            }
            Statement::CreateStage { .. } => {
                // snowflake not support
            }
            Statement::Assert { condition, message } => {
                self.c_on_expr(condition).await;
                // message should be a string.
            }

            Statement::Grant { objects, .. } => match objects {
                GrantObjects::AllSequencesInSchema { schemas } => todo!(),
                GrantObjects::AllTablesInSchema { schemas } => todo!(),
                GrantObjects::Schemas(vec) => todo!(),
                GrantObjects::Sequences(vec) => todo!(),
                GrantObjects::Tables(vec) => todo!(),
            },
            Statement::Revoke { .. } => {
                // fallback to lexer
            }
            Statement::Deallocate { name, prepare } => {
                // fallback to lexer
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
            Statement::ReleaseSavepoint { .. } => {}
            Statement::Merge { .. } => {
                //not support rightnow
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
                    self.c_on_table(&ObjectName(vec![t.table.clone()]));
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
            Statement::Unload { query, to, with } => {
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
