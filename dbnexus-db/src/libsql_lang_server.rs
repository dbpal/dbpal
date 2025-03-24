use std::{borrow::Cow, sync::Arc};

use libsql::params;

use crate::{
    db::SchemaTrait,
    language_server::{
        AstCompletionHelper, CompletionItem, CompletionKind, LangServerCommon, LanguageServer,
        LexerCompletionTrait, try_completion_on_syntax,
    },
    libsql::LibSqlDB,
    sqlite_lang_server::{SQLITE_FN_AND_DESC, SQLITE_KEY_WORDS, SQLITE_TYPES, sqlite_pragma},
};

#[async_trait::async_trait]

impl LanguageServer for LibSqlDB {
    async fn auto_completion(
        &self, sql: &str, line: u32, col: u32, use_lexer: bool,
    ) -> anyhow::Result<Vec<CompletionItem>> {
        let r = try_completion_on_syntax(
            &sql,
            line,
            col,
            sqlparser::dialect::SQLiteDialect {},
            Arc::new(Clone::clone(self)),
        )
        .await;

        match r {
            Ok(xs) => return Ok(xs),
            Err(err) => {
                log::error!("completion on ast failed,err:{:?}", err);
            }
        };
        if !use_lexer {
            log::warn!("disabled use lexer , we can return empty data.");
            return anyhow::Ok(vec![]);
        }
        return LexerCompletionTrait::do_auto_completion(
            self,
            sql,
            &sqlparser::dialect::SQLiteDialect {},
            sqlparser::tokenizer::Location {
                line: line as u64,
                column: col as u64,
            },
        )
        .await;
    }
}

#[async_trait::async_trait]

impl crate::language_server::LexerCompletionTrait for LibSqlDB {
    type Dialect = sqlparser::dialect::SQLiteDialect;

    // https://sqlite.org/lang_keywords.html
    const KEY_WORDS: &'static [&'static str] = SQLITE_KEY_WORDS;
    const TEXTS: &'static [&'static str] = &["true"];
    const TYPES: &'static [&'static str] = SQLITE_TYPES;

    async fn extra_completion_item(&self) -> Vec<CompletionItem> {
        AstCompletionHelper::sqlite_pragma(self)
    }

    async fn list_table_or_view(&self) -> anyhow::Result<Vec<String>> {
        let mut ret: Vec<_> = SchemaTrait::list_table(self)
            .await
            .map(|x| x.into_iter().map(|x| x.name).collect())?;
        ret.extend(
            SchemaTrait::list_view(self)
                .await
                .map(|x| x.into_iter().map(|x| x.name).collect::<Vec<_>>())?
                .into_iter(),
        );
        anyhow::Ok(ret)
    }

    async fn list_function(&self) -> anyhow::Result<Vec<CompletionItem>> {
        AstCompletionHelper::list_function(self, None, None, true).await
    }
}

#[async_trait::async_trait]
impl LangServerCommon for LibSqlDB {}

#[async_trait::async_trait]
impl AstCompletionHelper for LibSqlDB {
    async fn list_view(
        &self, _db: Option<&str>, _schema: Option<&str>,
    ) -> anyhow::Result<Vec<String>> {
        SchemaTrait::list_view(self)
            .await
            .map(|x| x.into_iter().map(|x| x.name).collect())
    }

    async fn list_table(
        &self, _db: Option<&str>, _schema: Option<&str>,
    ) -> anyhow::Result<Vec<String>> {
        SchemaTrait::list_table(self)
            .await
            .map(|x| x.into_iter().map(|x| x.name).collect())
    }

