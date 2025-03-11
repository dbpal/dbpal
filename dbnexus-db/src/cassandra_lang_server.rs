use cdrs_tokio::types::IntoRustByIndex;
use cql3_parser::{
    alter_table::AlterTableOperation,
    alter_type::AlterTypeOperation,
    cassandra_ast::{CassandraAST, ParsedStatement},
    cassandra_statement::CassandraStatement,
    common::{
        FQName, Identifier, IdentifierRef, Operand, RelationElement, Resource, Span, WithItem,
    },
    create_index::IndexColumnType,
    insert::InsertValues,
    select::SelectElement,
};

use crate::{
    cassandra::CassandraDB,
    db::DBTrait,
    language_server::{CompletionItem, CompletionKind, LanguageServer, LexerCompletionTrait},
};

#[async_trait::async_trait]
impl LanguageServer for CassandraDB {
    async fn auto_completion(
        &self, _query_id: uuid::Uuid, sql: &str, line: u32, col: u32, use_lexer: bool,
    ) -> anyhow::Result<Vec<CompletionItem>> {
        //
        let r = AstCompletion::completion(sql, line - 1, col - 1, Clone::clone(self)).await;
        match r {
            Ok(x) => return anyhow::Ok(x),
            Err(x) => {
                log::error!("try completion on ast failed:{:?}", x)
            }
        }
        if !use_lexer {
            return anyhow::Ok(vec![]);
        }
        return LexerCompletionTrait::do_auto_completion(
            self,
            sql,
            &sqlparser::dialect::GenericDialect {},
            sqlparser::tokenizer::Location {
                line: line as u64,
                column: col as u64,
            },
        )
        .await;
    }
}

#[async_trait::async_trait]
impl LexerCompletionTrait for CassandraDB {
    type Dialect = sqlparser::dialect::GenericDialect;
    const KEY_WORDS: &'static [&'static str] = KEY_WORDS;
    const TYPES: &'static [&'static str] = TYPES;
    const TEXTS: &'static [&'static str] = &["true", "false"];
    async fn list_function(&self) -> anyhow::Result<Vec<CompletionItem>> {
        let mut ret = Vec::new();

        if let Some(key_space) = self.config.db.as_ref() {
            {
                ret.extend(
                    self.list_aggregate_impl(&key_space)
                        .await?
                        .into_iter()
                        .map(|x| CompletionItem {
                            kind: CompletionKind::Function,
                            desc: x.cassandra_parameters.clone().map(|x| x.join(",").into()),
                            item: x.name.clone().into(),
                        }),
                );
            }
            {
                ret.extend(self.list_udf_impl(&key_space).await?.into_iter().map(|x| {
                    CompletionItem {
                        kind: CompletionKind::Function,
                        desc: x.cassandra_parameters.clone().map(|x| x.join(",").into()),
                        item: x.name.clone().into(),
                    }
                }));
            }
        }

        ret.extend(BUILD_IN_FN.iter().map(|x| CompletionItem {
            item: (*x).into(),
            kind: CompletionKind::Function,
            desc: None,
        }));

        anyhow::Ok(ret)
    }

    async fn list_table_or_view(&self) -> anyhow::Result<Vec<String>> {
        let sql = format!("SELECT table_name FROM system_schema.tables");
        log::info!("sql:{}", sql);
        let rows = self
            .session
            .query(sql)
            .await?
            .response_body()?
            .into_rows()
            .unwrap_or_default();

        let mut ret = Vec::new();

        for row in rows.iter() {
            let name: Option<String> = row.get_by_index(0)?;
            if let Some(name) = name {
                ret.push(name);
            }
        }
        anyhow::Ok(ret)
    }
}

#[derive(Debug, Clone)]
struct AstCompletion {
    line: usize,
    col: usize,
    on_def: bool,
    result: Option<Vec<CompletionItem>>,
    db: CassandraDB,
}

impl AstCompletion {
    async fn on_udf(&mut self, key_space: &str, name: &Identifier) -> anyhow::Result<()> {
        if !self.in_pos(name.span()) {
            return anyhow::Ok(());
        }
        let x = self.db.list_udf_impl(key_space).await?;

        self.result = Some(
            x.into_iter()
                .map(|x| CompletionItem {
                    item: x.name.clone().into(),
                    kind: CompletionKind::Function,
                    desc: x
                        .cassandra_parameters
                        .clone()
                        .map(|x| format!("{:?}", x).into()),
                })
                .collect(),
        );
        anyhow::Ok(())
    }

    async fn completion(
        sql: &str, line: u32, col: u32, db: CassandraDB,
    ) -> anyhow::Result<Vec<CompletionItem>> {
        let mut c = AstCompletion {
            line: line as usize,
            col: col as usize,
            on_def: false,
            result: None,
            db,
        };

        let ast = CassandraAST::new(sql);
        for stmt in ast.statements.iter() {
            log::info!("completion on ast:{:?}", stmt.statement);
            let _ = c.on_statement(stmt).await;
            if c.finished() {
                return anyhow::Ok(c.result());
            }
        }
        anyhow::bail!("no completion found on ast")
    }

    fn need_key_space(&self, key_space: Option<&str>) -> anyhow::Result<String> {
        if let Some(key_space) = key_space {
            return anyhow::Ok(key_space.to_string());
        }
        match &self.db.config.db {
            Some(x) => anyhow::Ok(x.clone()),
            None => Err(anyhow::Error::msg("need provide key space")),
        }
    }

