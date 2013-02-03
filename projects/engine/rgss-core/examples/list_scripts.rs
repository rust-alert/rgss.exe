fn main() {
    let path = std::env::args().nth(1).expect("path");
    let d = rgss_core::detect_game_root(std::path::Path::new(&path)).unwrap();
    let pack = rgss_core::load_scripts_with_report(std::path::Path::new(&path), &d).unwrap();
    println!("count={}", pack.entries.len());
    for (i, e) in pack.entries.iter().enumerate() {
        let empty = e.source.trim().is_empty();
        println!(
            "{i:03} bytes={:6} empty={empty} name={}",
            e.source.len(),
            e.name
        );
    }
}
