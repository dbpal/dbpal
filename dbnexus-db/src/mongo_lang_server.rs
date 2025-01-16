use bson::{Bson, Document, doc};
use futures_util::StreamExt;
use mongodb::{
    IndexModel, SearchIndexModel,
    results::{CreateIndexResult, DeleteResult, InsertManyResult, InsertOneResult, UpdateResult},
};

use crate::mongo::Mongo;

// database api
impl Mongo {
    pub async fn query_db_drop(&self) -> anyhow::Result<()> {
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
impl Mongo {
    pub async fn query_collection_count_docs(&self, col: &str) -> anyhow::Result<u64> {
        let db = self.get_db().await?;
        let c = db
            .collection::<Document>(col)
            .count_documents(doc! {})
            .await?;
        anyhow::Ok(c)
    }
    pub async fn query_collection_create_index(
        &self, col: &str, keys: Document,
    ) -> anyhow::Result<CreateIndexResult> {
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
        &self, col: &str, keys: Document, name: Option<String>,
    ) -> anyhow::Result<String> {
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
        &self, col: &str, query: Document,
    ) -> anyhow::Result<DeleteResult> {
        let db = self.get_db().await?;
        let c = db.collection::<Document>(col).delete_many(query).await?;
        anyhow::Ok(c)
    }
    pub async fn query_collection_delete_one(
        &self, col: &str, query: Document,
    ) -> anyhow::Result<DeleteResult> {
        let db = self.get_db().await?;
        let c = db.collection::<Document>(col).delete_one(query).await?;
        anyhow::Ok(c)
    }
    pub async fn query_collection_distinct(
        &self, col: &str, query: Document, field: &str,
    ) -> anyhow::Result<Vec<Bson>> {
        let db = self.get_db().await?;
        let c = db
            .collection::<Document>(col)
            .distinct(field, query)
            .await?;
        anyhow::Ok(c)
    }
    pub async fn query_collection_drop(&self, col: &str) -> anyhow::Result<()> {
        let db = self.get_db().await?;
        let c = db.collection::<Document>(col).drop().await?;
        anyhow::Ok(c)
    }
    pub async fn query_collection_drop_index(&self, col: &str, index: &str) -> anyhow::Result<()> {
        let db = self.get_db().await?;
        let c = db.collection::<Document>(col).drop_index(index).await?;
        anyhow::Ok(c)
    }
    pub async fn query_collection_drop_indexes(&self, col: &str) -> anyhow::Result<()> {
        let db = self.get_db().await?;
        let c = db.collection::<Document>(col).drop_indexes().await?;
        anyhow::Ok(c)
    }
    pub async fn query_collection_drop_search_index(
        &self, col: &str, index: &str,
    ) -> anyhow::Result<()> {
        let db = self.get_db().await?;
        let c = db
            .collection::<Document>(col)
            .drop_search_index(index)
            .await?;
        anyhow::Ok(c)
    }

    pub async fn query_collection_find(
        &self, col: &str, projection: Option<Document>, filter: Document, sort: Option<Document>,
        max: Option<Document>, min: Option<Document>, limit: Option<i64>, skip: Option<u64>,
    ) -> anyhow::Result<Vec<Document>> {
        let db = self.get_db().await?;
        let col = db.collection::<Document>(col);
        let mut f = col.find(filter);
        if let Some(projection) = projection {
            f = f.projection(projection)
        }
        if let Some(sort) = sort {
            f = f.sort(sort)
        }
        if let Some(max) = max {
            f = f.max(max)
        }
        if let Some(min) = min {
            f = f.min(min)
        }
        if let Some(limit) = limit {
            f = f.limit(limit)
        }
        if let Some(skip) = skip {
            f = f.skip(skip)
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
        &self, col: &str, projection: Option<Document>, filter: Document, sort: Option<Document>,
        max: Option<Document>, min: Option<Document>, skip: Option<u64>,
    ) -> anyhow::Result<Option<Document>> {
        let db = self.get_db().await?;
        let col = db.collection::<Document>(col);
        let mut f = col.find_one(filter);
        if let Some(projection) = projection {
            f = f.projection(projection)
        }
        if let Some(sort) = sort {
            f = f.sort(sort)
        }
        if let Some(max) = max {
            f = f.max(max)
        }
        if let Some(min) = min {
            f = f.min(min)
        }

        if let Some(skip) = skip {
            f = f.skip(skip)
        }
        let ret = f.await?;
        anyhow::Ok(ret)
    }

    pub async fn query_collection_find_one_and_delete(
        &self, col: &str, filter: Document,
    ) -> anyhow::Result<Option<Document>> {
        let db = self.get_db().await?;
        let col = db.collection::<Document>(col);
        let f = col.find_one_and_delete(filter);
        let ret = f.await?;
        anyhow::Ok(ret)
    }
    pub async fn query_collection_find_one_and_replace(
        &self, col: &str, filter: Document, replacement: Document,
    ) -> anyhow::Result<Option<Document>> {
        let db = self.get_db().await?;
        let col = db.collection::<Document>(col);
        let f = col.find_one_and_replace(filter, replacement);
        let ret = f.await?;
        anyhow::Ok(ret)
    }

    pub async fn query_collection_insert_many(
        &self, col: &str, docs: Vec<Document>,
    ) -> anyhow::Result<InsertManyResult> {
        let db = self.get_db().await?;
        let col = db.collection::<Document>(col);
        let f = col.insert_many(docs.into_iter());
        let ret = f.await?;
        anyhow::Ok(ret)
    }
    pub async fn query_collection_insert_one(
        &self, col: &str, doc: Document,
    ) -> anyhow::Result<InsertOneResult> {
        let db = self.get_db().await?;
        let col = db.collection::<Document>(col);
        let f = col.insert_one(doc);
        let ret = f.await?;
        anyhow::Ok(ret)
    }
    pub async fn query_collection_list_index_names(
        &self, col: &str,
    ) -> anyhow::Result<Vec<String>> {
        let db = self.get_db().await?;
        let col = db.collection::<Document>(col);
        let f = col.list_index_names();
        let ret = f.await?;
        anyhow::Ok(ret)
    }
    pub async fn query_collection_list_indexes(
        &self, col: &str,
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
        &self, col: &str,
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
        &self, col: &str, query: Document, replacement: Document,
    ) -> anyhow::Result<UpdateResult> {
        let db = self.get_db().await?;
        let col = db.collection::<Document>(col);
        let f = col.replace_one(query, replacement);
        let ret = f.await?;
        anyhow::Ok(ret)
    }

    pub async fn query_collection_update_many(
        &self, col: &str, query: Document, replacement: Document,
    ) -> anyhow::Result<UpdateResult> {
        let db = self.get_db().await?;
        let col = db.collection::<Document>(col);
        let f = col.update_many(query, replacement);
        let ret = f.await?;
        anyhow::Ok(ret)
    }

    pub async fn query_collection_update_one(
        &self, col: &str, query: Document, replacement: Document,
    ) -> anyhow::Result<UpdateResult> {
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
    #[tokio::test]
    async fn test_list_index_names() {
        let x = make_test_object();
        let names = x.query_collection_list_index_names("xxx").await.unwrap();
        println!("{:?}", names);
    }
}
