//! Source representation for FMPL values and AST.
//!
//! This module provides the ability to convert runtime values and AST nodes
//! back into valid FMPL source code. This is essential for:
//! - Debugging and introspection
//! - Serialization of code as source
//! - Pretty-printing of internal representations

use crate::ast::*;
use crate::grammar::{CharRange, Grammar, Pattern as GrammarPattern};
use crate::object::ObjectDb;
use crate::value::{Lambda, Partial, Stream, StreamOp, Value};
use smol_str::SmolStr;
use std::fmt::{self, Display};
use std::sync::Arc;

/// Trait for converting to FMPL source representation.
pub trait SourceRepr {
    /// Convert to FMPL source code string.
    fn source_repr(&self) -> String;
}

// =============================================================================
// AST Display implementations (unparser)
// =============================================================================

impl Display for BinOp {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            BinOp::Add => write!(f, "+"),
            BinOp::Sub => write!(f, "-"),
            BinOp::Mul => write!(f, "*"),
            BinOp::Div => write!(f, "/"),
            BinOp::Mod => write!(f, "%"),
            BinOp::Eq => write!(f, "=="),
            BinOp::NotEq => write!(f, "!="),
            BinOp::Lt => write!(f, "<"),
            BinOp::Gt => write!(f, ">"),
            BinOp::LtEq => write!(f, "<="),
            BinOp::GtEq => write!(f, ">="),
            BinOp::And => write!(f, "&&"),
            BinOp::Or => write!(f, "||"),
            BinOp::Pipe => write!(f, "|>"),
        }
    }
}

impl Display for UnaryOp {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            UnaryOp::Neg => write!(f, "-"),
            UnaryOp::Not => write!(f, "!"),
        }
    }
}

impl Display for Visibility {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Visibility::Private => write!(f, "#private"),
            Visibility::Public => write!(f, "#public"),
            Visibility::Protected => write!(f, "#protected"),
        }
    }
}

impl Display for Pattern {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Pattern::Var(name) => write!(f, "{}", name),
            Pattern::Wildcard => write!(f, "_"),
            Pattern::Int(n) => write!(f, "{}", n),
            Pattern::Float(n) => write!(f, "{}", n),
            Pattern::String(s) => write!(f, "\"{}\"", s),
            Pattern::Symbol(s) => write!(f, ":{}", s),
            Pattern::List(pats, tail) => {
                write!(f, "[")?;
                for (i, p) in pats.iter().enumerate() {
                    if i > 0 {
                        write!(f, ", ")?;
                    }
                    write!(f, "{}", p)?;
                }
                if let Some(t) = tail {
                    write!(f, " | {}", t)?;
                }
                write!(f, "]")
            }
            Pattern::Map(entries) => {
                write!(f, "%{{")?;
                for (i, (k, p)) in entries.iter().enumerate() {
                    if i > 0 {
                        write!(f, ", ")?;
                    }
                    write!(f, "{}: {}", k, p)?;
                }
                write!(f, "}}")
            }
            Pattern::Constructor(name, pats) => {
                write!(f, "{}(", name)?;
                for (i, p) in pats.iter().enumerate() {
                    if i > 0 {
                        write!(f, ", ")?;
                    }
                    write!(f, "{}", p)?;
                }
                write!(f, ")")
            }
            Pattern::As(inner, name) => write!(f, "{} as {}", inner, name),
        }
    }
}

impl Display for LetBinding {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            LetBinding::Simple(name, Some(val)) => write!(f, "{} = {}", name, val),
            LetBinding::Simple(name, None) => write!(f, "{}", name),
            LetBinding::Destructure(pat, val) => write!(f, "{} = {}", pat, val),
        }
    }
}

impl Display for MapEntry {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            MapEntry::Symbol(key, val) => write!(f, "{}: {}", key, val),
            MapEntry::Computed(key, val) => write!(f, "{} => {}", key, val),
        }
    }
}

impl Display for Arg {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Arg::Expr(e) => write!(f, "{}", e),
            Arg::Placeholder => write!(f, "_"),
        }
    }
}

impl Display for MatchCase {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.pattern)?;
        if let Some(guard) = &self.guard {
            write!(f, " when {}", guard)?;
        }
        write!(f, " => {}", self.body)
    }
}

impl Display for Binding {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.name)?;
        if self.has_params {
            write!(f, "(")?;
            for (i, p) in self.params.iter().enumerate() {
                if i > 0 {
                    write!(f, ", ")?;
                }
                write!(f, "{}", p)?;
            }
            write!(f, ")")?;
        }
        write!(f, ": {}", self.value)
    }
}

impl Display for ObjectDef {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "object ^{}", self.name)?;
        if !self.parents.is_empty() {
            write!(f, "(")?;
            for (i, p) in self.parents.iter().enumerate() {
                if i > 0 {
                    write!(f, ", ")?;
                }
                write!(f, "^{}", p)?;
            }
            write!(f, ")")?;
        }
        write!(f, " {{")?;

        let mut current_vis = Visibility::Private;
        for binding in &self.bindings {
            if binding.visibility != current_vis {
                write!(f, " .{}; ", binding.visibility)?;
                current_vis = binding.visibility;
            }
            write!(f, " {}; ", binding)?;
        }

        for facet in &self.facets {
            write!(f, " #facets {}(", facet.name)?;
            for (i, m) in facet.members.iter().enumerate() {
                if i > 0 {
                    write!(f, ", ")?;
                }
                write!(f, "{}", m)?;
            }
            write!(f, ")")?;
            if facet.terminal {
                write!(f, " terminal")?;
            }
            write!(f, "; ")?;
        }

        write!(f, "}}")
    }
}

