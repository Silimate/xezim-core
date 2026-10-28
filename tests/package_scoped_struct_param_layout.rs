//! §7.2.1 / §26.3: a packed-struct package parameter keeps its member layout
//! under the package-qualified `p::K` key, so `p::K.c` can slice the
//! package's own parameter in a module that does not import the package and
//! declares its own `K`.

#[test]
fn package_struct_parameter_layout_is_registered_under_its_scoped_name() {
    let src = r#"
package p;
  typedef struct packed {
    logic       a;
    logic       b;
    logic [4:0] c;
  } s_t;
  localparam s_t K = '{b: 1'b1, a: 1'b0, c: 5'd29};
endpackage
module top;
  typedef struct packed {
    logic [3:0] c;
  } s_t;
  s_t K;
  logic [4:0] w;
  assign w = p::K.c;
endmodule
"#;
    let (_, m) = xezim_core::parse_and_elaborate_multi(
        &[src.to_string()],
        Some("top"),
        &[],
        &["top.sv".to_string()],
        &[],
    )
    .expect("elaborate");
    let mut fields = m
        .packed_struct_fields
        .get("p::K")
        .unwrap_or_else(|| panic!("no layout for p::K"))
        .clone();
    fields.sort();
    assert_eq!(
        fields,
        [
            ("a".to_string(), 6, 1),
            ("b".to_string(), 5, 1),
            ("c".to_string(), 0, 5)
        ]
    );
}
