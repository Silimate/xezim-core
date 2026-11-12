//! §19.5.1.1: in a covergroup of an instantiated module, a bin filter's
//! `item` is the candidate value even where the module declares its own
//! `item`; the filter's other names still take the instance prefix.

use xezim_core::ast::decl::CovergroupItem;
use xezim_core::ast::expr::{ExprKind, Expression};

fn dotted(e: &Expression) -> String {
    let ExprKind::Ident(h) = &e.kind else {
        panic!("not a name: {e:?}");
    };
    let segs: Vec<_> = h.path.iter().map(|s| s.name.name.as_str()).collect();
    segs.join(".")
}

#[test]
fn bin_filter_item_shadows_module_item() {
    let src = "module sub; logic [3:0] v, item, lim;
  covergroup cg; coverpoint v { bins b[] = {[0:15]} with (item < lim); } endgroup
endmodule
module top; sub u(); endmodule";
    let (_, m) = xezim_core::parse_and_elaborate_multi(
        &[src.to_string()],
        Some("top"),
        &[],
        &["top.sv".to_string()],
        &[],
    )
    .expect("elaborate");
    let CovergroupItem::Coverpoint(cp) = &m.covergroups["u.cg"].items[0] else {
        panic!("expected a coverpoint");
    };
    let filter = cp.bins[0].with_filter.as_ref().expect("filter");
    let ExprKind::Binary { left, right, .. } = &filter.kind else {
        panic!("expected item < lim: {filter:?}");
    };
    assert_eq!([dotted(left), dotted(right)], ["item", "u.lim"]);
}