impl Display for Expr {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Expr::Int(n) => write!(f, "{}", n),
            Expr::Float(n) => write!(f, "{}", n),
            Expr::String(s) => write!(f, "\"{}\"", escape_string(s)),
            Expr::Symbol(s) => write!(f, ":{}", s),
            Expr::Tagged(tag, args) => {
                write!(f, ":{}", tag)?;
                if !args.is_empty() {
                    write!(f, "(")?;
                    for (i, arg) in args.iter().enumerate() {
                        if i > 0 {
                            write!(f, ", ")?;
                        }
                        write!(f, "{}", arg)?;
                    }
                    write!(f, ")")?;
                }
                Ok(())
            }
            Expr::Bool(b) => write!(f, "{}", b),
            Expr::Null => write!(f, "null"),
            Expr::Ident(name) => write!(f, "{}", name),
            Expr::Qualified(qn) => write!(f, "{}", qn),
            Expr::ObjTag(name) => write!(f, "^{}", name),
            Expr::FnTag(name) => write!(f, "@{}", name),
            Expr::Self_ => write!(f, "self"),
            Expr::Parent => write!(f, "parent"),
            Expr::Caller => write!(f, "caller"),
            Expr::User => write!(f, "user"),
            Expr::Args => write!(f, "args"),

            Expr::List(items) => {
                write!(f, "[")?;
                for (i, item) in items.iter().enumerate() {
                    if i > 0 {
                        write!(f, ", ")?;
                    }
                    write!(f, "{}", item)?;
                }
                write!(f, "]")
            }
            Expr::ListCons(head, tail) => write!(f, "[{} | {}]", head, tail),
            Expr::Map(entries) => {
                write!(f, "%{{")?;
                for (i, entry) in entries.iter().enumerate() {
                    if i > 0 {
                        write!(f, ", ")?;
                    }
                    write!(f, "{}", entry)?;
                }
                write!(f, "}}")
            }

            Expr::Binary(left, op, right) => write!(f, "({} {} {})", left, op, right),
            Expr::Unary(op, expr) => write!(f, "{}{}", op, expr),

            Expr::Index(expr, idx) => write!(f, "{}[{}]", expr, idx),
            Expr::Slice(expr, start, end) => write!(f, "{}[{}..{}]", expr, start, end),

            Expr::Call(func, args) => {
                write!(f, "{}(", func)?;
                for (i, arg) in args.iter().enumerate() {
                    if i > 0 {
                        write!(f, ", ")?;
                    }
                    write!(f, "{}", arg)?;
                }
                write!(f, ")")
            }
            Expr::PropAccess(expr, prop) => write!(f, "{}.{}", expr, prop),
            Expr::MethodCall(expr, method, args) => {
                write!(f, "{}.{}(", expr, method)?;
                for (i, arg) in args.iter().enumerate() {
                    if i > 0 {
                        write!(f, ", ")?;
                    }
                    write!(f, "{}", arg)?;
                }
                write!(f, ")")
            }

            Expr::If(cond, then_expr, else_expr) => {
                write!(f, "if {} then {}", cond, then_expr)?;
                if let Some(else_e) = else_expr {
                    write!(f, " else {}", else_e)?;
                }
                Ok(())
            }
            Expr::While(cond, body) => write!(f, "while {} do {}", cond, body),
            Expr::DoWhile(body, cond) => write!(f, "do {} while {}", body, cond),
            Expr::For(pattern, iterable, body) => {
                write!(f, "for {} in {} {}", pattern, iterable, body)
            }
            Expr::Fold {
                initial,
                acc_var,
                iterable,
                body,
            } => {
                write!(f, "fold {}, {} in {} {}", initial, acc_var, iterable, body)
            }
            Expr::Foldr {
                initial,
                acc_var,
                iterable,
                body,
            } => {
                write!(f, "foldr {}, {} in {} {}", initial, acc_var, iterable, body)
            }
            Expr::MapEach {
                elem_var,
                iterable,
                body,
            } => {
                write!(f, "map {} in {} {}", elem_var, iterable, body)
            }
            Expr::Filter {
                elem_var,
                iterable,
                body,
            } => {
                write!(f, "filter {} in {} {}", elem_var, iterable, body)
            }
            Expr::Return(Some(expr)) => write!(f, "return {}", expr),
            Expr::Return(None) => write!(f, "return"),

