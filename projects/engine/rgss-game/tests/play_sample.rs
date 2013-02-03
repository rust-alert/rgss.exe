//! 对外部样本《罪途箱庭》跑 play 链路。需设置 `RGSS_SAMPLE`。

use std::path::Path;

use rgss_core::{detect_game_root, load_scripts_with_report, strip_rgss_comments};
use rgss_game::play_game_root;

#[test]
#[ignore]
fn play_external_sample() {
    let path = std::env::var("RGSS_SAMPLE").expect("RGSS_SAMPLE");
    let report = play_game_root(Path::new(&path)).expect("play");
    eprintln!("{}", report.summary_line());
    for s in report.statuses.iter().filter(|s| !s.ok).take(30) {
        eprintln!("FAIL [{}] {}: {}", s.index, s.name, s.detail);
    }
    if std::env::var("RGSS_DUMP_FAILS").is_ok() {
        let detect = detect_game_root(Path::new(&path)).unwrap();
        let pack = load_scripts_with_report(Path::new(&path), &detect).unwrap();
        let out = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../../target/rgss-dump");
        let _ = std::fs::create_dir_all(&out);
        for s in report.statuses.iter().filter(|s| !s.ok).take(8) {
            if let Some(entry) = pack.entries.get(s.index) {
                let file = out.join(format!("{}_{}.rb", s.index, sanitize(&entry.name)));
                let body = strip_rgss_comments(&entry.source);
                std::fs::write(&file, body).unwrap();
                eprintln!("dump {}", file.display());
            }
        }
    }
    assert!(
        report.compiled + report.empty > 0,
        "expected some scripts compiled or empty"
    );
    assert!(
        report.ran_entry,
        "entry did not run: {}",
        report.run_detail
    );
    // 标题主循环应至少泵过若干帧（Graphics.update）。
    if let Some(frames) = report
        .run_detail
        .split_whitespace()
        .find_map(|p| p.strip_prefix("frames=")?.parse::<u32>().ok())
    {
        assert!(
            frames >= 1,
            "expected Graphics.update frames, got {frames}: {}",
            report.run_detail
        );
    }
}

fn sanitize(name: &str) -> String {
    name.chars()
        .map(|c| if c.is_ascii_alphanumeric() { c } else { '_' })
        .take(40)
        .collect()
}
