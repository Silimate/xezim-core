//! §25.3, §23.3.3: an interface port is connected to an interface instance
//! or to an interface port of the parent; an instance that leaves one
//! unconnected (absent from a named list, `.p()`, or an empty ordered slot)
//! is an error, as is a `.*` port with no interface of its name in the
//! parent, declared there or added by a `bind`.
//! `--relax-unconnected-interface-port`
//! (`RELAX_UNCONNECTED_INTERFACE_PORT`) elaborates it, with a warning. One
//! test, since the switch is process-wide.

fn elaborate(inst: &str) -> Result<xezim_core::ElaboratedModule, String> {
    let src = format!(
        r#"
interface mem_if; logic [7:0] rdata; logic rq; modport req (input rdata, output rq); endinterface
module dut (input logic clk, mem_if.req mem, output logic [7:0] q);
  assign mem.rq = 1'b1;
  always_ff @(posedge clk) q <= mem.rdata;
endmodule
module gen (interface bus, output logic o);
  assign o = bus.rq;
endmodule
module top;
  logic clk = 0, o; logic [7:0] q;
  mem_if mem ();
  {inst}
endmodule
"#
    );
    xezim_core::parse_and_elaborate_multi(&[src], Some("top"), &[], &["top.sv".into()], &[])
        .map(|(_, m)| m)
}

#[test]
fn an_unconnected_interface_port_is_an_error_unless_relaxed() {
    xezim_core::sv_parser::RELAX_UNCONNECTED_INTERFACE_PORT.set(false);
    for (inst, port) in [
        ("dut u (.clk(clk), .q(q));", "mem"),
        ("dut u (.clk(clk), .mem(), .q(q));", "mem"),
        ("dut u (clk, , q);", "mem"),
        ("gen g (.o(o));", "bus"),
        // `.*` with no interface named `bus` in the parent (§23.3.2.4).
        ("gen g (.*);", "bus"),
    ] {
        let err = elaborate(inst).expect_err(inst);
        assert!(
            err.contains(&format!("interface port '{port}'")) && err.contains("§25.3"),
            "{inst}: {err}"
        );
        assert!(err.contains("--relax-unconnected-interface-port"), "{err}");
    }
    for inst in [
        "dut u (.clk(clk), .mem(mem), .q(q));",
        "dut u (.clk(clk), .mem(mem.req), .q(q));",
        "dut u (clk, mem, q);",
        "dut u (.*);",
        "dut u (.clk, .mem, .q);",
        "gen g (.bus(mem), .o(o));",
    ] {
        elaborate(inst).unwrap_or_else(|e| panic!("{inst}: {e}"));
    }

    // A module with an interface port that nothing instantiates is a top
    // when the tops are found automatically; the shared root instantiates
    // it with no connections, which is not an unconnected port.
    let src = "interface i_if; logic a; endinterface\n\
               module lone (i_if p); endmodule\n\
               module top; logic x; endmodule\n"
        .to_string();
    xezim_core::parse_and_elaborate_multi(&[src], None, &[], &["top.sv".into()], &[])
        .map(|_| ())
        .expect("an automatically found top keeps its interface port");

    // §23.11: an interface a `bind` adds to the target fills a `.*` port
    // of an instance bound there too; without it the port is unconnected.
    let bound = |with_bus: bool| {
        let src = format!(
            "interface mem_if; logic rq; endinterface\n\
             module gen (interface bus, output logic o); assign o = bus.rq; endmodule\n\
             module tgt; logic o; endmodule\n\
             module top; tgt u (); endmodule\n\
             {}bind tgt gen g (.*);\n",
            if with_bus { "bind tgt mem_if bus ();\n" } else { "" }
        );
        xezim_core::parse_and_elaborate_multi(&[src], Some("top"), &[], &["top.sv".into()], &[])
            .map(|_| ())
    };
    bound(true).expect("a bound interface fills the bound instance's .* port");
    let err = bound(false).expect_err("no interface named bus in the target");
    assert!(err.contains("interface port 'bus'"), "{err}");

    xezim_core::sv_parser::RELAX_UNCONNECTED_INTERFACE_PORT.set(true);
    elaborate("dut u (.clk(clk), .q(q));").expect("elaborated with the switch");
    xezim_core::sv_parser::RELAX_UNCONNECTED_INTERFACE_PORT.set(false);
}