            Expr::Lambda(params, body) => {
                write!(f, "lambda (")?;
                for (i, p) in params.iter().enumerate() {
                    if i > 0 {
                        write!(f, ", ")?;
                    }
                    write!(f, "{}", p)?;
                }
                write!(f, ") {}", body)
            }
            Expr::ShortLambda(param, body) => write!(f, "\\{} {}", param, body),
            Expr::Let(bindings, body) => {
                write!(f, "let (")?;
                for (i, b) in bindings.iter().enumerate() {
                    if i > 0 {
                        write!(f, ", ")?;
                    }
                    write!(f, "{}", b)?;
                }
                write!(f, ") {}", body)
            }
            Expr::LetStmt(name, expr) => {
                write!(f, "let {} = {}", name, expr)
            }
            Expr::Assignment(target, value) => {
                write!(f, "{} = {}", target, value)
            }

            Expr::Sequence(exprs) => {
                write!(f, "{{")?;
                for (i, e) in exprs.iter().enumerate() {
                    if i > 0 {
                        write!(f, "; ")?;
                    }
                    write!(f, "{}", e)?;
                }
                write!(f, "}}")
            }

            Expr::ObjectDef(def) => write!(f, "{}", def),

            Expr::Match(expr, cases) => {
                write!(f, "match {} {{", expr)?;
                for case in cases {
                    write!(f, " {};", case)?;
                }
                write!(f, " }}")
            }

            Expr::Spawn(expr, args) => {
                write!(f, "spawn {}(", expr)?;
                for (i, arg) in args.iter().enumerate() {
                    if i > 0 {
                        write!(f, ", ")?;
                    }
                    write!(f, "{}", arg)?;
                }
                write!(f, ")")
            }
            Expr::SyncCall(expr) => write!(f, "${}", expr),
            Expr::AsyncCall(expr) => write!(f, "<- {}", expr),

            Expr::TryCatch {
                body,
                error_binding,
                catch_body,
            } => {
                write!(
                    f,
                    "try {{ {} }} catch {} {{ {} }}",
                    body, error_binding, catch_body
                )
            }

            Expr::Throw(expr) => write!(f, "throw {}", expr),

            Expr::FacetAccess(expr, facet) => write!(f, "{}.as(:{})", expr, facet),

            Expr::Placeholder => write!(f, "_"),

            Expr::GrammarApply {
                input,
                grammar,
                rule,
            } => {
                write!(f, "{} @ {}.{}", input, grammar, rule)
            }

            Expr::GrammarLiteral(g) => write!(f, "{}", GrammarRepr(g)),

            Expr::GrammarExtend { base, rules } => {
                write!(f, "{} <: {}", base, GrammarRepr(rules))
            }

            Expr::StreamLiteral(expr) => write!(f, "stream {{ {} }}", expr),
        }
    }
}

// =============================================================================
// Grammar representation
// =============================================================================

/// Wrapper for Grammar Display.
struct GrammarRepr<'a>(&'a Grammar);

impl<'a> Display for GrammarRepr<'a> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "grammar")?;
        if !self.0.name.is_empty() && self.0.name != "_anon" {
            write!(f, " {}", self.0.name)?;
        }
        if let Some(parent) = &self.0.parent {
            write!(f, " <: {}", parent)?;
        }
        write!(f, " {{")?;
        for (name, rule) in &self.0.rules {
            write!(f, " {} = {}", name, GrammarPatternRepr(&rule.pattern))?;
            if let Some(action) = &rule.action {
                write!(f, " => {}", action)?;
            }
            write!(f, ";")?;
        }
        write!(f, " }}")
    }
}

/// Wrapper for grammar Pattern Display.
struct GrammarPatternRepr<'a>(&'a GrammarPattern);

