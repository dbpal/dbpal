use std::borrow::Cow;
use std::collections::HashMap;
use std::sync::Arc;

use chrono::{DateTime, FixedOffset};
use line_index::{LineCol, TextSize};
use serde::ser::SerializeStruct;
use sqlparser::ast::Statement;
use sqlparser::dialect::Dialect;
use sqlparser::tokenizer::{Location, Token};
use tokio::io::AsyncWriteExt;

/// A db representation of database
#[async_trait::async_trait]
pub trait DBTrait: Sync + Send + QuerySql + 'static {
    fn as_any(&self) -> &dyn std::any::Any;

    async fn list_table(&self) -> anyhow::Result<Vec<ListTable>>;
    async fn show_columns(&self, table_name: &str) -> anyhow::Result<Vec<Field>>;
    async fn show_table(
        &self, table_name: &str, where_str: Option<&str>, order_by: Option<&str>, page: usize,
        page_size: usize,
    ) -> anyhow::Result<TableListResp>;
    async fn show_indexes(&self, table_name: &str) -> anyhow::Result<Vec<Index>>;
    async fn insert(
        &self, table_name: &str, values: HashMap<String, Option<String>>,
    ) -> anyhow::Result<SqlExecuteRet>;
    async fn update(
        &self, table_name: &str, values: HashMap<String, Option<String>>,
        wheres: HashMap<String, Option<String>>,
    ) -> anyhow::Result<SqlExecuteRet>;

    /// format a value make it suitable for insert into sql.
    /// this method current use in `insert_sql` etc.
    fn put_value_in_sql(&self, _f: &Field, _value: Option<&str>) -> anyhow::Result<String> {
        anyhow::bail!("this backend no need impl this")
    }

    async fn truncate_table(&self, table_name: &str) -> anyhow::Result<()>;

    async fn delete(
        &self, _table_name: &str, _wheres: HashMap<String, Option<String>>,
    ) -> anyhow::Result<SqlExecuteRet>;

    async fn list_view(&self) -> anyhow::Result<Vec<ListView>>;
    async fn drop_view(&self, view_name: &str) -> anyhow::Result<()>;
    async fn drop_table(&self, table: &str) -> anyhow::Result<()>;
    async fn show_view(
        &self, view_name: &str, _where_str: Option<&str>, _order_by: Option<&str>, _page: usize,
        _page_size: usize,
    ) -> anyhow::Result<TableListResp>;

    async fn export_table(
        &self, _table_name: &str, _kind: ExportKind, _sender: tokio::sync::mpsc::Sender<String>,
    ) -> anyhow::Result<()> {
        unimplemented!()
    }

    async fn export_table_to_file(
        &self, table_name: &str, filepath: &str, kind: ExportKind,
    ) -> anyhow::Result<()> {
        let (sender, mut receiver) = tokio::sync::mpsc::channel(10);
        let h1 = {
            let x = self.clone();
            let table_name = table_name.to_string();
            let h1 = tokio::spawn(async move { x.export_table(&table_name, kind, sender).await });
            h1
        };
        let h2 = {
            let filepath = filepath.to_string();
            tokio::spawn(async move {
                let mut file = tokio::fs::File::create(&filepath).await?;
                while let Some(r) = receiver.recv().await {
                    file.write_all(r.as_bytes()).await?;
                }
                anyhow::Ok(())
            })
        };
        h1.await??;
        h2.await??;
        anyhow::Ok(())
    }

    fn clone(&self) -> Box<dyn DBTrait>;
    async fn create_table_sql(&self, _table_name: &str) -> anyhow::Result<CreateTableAndIndexSql> {
        unimplemented!()
    }

    async fn create_view_sql(&self, _view_name: &str) -> anyhow::Result<String> {
        unimplemented!()
    }
}

/// create table and all index sql.
#[derive(Clone, serde::Serialize)]
pub struct CreateTableAndIndexSql {
    pub create_table: String,
    pub indexes: Vec<String>,
}
impl dyn DBTrait {
    #[allow(unused)]
    pub async fn export_table_to_string(
        &self, table_name: &str, kind: ExportKind,
    ) -> anyhow::Result<String> {
        let (sender, mut receiver) = tokio::sync::mpsc::channel(10);
        let h1 = {
            let x = self.clone();
            let table_name = table_name.to_string();
            let h1 = tokio::spawn(async move { x.export_table(&table_name, kind, sender).await });
            h1
        };
        let h2 = {
            tokio::spawn(async move {
                let mut s = String::new();
                while let Some(r) = receiver.recv().await {
                    s.push_str(r.as_str());
                }
                anyhow::Ok(s)
            })
        };
        h1.await??;
        h2.await?
    }
}

