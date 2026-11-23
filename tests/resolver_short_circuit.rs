//! A static nettype resolver must write each local on every path before reading it (§6.6.7).

#[test]
fn conditional_rhs_write_cannot_initialize_a_static_local() {
    for op in ["&&", "||"] {
        let src = format!("function static int r(input int d[]); int x; r = (d[0] != 0) {op} ((x = 1) != 0); return x; endfunction
nettype int resolved with r; module top; resolved n; assign n = 1; endmodule");
        let err = xezim_core::parse_and_elaborate_multi(&[src], None, &[], &["t.sv".into()], &[])
            .expect_err("short-circuited assignment may leave x unchanged");
        assert!(
            err.contains("reads 'x' of 'r' before writing it"),
            "{op}: {err}"
        );
    }
}
