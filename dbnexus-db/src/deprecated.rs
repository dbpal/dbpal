// TODO don't delete maybe sqlparser will add Location on ast
// see this issue https://github.com/sqlparser-rs/sqlparser-rs/issues/1176

// fn completetetion_base_on_sqlparser<T: CompletetionBackend>(
//     sql: &str, dialect: &dyn sqlparser::dialect::Dialect, backend: &T, pos:
// Pos,
// ) -> Vec<CompletetionItem> { let seqs = match
//   backend.sql_to_sql_seq(sql).ok() { Some(s) => s, None => return
//   backend.default_completion_items(), };

//     let ss: Vec<_> = seqs
//         .iter()
//         .map(|x| {
//             let mut p = sqlparser::parser::Parser::new(dialect);
//             p = p.with_options({
//                 let mut opts = ParserOptions::new();
//                 opts = opts.with_trailing_commas(true);
//                 opts
//             });
//             let parser = p.try_with_sql(&x.sql).ok();
//             parser.map(|mut x| x.parse_statement().ok()).flatten()
//         })
//         .collect();

//     for s in ss.iter() {
//         match s {
//             Some(x) => {
//                 let items = completion_on_stmt(x, pos);
//                 if items.len() > 0 {
//                     return items;
//                 }
//             }
//             None => {}
//         }
//     }
//     backend.default_completion_items()
// }

