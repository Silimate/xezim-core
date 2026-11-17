//! §6.19 / §6.10: an enum label used in a port actual of an instance inside
//! a module (`.din(st == STREAM)`, the enum typedef declared in that module)
//! names a declared constant, so it is not an implicit net of the instance.

fn elaborate(src: &str) -> xezim_core::ElaboratedModule {
    xezim_core::parse_and_elaborate_multi(
        &[src.to_string()],
        Some("top"),
        &[],
        &["top.sv".to_string()],
        &[],
    )
    .expect("elaborate")
    .1
}

#[test]
fn enum_label_in_a_port_actual_is_no_implicit_net() {
    let m = elaborate(
        r#"
module ff (input logic clk, input logic din, output logic dout);
  always_ff @(posedge clk) dout <= din;
endmodule
module thr #(parameter enum logic [1:0] {HP0, HP1} HP = HP1) (input logic clk,
    input enum logic [1:0] {PA, PB} pin);
  typedef enum logic [3:0] {IDLE = 4'd0, STREAM = 4'd6, RUN[2]} st_t;
  st_t miss_state = STREAM;
  struct packed { enum logic {SA, SB} k; } s;
  enum logic [1:0] {DA, DB, DC} d = DC;
  logic p0, p1, p2, p3, p4, p5, p6;
  ff u0 (.clk(clk), .din(miss_state == STREAM), .dout(p0));
  ff u1 (.clk(clk), .din(miss_state != IDLE && d == DC), .dout(p1));
  ff u2 (clk, (miss_state == RUN1), p2);
  ff u3 (.clk(clk), .din(s.k == SB), .dout(p3));
  ff u4 (.clk(clk), .din(pin == PB), .dout(p4));
  ff u5 (.clk(clk), .din(HP == HP1), .dout(p5));
  if (1) begin : g
    ff u6 (.clk(clk), .din(miss_state == RUN0), .dout(p6));
  end
endmodule
module top;
  logic clk = 0;
  thr inst (.clk(clk), .pin(2'd1));
endmodule
"#,
    );
    for label in [
        "STREAM", "IDLE", "RUN0", "RUN1", "SB", "DC", "PB", "HP1",
    ] {
        let key = format!("inst.{label}");
        assert!(
            !m.implicit_nets.contains(&key),
            "enum label {label} became the implicit net {key}"
        );
    }
}
