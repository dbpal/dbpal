use std::collections::HashSet;

use sqlparser::{
    ast::{
        CloseCursor, CopyLegacyCsvOption, CopyLegacyOption, CopyOption, CopySource, Cte, Distinct,
        Expr, Function, FunctionArg, FunctionArgExpr, FunctionArguments, GroupByExpr, Ident,
        Insert, Join, JoinConstraint, JoinOperator, ObjectName, OrderByExpr, Query, Select,
        SelectItem, SetExpr, Statement, Subscript, TableFactor, TableWithJoins, TopQuantity, With,
    },
    tokenizer::{Span, Token},
};

use crate::completion::{CompletionKind, SemanticToken};

#[derive(Default)]
struct CollectAst {
    result: Vec<SemanticToken>,
}
fn push_on_span(
    result: &mut Vec<SemanticToken>, span: &Span, token_type: CompletionKind,
    token_modifiers: impl Into<Option<u32>>,
) {
    result.push(SemanticToken::new(
        span.start.line as u32,
        span.start.column as u32,
        (span.end.column - span.start.column) as u32,
        token_type.into(),
        token_modifiers.into(),
    ));
}
impl CollectAst {
    // fn push_result(&mut self, token: SemanticToken) {
    //     self.result.push(token);
    // }
    fn push_on_span(
        &mut self, span: &Span, token_type: CompletionKind, token_modifiers: impl Into<Option<u32>>,
    ) {
        push_on_span(&mut self.result, span, token_type, token_modifiers);
    }
}

impl CollectAst {
    fn on_table(&mut self, table: &ObjectName) {
        match table.0.len() {
            1 => {
                let span = &table.0.last().unwrap().span;
                self.push_on_span(&span, CompletionKind::Table.into(), None);
            }
            2 => {
                //schema
                let span = &table.0.first().unwrap().span;
                self.push_on_span(&span, CompletionKind::Schema.into(), None);

                // table
                let span = &table.0.last().unwrap().span;
                self.push_on_span(&span, CompletionKind::Table.into(), None);
            }
            3 => {
                // database
                let span = &table.0.first().unwrap().span;
                self.push_on_span(&span, CompletionKind::Database.into(), None);

                // schema
                let span = &table.0.get(1).unwrap().span;
                self.push_on_span(&span, CompletionKind::Schema.into(), None);

                // table
                let span = &table.0.last().unwrap().span;
                self.push_on_span(&span, CompletionKind::Table.into(), None);
            }
            _ => {
                log::error!("very long table names:{:?}", table);
            }
        }
    }
    fn on_column(&mut self, c: &Ident) {
        let span = &c.span;
        self.push_on_span(&span, CompletionKind::Column.into(), None);
    }

    fn on_partion_expr(&mut self, expr: &Expr) {
        match expr {
            Expr::Identifier(x) => {
                let span = &x.span;
                self.push_on_span(&span, CompletionKind::Partition.into(), None);
            }
            _ => {
                log::error!("not handle expr type on partition:{:?}", expr)
            }
        }
    }
    fn on_query(&mut self, query: &Query) {
        if let Some(w) = query.with.as_ref() {
            self.on_with(w);
        }
        self.on_set_expr(query.body.as_ref());
        if let Some(orders) = &query.order_by {
            for order in orders.exprs.iter() {
                self.on_order_by_expr(order);
            }
        }
        if let Some(limit) = &query.limit {
            self.on_expr(limit);
        }
        for l in query.limit_by.iter() {
            self.on_expr(l);
        }
        if let Some(offset) = &query.offset {
            self.on_expr(&offset.value);
        }
        if let Some(fetch) = &query.fetch {
            if let Some(quantity) = &fetch.quantity {
                self.on_expr(quantity);
            }
        }
    }

    fn on_set_expr(&mut self, w: &SetExpr) {
        match w {
            SetExpr::Select(select) => self.on_select(&select),
            SetExpr::Query(query) => self.on_query(&query),
            SetExpr::SetOperation { left, right, .. } => {
                self.on_set_expr(&left);
                self.on_set_expr(&right);
            }
            SetExpr::Values(values) => {
                for x in values.rows.iter() {
                    for xx in x.iter() {
                        self.on_expr(xx);
                    }
                }
            }
            SetExpr::Insert(statement) => self.on_statement(statement),
            SetExpr::Update(statement) => self.on_statement(statement),
            SetExpr::Table(_table) => {
                // no span for location
                // there is nothing we can do.
            }
        }
    }

    fn on_function_call_name(&mut self, name: &ObjectName) {
        match name.0.len() {
            1 => {
                let span = &name.0.last().unwrap().span;
                self.push_on_span(&span, CompletionKind::Table.into(), None);
            }
            2 => {
                //schema
                let span = &name.0.first().unwrap().span;
                self.push_on_span(&span, CompletionKind::Schema.into(), None);

                // table
                let span = &name.0.last().unwrap().span;
                self.push_on_span(&span, CompletionKind::Table.into(), None);
            }
            3 => {
                // database
                let span = &name.0.first().unwrap().span;
                self.push_on_span(&span, CompletionKind::Database.into(), None);

                // schema
                let span = &name.0.get(1).unwrap().span;
                self.push_on_span(&span, CompletionKind::Schema.into(), None);

                // table
                let span = &name.0.last().unwrap().span;
                self.push_on_span(&span, CompletionKind::Table.into(), None);
            }
            _ => {
                log::error!("very long table names:{:?}", name);
            }
        }
    }