    fn need_key_space_identifier(&self, key_space: Option<&Identifier>) -> anyhow::Result<String> {
        let x = key_space.map(|x| x.to_string());
        self.need_key_space(x.as_ref().map(|x| x.as_str()))
    }
    fn in_pos(&self, span: Option<&Span>) -> bool {
        match span {
            Some(span) => {
                span.start.row == self.line
                    && (self.col >= span.start.column && self.col <= span.end.column)
            }
            None => false,
        }
    }
    fn finished(&self) -> bool {
        self.on_def || self.result.is_some()
    }
    fn result(self) -> Vec<CompletionItem> {
        let Self { result, .. } = self;
        result.unwrap_or_default()
    }

    async fn on_key_space(&mut self, key_space: &Identifier) -> anyhow::Result<()> {
        if !self.in_pos(key_space.span()) {
            return anyhow::Ok(());
        }
        self.result = Some(
            DBTrait::list_schema(&self.db)
                .await?
                .into_iter()
                .map(|x| CompletionItem {
                    item: x.into(),
                    desc: None,
                    kind: CompletionKind::Schema,
                })
                .collect(),
        );
        anyhow::Ok(())
    }

    async fn on_meterialized_view(&mut self, name: &FQName) -> anyhow::Result<()> {
        if let Some(key_space) = name.keyspace.as_ref() {
            self.on_key_space(key_space).await?;
            if self.finished() {
                return anyhow::Ok(());
            }
        }
        if !self.in_pos(name.name.span()) {
            return anyhow::Ok(());
        }
        let key_space = self
            .need_key_space_identifier(name.keyspace.as_ref())
            .map_err(|x| {
                log::error!("need key space:{}", x);
            })
            .unwrap_or_default();
        let mut result: Vec<CompletionItem> = if key_space.is_empty() {
            vec![]
        } else {
            self.db
                .list_materialized_view_impl(&key_space)
                .await?
                .into_iter()
                .map(|x| CompletionItem {
                    item: x.name.into(),
                    desc: None,
                    kind: CompletionKind::View,
                })
                .collect()
        };
        if name.keyspace.is_none() {
            result.extend(self.table_view_key_spaces().await?.into_iter());
        }
        self.result = Some(result);
        anyhow::Ok(())
    }
    async fn on_table_or_view(&mut self, table: &FQName) -> anyhow::Result<()> {
        if let Some(key_space) = table.keyspace.as_ref() {
            self.on_key_space(key_space).await?;
            if self.finished() {
                return anyhow::Ok(());
            }
        }

        if !self.in_pos(table.name.span()) {
            return anyhow::Ok(());
        }

        let key_space = self
            .need_key_space_identifier(table.keyspace.as_ref())
            .map_err(|e| log::error!("{}", e))
            .unwrap_or_default();

        let mut result: Vec<_> = if key_space.is_empty() {
            vec![]
        } else {
            self.db
                .list_table_impl(key_space.as_str())
                .await?
                .into_iter()
                .map(|x| CompletionItem {
                    item: x.name.into(),
                    desc: None,
                    kind: CompletionKind::Table,
                })
                .collect()
        };
        if !key_space.is_empty() {
            result.extend(
                self.db
                    .list_materialized_view_impl(&key_space)
                    .await?
                    .into_iter()
                    .map(|x| CompletionItem {
                        item: x.name.into(),
                        desc: None,
                        kind: CompletionKind::View,
                    }),
            );
        }
        if table.keyspace.is_none() {
            result.extend(self.table_view_key_spaces().await?.into_iter());
        }
        self.result = Some(result);
        anyhow::Ok(())
    }

    async fn table_view_key_spaces(&self) -> anyhow::Result<Vec<CompletionItem>> {
        let ret: Vec<_> = self
            .db
            .list_schema()
            .await?
            .into_iter()
            .map(|x| CompletionItem {
                item: x.into(),
                desc: None,
                kind: CompletionKind::Schema,
            })
            .collect();
        anyhow::Ok(ret)
    }

    async fn on_with_item(&mut self, table: &FQName, w: &WithItem) -> anyhow::Result<()> {
        match w {
            WithItem::Option { .. } => {}
            WithItem::ClusterOrder(order_clause) => {
                self.on_table_or_view_column(&table, &order_clause.name)
                    .await?;
                if self.finished() {
                    return anyhow::Ok(());
                }
            }
            WithItem::ID(_) => {}
            WithItem::CompactStorage => {}
        }
        anyhow::Ok(())
    }
    async fn on_role(&mut self, r: &Identifier) -> anyhow::Result<()> {
        if !self.in_pos(r.span()) {
            return anyhow::Ok(());
        }

        let sql = format!(" SELECT role FROM system_auth.roles;");
        log::info!("sql:{}", sql);
        let rows = self
            .db
            .session
            .query(sql)
            .await?
            .response_body()?
            .into_rows()
            .unwrap_or_default();
        let mut result = Vec::with_capacity(rows.len());
        for row in rows.iter() {
            let name: Option<String> = row.get_by_index(0)?;
            if let Some(name) = name {
                result.push(CompletionItem {
                    item: name.into(),
                    desc: None,
                    kind: CompletionKind::Role,
                });
            } else {
                log::error!("empty role name should not happen")
            }
        }

        self.result = Some(result);
        anyhow::Ok(())
    }
    async fn on_user(&mut self, r: &Identifier) -> anyhow::Result<()> {
        if !self.in_pos(r.span()) {
            return anyhow::Ok(());
        }
        let sql = format!(
            "SELECT role FROM system_auth.roles WHERE is_superuser = false   allow filtering  ;"
        );
        log::info!("sql:{}", sql);

        let rows = self
            .db
            .session
            .query(sql)
            .await?
            .response_body()?
            .into_rows()
            .unwrap_or_default();
        let mut result = Vec::with_capacity(rows.len());
        for row in rows.iter() {
            let name: Option<String> = row.get_by_index(0)?;
            if let Some(name) = name {
                result.push(CompletionItem {
                    item: name.into(),
                    desc: None,
                    kind: CompletionKind::User,
                });
            } else {
                log::error!("empty role name should not happen")
            }
        }

        self.result = Some(result);
        anyhow::Ok(())
    }

