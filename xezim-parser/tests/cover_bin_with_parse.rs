//! §19.5.1.1: a value bin may end with `with (expr)`. The covergroup parser
//! stopped at `with` ("unexpected token in covergroup").

use sv_parser::ast::decl::{CovergroupItem, ModuleItem};
use sv_parser::ast::expr::{BinaryOp, ExprKind, Expression, UnaryOp};
use sv_parser::ast::Description;
use sv_parser::parse;

fn unparen(e: &Expression) -> &ExprKind {
    match &e.kind {
        ExprKind::Paren(inner) => unparen(inner),
        k => k,
    }
}

fn is_item(e: &Expression) -> bool {
    matches!(unparen(e), ExprKind::Ident(id) if format!("{id:?}").contains("\"item\""))
}

#[test]
fn cover_bin_with_clause_parses() {
    let src = r#"
module m;
  logic [3:0] v;
  covergroup cg;
    coverpoint v {
      bins t = {6};
      bins others[4] = {[0:15]} with (!(item inside {6, 9}));
      bins odd[] = {[0:15]} with (item % 2 == 1);
    }
  endgroup
endmodule
"#;
    let r = parse(src);
    assert!(r.errors.is_empty(), "got: {:?}", r.errors);
    let Some(Description::Module(m)) = r.source.descriptions.first() else {
        panic!("no module");
    };
    let Some(ModuleItem::CovergroupDeclaration(cg)) = m
        .items
        .iter()
        .find(|i| matches!(i, ModuleItem::CovergroupDeclaration(_)))
    else {
        panic!("no covergroup");
    };
    let Some(CovergroupItem::Coverpoint(cp)) = cg.items.first() else {
        panic!("no coverpoint");
    };
    let names: Vec<&str> = cp.bins.iter().map(|b| b.name.name.as_str()).collect();
    assert_eq!(names, ["t", "others", "odd"]);

    assert!(cp.bins[0].with_filter.is_none());

    // others: !(item inside {6, 9})
    let f = cp.bins[1]
        .with_filter
        .as_ref()
        .expect("others has a filter");
    let ExprKind::Unary {
        op: UnaryOp::LogNot,
        operand,
    } = unparen(f)
    else {
        panic!("others filter: {f:?}");
    };
    let ExprKind::Inside { expr, ranges } = unparen(operand) else {
        panic!("others operand: {operand:?}");
    };
    assert!(is_item(expr));
    assert_eq!(ranges.len(), 2);

    // odd: item % 2 == 1
    let f = cp.bins[2].with_filter.as_ref().expect("odd has a filter");
    let ExprKind::Binary {
        op: BinaryOp::Eq,
        left,
        ..
    } = unparen(f)
    else {
        panic!("odd filter: {f:?}");
    };
    let ExprKind::Binary {
        op: BinaryOp::Mod,
        left,
        ..
    } = unparen(left)
    else {
        panic!("odd left: {left:?}");
    };
    assert!(is_item(left));
}