    fn on_parameter(&mut self, p: &Ident) {
        self.push_on_span(&p.span, CompletionKind::Parameter, None);
    }

    fn on_function_arg_expr(&mut self, arg: &FunctionArgExpr) {
        match arg {
            FunctionArgExpr::Expr(expr) => {
                self.on_expr(expr);
            }
            FunctionArgExpr::QualifiedWildcard(object_name) => {
                self.on_table(object_name);
            }
            FunctionArgExpr::Wildcard => {
                // nothing here
            }
        }
    }
    fn on_function_arg(&mut self, a: &FunctionArg) {
        match a {
            FunctionArg::Named { name, arg, .. } => {
                self.on_parameter(name);
                self.on_function_arg_expr(arg);
            }
            FunctionArg::ExprNamed { name, arg, .. } => {
                // should I enable this.
                self.on_expr(name);
                self.on_function_arg_expr(arg);
            }
            FunctionArg::Unnamed(function_arg_expr) => match function_arg_expr {
                FunctionArgExpr::Expr(expr) => {
                    self.on_expr(expr);
                }
                FunctionArgExpr::QualifiedWildcard(object_name) => {
                    self.on_table(&object_name);
                }
                FunctionArgExpr::Wildcard => {
                    // nothing to do here
                }
            },
        }
    }

    fn on_functon_call(&mut self, f: &Function) {
        self.on_function_call_name(&f.name);
        match &f.parameters {
            FunctionArguments::None => {}
            FunctionArguments::Subquery(query) => {
                self.on_query(&query);
            }
            FunctionArguments::List(function_argument_list) => {
                for a in function_argument_list.args.iter() {
                    self.on_function_arg(a);
                }
            }
        };
        if let Some(filter) = &f.filter {
            self.on_expr(&filter);
        }
        for o in f.within_group.iter() {
            self.on_order_by_expr(o);
        }
    }

    fn on_order_by_expr(&mut self, o: &OrderByExpr) {
        self.on_expr(&o.expr);
    }