    async fn list_column(
        &self, _db: Option<&str>, _schema: Option<&str>, name: &str,
    ) -> anyhow::Result<Vec<CompletionItem>> {
        let conn = self.get_conn().await?;
        let sql = format!(r#"select * from "{name}" limit 1"#);
        log::info!("sql:{}", sql);
        let mut stmt = conn.prepare(&sql).await?;
        let ret = stmt
            .columns()
            .iter()
            .map(|x| CompletionItem {
                item: x.name().to_string().into(),
                desc: Some(x.decl_type().unwrap_or_default().to_string().into()),
                kind: CompletionKind::Column,
            })
            .collect::<Vec<_>>();
        // check if prepare return the reuslt.
        if ret.len() > 0 {
            return anyhow::Ok(ret);
        }

        let row = stmt.query_row(params!()).await?;
        anyhow::Ok(
            (0..row.column_count())
                .map(|index| CompletionItem {
                    item: row.column_name(index).unwrap_or_default().to_owned().into(),
                    kind: CompletionKind::Column,
                    desc: row
                        .column_type(index)
                        .ok()
                        .map(|x| Cow::Owned(format!("{:?}", x))),
                })
                .collect(),
        )
    }

    async fn list_function(
        &self, _catelog: Option<&str>, _schema: Option<&str>, _include_build_in: bool,
    ) -> anyhow::Result<Vec<CompletionItem>> {
        let vec = SQLITE_FN_AND_DESC
            .iter()
            .map(|x| CompletionItem {
                item: x[0].into(),
                kind: CompletionKind::Function,
                desc: Some(x[1].into()),
            })
            .collect::<Vec<_>>();
        anyhow::Ok(vec)
    }

    async fn list_index(&self, _schema: Option<&str>) -> anyhow::Result<Vec<String>> {
        self.list_index_name_impl().await
    }

    fn sqlite_pragma(&self) -> Vec<CompletionItem> {
        sqlite_pragma()
    }
}

impl LibSqlDB {
    pub(crate) async fn list_index_name_impl(&self) -> anyhow::Result<Vec<String>> {
        let sql = r#"
        SELECT name
            FROM sqlite_master
            WHERE type = 'index';"#;
        log::info!("sql:{}", sql);
        let conn = self.get_conn().await?;
        let mut rows = conn.query(sql, params!()).await?;
        let mut ret = Vec::new();
        while let Some(r) = rows.next().await? {
            let name: String = r.get(0)?;
            ret.push(name);
        }
        anyhow::Ok(ret)
    }
}

#[cfg(test)]
mod tests {
    use crate::db::{QuerySql, SchemaTrait};
    use crate::enable_test_log;
    use crate::language_server::completion_test::test_file_auto_completion;
    use crate::libsql::make_test_object;

    use std::path::Path;

    #[tokio::test(flavor = "multi_thread", worker_threads = 4)]
    async fn test_auto_completion() {
        enable_test_log();

        let db = make_test_object().await;
        let _ = db.drop_table("departments", false).await;
        let _ = db.drop_table("employees", false).await;
        let query_id = db.open_connection().await.unwrap();
        let _ = db
            .query_sql(
                query_id,
                r#"
            -- 测试用到的表格 和建表语句
            -- 创建 departments 表
            CREATE TABLE departments (
                id INTEGER PRIMARY KEY,
                name TEXT NOT NULL
            );

            -- 创建 employees 表
            CREATE TABLE employees (
                id INTEGER PRIMARY KEY,
                name TEXT NOT NULL,
                department_id INTEGER
            );

            -- create view for test
            create view employees_view as select * from employees;
        "#,
            )
            .await;

        let _ = db
            .query_sql(
                query_id,
                r#"
                create index employees_name on employees(name)
        "#,
            )
            .await;

        let _ = db
            .query_sql(
                query_id,
                r#"
        
            insert into departments values (1 , 'DOD');
            insert into employees values(1 , 'yuyang' , 1) ;
        "#,
            )
            .await;

        let manifest_dir = env!("CARGO_MANIFEST_DIR");
        let mut file_path = Path::new(manifest_dir).to_path_buf();
        file_path.push("tests");
        file_path.push("sqlite_auto_completion_test.sql");
        let content = std::fs::read_to_string(file_path.as_path()).unwrap();

        test_file_auto_completion(&content, &db, false).await;
    }

    #[ignore = "xxx"]
    #[tokio::test(flavor = "multi_thread", worker_threads = 4)]
    async fn test_completion_xxx() {
        // enable_test_log();
        let db = make_test_object().await;
        test_file_auto_completion(
            r#"select employees.id  from employees
13 employees ; 19 name ; 32 departments"#,
            &db,
            false,
        )
        .await;
    }
}