#[async_trait::async_trait]
pub trait MultiDBTrait: Sync + Send + QuerySql + 'static {
    async fn list_db(&self) -> anyhow::Result<Vec<String>>;
    async fn open_db(&self) -> anyhow::Result<Arc<dyn DBTrait>>;
}

#[async_trait::async_trait]
pub trait QuerySql {
    async fn query_sql(
        &self, _sql: &str,
    ) -> anyhow::Result<Vec<Result<TableListResp, ExeSqlError>>>;
}

#[derive(Clone, Debug, serde::Serialize)]
pub struct ListView {
    pub name: String,
}

impl From<String> for ListView {
    fn from(value: String) -> Self {
        let value = value.into();
        Self { name: value }
    }
}
impl From<&str> for ListView {
    fn from(value: &str) -> Self {
        let value = value.to_string();
        Self { name: value }
    }
}

#[derive(Debug, Clone, serde::Serialize)]
pub struct ListTable {
    pub table: String,
    pub meta: HashMap<String, String>,
}
impl From<String> for ListTable {
    fn from(value: String) -> Self {
        Self {
            table: value,
            meta: Default::default(),
        }
    }
}
impl From<&str> for ListTable {
    fn from(value: &str) -> Self {
        Self {
            table: value.to_string(),
            meta: Default::default(),
        }
    }
}

impl dyn DBTrait {
    pub(crate) fn delete_sql(
        &self, table_name: &str, fs: &Vec<Field>, wheres: &HashMap<String, Option<String>>,
    ) -> anyhow::Result<String> {
        let mut field_by_name = HashMap::new();
        for f in fs.iter() {
            field_by_name.insert(&f.name, f);
        }
        let mut sql = format!("DELETE FROM {table_name} WHERE ");
        {
            let last = wheres.len() - 1;
            for (index, (k, v)) in wheres.iter().enumerate() {
                let f = field_by_name.get(k);
                if f.is_none() {
                    anyhow::bail!("field  '{}' not found", k);
                }
                sql.push_str(
                    format!(
                        "{} = {}",
                        k,
                        self.put_value_in_sql(*f.unwrap(), v.as_ref().map(|x| x.as_str()))?
                    )
                    .as_str(),
                );
                if index != last {
                    sql.push_str(" AND ")
                }
            }
        }
        log::info!("delete sql:{}", sql);
        anyhow::Ok(sql)
    }

    pub(crate) fn update_sql(
        &self, table_name: &str, fs: &Vec<Field>, values: &HashMap<String, Option<String>>,
        wheres: &HashMap<String, Option<String>>,
    ) -> anyhow::Result<String> {
        //

        let mut field_by_name = HashMap::new();
        for f in fs.iter() {
            field_by_name.insert(&f.name, f);
        }
        let mut sql = format!("UPDATE {table_name} SET ");
        {
            let last = values.len() - 1;
            for (index, (k, v)) in values.iter().enumerate() {
                let f = field_by_name.get(k);
                if f.is_none() {
                    anyhow::bail!("field '{}' not found", k);
                }
                sql.push_str(
                    format!(
                        "{} = {}",
                        k,
                        self.put_value_in_sql(*f.unwrap(), v.as_ref().map(|x| x.as_str()))?
                    )
                    .as_str(),
                );
                if index != last {
                    sql.push_str(",")
                }
            }
        }
        sql.push_str(" WHERE ");
        {
            let last = wheres.len() - 1;
            for (index, (k, v)) in wheres.iter().enumerate() {
                let f = field_by_name.get(k);
                if f.is_none() {
                    anyhow::bail!("field  '{}' not found", k);
                }
                sql.push_str(
                    format!(
                        "{} = {}",
                        k,
                        self.put_value_in_sql(*f.unwrap(), v.as_ref().map(|x| x.as_str()))?
                    )
                    .as_str(),
                );
                if index != last {
                    sql.push_str(" ABD ")
                }
            }
        }
        log::info!("update sql:{}", sql);

        anyhow::Ok(sql)
    }

