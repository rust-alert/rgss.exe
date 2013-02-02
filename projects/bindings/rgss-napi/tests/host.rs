use rgss_napi::RgssHost;

#[test]
fn info_names_package() {
    let host = RgssHost::new();
    assert_eq!(host.info().name, "RGSS");
    assert_eq!(host.info().npm_package, "@game-gpt/rgss");
}

#[test]
fn reject_missing_directory() {
    let host = RgssHost::new();
    let err = host.detect("Z:/definitely-not-a-game-root").unwrap_err();
    assert!(err.contains("rgss.detect.not_directory"));
}
