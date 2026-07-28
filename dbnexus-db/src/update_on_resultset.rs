use dashmap::DashMap;
use sqlparser::ast::*;

use crate::{
    language_server::AstCompletionHelper, sqlparser_completing::extract_schema_and_item,
    sqlparser_completion::TempTableColumn,
};

impl<'a> Extrat<'a> {
    async fn extratct(&self, ast: &Query) {
        self.extrat_table_from_query(ast).await;
    }

    fn new() -> Self {
        unimplemented!()
    }
}
#[derive()]
struct Extrat<'a> {
    tables: DashMap<String, TableHas>,

    pub(crate) helper: &'a dyn AstCompletionHelper,
}

struct TableHas {
    alias: Option<String>,
    cols: Vec<TempTableColumn>,
}

impl<'a> Extrat<'a> {
    async fn extrat_table_from_query(&self, ast: &Query) {}
    async fn extrat_table_from_table_with_joins(&self, j: &TableWithJoins) {
        self.extrat_table_from_table_factor(&j.relation);
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
            TableFactor::NestedJoin { .. } => {}
            TableFactor::Pivot { .. } => {}
            TableFactor::Unpivot { .. } => {}
            TableFactor::MatchRecognize { .. } => {}
            TableFactor::XmlTable { .. } => {}
            TableFactor::SemanticView { .. } => {}
        }
    }
}