    fn on_expr(&mut self, expr: &Expr) {
        match expr {
            Expr::Identifier(ident) => {
                self.on_column(ident);
            }

            Expr::CompoundIdentifier(vec) => {
                if vec.len() > 0 {
                    self.on_column(vec.last().unwrap());
                    self.on_table(&ObjectName((&vec[0..vec.len() - 1]).to_vec()));
                }
            }
            Expr::JsonAccess { value, .. } => {
                self.on_expr(&value);
            }

            Expr::CompositeAccess { expr, key } => {
                self.on_expr(&expr);
                self.on_column(key);
            }

            Expr::IsFalse(expr) => {
                self.on_expr(&expr);
            }
            Expr::IsNotFalse(expr) => {
                self.on_expr(&expr);
            }
            Expr::IsTrue(expr) => {
                self.on_expr(&expr);
            }
            Expr::IsNotTrue(expr) => {
                self.on_expr(&expr);
            }
            Expr::IsNull(expr) => {
                self.on_expr(&expr);
            }
            Expr::IsNotNull(expr) => {
                self.on_expr(&expr);
            }
            Expr::IsUnknown(expr) => {
                self.on_expr(&expr);
            }
            Expr::IsNotUnknown(expr) => {
                self.on_expr(&expr);
            }
            Expr::IsDistinctFrom(expr, expr1) => {
                self.on_expr(&expr);
                self.on_expr(&expr1);
            }

            Expr::IsNotDistinctFrom(expr, expr1) => {
                self.on_expr(&expr);
                self.on_expr(&expr1);
            }
            Expr::InList { expr, list, .. } => {
                self.on_expr(&expr);
                for l in list.iter() {
                    self.on_expr(l);
                }
            }
            Expr::InSubquery { expr, subquery, .. } => {
                self.on_expr(&expr);
                self.on_query(&subquery);
            }
            Expr::InUnnest {
                expr, array_expr, ..
            } => {
                self.on_expr(&expr);
                self.on_expr(&array_expr);
            }
            Expr::Between {
                expr, low, high, ..
            } => {
                self.on_expr(&expr);
                self.on_expr(&low);
                self.on_expr(&high);
            }
            Expr::BinaryOp { left, right, .. } => {
                self.on_expr(&left);
                self.on_expr(&right);
            }
            Expr::Like { expr, pattern, .. } => {
                self.on_expr(&expr);
                self.on_expr(&pattern);
            }
            Expr::ILike { expr, pattern, .. } => {
                self.on_expr(&expr);
                self.on_expr(&pattern);
            }
            Expr::SimilarTo { expr, pattern, .. } => {
                self.on_expr(&expr);
                self.on_expr(&pattern);
            }
            Expr::RLike { expr, pattern, .. } => {
                self.on_expr(&expr);
                self.on_expr(&pattern);
            }
            Expr::AnyOp { left, right, .. } => {
                self.on_expr(&left);
                self.on_expr(&right);
            }
            Expr::AllOp { left, right, .. } => {
                self.on_expr(&left);
                self.on_expr(&right);
            }
            Expr::UnaryOp { expr, .. } => {
                self.on_expr(&expr);
            }
            Expr::Convert { expr, styles, .. } => {
                self.on_expr(&expr);
                for s in styles.iter() {
                    self.on_expr(s);
                }
            }

            Expr::Cast { expr, .. } => {
                self.on_expr(&expr);
            }

            Expr::AtTimeZone {
                timestamp,
                time_zone,
            } => {
                self.on_expr(&time_zone);
                self.on_expr(&timestamp);
            }
            Expr::Extract { expr, .. } => {
                self.on_expr(&expr);
            }
            Expr::Ceil { expr, .. } => {
                self.on_expr(&expr);
            }
            Expr::Floor { expr, .. } => {
                self.on_expr(&expr);
            }
            Expr::Position { expr, r#in } => {
                self.on_expr(&expr);
                self.on_expr(r#in.as_ref());
            }
            Expr::Substring {
                expr,
                substring_from,
                substring_for,
                ..
            } => {
                self.on_expr(&expr);
                if let Some(x) = substring_from {
                    self.on_expr(x);
                }
                if let Some(x) = substring_for {
                    self.on_expr(x);
                }
            }

            Expr::Trim {
                expr,
                trim_what,
                trim_characters,
                ..
            } => {
                self.on_expr(&expr);

                if let Some(x) = trim_what {
                    self.on_expr(x.as_ref());
                }
                if let Some(trim_characters) = trim_characters {
                    for t in trim_characters.iter() {
                        self.on_expr(t);
                    }
                }
            }
            Expr::Overlay {
                expr,
                overlay_what,
                overlay_from,
                overlay_for,
            } => {
                self.on_expr(&expr);

                self.on_expr(&overlay_what);
                self.on_expr(&overlay_from);
                if let Some(overlay_for) = overlay_for {
                    self.on_expr(overlay_for);
                }
            }
            Expr::Collate { expr, .. } => {
                self.on_expr(&expr);
            }
            Expr::Nested(expr) => {
                self.on_expr(&expr);
            }
            Expr::Value(..) => {
                // no location info.
            }
            Expr::IntroducedString { .. } => {
                // no location info.
            }
            Expr::TypedString { .. } => {
                // no location info.
            }
            Expr::MapAccess { column, keys } => {
                self.on_expr(&column);
                for key in keys.iter() {
                    self.on_expr(&key.key);
                }
            }
            Expr::Function(function) => {
                self.on_functon_call(function);
            }

            Expr::Method(method) => {
                self.on_expr(&method.expr);
                for f in method.method_chain.iter() {
                    self.on_functon_call(f);
                }
            }
            Expr::Case {
                operand,
                conditions,
                results,
                else_result,
            } => {
                if let Some(operand) = operand {
                    self.on_expr(&operand);
                }
                for c in conditions.iter() {
                    self.on_expr(c);
                }
                for r in results.iter() {
                    self.on_expr(r);
                }
                if let Some(else_result) = else_result {
                    self.on_expr(else_result);
                }
            }
            Expr::Exists { subquery, .. } => {
                self.on_query(&subquery);
            }
            Expr::Subquery(query) => {
                self.on_query(&query);
            }
            Expr::GroupingSets(vec) => {
                for v in vec.iter() {
                    for e in v.iter() {
                        self.on_expr(e);
                    }
                }
            }
            Expr::Cube(vec) => {
                for v in vec.iter() {
                    for e in v.iter() {
                        self.on_expr(e);
                    }
                }
            }
            Expr::Rollup(vec) => {
                for v in vec.iter() {
                    for e in v.iter() {
                        self.on_expr(e);
                    }
                }
            }
            Expr::Tuple(vec) => {
                for v in vec.iter() {
                    self.on_expr(v);
                }
            }
            Expr::Struct { .. } => {
                // BigQuery
            }
            Expr::Named { .. } => {
                // BigQuery
            }
            Expr::Dictionary(_vec) => {
                // DuckDB
            }
            Expr::Map(_) => {
                // DuckDB
            }
            Expr::Subscript { expr, subscript } => {
                self.on_expr(expr.as_ref());
                match subscript.as_ref() {
                    Subscript::Index { index } => {
                        self.on_expr(index);
                    }
                    Subscript::Slice {
                        lower_bound,
                        upper_bound,
                        stride,
                    } => {
                        if let Some(lower_bound) = lower_bound {
                            self.on_expr(lower_bound);
                        }
                        if let Some(upper_bound) = upper_bound {
                            self.on_expr(upper_bound);
                        }
                        if let Some(stride) = stride {
                            self.on_expr(stride);
                        }
                    }
                }
            }
            Expr::Array(array) => {
                for e in array.elem.iter() {
                    self.on_expr(e);
                }
            }
            Expr::Interval(interval) => {
                self.on_expr(&interval.value);
            }
            Expr::MatchAgainst { columns, .. } => {
                for col in columns.iter() {
                    self.on_column(col);
                }
            }
            Expr::Wildcard(_) => {
                // wired looks like lexer element.
            }
            Expr::QualifiedWildcard(object_name, ..) => {
                self.on_table(object_name);
            }
            Expr::OuterJoin(expr) => {
                self.on_expr(&expr);
            }
            Expr::Prior(expr) => {
                self.on_expr(&expr);
            }
            Expr::Lambda(lambda_function) => {
                for p in lambda_function.params.iter() {
                    self.on_parameter(p);
                }
                self.on_expr(&lambda_function.body);
            }
        }
    }

    fn on_select(&mut self, select: &Select) {
        if let Some(d) = &select.distinct {
            match d {
                Distinct::Distinct => {}
                Distinct::On(vec) => {
                    for v in vec.iter() {
                        self.on_expr(v);
                    }
                }
            }
        }
        if let Some(top) = &select.top {
            if let Some(q) = &top.quantity {
                match q {
                    TopQuantity::Expr(expr) => {
                        self.on_expr(expr);
                    }
                    TopQuantity::Constant(_) => {}
                }
            }
        }
        for projection in select.projection.iter() {
            self.on_select_item(projection);
        }
        if let Some(into) = &select.into {
            self.on_table(&into.name);
        }
        for f in select.from.iter() {
            self.on_table_with_join(f);
        }
        if let Some(s) = &select.selection {
            self.on_expr(s);
        }
        match &select.group_by {
            GroupByExpr::All(_vec) => {}
            GroupByExpr::Expressions(vec, ..) => {
                for v in vec.iter() {
                    self.on_expr(v);
                }
            }
        }
        if let Some(having) = &select.having {
            self.on_expr(having);
        }
    }

    fn on_table_factor(&mut self, factor: &TableFactor) {
        #[allow(unused)]
        match factor {
            TableFactor::Table {
                name,
                alias,
                args,
                with_hints,
                version,
                with_ordinality,
                partitions,
                json_path,
            } => {
                self.on_table(name);
                if let Some(alias) = alias {
                    self.on_table(&ObjectName(vec![alias.name.clone()]));
                    for col in alias.columns.iter() {
                        self.on_column(&col.name);
                    }
                }
                for arg in args.iter() {
                    for a in arg.args.iter() {
                        self.on_function_arg(a);
                    }
                }
            }
            TableFactor::Derived {
                lateral,
                subquery,
                alias,
            } => todo!(),
            TableFactor::TableFunction { expr, alias } => todo!(),
            TableFactor::Function {
                lateral,
                name,
                args,
                alias,
            } => todo!(),
            TableFactor::UNNEST {
                alias,
                array_exprs,
                with_offset,
                with_offset_alias,
                with_ordinality,
            } => todo!(),
            TableFactor::JsonTable {
                json_expr,
                json_path,
                columns,
                alias,
            } => todo!(),
            TableFactor::OpenJsonTable {
                json_expr,
                json_path,
                columns,
                alias,
            } => todo!(),
            TableFactor::NestedJoin {
                table_with_joins,
                alias,
            } => todo!(),
            TableFactor::Pivot {
                table,
                aggregate_functions,
                value_column,
                value_source,
                default_on_null,
                alias,
            } => todo!(),
            TableFactor::Unpivot {
                table,
                value,
                name,
                columns,
                alias,
            } => todo!(),
            TableFactor::MatchRecognize {
                table,
                partition_by,
                order_by,
                measures,
                rows_per_match,
                after_match_skip,
                pattern,
                symbols,
                alias,
            } => todo!(),
        }
    }
    fn on_table_with_join(&mut self, table_with_join: &TableWithJoins) {
        self.on_table_factor(&table_with_join.relation);
    }
    fn on_join(&mut self, j: &Join) {
        self.on_table_factor(&j.relation);
        match &j.join_operator {
            JoinOperator::Inner(join_constraint)
            | JoinOperator::LeftOuter(join_constraint)
            | JoinOperator::RightOuter(join_constraint)
            | JoinOperator::FullOuter(join_constraint)
            | JoinOperator::Semi(join_constraint)
            | JoinOperator::LeftSemi(join_constraint)
            | JoinOperator::RightSemi(join_constraint)
            | JoinOperator::Anti(join_constraint)
            | JoinOperator::LeftAnti(join_constraint)
            | JoinOperator::RightAnti(join_constraint) => match join_constraint {
                JoinConstraint::On(expr) => {
                    self.on_expr(expr);
                }
                JoinConstraint::Using(vec) => {
                    for v in vec.iter() {
                        self.on_column(v);
                    }
                }
                JoinConstraint::Natural => {}
                JoinConstraint::None => {}
            },

            JoinOperator::CrossApply => {}
            JoinOperator::OuterApply => {}
            JoinOperator::CrossJoin => {}
            JoinOperator::AsOf {
                match_condition,
                constraint,
            } => {
                self.on_expr(match_condition);
                match constraint {
                    JoinConstraint::On(expr) => {
                        self.on_expr(expr);
                    }
                    JoinConstraint::Using(vec) => {
                        for v in vec.iter() {
                            self.on_column(v);
                        }
                    }
                    _ => {}
                }
            }
        }
    }
    fn on_with(&mut self, w: &With) {
        for t in w.cte_tables.iter() {
            self.on_cte(t);
        }
    }

    fn on_cte(&mut self, cte: &Cte) {
        self.on_table(&ObjectName(vec![cte.alias.name.clone()]));
        for c in cte.alias.columns.iter() {
            self.on_column(&c.name);
        }
        // TODO is this actual a table?
        if let Some(table) = cte.from.as_ref() {
            self.on_table(&ObjectName(vec![table.clone()]));
        }
        self.on_query(&cte.query);
    }

    fn on_insert(&mut self, insert: &Insert) {
        self.on_table(&insert.table_name);
        if let Some(alias) = &insert.table_alias {
            self.on_table(&ObjectName(vec![alias.clone()]));
        }
        for col in insert.columns.iter() {
            self.on_column(col);
        }
        if let Some(s) = &insert.source {
            self.on_query(&s);
        }
        for col in insert.after_columns.iter() {
            self.on_column(col);
        }
        if let Some(rs) = &insert.returning {
            for r in rs.iter() {
                self.on_select_item(r);
            }
        }
        if let Some(insert_alias) = &insert.insert_alias {
            self.on_table(&insert_alias.row_alias);
            if let Some(col_aliases) = &insert_alias.col_aliases {
                for col in col_aliases.iter() {
                    self.on_column(col);
                }
            }
        }
    }
    fn on_select_item(&mut self, item: &SelectItem) {
        match item {
            SelectItem::UnnamedExpr(expr) => {
                self.on_expr(expr);
            }
            SelectItem::ExprWithAlias { expr, alias } => {
                self.on_expr(expr);
                let span = &alias.span;

                self.push_on_span(span, CompletionKind::Column, None);
            }
            SelectItem::QualifiedWildcard(object_name, ..) => {
                self.on_table(object_name);
            }
            SelectItem::Wildcard(..) => {
                // nothing here
            }
        }
    }

    fn on_statement(&mut self, statement: &Statement) {
        #[allow(unused)]
        match statement {
            Statement::Analyze {
                table_name,
                partitions,
                columns,
                ..
            } => {
                self.on_table(&table_name);
                for c in columns.iter() {
                    self.on_column(c);
                }
                if let Some(partitions) = partitions {
                    for p in partitions.iter() {
                        self.on_partion_expr(p);
                    }
                }
            }

            Statement::Truncate {
                table_names,
                partitions,
                ..
            } => {
                for name in table_names.iter() {
                    self.on_table(&name.name);
                }

                if let Some(partitions) = partitions {
                    for p in partitions.iter() {
                        self.on_partion_expr(p);
                    }
                }
            }
            Statement::Msck {
                table_name,
                repair,
                partition_action,
            } => {
                // hive
            }
            Statement::Query(query) => {
                self.on_query(query.as_ref());
            }
            Statement::Insert(insert) => {
                self.on_insert(insert);
            }
            Statement::Install { extension_name } => {
                // DuckDB
            }
            Statement::Load { extension_name } => {
                // DuckDB
            }
            Statement::Directory { source, .. } => {
                self.on_query(&source);
            }
            Statement::Call(function) => {
                self.on_functon_call(function);
            }
            Statement::Copy {
                source,
                options,
                legacy_options,
                ..
            } => {
                //
                match source {
                    CopySource::Table {
                        table_name,
                        columns,
                    } => {
                        self.on_table(&table_name);
                        for col in columns.iter() {
                            self.on_column(col);
                        }
                    }
                    CopySource::Query(query) => {
                        self.on_query(&query);
                    }
                }
                for op in options.iter() {
                    match op {
                        CopyOption::ForceNotNull(xs)
                        | CopyOption::ForceQuote(xs)
                        | CopyOption::ForceNull(xs) => {
                            for col in xs.iter() {
                                self.on_column(col);
                            }
                        }
                        _ => {}
                    }
                }
                for op in legacy_options.iter() {
                    match op {
                        CopyLegacyOption::Csv(x) => {
                            for op in x.iter() {
                                match op {
                                    CopyLegacyCsvOption::ForceQuote(xs)
                                    | CopyLegacyCsvOption::ForceQuote(xs) => {
                                        for col in xs.iter() {
                                            self.on_column(col);
                                        }
                                    }
                                    _ => {}
                                }
                            }
                        }
                        _ => {}
                    }
                }
            }
            Statement::CopyIntoSnowflake { .. } => {
                //
            }
            Statement::Close { cursor } => {
                //
                match cursor {
                    CloseCursor::All => {}
                    CloseCursor::Specific { name } => {
                        let span = &name.span;
                        self.push_on_span(span, CompletionKind::Cursor, None);
                    }
                }
            }
            Statement::Update {
                table,
                assignments,
                from,
                selection,
                returning,
                or,
            } => {}
            Statement::Delete(delete) => {}
            Statement::CreateView {
                or_replace,
                materialized,
                name,
                columns,
                query,
                options,
                cluster_by,
                comment,
                with_no_schema_binding,
                if_not_exists,
                temporary,
                to,
            } => {}
            Statement::CreateTable(create_table) => {}
            Statement::CreateVirtualTable {
                name,
                if_not_exists,
                module_name,
                module_args,
            } => {}
            Statement::CreateIndex(create_index) => {}
            Statement::CreateRole {
                names,
                if_not_exists,
                login,
                inherit,
                bypassrls,
                password,
                superuser,
                create_db,
                create_role,
                replication,
                connection_limit,
                valid_until,
                in_role,
                in_group,
                role,
                user,
                admin,
                authorization_owner,
            } => {}
            Statement::CreateSecret {
                or_replace,
                temporary,
                if_not_exists,
                name,
                storage_specifier,
                secret_type,
                options,
            } => {}
            Statement::CreatePolicy {
                name,
                table_name,
                policy_type,
                command,
                to,
                using,
                with_check,
            } => {}
            Statement::AlterTable {
                name,
                if_exists,
                only,
                operations,
                location,
                on_cluster,
            } => {}
            Statement::AlterIndex { name, operation } => {}
            Statement::AlterView {
                name,
                columns,
                query,
                with_options,
            } => {}
            Statement::AlterRole { name, operation } => {}
            Statement::AlterPolicy {
                name,
                table_name,
                operation,
            } => {}
            Statement::AttachDatabase {
                schema_name,
                database_file_name,
                database,
            } => {}
            Statement::AttachDuckDBDatabase {
                if_not_exists,
                database,
                database_path,
                database_alias,
                attach_options,
            } => {}
            Statement::DetachDuckDBDatabase {
                if_exists,
                database,
                database_alias,
            } => {}
            Statement::Drop {
                object_type,
                if_exists,
                names,
                cascade,
                restrict,
                purge,
                temporary,
            } => {}
            Statement::DropFunction {
                if_exists,
                func_desc,
                option,
            } => {}
            Statement::DropProcedure {
                if_exists,
                proc_desc,
                option,
            } => {}
            Statement::DropSecret {
                if_exists,
                temporary,
                name,
                storage_specifier,
            } => {}
            Statement::DropPolicy {
                if_exists,
                name,
                table_name,
                option,
            } => {}
            Statement::Declare { stmts } => {}
            Statement::CreateExtension {
                name,
                if_not_exists,
                cascade,
                schema,
                version,
            } => {}
            Statement::Fetch {
                name,
                direction,
                into,
            } => {}
            Statement::Flush {
                object_type,
                location,
                channel,
                read_lock,
                export,
                tables,
            } => {}
            Statement::Discard { object_type } => {}
            Statement::SetRole {
                context_modifier,
                role_name,
            } => {}
            Statement::SetVariable {
                local,
                hivevar,
                variables,
                value,
            } => {}
            Statement::SetTimeZone { local, value } => {}
            Statement::SetNames {
                charset_name,
                collation_name,
            } => {}
            Statement::SetNamesDefault {} => {}
            Statement::ShowFunctions { filter } => {}
            Statement::ShowVariable { variable } => {}
            Statement::ShowStatus {
                filter,
                global,
                session,
            } => {}
            Statement::ShowVariables {
                filter,
                global,
                session,
            } => {}
            Statement::ShowCreate { obj_type, obj_name } => {}
            Statement::ShowColumns {
                extended,
                full,
                show_options,
            } => {}
            Statement::ShowDatabases {
                terse,
                history,
                show_options,
            } => {}
            Statement::ShowSchemas {
                terse,
                history,
                show_options,
            } => {}
            Statement::ShowTables {
                terse,
                history,
                extended,
                full,
                external,
                show_options,
            } => {}
            Statement::ShowViews {
                terse,
                materialized,
                show_options,
            } => {}
            Statement::ShowCollation { filter } => {}
            Statement::Use(_) => {}
            Statement::StartTransaction {
                modes,
                begin,
                modifier,
            } => {}
            Statement::SetTransaction {
                modes,
                snapshot,
                session,
            } => {}
            Statement::Comment {
                object_type,
                object_name,
                comment,
                if_exists,
            } => {}
            Statement::Commit { chain } => {}
            Statement::Rollback { chain, savepoint } => {}
            Statement::CreateSchema {
                schema_name,
                if_not_exists,
            } => {}
            Statement::CreateDatabase {
                db_name,
                if_not_exists,
                location,
                managed_location,
            } => {}
            Statement::CreateFunction {
                or_replace,
                temporary,
                if_not_exists,
                name,
                args,
                return_type,
                function_body,
                behavior,
                called_on_null,
                parallel,
                using,
                language,
                determinism_specifier,
                options,
                remote_connection,
            } => {}
            Statement::CreateTrigger {
                or_replace,
                is_constraint,
                name,
                period,
                events,
                table_name,
                referenced_table_name,
                referencing,
                trigger_object,
                include_each,
                condition,
                exec_body,
                characteristics,
            } => {}
            Statement::DropTrigger {
                if_exists,
                trigger_name,
                table_name,
                option,
            } => {}
            Statement::CreateProcedure {
                or_alter,
                name,
                params,
                body,
            } => {}
            Statement::CreateMacro {
                or_replace,
                temporary,
                name,
                args,
                definition,
            } => {}
            Statement::CreateStage {
                or_replace,
                temporary,
                if_not_exists,
                name,
                stage_params,
                directory_table_params,
                file_format,
                copy_options,
                comment,
            } => {}
            Statement::Assert { condition, message } => {}
            Statement::Grant {
                privileges,
                objects,
                grantees,
                with_grant_option,
                granted_by,
            } => {}
            Statement::Revoke {
                privileges,
                objects,
                grantees,
                granted_by,
                cascade,
            } => {}
            Statement::Deallocate { name, prepare } => {}
            Statement::Execute {
                name,
                parameters,
                has_parentheses,
                using,
            } => {}
            Statement::Prepare {
                name,
                data_types,
                statement,
            } => {}
            Statement::Kill { modifier, id } => {}
            Statement::ExplainTable {
                describe_alias,
                hive_format,
                has_table_keyword,
                table_name,
            } => {}
            Statement::Explain {
                describe_alias,
                analyze,
                verbose,
                query_plan,
                statement,
                format,
                options,
            } => {}
            Statement::Savepoint { name } => {}
            Statement::ReleaseSavepoint { name } => {}
            Statement::Merge {
                into,
                table,
                source,
                on,
                clauses,
            } => {}
            Statement::Cache {
                table_flag,
                table_name,
                has_as,
                options,
                query,
            } => {}
            Statement::UNCache {
                table_name,
                if_exists,
            } => {}
            Statement::CreateSequence {
                temporary,
                if_not_exists,
                name,
                data_type,
                sequence_options,
                owned_by,
            } => {}
            Statement::CreateType {
                name,
                representation,
            } => {}
            Statement::Pragma { name, value, is_eq } => {}
            Statement::LockTables { tables } => {}
            Statement::UnlockTables => {}
            Statement::Unload { query, to, with } => {}
            Statement::OptimizeTable {
                name,
                on_cluster,
                partition,
                include_final,
                deduplicate,
            } => {}
            Statement::LISTEN { channel } => {}
            Statement::UNLISTEN { channel } => {}
            Statement::NOTIFY { channel, payload } => {}
            Statement::LoadData {
                local,
                inpath,
                overwrite,
                table_name,
                partitioned,
                table_format,
            } => {}
        }
    }
}

fn samatic_token_on_parser(
    d: &dyn sqlparser::dialect::Dialect, sql: &str,
) -> anyhow::Result<Vec<SemanticToken>> {
    let s = sqlparser::parser::Parser::new(d);
    let mut parser = s.try_with_sql(sql)?;

    let mut statements = Vec::new();
    while parser.peek_token() != Token::EOF {
        while parser.peek_token() == Token::SemiColon {
            parser.next_token();
            continue;
        }
        if parser.peek_token() == Token::EOF {
            break;
        }
        let s = parser.parse_statement()?;
        statements.push(s);
    }
    let mut c = CollectAst::default();
    for s in statements.iter() {
        c.on_statement(&s);
    }
    anyhow::Ok(c.result)
}

fn samatic_token_on_lexer(
    d: &dyn sqlparser::dialect::Dialect, sql: &str, ast_fail: bool, keyword: &HashSet<String>,
) -> anyhow::Result<Vec<SemanticToken>> {
    let tokens = sqlparser::tokenizer::Tokenizer::new(d, sql).tokenize_with_location()?;

    let mut ret = Vec::new();
    for t in tokens.iter() {
        let span = &t.span;

        match &t.token {
            Token::EOF => {}
            Token::Word(word) if ast_fail => {
                let to_upper = word.value.to_uppercase();
                let token_type = if keyword.contains(&to_upper) {
                    CompletionKind::KeyWord
                } else {
                    CompletionKind::Identifer
                };

                let span = &t.span;
                push_on_span(&mut ret, span, token_type, None);
            }
            Token::Word(_) => {
                // nothing to do here.
            }
            Token::Number(_, _) => {
                push_on_span(&mut ret, span, CompletionKind::Literal, None);
            }
            Token::Char(_) => {}
            Token::SingleQuotedString(_) => {
                push_on_span(&mut ret, span, CompletionKind::Literal, None);
            }
            Token::DoubleQuotedString(_) => {
                // push_on_span(&mut ret, span, CompletionKind::Literal, None);
            }
            Token::TripleSingleQuotedString(_) => {
                push_on_span(&mut ret, span, CompletionKind::Literal, None);
            }
            Token::TripleDoubleQuotedString(_) => {
                push_on_span(&mut ret, span, CompletionKind::Literal, None);
            }
            Token::DollarQuotedString(_) => {
                // sometims is represet by `sqlparser` to idicate function  ody
            }
            Token::SingleQuotedByteStringLiteral(_) => {
                push_on_span(&mut ret, span, CompletionKind::Literal, None);
            }
            Token::DoubleQuotedByteStringLiteral(_) => {
                push_on_span(&mut ret, span, CompletionKind::Literal, None);
            }
            Token::TripleSingleQuotedByteStringLiteral(_) => {
                push_on_span(&mut ret, span, CompletionKind::Literal, None);
            }
            Token::TripleDoubleQuotedByteStringLiteral(_) => {
                push_on_span(&mut ret, span, CompletionKind::Literal, None);
            }
            Token::SingleQuotedRawStringLiteral(_) => {
                push_on_span(&mut ret, span, CompletionKind::Literal, None);
            }
            Token::DoubleQuotedRawStringLiteral(_) => {
                push_on_span(&mut ret, span, CompletionKind::Literal, None);
            }
            Token::TripleSingleQuotedRawStringLiteral(_) => {
                push_on_span(&mut ret, span, CompletionKind::Literal, None);
            }
            Token::TripleDoubleQuotedRawStringLiteral(_) => {
                push_on_span(&mut ret, span, CompletionKind::Literal, None);
            }
            Token::NationalStringLiteral(_) => {
                push_on_span(&mut ret, span, CompletionKind::Literal, None);
            }
            Token::EscapedStringLiteral(_) => {
                push_on_span(&mut ret, span, CompletionKind::Literal, None);
            }
            Token::UnicodeStringLiteral(_) => {
                push_on_span(&mut ret, span, CompletionKind::Literal, None);
            }
            Token::HexStringLiteral(_) => {
                push_on_span(&mut ret, span, CompletionKind::Literal, None);
            }
            Token::Comma => {}
            Token::Whitespace(_) => {}
            Token::DoubleEq => {}
            Token::Eq => {}
            Token::Neq => {}
            Token::Lt => {}
            Token::Gt => {}
            Token::LtEq => {}
            Token::GtEq => {}
            Token::Spaceship => {}
            Token::Plus => {}
            Token::Minus => {}
            Token::Mul => {}
            Token::Div => {}
            Token::DuckIntDiv => {}
            Token::Mod => {}
            Token::StringConcat => {}
            Token::LParen => {}
            Token::RParen => {}
            Token::Period => {}
            Token::Colon => {}
            Token::DoubleColon => {}
            Token::Assignment => {}
            Token::SemiColon => {}
            Token::Backslash => {}
            Token::LBracket => {}
            Token::RBracket => {}
            Token::Ampersand => {}
            Token::Pipe => {}
            Token::Caret => {}
            Token::LBrace => {}
            Token::RBrace => {}
            Token::RArrow => {}
            Token::Sharp => {}
            Token::Tilde => {}
            Token::TildeAsterisk => {}
            Token::ExclamationMarkTilde => {}
            Token::ExclamationMarkTildeAsterisk => {}
            Token::DoubleTilde => {}
            Token::DoubleTildeAsterisk => {}
            Token::ExclamationMarkDoubleTilde => {}
            Token::ExclamationMarkDoubleTildeAsterisk => {}
            Token::ShiftLeft => {}
            Token::ShiftRight => {}
            Token::Overlap => {}
            Token::ExclamationMark => {}
            Token::DoubleExclamationMark => {}
            Token::AtSign => {}
            Token::CaretAt => {}
            Token::PGSquareRoot => {}
            Token::PGCubeRoot => {}
            Token::Placeholder(_) => {}
            Token::Arrow => {}
            Token::LongArrow => {}
            Token::HashArrow => {}
            Token::HashLongArrow => {}
            Token::AtArrow => {}
            Token::ArrowAt => {}
            Token::HashMinus => {}
            Token::AtQuestion => {}
            Token::AtAt => {}
            Token::Question => {}
            Token::QuestionAnd => {}
            Token::QuestionPipe => {}
            Token::CustomBinaryOperator(_) => {}
        }
    }
    anyhow::Ok(ret)
}

pub fn samatic_token_base_on_sqlparser(
    d: &dyn sqlparser::dialect::Dialect, sql: &str, keyword: &HashSet<String>,
) -> anyhow::Result<Vec<SemanticToken>> {
    let mut ast_fail = false;
    let mut ret = samatic_token_on_parser(d, sql)
        .map_err(|e| {
            log::error!("sematic token on ast failed,err:{}", e);
            ast_fail = true;
        })
        .unwrap_or_default();
    log::info!("ast_fail :{:?}", ast_fail);
    ret.extend(
        samatic_token_on_lexer(d, sql, ast_fail, keyword)
            .map_err(|e| log::error!("sematic token on lexer failed:{}", e))
            .unwrap_or_default()
            .into_iter(),
    );
    ret.sort();
    return anyhow::Ok(ret);
}