    pub(crate) fn insert_sql(
        &self, table_name: &str, fs: &Vec<Field>, values: &HashMap<String, Option<String>>,
    ) -> anyhow::Result<String> {
        // fields
        let mut sql = format!("INSERT INTO {table_name}(");
        {
            let last = fs.len() - 1;
            for (index, f) in fs.iter().enumerate() {
                if f.is_auto_increment {
                    continue;
                }
                sql.push_str(&f.name);
                if index != last {
                    sql.push_str(",");
                }
            }
            sql.push_str(")");
        }
        // values
        sql.push_str(" VALUES(");
        {
            let last = fs.len() - 1;
            for (index, f) in fs.iter().enumerate() {
                if f.is_auto_increment {
                    continue;
                }
                let value = values
                    .get(&f.name)
                    .map(|x| x.as_ref().map(|x| x.as_str()))
                    .flatten();

                sql.push_str(&self.put_value_in_sql(f, value)?);
                if index != last {
                    sql.push_str(",");
                }
            }
            sql.push_str(")");
        }
        log::info!("insert sql:{}", sql);
        anyhow::Ok(sql)
    }
}

#[derive(Clone, Debug, serde::Serialize)]
pub struct SqlExecuteRet {
    pub sql: String,
    pub changes: u64,
    pub last_insert_row_id: Option<i64>,
    /// most of the database have id int but mongo have string
    pub last_insert_id_string: Option<String>,
}

impl From<(String, u64)> for SqlExecuteRet {
    fn from((sql, changes): (String, u64)) -> Self {
        Self {
            sql,
            changes,
            last_insert_id_string: None,
            last_insert_row_id: None,
        }
    }
}
impl From<u64> for SqlExecuteRet {
    fn from(changes: u64) -> Self {
        Self {
            sql: "".to_string(),
            changes,
            last_insert_id_string: None,
            last_insert_row_id: None,
        }
    }
}
#[derive(Debug, Clone, serde::Serialize)]
pub struct Index {
    pub(crate) name: String,
    pub(crate) kind: IndexKind,
    pub(crate) on: Vec<String>,
}

impl Index {
    pub fn new(name: impl Into<String>, kind: IndexKind, on: Vec<String>) -> Self {
        let name = name.into();
        Self { name, kind, on }
    }
}

#[derive(Debug)]
pub struct ExeSqlError {
    sql: String,
    err: anyhow::Error,
}

impl ExeSqlError {
    pub(crate) fn new(sql: impl Into<String>, err: anyhow::Error) -> Self {
        let sql = sql.into();
        Self { sql, err }
    }
}

impl serde::Serialize for ExeSqlError {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: serde::Serializer,
    {
        let mut state = serializer.serialize_struct("ExeSqlError", 2)?;
        state.serialize_field("sql", &self.sql)?;
        state.serialize_field("err", &self.err.to_string())?;
        state.end()
    }
}

#[cfg(test)]
#[test]
fn test_serde_json_exe_sql_error() {
    use std::vec;

    let s = serde_json::to_string(&Result::Err(ExeSqlError {
        sql: "SELECT * from xxxx".to_string(),
        err: anyhow::Error::msg("a error "),
    }) as &Result<Vec<TableListResp>, ExeSqlError>)
    .unwrap();
    println!("{:?}", s);

    let s = serde_json::to_string(&Result::Ok(vec![TableListResp::row_affected("a sql", 10)])
        as &Result<Vec<TableListResp>, ExeSqlError>)
    .unwrap();
    println!("{:?}", s);
}

#[derive(Clone, serde::Serialize, Copy, Debug)]
pub enum IndexKind {
    Normal = 0,
    Unique = 1,
    PriMary = 2,
}

#[derive(Clone, Debug, serde::Serialize)]
pub struct TableListResp {
    pub(crate) fields: Vec<Field>,
    pub(crate) total: usize,
    pub(crate) data: Vec<Vec<Option<String>>>,
    pub(crate) sql: String,
}

impl TableListResp {
    fn check(&self) {
        for x in self.data.iter() {
            assert_eq!(x.len(), self.fields.len());
        }
    }
}

