//! §19.5.1.1 / §26.3: a covergroup bin filter's `item` names the candidate
//! value, not a colliding package parameter a wildcard import would otherwise
//! qualify.

use xezim_core::ast::decl::{CovergroupItem, ModuleItem};
use xezim_core::ast::Description;
use xezim_core::ast::expr::{ExprKind, Expression};

fn plain_name(e: &Expression) -> &str {
    let ExprKind::Ident(h) = &e.kind else {
        panic!("expected a simple name: {e:?}");
    };
  assert_eq!(h.path.len(), 1);
    &h.path[0].name.name
}

#[test]
fn bin_filter_item_shadows_imported_parameter() {
    let src = r#"
package pkg_a; parameter int item = 1; endpackage
package pkg_b; parameter int item = 2; endpackage
module m;
  import pkg_a::*;
  logic [3:0] v;
  covergroup cg;
    coverpoint v { bins b[] = {[0:3]} with (item < 5); }
  endgroup
endmodule
"#;
    let mut parsed = xezim_core::parse(src);
    assert!(parsed.errors.is_empty(), "{:?}", parsed.errors);
    xezim_core::pkg_scope::qualify_colliding_package_types(&mut parsed.source.descriptions).unwrap();
    let filter = parsed
        .source
        .descriptions
        .into_iter()
        .find_map(|d| match d {
            Description::Module(m) => {
                let cg = m.items.iter().find_map(|it| match it {
                    ModuleItem::CovergroupDeclaration(cg) => Some(cg),
                    _ => None,
                })?;
                let CovergroupItem::Coverpoint(cp) = &cg.items[0] else {
                    panic!("expected a coverpoint");
                };
                Some(cp.bins[0].with_filter.clone().unwrap())
            }
            _ => None,
        })
        .expect("covergroup bin filter");
    let ExprKind::Binary { left, .. } = &filter.kind else {
        panic!("expected item < 5: {filter:?}");
    };
    assert_eq!(plain_name(left), "item");
    assert!(
        !format!("{filter:?}").contains("pkg_a::item"),
        "filter must not qualify the bin iterator: {filter:?}"
    );
}