// fn completion_on_stmt(s: &Statement, _pos: Pos) -> Vec<CompletetionItem> {
//     #[allow(unused)]
//     match s {
//         Statement::Analyze {
//             table_name,
//             partitions,
//             for_columns,
//             columns,
//             cache_metadata,
//             noscan,
//             compute_statistics,
//         } => {}
//         Statement::Truncate {
//             table_name,
//             partitions,
//             table,
//         } => {}
//         Statement::Msck {
//             table_name,
//             repair,
//             partition_action,
//         } => {}
//         Statement::Query(_) => {}
//         Statement::Insert {
//             or,
//             ignore,
//             into,
//             table_name,
//             table_alias,
//             columns,
//             overwrite,
//             source,
//             partitioned,
//             after_columns,
//             table,
//             on,
//             returning,
//             replace_into,
//             priority,
//         } => {}
//         Statement::Directory {
//             overwrite,
//             local,
//             path,
//             file_format,
//             source,
//         } => {}
//         Statement::Call(_) => {}
//         Statement::Copy {
//             source,
//             to,
//             target,
//             options,
//             legacy_options,
//             values,
//         } => {}
//         Statement::CopyIntoSnowflake {
//             into,
//             from_stage,
//             from_stage_alias,
//             stage_params,
//             from_transformations,
//             files,
//             pattern,
//             file_format,
//             copy_options,
//             validation_mode,
//         } => {}
//         Statement::Close { cursor } => {}
//         Statement::Update {
//             table,
//             assignments,
//             from,
//             selection,
//             returning,
//         } => {}
//         Statement::Delete {
//             tables,
//             from,
//             using,
//             selection,
//             returning,
//             order_by,
//             limit,
//         } => {}
//         Statement::CreateView {
//             or_replace,
//             materialized,
//             name,
//             columns,
//             query,
//             options,
//             cluster_by,
//             with_no_schema_binding,
//             if_not_exists,
//             temporary,
//         } => {}
//         Statement::CreateTable {
//             or_replace,
//             temporary,
//             external,
//             global,
//             if_not_exists,
//             transient,
//             name,
//             columns,
//             constraints,
//             hive_distribution,
//             hive_formats,
//             table_properties,
//             with_options,
//             file_format,
//             location,
//             query,
//             without_rowid,
//             like,
//             clone,
//             engine,
//             comment,
//             auto_increment_offset,
//             default_charset,
//             collation,
//             on_commit,
//             on_cluster,
//             order_by,
//             partition_by,
//             cluster_by,
//             options,
//             strict,
//         } => {}
//         Statement::CreateVirtualTable {
//             name,
//             if_not_exists,
//             module_name,
//             module_args,
//         } => {}
//         Statement::CreateIndex {
//             name,
//             table_name,
//             using,
//             columns,
//             unique,
//             concurrently,
//             if_not_exists,
//             include,
//             nulls_distinct,
//             predicate,
//         } => {}
//         Statement::CreateRole {
//             names,
//             if_not_exists,
//             login,
//             inherit,
//             bypassrls,
//             password,
//             superuser,
//             create_db,
//             create_role,
//             replication,
//             connection_limit,
//             valid_until,
//             in_role,
//             in_group,
//             role,
//             user,
//             admin,
//             authorization_owner,
//         } => {}
//         Statement::AlterTable {
//             name,
//             if_exists,
//             only,
//             operations,
//         } => {}
//         Statement::AlterIndex { name, operation } => {}
//         Statement::AlterView {
//             name,
//             columns,
//             query,
//             with_options,
//         } => {}
//         Statement::AlterRole { name, operation } => {}
//         Statement::AttachDatabase {
//             schema_name,
//             database_file_name,
//             database,
//         } => {}
//         Statement::Drop {
//             object_type,
//             if_exists,
//             names,
//             cascade,
//             restrict,
//             purge,
//             temporary,
//         } => {}
//         Statement::DropFunction {
//             if_exists,
//             func_desc,
//             option,
//         } => {}
//         Statement::Declare {
//             name,
//             binary,
//             sensitive,
//             scroll,
//             hold,
//             query,
//         } => {}
//         Statement::CreateExtension {
//             name,
//             if_not_exists,
//             cascade,
//             schema,
//             version,
//         } => {}
//         Statement::Fetch {
//             name,
//             direction,
//             into,
//         } => {}
//         Statement::Flush {
//             object_type,
//             location,
//             channel,
//             read_lock,
//             export,
//             tables,
//         } => {}
//         Statement::Discard { object_type } => {}
//         Statement::SetRole {
//             context_modifier,
//             role_name,
//         } => {}
//         Statement::SetVariable {
//             local,
//             hivevar,
//             variable,
//             value,
//         } => {}
//         Statement::SetTimeZone { local, value } => {}
//         Statement::SetNames {
//             charset_name,
//             collation_name,
//         } => {}
//         Statement::SetNamesDefault {} => {}
//         Statement::ShowFunctions { filter } => {}
//         Statement::ShowVariable { variable } => {}
//         Statement::ShowVariables {
//             filter,
//             global,
//             session,
//         } => {}
//         Statement::ShowCreate { obj_type, obj_name } => {}
//         Statement::ShowColumns {
//             extended,
//             full,
//             table_name,
//             filter,
//         } => {}
//         Statement::ShowTables {
//             extended,
//             full,
//             db_name,
//             filter,
//         } => {}
//         Statement::ShowCollation { filter } => {}
//         Statement::Use { db_name } => {}
//         Statement::StartTransaction {
//             modes,
//             begin,
//             modifier,
//         } => {}
//         Statement::SetTransaction {
//             modes,
//             snapshot,
//             session,
//         } => {}
//         Statement::Comment {
//             object_type,
//             object_name,
//             comment,
//             if_exists,
//         } => {}
//         Statement::Commit { chain } => {}
//         Statement::Rollback { chain, savepoint } => {}
//         Statement::CreateSchema {
//             schema_name,
//             if_not_exists,
//         } => {}
//         Statement::CreateDatabase {
//             db_name,
//             if_not_exists,
//             location,
//             managed_location,
//         } => {}
//         Statement::CreateFunction {
//             or_replace,
//             temporary,
//             name,
//             args,
//             return_type,
//             params,
//         } => {}
//         Statement::CreateProcedure {
//             or_alter,
//             name,
//             params,
//             body,
//         } => {}
//         Statement::CreateMacro {
//             or_replace,
//             temporary,
//             name,
//             args,
//             definition,
//         } => {}
//         Statement::CreateStage {
//             or_replace,
//             temporary,
//             if_not_exists,
//             name,
//             stage_params,
//             directory_table_params,
//             file_format,
//             copy_options,
//             comment,
//         } => {}
//         Statement::Assert { condition, message } => {}
//         Statement::Grant {
//             privileges,
//             objects,
//             grantees,
//             with_grant_option,
//             granted_by,
//         } => {}
//         Statement::Revoke {
//             privileges,
//             objects,
//             grantees,
//             granted_by,
//             cascade,
//         } => {}
//         Statement::Deallocate { name, prepare } => {}
//         Statement::Execute { name, parameters } => {}
//         Statement::Prepare {
//             name,
//             data_types,
//             statement,
//         } => {}
//         Statement::Kill { modifier, id } => {}
//         Statement::ExplainTable {
//             describe_alias,
//             table_name,
//         } => {}
//         Statement::Explain {
//             describe_alias,
//             analyze,
//             verbose,
//             statement,
//             format,
//         } => {}
//         Statement::Savepoint { name } => {}
//         Statement::ReleaseSavepoint { name } => {}
//         Statement::Merge {
//             into,
//             table,
//             source,
//             on,
//             clauses,
//         } => {}
//         Statement::Cache {
//             table_flag,
//             table_name,
//             has_as,
//             options,
//             query,
//         } => {}
//         Statement::UNCache {
//             table_name,
//             if_exists,
//         } => {}
//         Statement::CreateSequence {
//             temporary,
//             if_not_exists,
//             name,
//             data_type,
//             sequence_options,
//             owned_by,
//         } => {}
//         Statement::CreateType {
//             name,
//             representation,
//         } => {}
//         Statement::Pragma { name, value, is_eq } => {}
//         Statement::LockTables { tables } => {}
//         Statement::UnlockTables => {}
//     }
//     vec![]
// }
