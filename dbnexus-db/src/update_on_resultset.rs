use dashmap::DashMap;
use sqlparser::ast::*;

use crate::{
    language_server::LangServerCommon, sqlparser_completing::extract_schema_and_item,
    sqlparser_completion::TempTableColumn,
};

impl<'a> GetTableUpdateOnSqlParser<'a> {
    pub async fn extract_from_statement(
        &self,
        s: &Statement,
    ) -> anyhow::Result<Vec<UpdateTableColumn>> {
        match s {
            Statement::Query(x) => self.extract(x).await,
            _ => {
                anyhow::bail!(format!("statement is not query but:{:?}", s))
            }
        }
    }

    async fn extract(&self, ast: &Query) -> anyhow::Result<Vec<UpdateTableColumn>> {
        self.extrat_table_from_query(ast).await;

        todo!()
    }

    pub fn new(helper: &'a dyn LangServerCommon) -> Self {
        Self {
            helper,
            tables: Default::default(),
        }
    }
}

#[derive(Clone, Debug, specta::Type, serde::Serialize, serde::Deserialize)]

pub struct UpdateTableColumn {
    pub table: String,
    pub col: String,
}

#[derive(Clone)]

pub(crate) struct GetTableUpdateOnSqlParser<'a> {
    tables: DashMap<String, TableHas>,
    pub(crate) helper: &'a dyn LangServerCommon,
}
#[derive(Debug, Clone, Default)]
struct TableHas {
    alias: Option<String>,
    cols: Vec<TempTableColumn>,
}

impl<'a> GetTableUpdateOnSqlParser<'a> {
    async fn extrat_table_from_query(&self, ast: &Query) {
        self.extract_table_from_set(&ast.body).await;
    }

    async fn extract_table_from_select(&self, ast: &Select) {
        for w in ast.from.iter() {
            Box::pin(self.extrat_table_from_table_with_joins(w)).await;
        }
    }

    async fn extract_table_from_set(&self, set: &SetExpr) {
        match set {
            SetExpr::Select(select) => {
                Box::pin(self.extract_table_from_select(select.as_ref())).await
            }
            SetExpr::Query(query) => {
                Box::pin(self.extrat_table_from_query(query.as_ref())).await;
            }
            SetExpr::SetOperation { left, right, .. } => {
                Box::pin(self.extract_table_from_set(left.as_ref())).await;
                Box::pin(self.extract_table_from_set(right.as_ref())).await;
            }
            SetExpr::Values(..) => {}
            SetExpr::Insert(..) => {}
            SetExpr::Update(..) => {}
            SetExpr::Delete(..) => {}
            SetExpr::Merge(..) => {}
            SetExpr::Table(table) => {
                if let Some(table_name) = table.table_name.as_ref() {
                    let cols = self
                        .helper
                        .list_column_oked(table.schema_name.as_deref(), table_name, None)
                        .await;
                    self.tables
                        .insert(table_name.to_string(), TableHas { cols, alias: None });
                }
            }
        }
    }

    async fn extrat_table_from_table_with_joins(&self, j: &TableWithJoins) {
        self.extrat_table_from_table_factor(&j.relation).await;
        for j in j.joins.iter() {
            self.extrat_table_from_join(j).await;
        }
    }

    async fn extrat_table_from_join(&self, j: &Join) {
        self.extrat_table_from_table_factor(&j.relation).await;
    }

    async fn extrat_table_from_table_factor(&self, tb: &TableFactor) {
        match tb {
            TableFactor::Table { name, alias, .. } => {
                //
                let (schema_name, table_name) = extract_schema_and_item(name);
                let cols = self
                    .helper
                    .list_column_oked(schema_name.as_deref(), table_name.as_str(), None)
                    .await;
                self.tables.insert(
                    name.to_string(),
                    TableHas {
                        cols,
                        alias: alias.as_ref().map(|x| x.name.value.clone()),
                    },
                );
            }
            TableFactor::Derived { subquery, .. } => {
                self.extrat_table_from_query(subquery.as_ref()).await;
            }
            TableFactor::TableFunction { .. } => {}
            TableFactor::Function { .. } => {}
            TableFactor::UNNEST { .. } => {}
            TableFactor::JsonTable { .. } => {}
            TableFactor::OpenJsonTable { .. } => {}
            TableFactor::NestedJoin { .. } => {
                /*

                -- Nested Join：括号里的整个 JOIN 结果作为一张表
                SELECT *
                FROM (table1 JOIN table2 ON table1.id = table2.id) AS t
                JOIN table3 ON t.id = table3.id;
                */
                // this case too complex,dbpal have no support for this.
            }
            TableFactor::Pivot { .. } => {}
            TableFactor::Unpivot { .. } => {}
            TableFactor::MatchRecognize { .. } => {}
            TableFactor::XmlTable { .. } => {}
            TableFactor::SemanticView { .. } => {}
        }
    }
}
