use futures_util::StreamExt;
use mongodb::bson::{doc, Bson, Document};
use mongodb::options::Hint;
use mongodb::results::{
    CreateIndexResult, DeleteResult, InsertManyResult, InsertOneResult, UpdateResult,
};
use mongodb::{IndexModel, SearchIndexModel};

use crate::db::{GetBackendKind, ReadOnlyTrait, SchemaTrait, SqlBuilder};
use crate::language_server::{CompletionItem, CompletionKind, CompletionList, LanguageServer};
use crate::mongo::{MongoDB, MongoFindParameter};
impl SqlBuilder for MongoDB {
    fn get_schema(&self) -> Option<&str> {
        None
    }

    fn put_value_in_sql(&self, _typ: &str, _value: Option<&str>) -> anyhow::Result<String> {
        self.backend_kind().not_support_feature()
    }

    fn gen_show_sql(
        &self,
        _table_name: &str,
        _fields: Option<&[crate::db::Field]>,
        _para: Option<&crate::db::ShowTableParameter>,
    ) -> String {
        "".into()
    }
}

#[async_trait::async_trait]
impl LanguageServer for MongoDB {
    async fn ai_text_completion(
        &self,
        text: &str,
        _line: u32,
        _col: u32,
    ) -> anyhow::Result<CompletionList> {
        let db = self.get_db().await?;
        let tables = db.list_collection_names().await?;
        let mut ret = tables
            .iter()
            .map(|x| CompletionItem {
                item: x.to_string().into(),
                kind: CompletionKind::Table,
                desc: None,
            })
            .collect::<Vec<_>>();
        let text = text.to_lowercase();
        for table in tables.iter() {
            if text.contains(table.to_lowercase().as_str()) {
                let columns = SchemaTrait::show_table_columns(self, table).await?;
                ret.extend(columns.into_iter().map(CompletionItem::from));
            }
        }
        anyhow::Ok(CompletionList::from(ret))
    }

    fn extract_table_select_selection(&self, _sql: &str) -> anyhow::Result<Option<String>> {
        self.backend_kind().not_support_feature()
    }

    fn extract_table_select_sort(&self, _sql: &str) -> anyhow::Result<Option<String>> {
        self.backend_kind().not_support_feature()
    }

    async fn auto_completion(
        &self,
        _query_id: Option<String>,
        _sql: &str,
        _line: u32,
        _col: u32,
        _use_lexer: bool,
    ) -> anyhow::Result<crate::language_server::CompletionList> {
        self.backend_kind().not_support_feature()
    }

    fn is_dangerous_sql(&self, _sql: &str) -> anyhow::Result<bool> {
        self.backend_kind().not_support_feature()
    }
}

// database api
impl MongoDB {
    pub async fn query_db_drop(&self) -> anyhow::Result<()> {
        self.can_write()?;
        let db = self.get_db().await?;
        db.drop().await?;
        anyhow::Ok(())
    }

    pub async fn query_db_list_collection_names(&self) -> anyhow::Result<Vec<String>> {
        let x = self.list_collection_name_by_type(None).await?;
        anyhow::Ok(x)
    }

    pub async fn query_db_run_command(&self, command: Document) -> anyhow::Result<Document> {
        let x = self.get_db().await?;
        let ret = x.run_command(command).await?;
        anyhow::Ok(ret)
    }
}
// collection apis
impl MongoDB {
    pub async fn query_collection_count_docs(&self, col: &str) -> anyhow::Result<u64> {
        let db = self.get_db().await?;
        let c = db
            .collection::<Document>(col)
            .count_documents(doc! {})
            .await?;
        anyhow::Ok(c)
    }

    pub async fn query_collection_create_index(
        &self,
        col: &str,
        keys: Document,
    ) -> anyhow::Result<CreateIndexResult> {
        self.can_write()?;
        let mut index_model = IndexModel::default();
        index_model.keys = keys;

        let db = self.get_db().await?;
        let c = db
            .collection::<Document>(col)
            .create_index(index_model)
            .await?;
        anyhow::Ok(c)
    }

    pub async fn query_collection_create_search_index(
        &self,
        col: &str,
        keys: Document,
        name: Option<String>,
    ) -> anyhow::Result<String> {
        self.can_write()?;
        let mut index_model = SearchIndexModel::default();
        index_model.definition = keys;
        index_model.name = name;
        let db = self.get_db().await?;
        let c = db
            .collection::<Document>(col)
            .create_search_index(index_model)
            .await?;
        anyhow::Ok(c)
    }