impl TableListResp {
    pub fn new(
        fields: Vec<Field>, total: usize, data: Vec<Vec<Option<String>>>, sql: impl Into<String>,
    ) -> Self {
        let sql = sql.into();
        let ret = Self {
            fields,
            total,
            sql,
            data,
        };
        ret.check();
        ret
    }

    pub fn result(
        col_name: impl Into<String>, sql: impl Into<String>, result: Vec<Option<String>>,
    ) -> Self {
        let sql = sql.into();
        let col_name = col_name.into();

        Self {
            total: result.len(),
            sql,
            data: vec![result],
            fields: vec![Field {
                name: col_name,
                typ: "".to_string(),
                nullable: false,
                default_value: None,
                is_primary: false,
                is_auto_increment: false,
            }],
        }
    }

    pub fn row_affected(sql: impl Into<String>, affected: u64) -> Self {
        let sql = sql.into();
        Self {
            total: 1,
            sql,
            data: vec![vec![Some(affected.to_string())]],
            fields: vec![Field {
                name: "row affected".to_string(),
                typ: "int".to_string(),
                nullable: false,
                default_value: None,
                is_primary: false,
                is_auto_increment: false,
            }],
        }
    }
}

#[derive(Debug, Clone, serde::Serialize)]
pub struct Field {
    pub name: String,
    pub typ: String,
    pub nullable: bool,
    /// default value of this field
    pub default_value: Option<String>,
    /// is primary key??
    pub is_primary: bool,
    //
    pub is_auto_increment: bool,
}

impl Field {
    pub fn from_name_type(name: impl Into<String>, typ: impl Into<String>) -> Self {
        let name = name.into();
        let typ = typ.into();

        Self {
            name,
            typ,
            nullable: true,
            default_value: None,
            is_auto_increment: false,
            is_primary: false,
        }
    }
}

impl Field {
    pub fn new(
        name: impl Into<String>, typ: impl Into<String>, nullable: bool,
        default_value: Option<String>, is_primary: bool, is_auto_increment: bool,
    ) -> Self {
        let name = name.into();
        let typ = typ.into();
        Self {
            name,
            typ,
            nullable,
            default_value,
            is_primary,
            is_auto_increment,
        }
    }
}

