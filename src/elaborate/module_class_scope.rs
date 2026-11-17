//! §8.3 / §23.6: module-instance class names belong to their lexical scope.

use super::*;

/// Retain scoped-member shape while rebasing the owner of `C#(...)`.
/// Flattening only the owner produces a hybrid package path that the
/// specialization value reader cannot interpret as a static property.
pub(super) fn specialized_receiver(
    e: &Expression,
    prefix: &str,
    bindings: &HashMap<String, Expression>,
    local_names: &std::collections::HashSet<String>,
    interfaces: &HashMap<String, String>,
) -> Expression {
    if let ExprKind::MemberAccess { expr, member } = &e.kind {
        Expression::new(
            ExprKind::MemberAccess {
                expr: Box::new(specialized_receiver(
                    expr,
                    prefix,
                    bindings,
                    local_names,
                    interfaces,
                )),
                member: member.clone(),
            },
            e.span,
        )
    } else {
        rewrite_expr_impl(e, prefix, bindings, local_names, interfaces)
    }
}

/// §8.25, §23.10: the arguments of a `C#(...)` written in a module instance
/// name that instance's parameters. Qualify each name the instance declares
/// as an expression reference to it is qualified, so instances with
/// different parameter values name different specializations (a bare `W`
/// resolved to one instance's value for every instance). Identifiers in a
/// type position (`virtual iface`, a typedef or class name, §3.13) keep the
/// unqualified type spelling so a package `db#(virtual clk_rst_if)` and a
/// module `db#(virtual clk_rst_if)` name the same specialization. Named-
/// argument labels (`.W(`), scoped names (`p::N`, `C::N`), literals and
/// strings keep their spelling.
pub(super) fn spec_args_text(
    text: &str,
    prefix: &str,
    local_names: &std::collections::HashSet<String>,
) -> String {
    if prefix.is_empty() || local_names.is_empty() {
        return text.to_string();
    }
    let pieces = split_spec_arg_pieces(text);
    let mut out = String::with_capacity(text.len());
    for (n, piece) in pieces.iter().enumerate() {
        if n > 0 {
            out.push(',');
        }
        out.push_str(&qualify_one_spec_arg(piece, prefix, local_names));
    }
    out
}

/// Split `#(...)` text into top-level arguments without breaking string
/// literals or nested brackets (`C#("a,b", W + 1)` stays two arguments).
fn split_spec_arg_pieces(text: &str) -> Vec<&str> {
    let b = text.as_bytes();
    let ident = |c: u8| c.is_ascii_alphanumeric() || c == b'_' || c == b'$';
    let mut depth = 0i32;
    let mut start = 0usize;
    let mut pieces = Vec::new();
    let mut i = 0usize;
    while i < b.len() {
        let c = b[i];
        if c == b'"' {
            i += 1;
            while i < b.len() && b[i] != b'"' {
                i += if b[i] == b'\\' { 2 } else { 1 };
            }
            i = (i + 1).min(b.len());
            continue;
        }
        if c == b'\'' || c.is_ascii_digit() || c == b'$' {
            i += 1;
            while i < b.len() && (ident(b[i]) || b[i] == b'\'') {
                i += 1;
            }
            continue;
        }
        match c {
            b'(' | b'[' | b'{' => depth += 1,
            b')' | b']' | b'}' => depth -= 1,
            b',' if depth == 0 => {
                pieces.push(&text[start..i]);
                start = i + 1;
            }
            _ => {}
        }
        i += 1;
    }
    pieces.push(&text[start..]);
    pieces
}

/// Named arguments (`.W(PW)`) only rewrite the parenthesized body; labels and
/// punctuation outside it keep their original spacing.
fn qualify_named_spec_arg(
    piece: &str,
    prefix: &str,
    local_names: &std::collections::HashSet<String>,
) -> String {
    if let Some(open) = piece.find('(') {
        let mut depth = 0i32;
        let b = piece.as_bytes();
        for (i, &c) in b[open..].iter().enumerate() {
            match c {
                b'(' | b'[' | b'{' => depth += 1,
                b')' | b']' | b'}' => {
                    depth -= 1;
                    if depth == 0 && c == b')' {
                        let head = &piece[..open + 1];
                        let inner = &piece[open + 1..open + i];
                        let tail = &piece[open + i..];
                        let rewritten = match classify_spec_arg(inner) {
                            SpecArgKind::Type => {
                                qualify_spec_type_text(inner.trim(), prefix, local_names)
                            }
                            _ => qualify_spec_expr_text(inner, prefix, local_names),
                        };
                        return format!("{head}{rewritten}{tail}");
                    }
                }
                _ => {}
            }
        }
    }
    qualify_spec_expr_text(piece, prefix, local_names)
}

