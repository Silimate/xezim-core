//! IEEE 1800 §23.2.2.1: two explicit inout port expressions can name the same net.
use xezim_core::parse_and_elaborate_multi;

/// §23.2.2.1, §23.3.3.6: both explicit inout expressions denote one net.
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

/// §7.4.2, §23.2.2.1: a whole unpacked inout cannot be lowered to a packed-net alias. Its
/// unpacked dimension may be in the port declaration or its completion.
#[test]
fn whole_unpacked_inout_reports_the_unsupported_shape() {
    for declaration in ["inout w [3:0];", "inout w; wire w [3:0];"] {
        let source = format!("module bridge(.p(w)); {declaration} endmodule");
        let result = parse_and_elaborate_multi(
            &[source],
            Some("bridge"),
            &[],
            &["bridge.sv".to_owned()],
            &[],
        );
        let err = result
            .err()
            .expect("whole unpacked net accepted as a packed alias");
        assert!(err.contains("unpacked array"), "{err}");
    }
}
