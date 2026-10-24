//! §19.5.1.1: a value bin may end with `with (expr)`. The covergroup parser
//! stopped at `with` ("unexpected token in covergroup").

use sv_parser::parse;

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
    let ast = format!("{:?}", r.source);
    assert_eq!(ast.matches("with_filter: Some(").count(), 2);
    assert_eq!(ast.matches("with_filter: None").count(), 1);
}