fn leads_with_virtual_keyword(text: &str) -> bool {
    let t = text.trim();
    let Some(rest) = t.strip_prefix("virtual") else {
        return false;
    };
    rest.is_empty()
        || rest
            .as_bytes()
            .first()
            .is_some_and(|c| c.is_ascii_whitespace())
}

fn parse_spec_arg_type(text: &str) -> Option<DataType> {
    let t = text.trim();
    if t.is_empty() {
        return None;
    }
    let source = format!("module __type_override; typedef {t} __bound_type; endmodule");
    let parsed = crate::parse(&source);
    if !parsed.errors.is_empty() {
        return None;
    }
    let crate::ast::Description::Module(module) = parsed.source.descriptions.into_iter().next()?
    else {
        return None;
    };
    module.items.into_iter().find_map(|item| match item {
        ModuleItem::TypedefDeclaration(t) => Some(t.data_type),
        _ => None,
    })
}

enum SpecArgKind {
    Type,
    Expr,
    Named,
}

/// Choose whether a specialization-argument fragment is a type (`virtual Ifc`,
/// `int`), a value expression (`PW * 2`), or a named wrapper (`.W(PW)`).
fn classify_spec_arg(piece: &str) -> SpecArgKind {
    let t = piece.trim();
    if t.starts_with('.') {
        return SpecArgKind::Named;
    }
    if leads_with_virtual_keyword(t) {
        return SpecArgKind::Type;
    }
    let as_type = parse_spec_arg_type(t);
    let as_expr = crate::parse_override_value(t);
    match (as_type.is_some(), as_expr.is_some()) {
        (true, false) => SpecArgKind::Type,
        (false, true) | (false, false) => SpecArgKind::Expr,
        (true, true) => {
            if t.chars()
                .any(|c| matches!(c, '*' | '+' | '-' | '<' | '>' | '&' | '|' | '?' | ':'))
            {
                SpecArgKind::Expr
            } else if t.contains('\'') {
                SpecArgKind::Expr
            } else {
                // A bare name parses both ways when it could be a type or a
                // value parameter (`PW` of `parameter int PW`) — value wins.
                SpecArgKind::Expr
            }
        }
    }
}

fn qualify_one_spec_arg(
    piece: &str,
    prefix: &str,
    local_names: &std::collections::HashSet<String>,
) -> String {
    let lead = piece.len() - piece.trim_start().len();
    let trail = piece.len() - piece.trim_end().len();
    let (lead_s, rest) = piece.split_at(lead);
    let (body, trail_s) = rest.split_at(rest.len().saturating_sub(trail));
    let rewritten = match classify_spec_arg(body) {
        SpecArgKind::Named => qualify_named_spec_arg(body, prefix, local_names),
        SpecArgKind::Type => qualify_spec_type_text(body.trim(), prefix, local_names),
        SpecArgKind::Expr => qualify_spec_expr_text(body, prefix, local_names),
    };
    format!("{lead_s}{rewritten}{trail_s}")
}

