//! Packed inout connections share the original drivers of equivalent bits.
//! Copying resolved terminal values would feed a released drive back onto the bus.

use super::*;

/// Copy original drivers onto each connected packed bit. Feeding the resolved
/// value back through the opposite port would retain old values after release.
pub(super) fn project_selected_port_drivers(elab: &mut ElaboratedModule) {
    let pairs = std::mem::take(&mut elab.selected_port_connections);
    if pairs.is_empty() {
        return;
    }
    let mut ids = HashMap::default();
    let mut sets = super::super::nettypes::IndexDsu::new(0);
    for (first, second) in pairs {
        let (Some((a, a_lo, a_hi)), Some((b, b_lo, b_hi))) = (
            cont_assign_driver_span(&first, elab),
            cont_assign_driver_span(&second, elab),
        ) else {
            continue;
        };
        for (ai, bi) in (a_lo..=a_hi).zip(b_lo..=b_hi) {
            let x = *ids
                .entry((a.clone(), ai))
                .or_insert_with(|| sets.push_singleton());
            let y = *ids
                .entry((b.clone(), bi))
                .or_insert_with(|| sets.push_singleton());
            sets.union(x, y);
        }
    }
    if ids.is_empty() {
        return;
    }
    let mut groups: HashMap<u32, Vec<(String, u32)>> = HashMap::default();
    for (bit, id) in &ids {
        groups.entry(sets.find(*id)).or_default().push(bit.clone());
    }
    for group in groups.values_mut() {
        group.sort_unstable();
    }
    let mut lanes: HashMap<String, std::collections::BTreeMap<u32, u32>> = HashMap::default();
    for ((net, bit), id) in ids {
        lanes.entry(net).or_default().insert(bit, sets.find(id));
    }
    for pending in std::mem::take(&mut elab.pending_cont_assign) {
        let lhs = pending.materialize_lhs();
        if cont_assign_driver_span(&lhs, elab).is_some_and(|(n, _, _)| lanes.contains_key(&n)) {
            elab.continuous_assigns
                .push(pending.materialize(&elab.parameters));
        } else {
            elab.pending_cont_assign.push(pending);
        }
    }
    let mut next = 0;
    for mut ca in std::mem::take(&mut elab.continuous_assigns) {
        let Some((net, lo, hi)) = cont_assign_driver_span(&ca.lhs, elab) else {
            elab.continuous_assigns.push(ca);
            continue;
        };
        let Some(bits) = lanes.get(&net) else {
            elab.continuous_assigns.push(ca);
            continue;
        };
        // A generated inout connection copies the resolved terminals onto
        // themselves. Propagating that copy as a new source feeds the bus
        // back into itself after a real driver releases it.
        if ca.rhs_parent_scoped {
            if ident_flat_name(&ca.lhs).is_some_and(|n| elab.net_port_merges.contains_key(&n)) {
                elab.continuous_assigns.push(ca);
                continue;
            }
            if let Some((src, first, last)) = cont_assign_driver_span(&ca.rhs, elab)
                && hi - lo == last - first
                && let Some(source_bits) = lanes.get(&src)
                && (lo..=hi).zip(first..=last).all(|(a, b)| {
                    bits.get(&a)
                        .is_some_and(|group| source_bits.get(&b) == Some(group))
                })
            {
                continue;
            }
        }
        let mut targets = Vec::new();
        for (&bit, group) in bits.range(lo..=hi) {
            for (dst, dbit) in &groups[group] {
                if (dst != &net || *dbit != bit)
                    && let Some(label) = packed_port_label(elab, dst, *dbit)
                {
                    targets.push((bit - lo, make_index_expr(dst, label)));
                }
            }
        }
        if targets.is_empty() {
            elab.continuous_assigns.push(ca);
            continue;
        }

        // Evaluate and size the original driver once. Apply its control, drive
        // strength and delay when driving each equivalent terminal.
        let gate = gate_tristate_parts(&ca).map(|(ctl, _)| ctl.clone());
        let mut data = ca.rhs.clone();
        let strength = match &data.kind {
            ExprKind::SystemCall { name, args }
                if matches!(
                    name.as_str(),
                    "$__supply" | "$__drive" | "$__pull" | "$__weak"
                ) && !args.is_empty() =>
            {
                let marker = (name.clone(), args[1..].to_vec());
                data = args[0].clone();
                Some(marker)
            }
            _ => None,
        };
        if gate.is_some()
            && let ExprKind::Conditional { then_expr, .. } = &data.kind
        {
            data = (**then_expr).clone();
        }
        let stage = loop {
            let stage = format!("$port_driver${next}");
            next += 1;
            if !elab.signals.contains_key(&stage) {
                break stage;
            }
        };
        let width = hi - lo + 1;
        let mut signal = elab.signals[&net].clone();
        signal.name = stage.clone().into();
        signal.width = width;
        signal.direction = None;
        signal.is_signed = false;
        signal.value = Value::all_z(width);
        elab.signals.insert(stage.clone(), signal);
        elab.nets.insert(stage.clone());
        let span = data.span;
        let wrap = |mut value| {
            if let Some(ctl) = &gate {
                value = make_tristate(ctl.clone(), value, span);
            }
            if let Some((name, tail)) = &strength {
                let mut args = vec![value];
                args.extend(tail.iter().cloned());
                value = make_syscall(name, args, span);
            }
            value
        };
        elab.continuous_assigns.push(ContinuousAssignment {
            lhs: make_ident_expr(&stage),
            rhs: data,
            origin: ca.origin.clone(),
            rhs_parent_scoped: ca.rhs_parent_scoped,
            delay: 0,
            delay_select: None,
            delay_fall: None,
            delay_off: None,
            gate_delay_fall: None,
            delay_exprs: None,
        });
        for (offset, target) in targets {
            let mut driver = ca.clone();
            driver.lhs = target;
            driver.rhs = wrap(make_index_expr(&stage, i64::from(offset)));
            driver.rhs_parent_scoped = true;
            elab.continuous_assigns.push(driver);
        }
        ca.rhs = wrap(make_ident_expr(&stage));
        ca.rhs_parent_scoped = true;
        elab.continuous_assigns.push(ca);
    }
}