impl<'a> Display for GrammarPatternRepr<'a> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self.0 {
            GrammarPattern::Empty => Ok(()),
            GrammarPattern::Any => write!(f, "."),
            GrammarPattern::Char(c) => write!(f, "'{}'", escape_char(*c)),
            GrammarPattern::Literal(s) => write!(f, "\"{}\"", escape_string(s)),
            GrammarPattern::CharClass(ranges) => {
                write!(f, "[")?;
                for range in ranges {
                    match range {
                        CharRange::Char(c) => write!(f, "{}", c)?,
                        CharRange::Range(start, end) => write!(f, "{}-{}", start, end)?,
                    }
                }
                write!(f, "]")
            }
            GrammarPattern::NegCharClass(ranges) => {
                write!(f, "[^")?;
                for range in ranges {
                    match range {
                        CharRange::Char(c) => write!(f, "{}", c)?,
                        CharRange::Range(start, end) => write!(f, "{}-{}", start, end)?,
                    }
                }
                write!(f, "]")
            }
            GrammarPattern::Rule(name) => write!(f, "{}", name),
            GrammarPattern::Super(name) => write!(f, "^{}", name),
            GrammarPattern::Seq(pats) => {
                for (i, p) in pats.iter().enumerate() {
                    if i > 0 {
                        write!(f, " ")?;
                    }
                    write!(f, "{}", GrammarPatternRepr(p))?;
                }
                Ok(())
            }
            GrammarPattern::Choice(pats) => {
                for (i, p) in pats.iter().enumerate() {
                    if i > 0 {
                        write!(f, " | ")?;
                    }
                    write!(f, "{}", GrammarPatternRepr(p))?;
                }
                Ok(())
            }
            GrammarPattern::Star(p) => write!(f, "{}*", GrammarPatternRepr(p)),
            GrammarPattern::Plus(p) => write!(f, "{}+", GrammarPatternRepr(p)),
            GrammarPattern::Optional(p) => write!(f, "{}?", GrammarPatternRepr(p)),
            GrammarPattern::Lookahead(p) => write!(f, "&{}", GrammarPatternRepr(p)),
            GrammarPattern::Not(p) => write!(f, "!{}", GrammarPatternRepr(p)),
            GrammarPattern::Bind(p, name) => write!(f, "{}:{}", GrammarPatternRepr(p), name),
            GrammarPattern::Action(p, expr) => write!(f, "{} => {}", GrammarPatternRepr(p), expr),
            GrammarPattern::Predicate(expr) => write!(f, "&{{ {} }}", expr),
            GrammarPattern::Guard(p, expr) => write!(f, "{} when {}", GrammarPatternRepr(p), expr),
            GrammarPattern::Apply(p) => write!(f, "~{}", GrammarPatternRepr(p)),
            GrammarPattern::MatchValue(val) => write!(f, "{}", val.source_repr()),
            GrammarPattern::MatchType(ty) => write!(f, "<{}>", ty),
            GrammarPattern::ListMatch(pats, tail) => {
                write!(f, "[")?;
                for (i, p) in pats.iter().enumerate() {
                    if i > 0 {
                        write!(f, ", ")?;
                    }
                    write!(f, "{}", GrammarPatternRepr(p))?;
                }
                if let Some(t) = tail {
                    write!(f, " | {}", GrammarPatternRepr(t))?;
                }
                write!(f, "]")
            }
            GrammarPattern::MapMatch(entries) => {
                write!(f, "%{{")?;
                for (i, (k, p)) in entries.iter().enumerate() {
                    if i > 0 {
                        write!(f, ", ")?;
                    }
                    write!(f, "{}: {}", k, GrammarPatternRepr(p))?;
                }
                write!(f, "}}")
            }
            GrammarPattern::SymbolMatch(sym) => write!(f, ":{}", sym),
            GrammarPattern::SymbolLiteral(sym) => write!(f, ":{}", sym),
            GrammarPattern::TagMatch(tag, pats) => {
                write!(f, ":{}", tag)?;
                if !pats.is_empty() {
                    write!(f, "(")?;
                    for (i, p) in pats.iter().enumerate() {
                        if i > 0 {
                            write!(f, ", ")?;
                        }
                        write!(f, "{}", GrammarPatternRepr(p))?;
                    }
                    write!(f, ")")?;
                }
                Ok(())
            }
            GrammarPattern::End => write!(f, "end"),

            // Binary patterns
            GrammarPattern::Byte(b) => write!(f, "0x{:02x}", b),
            GrammarPattern::ByteRange(start, end) => write!(f, "0x{:02x}..0x{:02x}", start, end),
            GrammarPattern::Bytes(n) => write!(f, "bytes({})", n),
            GrammarPattern::UInt8 => write!(f, "uint8"),
            GrammarPattern::UInt16BE => write!(f, "uint16be"),
            GrammarPattern::UInt16LE => write!(f, "uint16le"),
            GrammarPattern::UInt32BE => write!(f, "uint32be"),
            GrammarPattern::UInt32LE => write!(f, "uint32le"),
            GrammarPattern::Int8 => write!(f, "int8"),
            GrammarPattern::Int16BE => write!(f, "int16be"),
            GrammarPattern::Int16LE => write!(f, "int16le"),
            GrammarPattern::Int32BE => write!(f, "int32be"),
            GrammarPattern::Int32LE => write!(f, "int32le"),
        }
    }
}

// =============================================================================
// Value source representation
// =============================================================================

impl SourceRepr for Value {
    fn source_repr(&self) -> String {
        match self {
            Value::Null => "null".to_string(),
            Value::Bool(b) => b.to_string(),
            Value::Int(n) => n.to_string(),
            Value::Float(n) => n.to_string(),
            Value::String(s) => format!("\"{}\"", escape_string(s)),
            Value::Symbol(s) => format!(":{}", s),
            Value::List(l) => {
                let mut result = String::from("[");
                for (i, v) in l.iter().enumerate() {
                    if i > 0 {
                        result.push_str(", ");
                    }
                    result.push_str(&v.source_repr());
                }
                result.push(']');
                result
            }
            Value::Map(m) => {
                let mut result = String::from("%{");
                for (i, (k, v)) in m.iter().enumerate() {
                    if i > 0 {
                        result.push_str(", ");
                    }
                    result.push_str(k);
                    result.push_str(": ");
                    result.push_str(&v.source_repr());
                }
                result.push('}');
                result
            }
            Value::Lambda(lambda) => lambda.source_repr(),
            Value::Partial(partial) => partial.source_repr(),
            Value::Grammar(g) => format!("{}", GrammarRepr(g)),
            Value::Tagged(tag, children) => {
                let mut result = format!(":{}", tag);
                if !children.is_empty() {
                    result.push('(');
                    for (i, child) in children.iter().enumerate() {
                        if i > 0 {
                            result.push_str(", ");
                        }
                        result.push_str(&child.source_repr());
                    }
                    result.push(')');
                }
                result
            }
            Value::Stream(s) => s.source_repr(),
            // Objects require ObjectDb access - return a placeholder that can be filled in
            Value::Object(id) => format!("<object #{}>", id),
            Value::AsyncStream(s) => format!("<async_stream #{}>", s.lock().unwrap().id()),
            Value::Sink(s) => format!("<sink #{}>", s.id()),
            Value::SuspendedStream(source) => format!("<suspended_stream {:?}>", source),
            Value::SuspendedSink(source) => format!("<suspended_sink {:?}>", source),
            Value::TupleSpace(_) => "<tuplespace>".to_string(),
            Value::TupleSpaceFacet(_) => "<tuplespace_facet>".to_string(),
            Value::Cursor(c) => format!("<cursor branch:{} pos:{}>", c.branch_id, c.position.index),
            Value::Code(_) => "<code>".to_string(),
        }
    }
}