/// Type positions in a specialization argument: spell type names verbatim,
/// but still qualify instance-local VALUE names inside nested `#(...)`.
fn qualify_spec_type_text(
    text: &str,
    prefix: &str,
    local_names: &std::collections::HashSet<String>,
) -> String {
    let b = text.as_bytes();
    let ident = |c: u8| c.is_ascii_alphanumeric() || c == b'_' || c == b'$';
    let mut out = String::with_capacity(text.len());
    let mut i = 0usize;
    let mut type_head = true;
    while i < b.len() {
        let start = i;
        let c = b[i];
        if c.is_ascii_whitespace() {
            i += text[i..].chars().next().map_or(1, char::len_utf8);
            out.push_str(&text[start..i]);
            continue;
        }
        if c == b'#' && i + 1 < b.len() && b[i + 1] == b'(' {
            out.push_str(&text[start..i + 2]);
            i += 2;
            let inner_start = i;
            let mut depth = 1i32;
            while i < b.len() && depth > 0 {
                match b[i] {
                    b'(' | b'[' | b'{' => depth += 1,
                    b')' | b']' | b'}' => depth -= 1,
                    _ => {}
                }
                if depth > 0 {
                    i += 1;
                }
            }
            out.push_str(&spec_args_text(
                &text[inner_start..i],
                prefix,
                local_names,
            ));
            if i < b.len() && b[i] == b')' {
                out.push(')');
                i += 1;
            }
            type_head = false;
            continue;
        }
        if c == b'"' {
            i += 1;
            while i < b.len() && b[i] != b'"' {
                i += if b[i] == b'\\' { 2 } else { 1 };
            }
            i = (i + 1).min(b.len());
            out.push_str(&text[start..i]);
            continue;
        }
        if c == b'\'' || c.is_ascii_digit() || c == b'$' {
            i += 1;
            while i < b.len() && (ident(b[i]) || b[i] == b'\'') {
                i += 1;
            }
            out.push_str(&text[start..i]);
            continue;
        }
        if c.is_ascii_alphabetic() || c == b'_' {
            while i < b.len() && ident(b[i]) {
                i += 1;
            }
            let tok = &text[start..i];
            let after = text[i..].trim_start();
            if type_head && matches!(tok, "virtual" | "interface" | "type") {
                out.push_str(tok);
                continue;
            }
            if type_head && !after.starts_with("::") {
                out.push_str(tok);
                if !after.starts_with('#') {
                    type_head = false;
                }
                continue;
            }
            let before = text[..start].trim_end();
            if local_names.contains(tok)
                && !before.ends_with('.')
                && !before.ends_with("::")
                && !after.starts_with("::")
            {
                out.push_str(prefix);
                out.push_str(tok);
            } else {
                out.push_str(tok);
            }
            continue;
        }
        i += text[i..].chars().next().map_or(1, char::len_utf8);
        out.push_str(&text[start..i]);
    }
    out
}

fn qualify_spec_expr_text(
    text: &str,
    prefix: &str,
    local_names: &std::collections::HashSet<String>,
) -> String {
    let b = text.as_bytes();
    let ident = |c: u8| c.is_ascii_alphanumeric() || c == b'_' || c == b'$';
    let mut out = String::with_capacity(text.len());
    let mut i = 0;
    while i < b.len() {
        let c = b[i];
        let start = i;
        if c == b'"' {
            i += 1;
            while i < b.len() && b[i] != b'"' {
                i += if b[i] == b'\\' { 2 } else { 1 };
            }
            i = (i + 1).min(b.len());
        } else if c == b'\'' || c.is_ascii_digit() || c == b'$' {
            i += 1;
            while i < b.len() && (ident(b[i]) || b[i] == b'\'') {
                i += 1;
            }
        } else if c.is_ascii_alphabetic() || c == b'_' {
            while i < b.len() && ident(b[i]) {
                i += 1;
            }
            let tok = &text[start..i];
            let before = text[..start].trim_end();
            let after = text[i..].trim_start();
            if local_names.contains(tok)
                && !before.ends_with('.')
                && !before.ends_with("::")
                && !after.starts_with("::")
            {
                out.push_str(prefix);
                out.push_str(tok);
                continue;
            }
        } else {
            i += text[i..].chars().next().map_or(1, char::len_utf8);
        }
        out.push_str(&text[start..i]);
    }
    out
}

fn receiver_text(e: &Expression) -> Option<String> {
    match &e.kind {
        ExprKind::Ident(h) if h.root.is_none() && h.path.iter().all(|p| p.selects.is_empty()) => {
            Some(
                h.path
                    .iter()
                    .map(|p| p.name.name.as_str())
                    .collect::<Vec<_>>()
                    .join("::"),
            )
        }
        ExprKind::MemberAccess { expr, member } => {
            Some(format!("{}::{}", receiver_text(expr)?, member.name))
        }
        ExprKind::Specialization {
            base,
            type_args_text,
        } => Some(format!("{}#({})", receiver_text(base)?, type_args_text)),
        _ => None,
    }
}