fn packed_port_label(elab: &ElaboratedModule, name: &str, offset: u32) -> Option<i64> {
    let offset = i64::from(offset);
    if let Some(&(_, hi)) = elab.ascending_packed.get(name) {
        return Some(hi - offset);
    }
    match elab.packed_full_dims.get(name).map(Vec::as_slice) {
        None => Some(offset),
        Some([(left, right)]) if left >= right => Some(right + offset),
        Some([(_, right)]) => Some(right - offset),
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::elaborate::implicit_nets::collect_ident_names;

    // The final wire-resolution and size-cast nodes are internal system
    // calls. Their arguments also contribute drivers to the output.
    fn driver_reads(e: &Expression, out: &mut Vec<String>) {
        match &e.kind {
            ExprKind::SystemCall { args, .. } | ExprKind::Concatenation(args) => {
                for arg in args {
                    driver_reads(arg, out);
                }
            }
            _ => collect_ident_names(e, out),
        }
    }

    /// §23.3.3.6, §29.3 and §14.16: both terminals see UDP and clocking drives.
    #[test]
    fn selected_ports_carry_udp_and_clocking_output_drivers() {
        for (declaration, on_b) in [
            ("gate1 g(a, d);", false),
            ("gate1 g(b[0], d);", true),
            ("clocking cb @(posedge clk); output a; endclocking", false),
            ("clocking cb @(posedge clk); output b; endclocking", true),
        ] {
            let source = format!(
                r#"
primitive gate1(o, i); output o; input i; table 0 : 0; 1 : 1; endtable endprimitive
module bridge(.p(w), .q(w)); inout w; endmodule
module top(input d, clk);
  wire a;
  wire [1:0] b;
  bridge u(a, b[0]);
  {declaration}
endmodule"#
            );
            let (_, m) = crate::parse_and_elaborate_multi(
                &[source],
                Some("top"),
                &[],
                &["top.sv".to_owned()],
                &[],
            )
            .unwrap();
            let stage = if let Some(udp) = m.udp_instances.first() {
                ident_flat_name(&udp.output).expect("UDP drives its own stage")
            } else {
                m.clocking_output_stages
                    .values()
                    .next()
                    .expect("clockvar drives its own stage")
                    .clone()
            };
            let mut pending = vec![alias_canonical(
                &m.alias_canon,
                if on_b { "a" } else { "b" }.to_owned(),
            )];
            let mut seen = HashSet::default();
            while let Some(net) = pending.pop() {
                if seen.insert(net.clone()) {
                    for ca in &m.continuous_assigns {
                        if cont_assign_driver_span(&ca.lhs, &m).is_some_and(|(n, _, _)| n == net) {
                            driver_reads(&ca.rhs, &mut pending);
                        }
                    }
                }
            }
            assert!(
                seen.contains(&stage),
                "{declaration}: missing {stage} in {seen:?}"
            );
        }
    }

    /// §6.6.1: overlapping writes and other sources resolve on each unrelated wire.
    #[test]
    fn unrelated_overlapping_drivers_keep_their_udp_or_clockvar_stage() {
        for source_driver in [
            "gate1 g(v, d);",
            "clocking cb @(posedge clk); output v; endclocking",
        ] {
            let source = format!(
                r#"
primitive gate1(o, i); output o; input i; table 0 : 0; 1 : 1; endtable endprimitive
module bridge(.p(w), .q(w)); inout w; endmodule
module top(input d, clk, a, b);
  wire x;
  wire [1:0] y;
  bridge unused_connection(x,y[0]);
  wire [0:0] v;
  assign v[0] = a;
  assign v[0] = b;
  {source_driver}
endmodule"#
            );
            let (_, m) = crate::parse_and_elaborate_multi(
                &[source],
                Some("top"),
                &[],
                &["top.sv".to_owned()],
                &[],
            )
            .unwrap();
            let stage = if let Some(udp) = m.udp_instances.first() {
                let net = ident_flat_name(&udp.output).unwrap();
                assert_ne!(net, "v", "UDP must join the resolution of v");
                net
            } else {
                m.clocking_output_stages
                    .values()
                    .next()
                    .expect("clockvar must join the resolution of v")
                    .clone()
            };
            let mut sources = Vec::new();
            for ca in &m.continuous_assigns {
                if ident_flat_name(&ca.lhs).as_deref() == Some("v") {
                    driver_reads(&ca.rhs, &mut sources);
                }
            }
            assert!(sources.contains(&stage), "{stage} absent from {sources:?}");
        }
    }

    /// §23.3.3.6: each bit of two inout actuals is driven from either side.
    #[test]
    fn part_selects_carry_the_original_drivers_both_ways() {
        for (a_decl, a_select, port_a, port_b) in [
            ("[3:0]", "[2:1]", "a[2:1]", "b[3:2]"),
            ("[1:0]", "", "a", "b[3:2]"),
            ("[1:0]", "", "b[3:2]", "a"),
        ] {
            let source = format!(
                r#"
module link(.a(w), .b(w));
  inout [1:0] w;
endmodule
module top(input [1:0] left, right);
  wire {a_decl} a;
  wire [3:0] b;
  assign a{a_select} = left;
  assign b[3:2] = right;
  link u({port_a}, {port_b});
endmodule"#
            );
            let (_, m) = crate::parse_and_elaborate_multi(
                &[source],
                Some("top"),
                &[],
                &["top.sv".to_owned()],
                &[],
            )
            .unwrap();
            let mut reads: HashMap<String, Vec<String>> = HashMap::default();
            for ca in &m.continuous_assigns {
                if let Some((name, _, _)) = cont_assign_driver_span(&ca.lhs, &m) {
                    driver_reads(&ca.rhs, reads.entry(name).or_default());
                }
            }
            for net in ["a", "b"] {
                let mut seen = std::collections::HashSet::new();
                let mut pending = vec![alias_canonical(&m.alias_canon, net.to_owned())];
                while let Some(name) = pending.pop() {
                    if seen.insert(name.clone())
                        && let Some(deps) = reads.get(&name)
                    {
                        pending.extend(deps.iter().cloned());
                    }
                }
                assert!(
                    seen.contains("left") && seen.contains("right"),
                    "{net}: {seen:?}"
                );
            }
        }
    }
}
