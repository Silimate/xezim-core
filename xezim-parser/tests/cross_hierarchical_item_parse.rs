//! §19.6: a cross item may be a variable, and §19.6.1.1 `binsof` may name
//! it. Simulators also take a hierarchical reference (`u.a`) as the item.
//! The cross list stopped at the first `.`.

use sv_parser::parse;

#[test]
fn cross_of_hierarchical_variables_parses() {
    let src = r#"
interface fcov_if (input logic clk);
  covergroup cg @(posedge clk);
    cp_a: coverpoint u.a { bins lo = {0}; bins hi = {1}; }
    x1: cross u.a, u.b;
    x2: cross cp_a, s.q.m iff (u.b) {
      ignore_bins i1 = binsof(cp_a.lo) && binsof(s.q.m) intersect {1};
    }
  endgroup
endinterface
"#;
    let r = parse(src);
    assert!(r.errors.is_empty(), "got: {:?}", r.errors);
    let ast = format!("{:?}", r.source);
    for name in ["\"u.a\"", "\"u.b\"", "\"s.q.m\""] {
        assert!(ast.contains(&format!("name: {name}")), "missing item {name}");
    }
    // `cp_a.lo` stays coverpoint `cp_a`, bin `lo`; `s.q.m` is one item.
    assert!(ast.contains("bin: Some(Identifier { name: \"lo\""));
    assert_eq!(ast.matches("bin: None").count(), 1);
}
