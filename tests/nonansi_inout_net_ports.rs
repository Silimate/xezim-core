//! IEEE 1800 §23.2.2.1: two explicit inout port expressions can name the same net.
use xezim_core::parse_and_elaborate_multi;

#[test]
fn renamed_inout_ports_join_their_parent_nets() {
    let src = r#"module bridge(.plus(w), .minus(w));
  inout [15:0] w;
  wire [15:0] w;
endmodule
module top;
  wire [15:0] a, b;
  bridge u(.plus(a), .minus(b));
endmodule"#;
    let (_, m) = parse_and_elaborate_multi(
        &[src.to_string()],
        Some("top"),
        &[],
        &["bridge.sv".to_string()],
        &[],
    )
    .expect("elaborate whole-net inout port expressions");
    let canonical = |name: &str| {
        m.alias_canon
            .get(name)
            .cloned()
            .unwrap_or_else(|| name.to_owned())
    };
    assert_eq!(canonical("a"), canonical("b"));
    assert_eq!(m.signals[&canonical("a")].width, 16);
}