    async fn on_table_or_view_column(
        &mut self, table: &FQName, col: &Identifier,
    ) -> anyhow::Result<()> {
        if !self.in_pos(col.span()) {
            return anyhow::Ok(());
        }
        let key_space = self.need_key_space_identifier(table.keyspace.as_ref())?;

        let sql = format!(
            "SELECT column_name 
                            FROM system_schema.columns 
                            WHERE keyspace_name = '{key_space}' 
                            AND table_name = '{}';",
            match table.name.as_ref() {
                IdentifierRef::Quoted(x) => x.to_owned(),
                IdentifierRef::Unquoted(x) => x.to_owned().to_lowercase(),
            }
        );
        log::info!("sql:{}", sql);

        let rows = self
            .db
            .session
            .query(sql)
            .await?
            .response_body()?
            .into_rows()
            .unwrap_or_default();
        let mut result = Vec::with_capacity(rows.len());
        for row in rows.iter() {
            let name: Option<String> = row.get_by_index(0)?;
            if let Some(name) = name {
                result.push(CompletionItem {
                    item: name.into(),
                    desc: None,
                    kind: CompletionKind::Column,
                });
            }
        }
        self.result = Some(result);
        anyhow::Ok(())
    }
    async fn on_type(&mut self, typ: &FQName) -> anyhow::Result<()> {
        if let Some(key_space) = typ.keyspace.as_ref() {
            self.on_key_space(key_space).await?;
            if self.finished() {
                return anyhow::Ok(());
            }
        }

        if !self.in_pos(typ.name.span()) {
            return anyhow::Ok(());
        }

        let key_space = self.need_key_space_identifier(typ.keyspace.as_ref())?;

        let sql = format!(
            r#"
              SELECT type_name 
                FROM system_schema.types 
                WHERE keyspace_name = '{key_space}';"#
        );
        log::info!("sql:{}", sql);
        let rows = self
            .db
            .session
            .query(sql)
            .await?
            .response_body()?
            .into_rows()
            .unwrap_or_default();
        let mut result = Vec::with_capacity(rows.len());
        for row in rows.iter() {
            let name: Option<String> = row.get_by_index(0)?;
            if let Some(name) = name {
                result.push(CompletionItem {
                    item: name.into(),
                    desc: None,
                    kind: CompletionKind::Type,
                });
            }
        }
        self.result = Some(result);
        anyhow::Ok(())
    }
    async fn on_type_field(&mut self, typ: &FQName, field: &Identifier) -> anyhow::Result<()> {
        self.on_type(typ).await?;
        if self.finished() {
            return anyhow::Ok(());
        }
        //
        if !self.in_pos(field.span()) {
            return anyhow::Ok(());
        }
        let body = self
            .db
            .session
            .query(format!("DESCRIBE TYPE {};", typ.name.to_string()))
            .await?
            .response_body()?;

        let rows = body.into_rows().unwrap_or_default();

        if rows.len() != 1 {
            anyhow::bail!("result shold contains one result");
        }

        let def: Option<String> = rows[0].get_by_index(3)?;

        let def = def.unwrap();

        for stmt in CassandraAST::new(&def).statements.iter() {
            match &stmt.statement {
                CassandraStatement::CreateType(x) => {
                    self.result = Some(
                        x.columns
                            .iter()
                            .map(|x| CompletionItem {
                                kind: CompletionKind::Column,
                                item: x.name.to_string().into(),
                                desc: Some(x.data_type.to_string().into()),
                            })
                            .collect(),
                    );
                }
                _ => {
                    log::error!(
                        "should contains only one create type statement. but:{:?}",
                        &stmt
                    )
                }
            }
        }

        anyhow::Ok(())
    }

