//! §23.3.3: a port connection is warned about when its width differs from
//! the port's. §7.4.3 and §11.5.1: a part-select after an index into a
//! multi-dimensional packed array selects elements of the next dimension, so
//! `.o(a[2][4:2])` on `logic [3:0][5:0][7:0] a` is three 8-bit elements,
//! 24 bits. The connection width counted 3 and every such connection drew a
//! bogus "port width mismatch … the connection is 3 bit(s)" (Silimate/xezim#316).

fn diagnostics(src: &str, top: &str) -> Vec<String> {
    xezim_core::elab_diag_capture_begin();
    let result = xezim_core::parse_and_elaborate_multi(
        &[src.to_string()],
        Some(top),
        &[],
        &[format!("{top}.sv")],
        &[],
    );
    let diags = xezim_core::elab_diag_capture_take();
    result.expect("elaborate");
    diags
}

#[test]
fn range_after_index_counts_element_widths() {
    let diags = diagnostics(
        r#"
module pcw_drv (output logic [2:0][7:0] o);
  assign o = 24'hc3c2c1;
endmodule
module pcw_snk (input logic [2:0][7:0] i, output logic [23:0] y);
  assign y = i;
endmodule
module pcw_top;
  logic [3:0][5:0][7:0] a;
  logic [5:0][7:0] u [2];
  logic [23:0] y0, y1;
  pcw_drv d0 (.o(a[2][4:2]));
  pcw_snk s0 (.i(a[1][3:1]), .y(y0));
  pcw_snk s1 (.i(u[1][5:3]), .y(y1));
endmodule
"#,
        "pcw_top",
    );
    assert!(
        !diags.iter().any(|d| d.contains("port width mismatch")),
        "no width mismatch expected: {diags:#?}"
    );
}

#[test]
fn a_real_mismatch_after_an_index_is_still_reported() {
    let diags = diagnostics(
        r#"
module pcw_short (output logic [2:0][7:0] o);
  assign o = 24'hc3c2c1;
endmodule
module pcw_top2;
  logic [3:0][5:0][7:0] a;
  pcw_short d0 (.o(a[2][4:3]));
endmodule
"#,
        "pcw_top2",
    );
    assert!(
        diags
            .iter()
            .any(|d| d.contains("port width mismatch") && d.contains("16 bit(s)")),
        "a 16-bit connection to a 24-bit port is reported: {diags:#?}"
    );
}
