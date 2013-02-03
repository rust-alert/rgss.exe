//! 产品 CLI 二进制 `rgss`。
//!
//! ```text
//! rgss --path <游戏根>
//! cargo run -p rgss-game --bin rgss -- --path <游戏根>
//! ```

use std::env;
use std::path::PathBuf;
use std::process;

use rgss_game::play_game_windowed;

fn usage() -> ! {
    eprintln!(
        "用法：
  rgss --path <游戏根>

打开 Spark 窗口，经 oak-ruby → spark-script-ruby → spark-vm 跑 RGSS 标题循环。
Esc 或 RGSS_MAX_FRAMES 退出。
也可用 -p / --path=。"
    );
    process::exit(2);
}

fn parse_path(argv: &[String]) -> PathBuf {
    let mut game_root: Option<String> = None;
    let mut i = 0usize;
    while i < argv.len() {
        let arg = &argv[i];
        if arg == "--path" || arg == "-p" {
            i += 1;
            let Some(value) = argv.get(i) else {
                eprintln!("{arg} 缺少值。");
                usage();
            };
            if value.starts_with('-') {
                eprintln!("{arg} 缺少值。");
                usage();
            }
            game_root = Some(value.clone());
            i += 1;
            continue;
        }
        if let Some(value) = arg.strip_prefix("--path=") {
            if value.is_empty() {
                eprintln!("--path= 缺少值。");
                usage();
            }
            game_root = Some(value.to_string());
            i += 1;
            continue;
        }
        if arg == "--help" || arg == "-h" {
            usage();
        }
        eprintln!("未知参数：{arg}");
        usage();
    }
    let Some(root) = game_root.filter(|s| !s.trim().is_empty()) else {
        eprintln!("缺少 --path。");
        usage();
    };
    PathBuf::from(root.trim())
}

fn main() {
    let argv: Vec<String> = env::args().skip(1).collect();
    if argv.is_empty() {
        usage();
    }
    let path = parse_path(&argv);
    if !path.is_dir() {
        eprintln!("不是目录：{}", path.display());
        process::exit(2);
    }
    eprintln!("rgss: 打开窗口 {}", path.display());
    if let Err(e) = play_game_windowed(&path) {
        eprintln!("rgss: {e}");
        process::exit(1);
    }
}