    async fn on_relation_element(
        &mut self, table: &FQName, r: &RelationElement,
    ) -> anyhow::Result<()> {
        self.on_operand(table, &r.obj).await?;
        self.on_operand(table, &r.value).await?;
        anyhow::Ok(())
    }
    async fn on_aggregate(&mut self, a: &FQName) -> anyhow::Result<()> {
        if let Some(key_space) = a.keyspace.as_ref() {
            self.on_key_space(key_space).await?;
            if self.finished() {
                return anyhow::Ok(());
            }
        }

        if !self.in_pos(a.name.span()) {
            return anyhow::Ok(());
        }
        let key_space = self.need_key_space_identifier(a.keyspace.as_ref())?;

        let sql = format!(
            r#"
        SELECT aggregate_name 
FROM system_schema.aggregates 
WHERE keyspace_name = '{key_space}';"#
        );
        log::info!("sql:{}", sql);
        let rows = self
            .db
            .session
            .query(sql)
            .await?
            .response_body()?
            .into_rows()
            .unwrap_or_default();
        let mut result = Vec::with_capacity(rows.len());
        for row in rows.iter() {
            let name: Option<String> = row.get_by_index(0)?;
            if let Some(name) = name {
                result.push(CompletionItem {
                    item: name.into(),
                    desc: None,
                    kind: CompletionKind::Function,
                });
            }
        }
        self.result = Some(result);
        anyhow::Ok(())
    }
    async fn on_function(&mut self, a: &FQName) -> anyhow::Result<()> {
        if let Some(key_space) = a.keyspace.as_ref() {
            self.on_key_space(key_space).await?;
            if self.finished() {
                return anyhow::Ok(());
            }
        }

        if !self.in_pos(a.name.span()) {
            return anyhow::Ok(());
        }
        let key_space = self.need_key_space_identifier(a.keyspace.as_ref())?;

        let sql = format!(
            r#"
            SELECT function_name 
                FROM system_schema.functions 
                WHERE keyspace_name = '{key_space}'
        "#
        );
        log::info!("sql:{}", sql);

        let rows = self
            .db
            .session
            .query(sql)
            .await?
            .response_body()?
            .into_rows()
            .unwrap_or_default();
        let mut result = Vec::with_capacity(rows.len());
        for row in rows.iter() {
            let name: Option<String> = row.get_by_index(0)?;
            if let Some(name) = name {
                result.push(CompletionItem {
                    item: name.into(),
                    desc: None,
                    kind: CompletionKind::Function,
                });
            }
        }
        self.result = Some(result);
        anyhow::Ok(())
    }
    async fn on_index(&mut self, a: &FQName) -> anyhow::Result<()> {
        if let Some(key_space) = a.keyspace.as_ref() {
            self.on_key_space(key_space).await?;
            if self.finished() {
                return anyhow::Ok(());
            }
        }

        if !self.in_pos(a.name.span()) {
            return anyhow::Ok(());
        }
        let key_space = self.need_key_space_identifier(a.keyspace.as_ref())?;

        let sql = format!(
            r#"
           SELECT index_name 
                FROM system_schema.indexes 
                WHERE keyspace_name = '{key_space}'
        "#
        );
        log::info!("sql:{}", sql);

        let rows = self
            .db
            .session
            .query(sql)
            .await?
            .response_body()?
            .into_rows()
            .unwrap_or_default();
        let mut result = Vec::with_capacity(rows.len());
        for row in rows.iter() {
            let name: Option<String> = row.get_by_index(0)?;
            if let Some(name) = name {
                result.push(CompletionItem {
                    item: name.into(),
                    desc: None,
                    kind: CompletionKind::Function,
                });
            }
        }
        self.result = Some(result);
        anyhow::Ok(())
    }

    async fn on_trigger(&mut self, a: &FQName) -> anyhow::Result<()> {
        if let Some(key_space) = a.keyspace.as_ref() {
            self.on_key_space(key_space).await?;
            if self.finished() {
                return anyhow::Ok(());
            }
        }

        if !self.in_pos(a.name.span()) {
            return anyhow::Ok(());
        }
        let key_space = self.need_key_space_identifier(a.keyspace.as_ref())?;

        let sql = format!(
            r#"
           SELECT trigger_name 
                FROM system_schema.triggers 
                WHERE keyspace_name = '{key_space}'
        "#
        );
        log::info!("sql:{}", sql);

        let rows = self
            .db
            .session
            .query(sql)
            .await?
            .response_body()?
            .into_rows()
            .unwrap_or_default();
        let mut result = Vec::with_capacity(rows.len());
        for row in rows.iter() {
            let name: Option<String> = row.get_by_index(0)?;
            if let Some(name) = name {
                result.push(CompletionItem {
                    item: name.into(),
                    desc: None,
                    kind: CompletionKind::Function,
                });
            }
        }
        self.result = Some(result);
        anyhow::Ok(())
    }