    pub async fn query_collection_delete_many(
        &self,
        col: &str,
        query: Document,
    ) -> anyhow::Result<DeleteResult> {
        self.can_write()?;
        let db = self.get_db().await?;
        let c = db.collection::<Document>(col).delete_many(query).await?;
        anyhow::Ok(c)
    }

    pub async fn query_collection_delete_one(
        &self,
        col: &str,
        query: Document,
    ) -> anyhow::Result<DeleteResult> {
        self.can_write()?;
        let db = self.get_db().await?;
        let c = db.collection::<Document>(col).delete_one(query).await?;
        anyhow::Ok(c)
    }

    pub async fn query_collection_distinct(
        &self,
        col: &str,
        query: Document,
        field: &str,
    ) -> anyhow::Result<Vec<Bson>> {
        let db = self.get_db().await?;
        let c = db
            .collection::<Document>(col)
            .distinct(field, query)
            .await?;
        anyhow::Ok(c)
    }

    pub async fn query_collection_drop(&self, col: &str) -> anyhow::Result<()> {
        self.can_write()?;
        let db = self.get_db().await?;
        db.collection::<Document>(col).drop().await?;
        anyhow::Ok(())
    }

    pub async fn query_collection_drop_index(&self, col: &str, index: &str) -> anyhow::Result<()> {
        self.can_write()?;
        let db = self.get_db().await?;
        db.collection::<Document>(col).drop_index(index).await?;
        anyhow::Ok(())
    }

    pub async fn query_collection_drop_indexes(&self, col: &str) -> anyhow::Result<()> {
        self.can_write()?;
        let db = self.get_db().await?;
        db.collection::<Document>(col).drop_indexes().await?;
        anyhow::Ok(())
    }

    pub async fn query_collection_drop_search_index(
        &self,
        col: &str,
        index: &str,
    ) -> anyhow::Result<()> {
        let db = self.get_db().await?;
        db.collection::<Document>(col)
            .drop_search_index(index)
            .await?;
        anyhow::Ok(())
    }

    pub async fn query_collection_find(
        &self,
        col: &str,
        para: MongoFindParameter,
    ) -> anyhow::Result<Vec<Document>> {
        let MongoFindParameter {
            filter,
            hint,
            projection,
            sort,
            min,
            max,
            limit,
            skip,
        } = para;
        log::info!(
            "find parameter project:{:?} filter:{:?} sort:{:?} max:{:?} min:{:?} limit:{:?} skip:{:?}",
            projection,
            filter,
            sort,
            max,
            min,
            limit,
            skip
        );

        let db = self.get_db().await?;
        let col = db.collection::<Document>(col);
        let mut f = col.find(filter);
        if let Some(projection) = projection {
            f = f.projection(projection)
        }
        if let Some(sort) = sort {
            f = f.sort(sort);
        }
        if let Some(max) = max {
            f = f.max(max);
        }
        if let Some(min) = min {
            f = f.min(min);
        }
        if let Some(limit) = limit {
            f = f.limit(limit);
        }
        if let Some(hint) = hint {
            f = f.hint(Hint::Keys(hint));
        }
        if let Some(skip) = skip {
            f = f.skip(skip);
        }
        let mut cursor = f.await?;
        let mut ret = Vec::new();
        while let Some(doc) = cursor.next().await {
            let doc = doc?;
            ret.push(doc);
        }
        anyhow::Ok(ret)
    }

    pub async fn query_collection_find_one(
        &self,
        col: &str,
        para: MongoFindParameter,
    ) -> anyhow::Result<Option<Document>> {
        let MongoFindParameter {
            filter,
            hint,
            projection,
            sort,
            min,
            max,
            skip,
            limit: _,
        } = para;
        log::info!(
            "find parameter project:{:?} filter:{:?} sort:{:?} max:{:?} min:{:?} skip:{:?}",
            projection,
            filter,
            sort,
            max,
            min,
            skip
        );
        let db = self.get_db().await?;
        let col = db.collection::<Document>(col);
        let mut f = col.find_one(filter);
        if let Some(projection) = projection {
            f = f.projection(projection)
        }
        if let Some(sort) = sort {
            f = f.sort(sort);
        }
        if let Some(hint) = hint {
            f = f.hint(Hint::Keys(hint));
        }
        if let Some(max) = max {
            f = f.max(max);
        }
        if let Some(min) = min {
            f = f.min(min);
        }

        if let Some(skip) = skip {
            f = f.skip(skip);
        }
        let ret = f.await?;
        anyhow::Ok(ret)
    }