/// front can execute a sequence of sql.
/// this represent a single query and a kind.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SqlSeq {
    pub sql: String,
    pub kind: SqlKind,
}
impl SqlSeq {
    pub fn new(sql: impl Into<String>, kind: SqlKind) -> Self {
        let sql = sql.into();
        Self { sql, kind }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SqlKind {
    Query,
    Execution,
}

/// cut statement into smaller base on statement or semi_colon.
pub fn sql_to_sql_seq(
    sql: &str, dialect: &dyn Dialect, statement_kind_stmt: impl Fn(&Statement) -> SqlKind,
    statement_kind_sql: impl Fn(&str) -> SqlKind,
) -> anyhow::Result<Vec<SqlSeq>> {
    fn get_sql(
        lineindex: &line_index::LineIndex, sql: &str, start: Location, end: Option<Location>,
    ) -> String {
        let start_offset = lineindex
            .offset(LineCol {
                line: (start.line - 1) as u32,
                col: (start.column - 1) as u32,
            })
            .unwrap();

        let start_offset = usize::from(start_offset);

        let end_offset = end
            .map(|x| {
                {
                    lineindex.offset(LineCol {
                        line: (x.line - 1) as u32,
                        col: (x.column - 1) as u32,
                    })
                }
                .unwrap()
            })
            .unwrap_or(TextSize::from(sql.len() as u32));
        let end_offset = usize::from(end_offset);

        let s = sql[start_offset..end_offset].to_string();
        s.trim().to_string()
    }

    /// cut statement into smaller base on statement.
    fn cut_sql_to_each_statement_ast(
        sql: &str, dialect: &dyn Dialect, statement_kind: impl Fn(&Statement) -> SqlKind,
    ) -> anyhow::Result<Vec<SqlSeq>> {
        let lines = line_index::LineIndex::new(sql);
        let parser = sqlparser::parser::Parser::new(dialect);
        let mut parser = parser.try_with_sql(sql)?;
        let mut ret = Vec::new();
        while parser.peek_token().token != Token::EOF {
            while Token::SemiColon == parser.peek_token() {
                parser.next_token();
            }
            if parser.peek_token() == Token::EOF {
                break;
            }
            let start = parser.peek_token();
            let stmt = parser.parse_statement()?;
            let kind = statement_kind(&stmt);
            let sql = get_sql(&lines, sql, start.location, {
                let t = parser.peek_token();
                if t.token == Token::EOF {
                    None
                } else {
                    Some(t.location)
                }
            });
            ret.push(SqlSeq::new(sql, kind));
        }
        anyhow::Ok(ret)
    }
    match cut_sql_to_each_statement_ast(sql, dialect, statement_kind_stmt) {
        Ok(x) => return Ok(x),
        Err(_) => {}
    }
    // cut sql by semicolon
    fn cut_sql_to_each_statement_semi_colon(
        sql: &str, dialect: &dyn Dialect, statement_kind: impl Fn(&str) -> SqlKind,
    ) -> anyhow::Result<Vec<SqlSeq>> {
        let lines = line_index::LineIndex::new(sql);
        let tokens = sqlparser::tokenizer::Tokenizer::new(dialect, sql).tokenize_with_location()?;
        if tokens.len() == 0 {
            anyhow::bail!("empty sql.");
        }

        let mut start = &tokens[0];
        let mut ret = Vec::new();
        for index in 1..tokens.len() {
            let t = &tokens[index];
            if t.token == Token::SemiColon || t.token == Token::EOF {
                let sql = get_sql(
                    &lines,
                    sql,
                    start.location,
                    if t.token == Token::EOF {
                        None
                    } else {
                        Some(t.location)
                    },
                );
                let kind = statement_kind(sql.as_str());
                if sql != ";" && !sql.is_empty() {
                    ret.push(SqlSeq::new(sql, kind));
                }
                if index + 1 < tokens.len() {
                    start = &tokens[index + 1];
                }
            }
        }
        anyhow::Ok(ret)
    }
    cut_sql_to_each_statement_semi_colon(sql, dialect, statement_kind_sql)
}

pub(crate) fn is_statement_absolute_query(s: &Statement) -> bool {
    match s {
        Statement::Query(_) => true,
        Statement::Fetch {
            name: _,
            direction: _,
            into: _,
        } => true,
        Statement::ShowFunctions { filter: _ } => true,
        Statement::ShowVariable { variable: _ } => true,
        Statement::ShowVariables {
            filter: _,
            global: _,
            session: _,
        } => true,
        Statement::ShowCreate {
            obj_type: _,
            obj_name: _,
        } => true,
        Statement::ShowColumns {
            extended: _,
            full: _,
            table_name: _,
            filter: _,
        } => true,
        Statement::ShowTables {
            extended: _,
            full: _,
            db_name: _,
            filter: _,
        } => true,
        Statement::ShowCollation { filter: _ } => true,
        Statement::ExplainTable {
            describe_alias: _,
            table_name: _,
        } => true,
        Statement::Explain {
            describe_alias: _,
            analyze: _,
            verbose: _,
            statement: _,
            format: _,
        } => true,
        _ => false,
    }
}

pub fn format_data_time_fix_offset(x: &DateTime<FixedOffset>) -> String {
    format!("{} {}", x.naive_local(), x.offset())
}

#[derive(Clone, Debug, Copy)]
pub enum ExportKind {
    CSV {
        include_header: bool,
        delimiter: char,
    },
    Json {
        pretty: bool,
    },

    Sql {
        include_table_schema: bool,
    },
}

pub(crate) fn create_csv(ds: &Vec<Vec<Option<String>>>, delimiter: char) -> String {
    let mut s = String::new();
    for d in ds.iter() {
        let mut s2 = String::new();
        let last = d.len() - 1;
        for (index, x) in d.iter().enumerate() {
            if let Some(x) = x {
                s2.push_str(&escape_csv(x, delimiter));
            }

            if index != last {
                s2.push(delimiter);
            }
        }
        s.push_str(s2.as_str());
    }
    s
}

fn escape_csv(s: &str, delimiter: char) -> Cow<'_, str> {
    if s.contains(delimiter) {
        let s = s.replace("\"", "\\\"");
        Cow::Owned(format!("\"{}\"", s))
    } else {
        Cow::Borrowed(s)
    }
}

#[cfg(test)]
#[test]
fn test_escape_csv() {
    assert_eq!(escape_csv("\" aaaa,", ','), "\"\\\" aaaa,\"");
}
