fn main() {
    let path = std::env::args().nth(1).expect("path");
    let want: Vec<String> = std::env::args().skip(2).collect();
    let d = rgss_core::detect_game_root(std::path::Path::new(&path)).unwrap();
    let pack = rgss_core::load_scripts_with_report(std::path::Path::new(&path), &d).unwrap();
    let out = std::path::Path::new("target/rgss-dump");
    let _ = std::fs::create_dir_all(out);
    for e in &pack.entries {
        if want.is_empty() || want.iter().any(|w| e.name.contains(w)) {
            let file = out.join(format!("{}.rb", e.name.chars().map(|c| if c.is_ascii_alphanumeric(){c}else{'_'}).collect::<String>()));
            std::fs::write(&file, rgss_core::strip_rgss_comments(&e.source)).unwrap();
            println!("wrote {} bytes={}", file.display(), e.source.len());
        }
    }
}
