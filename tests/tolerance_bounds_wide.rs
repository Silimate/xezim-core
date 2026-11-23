//! Integral tolerance ranges use the centre's width and signedness (§11.4.13).

use xezim_core::value::Value;

#[test]
fn wide_bounds_preserve_high_bits_and_centre_type() {
    let high = Value::from_u64(1, 256).shift_left(&Value::from_u64(180, 32));
    let c = high.add(&Value::from_u64(10, 256));
    for (tol, relative) in [
        (Value::from_u64(2, 16), false),
        (Value::from_f64(2.9), false),
    ] {
        assert_eq!(
            Value::tolerance_bound(&c, &tol, relative, false),
            high.add(&Value::from_u64(8, 256))
        );
        assert_eq!(
            Value::tolerance_bound(&c, &tol, relative, true),
            high.add(&Value::from_u64(12, 256))
        );
    }
    let mut neg = Value::from_u64(100, 256).negate();
    neg.is_signed = true;
    let bounds = [false, true]
        .map(|upper| Value::tolerance_bound(&neg, &Value::from_u64(10, 8), true, upper));
    for (b, expected) in bounds.iter().zip([-110.0, -90.0]) {
        assert_eq!(b.to_f64(), expected);
        assert_eq!(b.width, 256);
        assert!(b.is_signed);
    }
    assert!(Value::tolerance_bound(&c, &Value::all_x(8), false, true).has_xz());
}