    async fn on_operand(&mut self, table: &FQName, o: &Operand) -> anyhow::Result<()> {
        match o {
            Operand::Const(_) => {}
            Operand::Map(..) => {}
            Operand::Set(..) => {}
            Operand::List(..) => {}
            Operand::Tuple(operands) => {
                for o in operands.iter() {
                    Box::pin(self.on_operand(table, o)).await?;
                    if self.finished() {
                        return anyhow::Ok(());
                    }
                }
            }
            Operand::Column(identifier) => {
                self.on_table_or_view_column(table, identifier).await?;
            }
            Operand::Func(_) => {}
            Operand::Param(_) => {}
            Operand::Null => {}
        }
        anyhow::Ok(())
    }
    async fn on_resource(&mut self, r: &Resource) -> anyhow::Result<()> {
        match r {
            Resource::AllFunctions(_) => {}
            Resource::AllKeyspaces => {}
            Resource::AllRoles => {}
            Resource::Function(fqname) => {
                self.on_function(fqname).await?;
            }
            Resource::Keyspace(identifier) => {
                self.on_key_space(identifier).await?;
            }
            Resource::Role(_) => {
                // self.on_role(r)
            }
            Resource::Table(fqname) => self.on_table_or_view(fqname).await?,
        }
        anyhow::Ok(())
    }
    async fn on_statement(&mut self, s: &ParsedStatement) -> anyhow::Result<()> {
        match &s.statement {
            CassandraStatement::AlterKeyspace(create_keyspace) => {
                self.on_key_space(&create_keyspace.name).await?;
            }
            CassandraStatement::AlterMaterializedView(alter_materialized_view) => {
                self.on_meterialized_view(&alter_materialized_view.name)
                    .await?;
            }
            CassandraStatement::AlterRole(role_common) => {
                self.on_role(&role_common.name).await?;
            }
            CassandraStatement::AlterTable(alter_table) => {
                self.on_table_or_view(&alter_table.name).await?;
                match &alter_table.operation {
                    AlterTableOperation::Add(column_definitions) => {
                        for d in column_definitions.iter() {
                            if self.in_pos(d.name.span()) {
                                self.on_def = true;
                                return anyhow::Ok(());
                            }
                        }
                    }
                    AlterTableOperation::DropColumns(identifiers) => {
                        for col in identifiers.iter() {
                            self.on_table_or_view_column(&alter_table.name, col).await?;
                            if self.finished() {
                                return anyhow::Ok(());
                            }
                        }
                    }
                    AlterTableOperation::DropCompactStorage => {}
                    AlterTableOperation::Rename(x) => {
                        self.on_table_or_view_column(&alter_table.name, &x.0)
                            .await?;

                        if self.in_pos(x.1.span()) {
                            self.on_def = true;
                            return anyhow::Ok(());
                        }
                    }
                    AlterTableOperation::With(items) => {
                        for item in items.iter() {
                            self.on_with_item(&alter_table.name, item).await?;
                            if self.finished() {
                                return anyhow::Ok(());
                            }
                        }
                    }
                }
            }
            CassandraStatement::AlterType(alter_type) => {
                self.on_type(&alter_type.name).await?;
                match &alter_type.operation {
                    AlterTypeOperation::AlterColumnType(alter_column_type) => {
                        self.on_type_field(&alter_type.name, &alter_column_type.name)
                            .await?;
                    }
                    AlterTypeOperation::Add(column_definitions) => {
                        for col in column_definitions.iter() {
                            if self.in_pos(col.name.span()) {
                                self.on_def = true;
                                return anyhow::Ok(());
                            }
                        }
                    }
                    AlterTypeOperation::Rename(items) => {
                        for (from, to) in items.iter() {
                            self.on_type_field(&alter_type.name, from).await?;
                            if self.finished() {
                                return anyhow::Ok(());
                            }
                            if self.in_pos(to.span()) {
                                self.on_def = true;
                                return anyhow::Ok(());
                            }
                        }
                    }
                }
            }
            CassandraStatement::AlterUser(create_user) => {
                self.on_user(&create_user.name).await?;
            }
            CassandraStatement::ApplyBatch => {}
            CassandraStatement::CreateAggregate(aggregate) => {
                if let Some(key_space) = aggregate.name.keyspace.as_ref() {
                    self.on_key_space(key_space).await?;
                    if self.finished() {
                        return anyhow::Ok(());
                    }
                }
                if self.in_pos(aggregate.name.name.span()) {
                    self.on_def = true;
                    return anyhow::Ok(());
                }
                if let Some(key_space) = aggregate.sfunc.keyspace.as_ref() {
                    self.on_key_space(key_space).await?;
                    if self.finished() {
                        return anyhow::Ok(());
                    }
                }
                if let Some(key_space) = aggregate.finalfunc.keyspace.as_ref() {
                    self.on_key_space(key_space).await?;
                    if self.finished() {
                        return anyhow::Ok(());
                    }
                }
                {
                    let key_space =
                        self.need_key_space_identifier(aggregate.sfunc.keyspace.as_ref())?;

                    self.on_udf(&key_space, &aggregate.sfunc.name).await?;
                }

                {
                    let key_space =
                        self.need_key_space_identifier(aggregate.finalfunc.keyspace.as_ref())?;

                    self.on_udf(&key_space, &aggregate.finalfunc.name).await?;
                }
            }
            CassandraStatement::CreateFunction(create_function) => {
                if let Some(x) = create_function.name.keyspace.as_ref() {
                    self.on_key_space(x).await?;
                    if self.finished() {
                        return anyhow::Ok(());
                    }
                }
                if self.in_pos(create_function.name.name.span()) {
                    self.on_def = true;
                    return anyhow::Ok(());
                }
                for p in create_function.params.iter() {
                    if self.in_pos(p.name.span()) {
                        self.on_def = true;
                        return anyhow::Ok(());
                    }
                }
                // TODO code_block is a string??
            }
            CassandraStatement::CreateIndex(create_index) => {
                if let Some(name) = create_index.name.as_ref() {
                    if self.in_pos(name.span()) {
                        self.on_def = true;
                        return anyhow::Ok(());
                    }
                }
                self.on_table_or_view(&create_index.table).await?;
                if self.finished() {
                    return anyhow::Ok(());
                }
                match &create_index.column {
                    IndexColumnType::Column(identifier)
                    | IndexColumnType::Keys(identifier)
                    | IndexColumnType::Entries(identifier)
                    | IndexColumnType::Full(identifier) => {
                        self.on_table_or_view_column(&create_index.table, identifier)
                            .await?;
                    }
                }
            }
            CassandraStatement::CreateKeyspace(create_keyspace) => {
                if self.in_pos(create_keyspace.name.span()) {
                    self.on_def = true;
                    return anyhow::Ok(());
                }
            }
            CassandraStatement::CreateMaterializedView(create_materialized_view) => {
                if let Some(space) = create_materialized_view.name.keyspace.as_ref() {
                    self.on_key_space(space).await?;
                    if self.finished() {
                        return anyhow::Ok(());
                    }
                }
                if self.in_pos(create_materialized_view.name.name.span()) {
                    self.on_def = true;
                    return anyhow::Ok(());
                }
                self.on_table_or_view(&create_materialized_view.table)
                    .await?;
                if self.finished() {
                    return anyhow::Ok(());
                }
                for col in create_materialized_view
                    .columns
                    .iter()
                    .chain(create_materialized_view.key.partition.iter())
                    .chain(create_materialized_view.key.clustering.iter())
                {
                    self.on_table_or_view_column(&create_materialized_view.table, col)
                        .await?;
                    if self.finished() {
                        return anyhow::Ok(());
                    }
                }
                for r in create_materialized_view.where_clause.iter() {
                    self.on_relation_element(&create_materialized_view.table, r)
                        .await?;
                    if self.finished() {
                        return anyhow::Ok(());
                    }
                }
                for w in create_materialized_view.with_clause.iter() {
                    match w {
                        WithItem::Option { .. } => {}
                        WithItem::ClusterOrder(order_clause) => {
                            if self.in_pos(order_clause.name.span()) {
                                self.result = Some(
                                    create_materialized_view
                                        .columns
                                        .iter()
                                        .map(|x| CompletionItem {
                                            item: x.to_string().into(),
                                            kind: CompletionKind::Column,
                                            desc: None,
                                        })
                                        .collect(),
                                );

                                return anyhow::Ok(());
                            }
                        }
                        WithItem::ID(_) => {}
                        WithItem::CompactStorage => {}
                    }
                }
            }
            CassandraStatement::CreateRole(role_common) => {
                if self.in_pos(role_common.name.span()) {
                    self.on_def = true;
                    return anyhow::Ok(());
                }
            }
            CassandraStatement::CreateTable(create_table) => {
                if let Some(key_space) = create_table.name.keyspace.as_ref() {
                    self.on_key_space(key_space).await?;
                    if self.finished() {
                        return anyhow::Ok(());
                    }
                }
                if self.in_pos(create_table.name.name.span()) {
                    self.on_def = true;
                    return anyhow::Ok(());
                }
                for col in create_table.columns.iter() {
                    if self.in_pos(col.name.span()) {
                        self.on_def = true;
                        return anyhow::Ok(());
                    }
                }
                if let Some(p) = create_table.key.as_ref() {
                    for col in p.clustering.iter().chain(p.partition.iter()) {
                        if self.in_pos(col.span()) {
                            self.result = Some(
                                create_table
                                    .columns
                                    .iter()
                                    .map(|x| CompletionItem {
                                        item: x.name.to_string().into(),
                                        desc: None,
                                        kind: CompletionKind::Column,
                                    })
                                    .collect(),
                            );
                            return anyhow::Ok(());
                        }
                    }
                }
                for w in create_table.with_clause.iter() {
                    match w {
                        WithItem::Option { .. } => {}
                        WithItem::ClusterOrder(order_clause) => {
                            if self.in_pos(order_clause.name.span()) {
                                self.result = Some(
                                    create_table
                                        .columns
                                        .iter()
                                        .map(|x| CompletionItem {
                                            item: x.name.to_string().into(),
                                            desc: None,
                                            kind: CompletionKind::Column,
                                        })
                                        .collect(),
                                );
                                return anyhow::Ok(());
                            }
                        }
                        WithItem::ID(_) => {}
                        WithItem::CompactStorage => {}
                    }
                }
            }
            CassandraStatement::CreateTrigger(create_trigger) => {
                if let Some(key_space) = create_trigger.name.keyspace.as_ref() {
                    self.on_key_space(key_space).await?;
                    if self.finished() {
                        return anyhow::Ok(());
                    }
                }
                if self.in_pos(create_trigger.name.name.span()) {
                    self.on_def = true;
                    return anyhow::Ok(());
                }
            }
            CassandraStatement::CreateType(create_type) => {
                if let Some(key_space) = create_type.name.keyspace.as_ref() {
                    self.on_key_space(key_space).await?;
                    if self.finished() {
                        return anyhow::Ok(());
                    }
                }
                if self.in_pos(create_type.name.name.span()) {
                    self.on_def = true;
                    return anyhow::Ok(());
                }
                for col in create_type.columns.iter() {
                    if self.in_pos(col.name.span()) {
                        self.on_def = true;
                        return anyhow::Ok(());
                    }
                }
            }
            CassandraStatement::CreateUser(create_user) => {
                if self.in_pos(create_user.name.span()) {
                    self.on_def = true;
                    return anyhow::Ok(());
                }
            }
            CassandraStatement::Delete(delete) => {
                self.on_table_or_view(&delete.table_name).await?;
                if self.finished() {
                    return anyhow::Ok(());
                }
                for col in delete.columns.iter() {
                    self.on_table_or_view_column(&delete.table_name, &col.column)
                        .await?;
                    if self.finished() {
                        return anyhow::Ok(());
                    }
                }
                for w in delete.where_clause.iter().chain(delete.if_clause.iter()) {
                    self.on_relation_element(&delete.table_name, w).await?;
                    if self.finished() {
                        return anyhow::Ok(());
                    }
                }
            }
            CassandraStatement::DropAggregate(common_drop) => {
                self.on_aggregate(&common_drop.name).await?;
            }
            CassandraStatement::DropFunction(common_drop) => {
                self.on_function(&common_drop.name).await?;
            }
            CassandraStatement::DropIndex(common_drop) => {
                self.on_index(&common_drop.name).await?;
            }
            CassandraStatement::DropKeyspace(common_drop) => {
                self.on_key_space(&common_drop.name.name).await?;
            }
            CassandraStatement::DropMaterializedView(common_drop) => {
                self.on_meterialized_view(&common_drop.name).await?;
            }
            CassandraStatement::DropRole(common_drop) => {
                self.on_role(&common_drop.name.name).await?;
            }
            CassandraStatement::DropTable(common_drop) => {
                self.on_table_or_view(&common_drop.name).await?;
            }
            CassandraStatement::DropTrigger(drop_trigger) => {
                self.on_trigger(&drop_trigger.name).await?;
                if self.finished() {
                    return anyhow::Ok(());
                }
                self.on_table_or_view(&drop_trigger.table).await?;
            }
            CassandraStatement::DropType(common_drop) => {
                self.on_type(&common_drop.name).await?;
            }
            CassandraStatement::DropUser(common_drop) => {
                self.on_user(&common_drop.name.name).await?;
            }
            CassandraStatement::Grant(privilege) => {
                if let Some(r) = privilege.resource.as_ref() {
                    self.on_resource(r).await?;
                }
            }
            CassandraStatement::Insert(insert) => {
                self.on_table_or_view(&insert.table_name).await?;
                if self.finished() {
                    return anyhow::Ok(());
                }
                for col in insert.columns.iter() {
                    self.on_table_or_view_column(&insert.table_name, col)
                        .await?;
                    if self.finished() {
                        return anyhow::Ok(());
                    }
                }
                match &insert.values {
                    InsertValues::Values(operands) => {
                        for o in operands.iter() {
                            self.on_operand(&insert.table_name, o).await?;
                            if self.finished() {
                                return anyhow::Ok(());
                            }
                        }
                    }
                    InsertValues::Json(_) => {}
                }
            }
            CassandraStatement::ListPermissions(privilege) => {
                if let Some(r) = privilege.resource.as_ref() {
                    self.on_resource(r).await?;
                }
            }
            CassandraStatement::ListRoles(list_role) => {
                if let Some(role) = list_role.of.as_ref() {
                    self.on_role(role).await?;
                }
            }
            CassandraStatement::Revoke(privilege) => {
                if let Some(r) = privilege.resource.as_ref() {
                    self.on_resource(r).await?;
                }
            }
            CassandraStatement::Select(select) => {
                self.on_table_or_view(&select.table_name).await?;
                if self.finished() {
                    return anyhow::Ok(());
                }
                for col in select.columns.iter() {
                    match col {
                        SelectElement::Star => {}
                        SelectElement::Column(named) => {
                            self.on_table_or_view_column(&select.table_name, &named.name)
                                .await?;
                            if self.finished() {
                                return anyhow::Ok(());
                            }
                            if self.in_pos(named.alias.as_ref().map(|x| x.span()).flatten()) {
                                self.on_def = true;
                                return anyhow::Ok(());
                            }
                        }
                        SelectElement::Function(named) => {
                            self.on_table_or_view_column(&select.table_name, &named.name)
                                .await?;
                            if self.finished() {
                                return anyhow::Ok(());
                            }
                            if self.in_pos(named.alias.as_ref().map(|x| x.span()).flatten()) {
                                self.on_def = true;
                                return anyhow::Ok(());
                            }
                        }
                    }
                }
            }
            CassandraStatement::Truncate(fqname) => {
                self.on_table_or_view(fqname).await?;
            }
            CassandraStatement::Update(update) => {
                self.on_table_or_view(&update.table_name).await?;
                if self.finished() {
                    return anyhow::Ok(());
                }
                for ass in update.assignments.iter() {
                    self.on_table_or_view_column(&update.table_name, &ass.name.column)
                        .await?;
                    self.on_operand(&update.table_name, &ass.value).await?;
                    if self.finished() {
                        return anyhow::Ok(());
                    }
                }
                for w in update.where_clause.iter().chain(update.if_clause.iter()) {
                    self.on_relation_element(&update.table_name, w).await?;
                    if self.finished() {
                        return anyhow::Ok(());
                    }
                }
            }
            CassandraStatement::Use(identifier) => {
                self.on_key_space(identifier).await?;
            }
            CassandraStatement::Unknown(_) => {}
        }
        anyhow::Ok(())
    }
}

