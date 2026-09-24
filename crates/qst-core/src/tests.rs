use super::*;

#[test]
fn fixed_time_limits_catch_up() {
    let mut time = FixedTime::default();
    assert_eq!(time.push(Duration::from_secs(1)), 4);
    assert!(time.accumulator_seconds < time.step_seconds);
}

#[test]
fn transforms_interpolate() {
    let a = TransformState::identity();
    let mut b = a;
    b.translation = Vec3::X * 10.0;
    assert_eq!(a.lerp(b, 0.5).translation, Vec3::X * 5.0);
}