/// §6.20.3: expression parsing keeps `C#(...)` as a specialization node.
/// Reparse only that uncommon override shape in a type context, preserving
/// builtin type arguments, named arguments and package/nested qualifiers.
pub(super) fn type_override(e: &Expression) -> Option<DataType> {
    let text = receiver_text(e)?;
    if !text.contains("#(") {
        return None;
    }
    let source = format!("module __type_override; typedef {text} __bound_type; endmodule");
    let parsed = crate::parse(&source);
    if !parsed.errors.is_empty() {
        return None;
    }
    let crate::ast::Description::Module(module) = parsed.source.descriptions.into_iter().next()?
    else {
        return None;
    };
    module.items.into_iter().find_map(|item| match item {
        ModuleItem::TypedefDeclaration(t) => Some(t.data_type),
        _ => None,
    })
}

fn argument_text(e: &Expression) -> String {
    if let ExprKind::TypeLiteral(dt) = &e.kind {
        if let Some(receiver) = receiver(dt) {
            if let Some(text) = receiver_text(&receiver) {
                return text;
            }
        }
        if let Some(text) = data_type_to_spec_fragment(dt) {
            return text;
        }
    }
    expr_to_arg_string(e)
}

/// §6.20.3 / §8.23: preserve the class specialization when a module type
/// parameter becomes a scoped receiver in an inlined subroutine or process.
pub(super) fn receiver(dt: &DataType) -> Option<Expression> {
    let DataType::TypeReference {
        name, type_args, ..
    } = dt
    else {
        return None;
    };
    let mut result = None;
    for (name, args) in name
        .scopes
        .iter()
        .map(|s| (&s.name, &s.type_args))
        .chain(std::iter::once((&name.name, type_args)))
    {
        let base = if let Some(expr) = result {
            Expression::new(
                ExprKind::MemberAccess {
                    expr: Box::new(expr),
                    member: name.clone(),
                },
                name.span,
            )
        } else {
            make_ident_expr(&name.name)
        };
        result = Some(if args.is_empty() {
            base
        } else {
            Expression::new(
                ExprKind::Specialization {
                    base: Box::new(base),
                    type_args_text: args.iter().map(argument_text).collect::<Vec<_>>().join(","),
                },
                name.span,
            )
        });
    }
    result
}

fn target<'a>(
    name: &str,
    prefix: &str,
    bindings: &'a HashMap<String, Expression>,
) -> Option<&'a str> {
    let ExprKind::Ident(h) = &bindings.get(name)?.kind else {
        return None;
    };
    let [segment] = h.path.as_slice() else {
        return None;
    };
    (h.root.is_none()
        && segment.selects.is_empty()
        && segment.name.name.strip_prefix(prefix) == Some(name))
    .then_some(segment.name.name.as_str())
}

pub(super) fn data_type(
    dt: &DataType,
    prefix: &str,
    bindings: &HashMap<String, Expression>,
) -> DataType {
    match dt {
        DataType::TypeReference {
            name,
            dimensions,
            type_args,
            span,
        } => {
            let mut name = name.clone();
            if let Some(scope) = name.scopes.first_mut() {
                if let Some(to) = target(&scope.name.name, prefix, bindings) {
                    scope.name.name = to.to_string();
                }
            } else if let Some(to) = target(&name.name.name, prefix, bindings) {
                name.name.name = to.to_string();
            }
            DataType::TypeReference {
                name,
                dimensions: dimensions.clone(),
                type_args: type_args.clone(),
                span: *span,
            }
        }
        DataType::Struct(s) => {
            let mut s = s.clone();
            for member in &mut s.members {
                member.data_type = data_type(&member.data_type, prefix, bindings);
            }
            DataType::Struct(s)
        }
        DataType::Enum(e) => {
            let mut e = e.clone();
            if let Some(base) = &e.base_type {
                e.base_type = Some(Box::new(data_type(base, prefix, bindings)));
            }
            DataType::Enum(e)
        }
        _ => dt.clone(),
    }
}