impl SourceRepr for Lambda {
    fn source_repr(&self) -> String {
        // If we have stored source, use it
        if let Some(source) = &self.code.source {
            return source.to_string();
        }

        // Otherwise, generate from params (body is compiled, can't decompile yet)
        let params = self.params.join(", ");
        if self.params.len() == 1 {
            format!("\\{} <compiled>", self.params[0])
        } else {
            format!("lambda ({}) <compiled>", params)
        }
    }
}

impl SourceRepr for Partial {
    fn source_repr(&self) -> String {
        let func_repr = self.func.source_repr();
        let mut args = Vec::new();
        for arg in &self.args {
            match arg {
                Some(v) => args.push(v.source_repr()),
                None => args.push("_".to_string()),
            }
        }
        format!("{}({})", func_repr, args.join(", "))
    }
}

impl SourceRepr for Stream {
    fn source_repr(&self) -> String {
        let mut result = format!("stream {{ {} }}", self.source.source_repr());
        for op in &self.ops {
            match op {
                StreamOp::Map(f) => {
                    result = format!("{} |> map({})", result, f.source_repr());
                }
                StreamOp::Filter(f) => {
                    result = format!("{} |> filter({})", result, f.source_repr());
                }
                StreamOp::FlatMap(f) => {
                    result = format!("{} |> flatMap({})", result, f.source_repr());
                }
                StreamOp::Reduce(f) => {
                    result = format!("{} |> reduce({})", result, f.source_repr());
                }
                StreamOp::Collect => {
                    result = format!("{} |> collect", result);
                }
                StreamOp::Take { n } => {
                    result = format!("{} |> take({})", result, n.source_repr());
                }
                StreamOp::Drop { n } => {
                    result = format!("{} |> drop({})", result, n.source_repr());
                }
                StreamOp::Parse { grammar, rule } => {
                    result = format!("{} |> parse({}.{})", result, grammar.source_repr(), rule);
                }
                StreamOp::AsyncParse { grammar, rule } => {
                    result = format!(
                        "{} |> asyncParse({}.{})",
                        result,
                        grammar.source_repr(),
                        rule
                    );
                }
            }
        }
        result
    }
}

// =============================================================================
// Object source representation (needs ObjectDb access)
// =============================================================================

/// Generate source representation for an object given access to the ObjectDb.
pub fn object_source_repr(db: &Arc<std::sync::Mutex<ObjectDb>>, id: u64) -> String {
    let db_guard = db.lock().unwrap();
    let Some(obj) = db_guard.get(id) else {
        return format!("<object #{} not found>", id);
    };

    let mut result = String::new();

    // Try to find the object's name
    let name = find_object_name(&db_guard, id)
        .map(|n| n.to_string())
        .unwrap_or_else(|| format!("_obj{}", id));

    result.push_str("object ");
    result.push_str(&name);

    // Parents
    if let Some(parent_id) = obj.parent
        && let Some(parent_name) = find_object_name(&db_guard, parent_id)
    {
        result.push_str(&format!("({})", parent_name));
    }

    result.push_str(" {\n");

    // Properties
    for (prop_name, value) in &obj.properties {
        result.push_str(&format!("    {}: {};\n", prop_name, value.source_repr()));
    }

    // Methods
    for (method_name, method) in &obj.methods {
        let params = method.params.join(", ");
        if let Some(source) = &method.code.source {
            result.push_str(&format!("    {}({}): {};\n", method_name, params, source));
        } else {
            result.push_str(&format!("    {}({}): <compiled>;\n", method_name, params));
        }
    }

    result.push('}');
    result
}

/// Find the name of an object by ID (reverse lookup).
fn find_object_name(db: &ObjectDb, id: u64) -> Option<SmolStr> {
    for (name, &obj_id) in db.named_objects() {
        if obj_id == id {
            return Some(name.clone());
        }
    }
    None
}

// =============================================================================
// Helper functions
// =============================================================================

/// Escape a string for FMPL source output.
fn escape_string(s: &str) -> String {
    let mut result = String::with_capacity(s.len());
    for c in s.chars() {
        result.push_str(&escape_char(c));
    }
    result
}

