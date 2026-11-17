//! §8.25 / §25.9 / §3.13: a type argument `virtual iface` names the interface
//! TYPE, not an interface instance spelled with the same identifier. Instance
//! qualification must not rewrite the type name (regression for aionhw/xezim#1129).
use xezim_core::ast::expr::{ExprKind, Expression};

fn elaborate(source: &str) -> xezim_core::ElaboratedModule {
    xezim_core::parse_and_elaborate_multi(
        &[source.to_string()],
        Some("top"),
        &[],
        &["top.sv".to_string()],
        &[],
    )
    .expect("elaborate")
    .1
}

fn spec_texts(e: &Expression, out: &mut Vec<String>) {
    match &e.kind {
        ExprKind::Specialization {
            base,
            type_args_text,
        } => {
            out.push(type_args_text.clone());
            spec_texts(base, out);
        }
        ExprKind::Call { func, args } => {
            spec_texts(func, out);
            args.iter().for_each(|a| spec_texts(a, out));
        }
        ExprKind::MemberAccess { expr, .. } => spec_texts(expr, out),
        _ => {}
    }
}

#[test]
fn virtual_interface_type_arg_keeps_type_spelling() {
    let mut d = elaborate(
        r#"
package p;
  class db #(type T = int);
    static bit found[string];
  endclass
endpackage
interface clk_rst_if; logic clk; endinterface
module u_tb(output bit a, output bit b);
  import p::*;
  clk_rst_if clk_rst_if();
  assign a = db#(virtual clk_rst_if)::found.exists("k");
  assign b = db#(.T(virtual clk_rst_if))::found.exists("k");
endmodule
module top;
  bit a, b;
  u_tb u(a, b);
endmodule
"#,
    );
    d.materialize_pending();
    let mut texts = Vec::new();
    for ca in &d.continuous_assigns {
        spec_texts(&ca.rhs, &mut texts);
        spec_texts(&ca.lhs, &mut texts);
    }
    assert!(
        texts.iter().any(|t| t == "virtual clk_rst_if"),
        "expected unqualified virtual interface type arg, got {texts:?}"
    );
    assert!(
        !texts
            .iter()
            .any(|t| t.contains("u_tb.clk_rst_if") || t.contains("u.clk_rst_if")),
        "instance-qualified virtual interface type args: {texts:?}"
    );
}