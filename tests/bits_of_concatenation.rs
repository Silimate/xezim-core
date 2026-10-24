//! `$bits` of a concatenation in an instance body is the operands' total width (§20.6.2 / §11.4.12).
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

#[test]
fn bits_of_concatenation_in_instance_body() {
    let design = elaborate(
        r#"
package p; typedef struct packed { logic [31:0] a; logic [31:0] b; } s_t; endpackage
module m import p::*; #(parameter int W = 8) (input logic [31:0] a, input logic [W-1:0] d, output s_t s);
  logic [3:0] q;
  localparam int N1 = $bits({a});
  localparam int N4 = $bits({a, d, s, q, 1'b0});
endmodule
module top; p::s_t s; m u(.a(32'h0), .d(8'h0), .s(s)); endmodule
"#,
    );
    let param = |name: &str| design.parameters.get(name).and_then(|v| v.to_i64());
    assert_eq!(param("u.N1"), Some(32));
    assert_eq!(param("u.N4"), Some(32 + 8 + 64 + 4 + 1));
}