/// Escape a single character for FMPL source output.
fn escape_char(c: char) -> String {
    match c {
        '"' => "\\\"".to_string(),
        '\'' => "\\'".to_string(),
        '\\' => "\\\\".to_string(),
        '\n' => "\\n".to_string(),
        '\r' => "\\r".to_string(),
        '\t' => "\\t".to_string(),
        c => c.to_string(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{Vm, eval};
    use std::sync::Arc;

    #[test]
    fn test_value_primitives() {
        assert_eq!(Value::Null.source_repr(), "null");
        assert_eq!(Value::Bool(true).source_repr(), "true");
        assert_eq!(Value::Int(42).source_repr(), "42");
        assert_eq!(Value::Float(3.125).source_repr(), "3.125");
        assert_eq!(
            Value::String(SmolStr::new("hello")).source_repr(),
            "\"hello\""
        );
        assert_eq!(Value::Symbol(SmolStr::new("foo")).source_repr(), ":foo");
    }

    #[test]
    fn test_value_list() {
        let list = Value::List(Arc::new(vec![Value::Int(1), Value::Int(2), Value::Int(3)]));
        assert_eq!(list.source_repr(), "[1, 2, 3]");
    }

    #[test]
    fn test_value_map() {
        use std::collections::HashMap;
        let mut map = HashMap::new();
        map.insert(SmolStr::new("a"), Value::Int(1));
        let map_val = Value::Map(Arc::new(map));
        assert_eq!(map_val.source_repr(), "%{a: 1}");
    }

    #[test]
    fn test_expr_display() {
        let expr = Expr::Binary(Box::new(Expr::Int(1)), BinOp::Add, Box::new(Expr::Int(2)));
        assert_eq!(format!("{}", expr), "(1 + 2)");
    }

    #[test]
    fn test_lambda_display() {
        let expr = Expr::Lambda(
            vec![SmolStr::new("x"), SmolStr::new("y")],
            Box::new(Expr::Binary(
                Box::new(Expr::Ident(SmolStr::new("x"))),
                BinOp::Add,
                Box::new(Expr::Ident(SmolStr::new("y"))),
            )),
        );
        assert_eq!(format!("{}", expr), "lambda (x, y) (x + y)");
    }

    #[test]
    fn test_string_escaping() {
        let s = Value::String(SmolStr::new("hello\nworld\"!"));
        assert_eq!(s.source_repr(), "\"hello\\nworld\\\"!\"");
    }

    #[test]
    fn test_lambda_source_preserved() {
        let mut vm = Vm::new();
        // Create a lambda and check its source representation
        let lambda = eval(&mut vm, r#"\x x + 1"#).unwrap();
        let repr = lambda.source_repr();
        // The body should be preserved (x + 1 becomes (x + 1) with parens)
        assert!(
            repr.contains("x") && repr.contains("+") && repr.contains("1"),
            "got: {}",
            repr
        );
    }

    #[test]
    fn test_lambda_with_multiple_params_source() {
        let mut vm = Vm::new();
        let lambda = eval(&mut vm, r#"lambda (x, y) x + y"#).unwrap();
        let repr = lambda.source_repr();
        assert!(
            repr.contains("x") && repr.contains("+") && repr.contains("y"),
            "got: {}",
            repr
        );
    }

    #[test]
    fn test_object_source_repr() {
        let mut vm = Vm::new();
        // Create an object
        let _ = eval(&mut vm, r#"object ^test { foo: 42; bar(x): x + 1 }"#).unwrap();
        // Get the source representation
        let repr = object_source_repr(&vm.objects, 1);
        assert!(
            repr.contains("object test"),
            "should have object name: {}",
            repr
        );
        assert!(repr.contains("foo:"), "should have property: {}", repr);
        assert!(repr.contains("bar("), "should have method: {}", repr);
    }

    // =========================================================================
    // Grammar unparsing tests
    // =========================================================================

    #[test]
    fn test_grammar_pattern_any() {
        use crate::grammar::Pattern as GP;
        let repr = format!("{}", GrammarPatternRepr(&GP::Any));
        assert_eq!(repr, ".");
    }

    #[test]
    fn test_grammar_pattern_char() {
        use crate::grammar::Pattern as GP;
        let repr = format!("{}", GrammarPatternRepr(&GP::Char('x')));
        assert_eq!(repr, "'x'");
    }

    #[test]
    fn test_grammar_pattern_literal() {
        use crate::grammar::Pattern as GP;
        let repr = format!("{}", GrammarPatternRepr(&GP::Literal("hello".into())));
        assert_eq!(repr, "\"hello\"");
    }

    #[test]
    fn test_grammar_pattern_literal_escaping() {
        use crate::grammar::Pattern as GP;
        let repr = format!(
            "{}",
            GrammarPatternRepr(&GP::Literal("hello\nworld".into()))
        );
        assert_eq!(repr, "\"hello\\nworld\"");
    }

    #[test]
    fn test_grammar_pattern_char_class() {
        use crate::grammar::{CharRange, Pattern as GP};
        let repr = format!(
            "{}",
            GrammarPatternRepr(&GP::CharClass(vec![
                CharRange::Range('a', 'z'),
                CharRange::Char('_'),
            ]))
        );
        assert_eq!(repr, "[a-z_]");
    }

    #[test]
    fn test_grammar_pattern_neg_char_class() {
        use crate::grammar::{CharRange, Pattern as GP};
        let repr = format!(
            "{}",
            GrammarPatternRepr(&GP::NegCharClass(vec![CharRange::Range('0', '9'),]))
        );
        assert_eq!(repr, "[^0-9]");
    }

    #[test]
    fn test_grammar_pattern_rule() {
        use crate::grammar::Pattern as GP;
        let repr = format!("{}", GrammarPatternRepr(&GP::Rule("digit".into())));
        assert_eq!(repr, "digit");
    }

    #[test]
    fn test_grammar_pattern_super() {
        use crate::grammar::Pattern as GP;
        let repr = format!("{}", GrammarPatternRepr(&GP::Super("expr".into())));
        assert_eq!(repr, "^expr");
    }

    #[test]
    fn test_grammar_pattern_seq() {
        use crate::grammar::Pattern as GP;
        let repr = format!(
            "{}",
            GrammarPatternRepr(&GP::Seq(vec![
                GP::Literal("hello".into()),
                GP::Any,
                GP::Literal("world".into()),
            ]))
        );
        assert_eq!(repr, "\"hello\" . \"world\"");
    }

    #[test]
    fn test_grammar_pattern_choice() {
        use crate::grammar::Pattern as GP;
        let repr = format!(
            "{}",
            GrammarPatternRepr(&GP::Choice(vec![
                GP::Char('a'),
                GP::Char('b'),
                GP::Char('c'),
            ]))
        );
        assert_eq!(repr, "'a' | 'b' | 'c'");
    }

    #[test]
    fn test_grammar_pattern_star() {
        use crate::grammar::Pattern as GP;
        let repr = format!(
            "{}",
            GrammarPatternRepr(&GP::Star(Box::new(GP::Rule("digit".into()))))
        );
        assert_eq!(repr, "digit*");
    }

    #[test]
    fn test_grammar_pattern_plus() {
        use crate::grammar::Pattern as GP;
        let repr = format!(
            "{}",
            GrammarPatternRepr(&GP::Plus(Box::new(GP::Rule("digit".into()))))
        );
        assert_eq!(repr, "digit+");
    }

    #[test]
    fn test_grammar_pattern_optional() {
        use crate::grammar::Pattern as GP;
        let repr = format!(
            "{}",
            GrammarPatternRepr(&GP::Optional(Box::new(GP::Char('-'))))
        );
        assert_eq!(repr, "'-'?");
    }

    #[test]
    fn test_grammar_pattern_lookahead() {
        use crate::grammar::Pattern as GP;
        let repr = format!(
            "{}",
            GrammarPatternRepr(&GP::Lookahead(Box::new(GP::Rule("eof".into()))))
        );
        assert_eq!(repr, "&eof");
    }

    #[test]
    fn test_grammar_pattern_not() {
        use crate::grammar::Pattern as GP;
        let repr = format!("{}", GrammarPatternRepr(&GP::Not(Box::new(GP::Char('\n')))));
        assert_eq!(repr, "!'\\n'");
    }

    #[test]
    fn test_grammar_pattern_bind() {
        use crate::grammar::Pattern as GP;
        let repr = format!(
            "{}",
            GrammarPatternRepr(&GP::Bind(Box::new(GP::Rule("digit".into())), "d".into()))
        );
        assert_eq!(repr, "digit:d");
    }

    #[test]
    fn test_grammar_pattern_predicate() {
        use crate::grammar::Pattern as GP;
        let repr = format!(
            "{}",
            GrammarPatternRepr(&GP::Predicate(Expr::Binary(
                Box::new(Expr::Ident("x".into())),
                BinOp::Gt,
                Box::new(Expr::Int(0)),
            )))
        );
        assert_eq!(repr, "&{ (x > 0) }");
    }

    #[test]
    fn test_grammar_pattern_guard() {
        use crate::grammar::Pattern as GP;
        let repr = format!(
            "{}",
            GrammarPatternRepr(&GP::Guard(
                Box::new(GP::Rule("digit".into())),
                Expr::Binary(
                    Box::new(Expr::Ident("d".into())),
                    BinOp::Lt,
                    Box::new(Expr::Int(5)),
                )
            ))
        );
        assert_eq!(repr, "digit when (d < 5)");
    }

    #[test]
    fn test_grammar_pattern_action() {
        use crate::grammar::Pattern as GP;
        let repr = format!(
            "{}",
            GrammarPatternRepr(&GP::Action(
                Box::new(GP::Bind(Box::new(GP::Rule("digit".into())), "d".into())),
                Expr::Ident("d".into()),
            ))
        );
        assert_eq!(repr, "digit:d => d");
    }

    #[test]
    fn test_grammar_pattern_list_match() {
        use crate::grammar::Pattern as GP;
        let repr = format!(
            "{}",
            GrammarPatternRepr(&GP::ListMatch(
                vec![GP::Rule("head".into()), GP::Rule("second".into())],
                Some(Box::new(GP::Rule("tail".into()))),
            ))
        );
        assert_eq!(repr, "[head, second | tail]");
    }

    #[test]
    fn test_grammar_pattern_list_match_no_tail() {
        use crate::grammar::Pattern as GP;
        let repr = format!(
            "{}",
            GrammarPatternRepr(&GP::ListMatch(
                vec![GP::Literal("add".into()), GP::Any, GP::Any],
                None,
            ))
        );
        assert_eq!(repr, "[\"add\", ., .]");
    }

    #[test]
    fn test_grammar_pattern_map_match() {
        use crate::grammar::Pattern as GP;
        let repr = format!(
            "{}",
            GrammarPatternRepr(&GP::MapMatch(vec![
                ("type".into(), GP::Literal("node".into())),
                ("value".into(), GP::Rule("expr".into())),
            ]))
        );
        assert_eq!(repr, "%{type: \"node\", value: expr}");
    }

    #[test]
    fn test_grammar_pattern_symbol_match() {
        use crate::grammar::Pattern as GP;
        let repr = format!("{}", GrammarPatternRepr(&GP::SymbolMatch("foo".into())));
        assert_eq!(repr, ":foo");
    }

    #[test]
    fn test_grammar_pattern_end() {
        use crate::grammar::Pattern as GP;
        let repr = format!("{}", GrammarPatternRepr(&GP::End));
        assert_eq!(repr, "end");
    }

    #[test]
    fn test_grammar_pattern_apply() {
        use crate::grammar::Pattern as GP;
        let repr = format!(
            "{}",
            GrammarPatternRepr(&GP::Apply(Box::new(GP::Rule("expr".into()))))
        );
        assert_eq!(repr, "~expr");
    }

    #[test]
    fn test_grammar_pattern_match_value() {
        use crate::grammar::Pattern as GP;
        let repr = format!("{}", GrammarPatternRepr(&GP::MatchValue(Value::Int(42))));
        assert_eq!(repr, "42");
    }

    #[test]
    fn test_grammar_pattern_match_type() {
        use crate::grammar::Pattern as GP;
        let repr = format!("{}", GrammarPatternRepr(&GP::MatchType("string".into())));
        assert_eq!(repr, "<string>");
    }

    #[test]
    fn test_grammar_pattern_binary() {
        use crate::grammar::Pattern as GP;
        // Test various binary patterns
        assert_eq!(format!("{}", GrammarPatternRepr(&GP::Byte(0x89))), "0x89");
        assert_eq!(
            format!("{}", GrammarPatternRepr(&GP::ByteRange(0x00, 0x7f))),
            "0x00..0x7f"
        );
        assert_eq!(format!("{}", GrammarPatternRepr(&GP::Bytes(4))), "bytes(4)");
        assert_eq!(format!("{}", GrammarPatternRepr(&GP::UInt8)), "uint8");
        assert_eq!(format!("{}", GrammarPatternRepr(&GP::UInt16BE)), "uint16be");
        assert_eq!(format!("{}", GrammarPatternRepr(&GP::UInt32LE)), "uint32le");
        assert_eq!(format!("{}", GrammarPatternRepr(&GP::Int8)), "int8");
        assert_eq!(format!("{}", GrammarPatternRepr(&GP::Int16LE)), "int16le");
        assert_eq!(format!("{}", GrammarPatternRepr(&GP::Int32BE)), "int32be");
    }

    #[test]
    fn test_grammar_full_repr() {
        use crate::grammar::{Grammar, Pattern as GP, Rule};
        use std::collections::HashMap;

        let mut rules = HashMap::new();
        rules.insert(
            "digit".into(),
            Rule {
                pattern: GP::CharClass(vec![crate::grammar::CharRange::Range('0', '9')]),
                action: None,
            },
        );
        rules.insert(
            "number".into(),
            Rule {
                pattern: GP::Plus(Box::new(GP::Rule("digit".into()))),
                action: Some(Expr::Ident("digits".into())),
            },
        );

        let grammar = Grammar {
            name: "calc".into(),
            parent: None,
            parent_grammar: None,
            rules,
        };

        let repr = format!("{}", GrammarRepr(&grammar));
        assert!(
            repr.contains("grammar calc"),
            "should have grammar name: {}",
            repr
        );
        assert!(repr.contains("digit ="), "should have digit rule: {}", repr);
        assert!(
            repr.contains("number ="),
            "should have number rule: {}",
            repr
        );
        assert!(repr.contains("[0-9]"), "should have char class: {}", repr);
        assert!(repr.contains("digit+"), "should have plus: {}", repr);
        assert!(repr.contains("=> digits"), "should have action: {}", repr);
    }

    #[test]
    fn test_grammar_value_repr() {
        use crate::grammar::{Grammar, Pattern as GP, Rule};
        use std::collections::HashMap;

        let mut rules = HashMap::new();
        rules.insert(
            "main".into(),
            Rule {
                pattern: GP::Any,
                action: None,
            },
        );

        let grammar = Grammar {
            name: "test".into(),
            parent: None,
            parent_grammar: None,
            rules,
        };

        let val = Value::Grammar(Arc::new(grammar));
        let repr = val.source_repr();
        assert!(repr.contains("grammar test"), "got: {}", repr);
        assert!(repr.contains("main = ."), "got: {}", repr);
    }
}
