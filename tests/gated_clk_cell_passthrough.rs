//! An instance of a module named `gated_clk_cell` is replaced by a direct
//! assignment of its clock only when the module's body is the OpenC910
//! pass-through (`assign clk_out = clk_in;` plus dead enable logic). Another
//! design's `gated_clk_cell` that really gates its clock (Wujian100 latches
//! the enable) is elaborated as written, with its signals and sub-instances.

fn elaborate(src: &str, top: &str) -> xezim_core::ElaboratedModule {
    xezim_core::parse_and_elaborate_multi(
        &[src.to_string()],
        Some(top),
        &[],
        &[format!("{top}.sv")],
        &[],
    )
    .expect("elaborate")
    .1
}

#[test]
fn passthrough_cell_is_a_direct_assignment() {
    let m = elaborate(
        r#"
module gated_clk_cell(clk_in, global_en, module_en, local_en, external_en,
                      pad_yy_icg_scan_en, clk_out);
  input  clk_in;
  input  global_en;
  input  module_en;
  input  local_en;
  input  external_en;
  input  pad_yy_icg_scan_en;
  output clk_out;
  wire   clk_en_bf_latch;
  wire   SE;
  assign clk_en_bf_latch = (global_en && (module_en || local_en)) || external_en;
  assign SE = pad_yy_icg_scan_en;
  assign clk_out = clk_in;
endmodule
module top;
  reg clk = 0, en = 0;
  wire gclk;
  gated_clk_cell u (.clk_in(clk), .global_en(en), .module_en(en), .local_en(en),
                    .external_en(en), .pad_yy_icg_scan_en(1'b0), .clk_out(gclk));
endmodule
"#,
        "top",
    );
    assert!(
        !m.signals.contains_key("u.clk_en_bf_latch"),
        "the pass-through cell is not inlined"
    );
}

#[test]
fn real_clock_gate_named_gated_clk_cell_is_elaborated() {
    let m = elaborate(
        r#"
module latch_cell(input clk, input d, output reg q);
  always @(*) if (!clk) q = d;
endmodule
module gated_clk_cell(input clk_in, input en, output clk_out);
  wire en_l;
  latch_cell x_lat (.clk(clk_in), .d(en), .q(en_l));
  assign clk_out = clk_in & en_l;
endmodule
module top;
  reg clk = 0;
  wire gclk;
  gated_clk_cell u (.clk_in(clk), .en(1'b0), .clk_out(gclk));
endmodule
"#,
        "top",
    );
    assert!(
        m.signals.contains_key("u.en_l"),
        "the gate's body is elaborated"
    );
    assert!(
        m.signals.contains_key("u.x_lat.q"),
        "its sub-instance exists for hierarchical references"
    );
}
