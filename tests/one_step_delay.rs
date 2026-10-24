//! `#1step` is a procedural delay value (§A.2.2.3), not `#1` followed by an
//! identifier `step`.

#[test]
fn one_step_procedural_delay_elaborates() {
    let source = r#"
`timescale 1ns/1ps
class c;
  task t(); #1step; endtask
endclass
module top;
  initial begin
    #5;
    #1step;
    fork #1step; join
  end
endmodule
"#;
    let result = xezim_core::parse_and_elaborate_multi(
        &[source.to_string()],
        Some("top"),
        &[],
        &["top.sv".to_string()],
        &[],
    );
    assert!(result.is_ok(), "{:?}", result.err());
}
