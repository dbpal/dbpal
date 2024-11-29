use std::{
    cell::{Cell, RefCell},
    collections::HashMap,
    sync::Arc,
};

use sqlparser::{
    ast::{CopySource, GrantObjects, ObjectName, Statement},
    tokenizer::{Location, Span},
};

use crate::completion::{CompletionItem, CompletionKind, CompletionOnSynTaxHelper};

pub struct CompletionStament {
    pub(crate) loc: Location,
    pub(crate) result: RefCell<Vec<CompletionItem>>,
    pub(crate) statement: Statement,
    pub(crate) helper: Arc<dyn CompletionOnSynTaxHelper>,
    pub(crate) scopes: Scopes,
    pub(crate) on_def: Cell<bool>,
}

#[derive(Clone, Default)]
pub struct Scopes {
    ss: Arc<RefCell<Vec<Scope>>>,
}
pub(crate) struct ScopesGuard {
    ss: Arc<RefCell<Vec<Scope>>>,
}

impl Drop for ScopesGuard {
    fn drop(&mut self) {
        self.ss.borrow_mut().pop();
    }
}

impl Scopes {
    pub(crate) fn new() -> Self {
        Self::default()
    }
    pub(crate) fn push(&self, s: Scope) -> ScopesGuard {
        self.ss.borrow_mut().push(s);
        ScopesGuard {
            ss: self.ss.clone(),
        }
    }
    pub(crate) fn inner_first<R>(&self, f: impl Fn(&Scope) -> Option<R>) -> Option<R> {
        for s in self.ss.borrow().iter().rev() {
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
}
#[derive(Clone, Default)]
pub struct Scope {
    table_alias: HashMap<String, ObjectName>,
}

impl CompletionStament {
    pub(crate) fn set_on_def(&self) {
        self.on_def.set(true);
    }
    pub(crate) fn oked(&self) -> bool {
        self.result.borrow().len() > 0 || self.on_def.get()
    }
    pub(crate) fn new(
        statement: Statement, loc: Location, helper: Arc<dyn CompletionOnSynTaxHelper>,
    ) -> Self {
        Self {
            loc,
            statement,
            helper,
            result: RefCell::new(vec![]),
            scopes: Scopes::new(),
            on_def: Cell::new(false),
        }
    }

    pub(crate) fn in_pos(&self, span: &Span) -> bool {
        if self.loc.line != span.start.line {
            return false;
        }
        return self.loc.column >= span.start.column && self.loc.column <= span.end.column;
    }

    pub(crate) async fn do_auto_completion(&self) -> anyhow::Result<()> {
        #[allow(unused)]
        match &self.statement {
            Statement::Analyze {
                table_name,
                partitions,
                for_columns,
                columns,
                cache_metadata,
                noscan,
                compute_statistics,
            } => {
                self.c_on_table(table_name).await;
                if self.oked() {
                    return Ok(());
                }
            }
            Statement::Truncate {
                table_names,
                partitions,
                table,
                only,
                identity,
                cascade,
                on_cluster,
            } => {
                for t in table_names.iter() {
                    self.c_on_table(&t.name).await;
                    if self.oked() {
                        return Ok(());
                    }
                }
            }
            Statement::Msck { .. } => {
                // don't know what is this.
            }
            Statement::Query(query) => todo!(),
            Statement::Insert(insert) => {
                self.c_on_table(&insert.table_name).await?;
                if (self.oked()) {
                    return Ok(());
                }
                for c in insert.columns.iter() {
                    self.c_table_column(&insert.table_name, c).await?;
                    if self.oked() {
                        return Ok(());
                    }
                }
            }
            Statement::Install { extension_name } => {
                // no support at all.
            }
            Statement::Load { extension_name } => {
                // no support at all.
            }
            Statement::Directory {
                overwrite,
                local,
                path,
                file_format,
                source,
            } => {
                // TODO
            }
            Statement::Call(function) => {
                //
                self.c_on_function(&function.name).await;
            }
            Statement::Copy {
                source,
                to,
                target,
                options,
                legacy_options,
                values,
            } => match source {
                CopySource::Table {
                    table_name,
                    columns,
                } => {
                    self.c_on_table(table_name).await;
                    if self.oked() {
                        return Ok(());
                    }
                    for c in columns.iter() {
                        self.c_table_column(table_name, &c);
                    }
                }

                CopySource::Query(query) => todo!(),
            },
            Statement::CopyIntoSnowflake {
                into,
                from_stage,
                from_stage_alias,
                stage_params,
                from_transformations,
                files,
                pattern,
                file_format,
                copy_options,
                validation_mode,
            } => {}
            Statement::Close { cursor } => {}
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
                    self.c_on_table(t).await?;
                    if self.oked() {
                        return Ok(());
                    }
                }
                self.c_from_table(&delete.from);
                if self.oked() {
                    return Ok(());
                }
            }
            Statement::CreateView { query, .. } => {
                // name is user defined no need completion.
                self.c_on_query(&query).await;
            }
            Statement::CreateTable(create_table) => {
                // 没有特别需要的完成的
                // 大部分都可以fallback 到lexer的实现
            }
            Statement::CreateVirtualTable {
                name,
                if_not_exists,
                module_name,
                module_args,
            } => {
                // 应该可以用lexer的自动完成
            }
            Statement::CreateIndex(create_index) => {
                self.c_on_table(&create_index.table_name).await;
                if self.oked() {
                    return Ok(());
                }
                // using 在 PostgreSQL 中，USING 关键字用于指定索引的类型，例如 B-tree、HASH、GIN 等。示例如下：
                // using 不需要用lexer的自动完成即可
                for c in create_index.columns.iter() {
                    self.c_order_by_expr(c);
                    if self.oked() {
                        return Ok(());
                    }
                }
                for i in create_index.include.iter() {
                    self.c_table_column(&create_index.table_name, i);
                    if self.oked() {
                        return Ok(());
                    }
                }
                if let Some(expr) = &create_index.predicate {
                    self.c_on_expr(expr).await;
                    if self.oked() {
                        return Ok(());
                    }
                }
            }
            Statement::CreateRole { .. } => {
                // fallback to lexer
            }
            Statement::CreateSecret { .. } => {
                // duckdb syntax
            }
            Statement::CreatePolicy { table_name, .. } => {
                self.c_on_table(table_name).await;
                // other field fallback to lexer.
            }
            Statement::AlterTable { name, .. } => {
                self.c_on_table(name).await;
                if self.oked() {
                    return Ok(());
                }
            }

            Statement::AlterIndex { name, operation } => {
                // TODO alert index应该比较复杂啊
                // 这个ast好像没啥东西
            }
            Statement::AlterView { name, query, .. } => {
                self.c_on_view(name).await;
                if self.oked() {
                    return Ok(());
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
                let items = self
                    .helper
                    .list_drop_by_type(*object_type)
                    .await?
                    .into_iter()
                    .map(|x| CompletionItem {
                        item: x.into(),
                        kind: kind.clone(),
                    })
                    .collect::<Vec<_>>();
                *self.result.borrow_mut() = items;
            }
            Statement::DropFunction { func_desc, .. } => {
                for d in func_desc.iter() {
                    self.c_on_function(&d.name).await;
                    if self.oked() {
                        return Ok(());
                    }
                }
            }

            Statement::DropProcedure { proc_desc, .. } => {
                for d in proc_desc.iter() {
                    self.c_on_function(&d.name).await;
                    if self.oked() {
                        return Ok(());
                    }
                }
            }
            Statement::DropSecret { .. } => {}
            Statement::DropPolicy { .. } => {}
            Statement::Declare { stmts } => {
                // fallback to lexer
            }
            Statement::CreateExtension { .. } => {
                // fallback to lexer
            }
            Statement::Fetch { .. } => {
                // fallback to lexer
            }
            Statement::Flush { .. } => {
                // fallback to lexer
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
                // not supported
            }
            Statement::ShowVariable { .. } => {
                // no support
            }
            Statement::ShowStatus { .. } => {
                //
            }
            Statement::ShowVariables { .. } => {}
            Statement::ShowCreate { obj_type, obj_name } => {
                let kind = obj_type.clone().into();

                let items = self
                    .helper
                    .mysql_list_show_create(obj_type.clone(), obj_name)
                    .await?
                    .into_iter()
                    .map(|x| CompletionItem {
                        item: x.into(),
                        kind,
                    })
                    .collect::<Vec<_>>();
                *self.result.borrow_mut() = items;
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
                    .await?
                    .into_iter()
                    .map(|x| CompletionItem {
                        item: x.into(),
                        kind,
                    })
                    .collect::<Vec<_>>();
                *self.result.borrow_mut() = x;
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
            }
            Statement::CreateDatabase { .. } => {
                //
            }
            Statement::CreateFunction { .. } => {
                // fallback to lexer
            }
            Statement::CreateTrigger { .. } => {
                // fallback to lexer
            }
            Statement::DropTrigger {
                trigger_name,
                table_name,
                ..
            } => {
                self.c_on_table(table_name).await?;
                if self.oked() {
                    return Ok(());
                }
                self.c_on_trigger(trigger_name);
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
            Statement::Revoke {
                privileges,
                objects,
                grantees,
                granted_by,
                cascade,
            } => todo!(),
            Statement::Deallocate { name, prepare } => todo!(),
            Statement::Execute {
                name,
                parameters,
                has_parentheses,
                using,
            } => todo!(),
            Statement::Prepare {
                name,
                data_types,
                statement,
            } => todo!(),
            Statement::Kill { modifier, id } => todo!(),
            Statement::ExplainTable {
                describe_alias,
                hive_format,
                has_table_keyword,
                table_name,
            } => todo!(),
            Statement::Explain {
                describe_alias,
                analyze,
                verbose,
                query_plan,
                statement,
                format,
                options,
            } => todo!(),
            Statement::Savepoint { name } => todo!(),
            Statement::ReleaseSavepoint { name } => todo!(),
            Statement::Merge {
                into,
                table,
                source,
                on,
                clauses,
            } => todo!(),
            Statement::Cache {
                table_flag,
                table_name,
                has_as,
                options,
                query,
            } => todo!(),
            Statement::UNCache {
                table_name,
                if_exists,
            } => todo!(),
            Statement::CreateSequence {
                temporary,
                if_not_exists,
                name,
                data_type,
                sequence_options,
                owned_by,
            } => todo!(),
            Statement::CreateType {
                name,
                representation,
            } => todo!(),
            Statement::Pragma { name, value, is_eq } => todo!(),
            Statement::LockTables { tables } => todo!(),
            Statement::UnlockTables => todo!(),
            Statement::Unload { query, to, with } => todo!(),
            Statement::OptimizeTable {
                name,
                on_cluster,
                partition,
                include_final,
                deduplicate,
            } => todo!(),
            Statement::LISTEN { channel } => todo!(),
            Statement::UNLISTEN { channel } => todo!(),
            Statement::NOTIFY { channel, payload } => todo!(),
            Statement::LoadData {
                local,
                inpath,
                overwrite,
                table_name,
                partitioned,
                table_format,
            } => todo!(),
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use sqlparser::ast::Statement;

    #[test]
    fn test_sql_statement() {
        let sql = r#"
        DELETE orders, customers
            FROM orders , t1
            JOIN customers ON orders.customer_id = customers.id
            WHERE customers.name = 'Bob';
        "#;
        let dialect = sqlparser::dialect::MySqlDialect {};
        let p = sqlparser::parser::Parser::new(&dialect);
        let mut p = p.try_with_sql(sql).unwrap();
        let statement = p.parse_statement().unwrap();
        let s = serde_json::to_string(match &statement {
            Statement::Delete(x) => &x.from,
            _ => {
                unreachable!()
            }
        })
        .unwrap();
        println!("{}", s);
    }
}
