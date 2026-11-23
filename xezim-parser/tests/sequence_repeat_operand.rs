use sv_parser::ast::Description;
use sv_parser::ast::decl::ModuleItem;
use sv_parser::ast::expr::{BinaryOp, ExprKind, Expression};
use sv_parser::parse;

fn antecedent(expr: &str) -> Expression {
    let src = format!(
        "module m(input clk, sel, p, n, out); property pr; @(posedge clk) {expr} |-> out; endproperty endmodule"
    );
    let parsed = parse(&src);
    assert!(parsed.errors.is_empty(), "{:?}", parsed.errors);
    let Description::Module(m) = &parsed.source.descriptions[0] else {
        panic!("expected module")
    };
    let ModuleItem::PropertyDeclaration(p) = &m.items[0] else {
        panic!("expected property")
    };
    let ExprKind::SvaClocked { body, .. } = &p.body.as_ref().unwrap().kind else {
        panic!("expected sampling clock")
    };
    let ExprKind::Binary {
        op: BinaryOp::OrMinusArrow,
        left,
        ..
    } = &body.kind
    else {
        panic!("expected implication: {:?}", p.body)
    };
    *left.clone()
}

#[test]
fn repeat_takes_the_complete_sampled_expression() {
    for (abbrev, marker) in [
        ("[*2]", "$sva_rep_consec"),
        ("[=2]", "$sva_rep_noncon"),
        ("[->2]", "$sva_rep_goto"),
    ] {
        let lhs = antecedent(&format!("p[0] == n[0] {abbrev}"));
        let ExprKind::SystemCall { name, args } = lhs.kind else {
            panic!("expected repeated equality")
        };
        assert_eq!(name, marker);
        assert!(matches!(
            args[0].kind,
            ExprKind::Binary {
                op: BinaryOp::Eq,
                ..
            }
        ));
    }
    let lhs = antecedent("sel ? p : n [*2]");
    let ExprKind::SystemCall { args, .. } = lhs.kind else {
        panic!("expected repeated conditional")
    };
    assert!(matches!(args[0].kind, ExprKind::Conditional { .. }));
}