/// §23.9: a nearer declaration of `name` hides it and every block key it
/// heads (`name.k`, `name[0].k`).
fn hide(bindings: &mut HashMap<String, Expression>, name: &str) {
    bindings.retain(|k, _| k.split(['.', '[']).next() != Some(name));
}

pub(super) fn class(
    cd: &ClassDeclaration,
    prefix: &str,
    bindings: &HashMap<String, Expression>,
) -> ClassDeclaration {
    let mut result = cd.clone();
    result.name.name = cat2(prefix, &cd.name.name);
    // §8.25: a class parameter shadows an enclosing class with the same name.
    let mut bindings = bindings.clone();
    for parameter in &cd.params {
        match &parameter.kind {
            ParameterKind::Type { assignments } => {
                for a in assignments {
                    hide(&mut bindings, &a.name.name);
                }
            }
            ParameterKind::Data { assignments, .. } => {
                for a in assignments {
                    hide(&mut bindings, &a.name.name);
                }
            }
        }
    }
    let bindings = &bindings;
    if let Some(base) = &mut result.extends {
        if base.scopes.is_empty() {
            if let Some(to) = target(&base.name.name, prefix, bindings) {
                base.name.name = to.to_string();
            }
        } else if let Some(scope) = base.scopes.first_mut() {
            if let Some(to) = target(&scope.name, prefix, bindings) {
                scope.name = to.to_string();
            }
        }
    }
    // Body-local types shadow module types, just as class parameters do.
    // Their unqualified spellings are resolved by the class context.
    let outer_bindings = bindings;
    let mut body_bindings = outer_bindings.clone();
    for item in &cd.items {
        match item {
            ClassItem::Typedef(t) => {
                hide(&mut body_bindings, &t.name.name);
            }
            ClassItem::Class(c) => {
                hide(&mut body_bindings, &c.name.name);
            }
            // §8.3: a property hides a block's data of its name (`cfg.depth`).
            ClassItem::Property(p) => {
                for d in &p.declarators {
                    hide(&mut body_bindings, &d.name.name);
                }
            }
            _ => {}
        }
    }
    let bindings = &body_bindings;
    let empty_names = std::collections::HashSet::new();
    let empty_interfaces = HashMap::default();
    for item in &mut result.items {
        match item {
            ClassItem::Property(p) => {
                p.data_type = data_type(&p.data_type, prefix, bindings);
                for d in &mut p.declarators {
                    if let Some(e) = &d.init {
                        d.init = Some(rewrite_expr(
                            e,
                            "",
                            bindings,
                            &empty_names,
                            &empty_interfaces,
                        ));
                    }
                }
            }
            ClassItem::Typedef(t) => t.data_type = data_type(&t.data_type, prefix, outer_bindings),
            ClassItem::Method(m) => {
                let (ports, body) = match &mut m.kind {
                    ClassMethodKind::Function(f)
                    | ClassMethodKind::Extern(f)
                    | ClassMethodKind::PureVirtual(f) => {
                        f.return_type = data_type(&f.return_type, prefix, bindings);
                        (&mut f.ports, &mut f.items)
                    }
                    ClassMethodKind::Task(t) => (&mut t.ports, &mut t.items),
                };
                let mut method_bindings = bindings.clone();
                for port in ports {
                    port.data_type = data_type(&port.data_type, prefix, bindings);
                    hide(&mut method_bindings, &port.name.name);
                    if let Some(e) = &port.default {
                        port.default = Some(rewrite_expr(
                            e,
                            "",
                            bindings,
                            &empty_names,
                            &empty_interfaces,
                        ));
                    }
                }
                for name in stmt_list_declared_names(body) {
                    hide(&mut method_bindings, name);
                }
                // §8.3/§13.4: rewrite from the shared source directly. Mutating
                // the cloned body first would copy every statement before the
                // rewriter builds a second copy.
                *body = crate::ast::cache::without_scope_caches(|| body.iter().map(|s| {
                    // Prefix only class aliases, keeping existing ownership
                    // rules for ordinary method locals and module variables.
                    rewrite_stmt(s, prefix, &method_bindings, &empty_names, &empty_interfaces)
                }).collect());
            }
            _ => {}
        }
    }
    result
}