#[cfg(test)]
mod tests {

    use crate::{
        cassandra::make_test_object, db::QuerySql, enable_test_log,
        language_server::completion_test::test_file_auto_completion,
    };
    use std::path::Path;

    #[tokio::test(flavor = "multi_thread", worker_threads = 4)]
    async fn test_auto_completion() {
        let db = make_test_object(Some("test".to_string())).await;
        enable_test_log();
        let query_id = db.open_connection().await.unwrap();
        let _ = db
            .query_sql(
                query_id,
                r#"
                CREATE TABLE employees (
                    employee_id UUID PRIMARY KEY,
                    first_name TEXT,
                    last_name TEXT,
                    email TEXT,
                    hire_date TIMESTAMP,
                    department TEXT
                );
            "#,
            )
            .await
            .unwrap();

        let _ = db
            .query_sql(
                query_id,
                r#"
                CREATE INDEX email_index ON employees (email);
            "#,
            )
            .await
            .unwrap();

        let _ = db
            .query_sql(
                query_id,
                r#"
               
                CREATE MATERIALIZED VIEW employees_by_department AS
                    SELECT employee_id, first_name, last_name, email, hire_date
                    FROM employees
                    WHERE employee_id IS NOT NULL
                    PRIMARY KEY (employee_id);

            "#,
            )
            .await
            .unwrap();

        let _ = db
            .query_sql(
                query_id,
                r#"
                CREATE ROLE IF NOT EXISTS role_test WITH PASSWORD = 'password123' AND LOGIN = true;
            "#,
            )
            .await
            .unwrap();

        let _ = db
            .query_sql(
                query_id,
                r#"
 
                CREATE OR REPLACE FUNCTION no_op_state_function(state int, input int)
                RETURNS NULL ON NULL INPUT
                RETURNS int
                LANGUAGE java
                AS '
                    return state;  
                ';

            "#,
            )
            .await
            .unwrap();

        let _ = db
            .query_sql(
                query_id,
                r#"
                CREATE OR REPLACE FUNCTION no_op_final_function(state int)
                RETURNS NULL ON NULL INPUT
                RETURNS int
                LANGUAGE java
                AS '
                    return 42;  
                ';
            "#,
            )
            .await
            .unwrap();
        let _ = db
            .query_sql(
                query_id,
                r#"
           
                CREATE OR REPLACE AGGREGATE no_op_aggregate(int)
                SFUNC no_op_state_function
                STYPE int
                FINALFUNC no_op_final_function
                INITCOND 0;  
        "#,
            )
            .await
            .unwrap();

        let _ = db
            .query_sql(
                query_id,
                r#"
           
               CREATE TYPE IF NOT EXISTS completion_type (
                    status TEXT,
                    completion_date TIMESTAMP,
                    remarks TEXT
                );

        "#,
            )
            .await
            .unwrap();

        let manifest_dir =
            std::env::var("CARGO_MANIFEST_DIR").expect("无法获取 CARGO_MANIFEST_DIR");
        let mut file_path = Path::new(manifest_dir.as_str()).to_path_buf();
        file_path.push("tests");
        file_path.push("cassandra_auto_completion_test.sql");
        let content = std::fs::read_to_string(file_path.as_path()).unwrap();
        test_file_auto_completion(&content, &db, uuid::Uuid::new_v4(), false).await;
    }