    pub async fn query_collection_find_one_and_delete(
        &self,
        col: &str,
        filter: Document,
    ) -> anyhow::Result<Option<Document>> {
        self.can_write()?;
        let db = self.get_db().await?;
        let col = db.collection::<Document>(col);
        let f = col.find_one_and_delete(filter);
        let ret = f.await?;
        anyhow::Ok(ret)
    }

    pub async fn query_collection_find_one_and_replace(
        &self,
        col: &str,
        filter: Document,
        replacement: Document,
    ) -> anyhow::Result<Option<Document>> {
        self.can_write()?;
        let db = self.get_db().await?;
        let col = db.collection::<Document>(col);
        let f = col.find_one_and_replace(filter, replacement);
        let ret = f.await?;
        anyhow::Ok(ret)
    }

    pub async fn query_collection_insert_many(
        &self,
        col: &str,
        docs: Vec<Document>,
    ) -> anyhow::Result<InsertManyResult> {
        self.can_write()?;
        let db = self.get_db().await?;
        let col = db.collection::<Document>(col);
        let f = col.insert_many(docs.into_iter());
        let ret = f.await?;
        anyhow::Ok(ret)
    }

    pub async fn query_collection_insert_one(
        &self,
        col: &str,
        doc: Document,
    ) -> anyhow::Result<InsertOneResult> {
        self.can_write()?;
        let db = self.get_db().await?;
        let col = db.collection::<Document>(col);
        let f = col.insert_one(doc);
        let ret = f.await?;
        anyhow::Ok(ret)
    }

    pub async fn query_collection_list_index_names(
        &self,
        col: &str,
    ) -> anyhow::Result<Vec<String>> {
        let db = self.get_db().await?;
        let col = db.collection::<Document>(col);
        let f = col.list_index_names();
        let ret = f.await?;
        anyhow::Ok(ret)
    }

    pub async fn query_collection_list_indexes(
        &self,
        col: &str,
    ) -> anyhow::Result<Vec<IndexModel>> {
        let db = self.get_db().await?;
        let col = db.collection::<Document>(col);
        let f = col.list_indexes();
        let mut cursor = f.await?;
        let mut ret = Vec::new();
        while let Some(item) = cursor.next().await {
            let item = item?;
            ret.push(item);
        }
        anyhow::Ok(ret)
    }

    pub async fn query_collection_list_search_indexes(
        &self,
        col: &str,
    ) -> anyhow::Result<Vec<Document>> {
        let db = self.get_db().await?;
        let col = db.collection::<Document>(col);
        let f = col.list_search_indexes();
        let mut cursor = f.await?;
        let mut ret = Vec::new();
        while let Some(item) = cursor.next().await {
            let item = item?;
            ret.push(item);
        }
        anyhow::Ok(ret)
    }

    pub async fn query_collection_replace_one(
        &self,
        col: &str,
        query: Document,
        replacement: Document,
    ) -> anyhow::Result<UpdateResult> {
        self.can_write()?;
        let db = self.get_db().await?;
        let col = db.collection::<Document>(col);
        let f = col.replace_one(query, replacement);
        let ret = f.await?;
        anyhow::Ok(ret)
    }

    pub async fn query_collection_update_many(
        &self,
        col: &str,
        query: Document,
        replacement: Document,
    ) -> anyhow::Result<UpdateResult> {
        self.can_write()?;
        let db = self.get_db().await?;
        let col = db.collection::<Document>(col);
        let f = col.update_many(query, replacement);
        let ret = f.await?;
        anyhow::Ok(ret)
    }

    pub async fn query_collection_update_one(
        &self,
        col: &str,
        query: Document,
        replacement: Document,
    ) -> anyhow::Result<UpdateResult> {
        self.can_write()?;
        let db = self.get_db().await?;
        let col = db.collection::<Document>(col);
        let f = col.update_one(query, replacement);
        let ret = f.await?;
        anyhow::Ok(ret)
    }
}

#[cfg(test)]
mod tests {
    use crate::mongo::make_test_object;

    #[tokio::test]
    async fn test_xxx() {
        let x = make_test_object();
        let names = x.query_db_list_collection_names().await.unwrap();
        println!("{:?}", names);
    }
    #[ignore = "xxx maybe not exists"]
    #[tokio::test]
    async fn test_list_index_names() {
        let x = make_test_object();
        let names = x.query_collection_list_index_names("xxx").await.unwrap();
        println!("{:?}", names);
    }
}
