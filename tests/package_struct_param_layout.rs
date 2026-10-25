//! §7.2.1 / §26.3: a packed-struct package parameter that only an
//! instantiated module imports keeps its member layout, so `K.c` in that
//! module can slice the parameter.

#[test]
fn package_struct_parameter_layout_is_registered_for_a_submodule_import() {
    let src = r#"
package p;
  typedef struct packed {
    logic       a;
    logic       b;
    logic [4:0] c;
  } s_t;
  localparam s_t K = '{b: 1'b1, a: 1'b0, c: 5'd29};
endpackage
module probe;
  import p::*;
  logic [4:0] w;
  assign w = K.c;
endmodule
module top;
  probe u();
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
        .get("K")
        .unwrap_or_else(|| panic!("no layout for K"))
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
