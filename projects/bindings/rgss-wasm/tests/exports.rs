#[test]
fn exports() {
    assert!((rgss_wasm::rgss_vec2_length(3.0, 4.0) - 5.0).abs() < 1e-9);
    assert_eq!(rgss_wasm::rgss_version_code(), 0);
}