    #[ignore = "xxx"]
    #[tokio::test(flavor = "multi_thread", worker_threads = 4)]
    async fn test_completion_xxx() {
        enable_test_log();
        let db = make_test_object(Some("test".to_string())).await;
        test_file_auto_completion(
            r#"SELECT * from sys
18 system"#,
            &db,
            uuid::Uuid::new_v4(),
            false,
        )
        .await;
    }
}
const TYPES: &[&str] = &[
    "CUSTOM",
    "ASCII",
    "BIGINT",
    "BLOB",
    "BOOLEAN",
    "COUNTER",
    "DECIMAL",
    "DOUBLE",
    "FLOAT",
    "INT",
    "TIMESTAMP",
    "UUID",
    "VARCHAR",
    "VARINT",
    "TIMEUUID",
    "INET",
    "DATE",
    "TIME",
    "SMALLINT",
    "TINYINT",
    "DURATION",
    "LIST",
    "MAP",
    "SET",
    "UDT",
    "TUPLE",
];
const KEY_WORDS: &[&str] = &[
    "ADD",
    "AGGREGATE",
    "ALLOW",
    "ALTER",
    "AND",
    "ANY",
    "APPLY",
    "ASC",
    "AUTHORIZE",
    "BATCH",
    "BEGIN",
    "DROP",
    "EACH_QUORUM",
    "ENTRIES",
    "FROM",
    "FULL",
    "GRANT",
    "IF",
    "IN",
    "INDEX",
    "INET",
    "INFINITY",
    "INSERT",
    "INTO",
    "KEYSPACE",
    "KEYSPACES",
    "LIMIT",
    "LOCAL_ONE",
    "LOCAL_QUORUM",
    "MATERIALIZED",
    "MODIFY",
    "NAN",
    "NORECURSIVE",
    "NOT",
    "OF",
    "ON",
    "ONE",
    "ORDER",
    "PARTITION",
    "PASSWORD",
    "PER",
    "PRIMARY",
    "QUORUM",
    "RENAME",
    "REVOKE",
    "SCHEMA",
    "SELECT",
    "SET",
    "THREE",
    "TO",
    "TOKEN",
    "TRUNCATE",
    "TWO",
    "UNLOGGED",
    "UPDATE",
    "USE",
    "USING",
    "VIEW",
    "WHERE",
    "WITH",
    "TABLE",
];

// https://docs.datastax.com/en/cql-oss/3.x/cql/cql_reference/cql_function_r.html
const BUILD_IN_FN: &[&str] = &[
    "typeAsBlob",
    "blobAsType",
    "dateOf",
    "now",
    "minTimeuuid",
    "unixTimestampOf",
    "toDate",
    "toTimestamp",
    "toUnixTimestamp",
    "toDate",
    "toUnixTimestamp",
    "toTimestamp",
    "toUnixTimestamp",
    "TOKEN",
    "WRITETIME",
    "TTL",
    // TODO. I don't find full list on build in aggregate function
    "COUNT",
    "MAX",
    "MIN",
    "SUM",
    "AVG",
];
