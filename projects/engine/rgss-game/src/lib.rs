#![warn(missing_docs)]
//! RGSS 游戏根：检测、脚本加载，经 Spark Ruby 前端执行。
//!
//! 产品入口：`rgss --path <游戏根>`（窗口），或 `rgss detect` / `rgss play`。

mod display;
mod play;
mod window;

use std::path::Path;

pub use play::{
    FrameSync, PlayError, PlayReport, PlaySession, ScriptCompileStatus, play_game_root,
    play_script_pack, prepare_game_root, prepare_script_pack, run_session_threaded,
};
pub use rgss_core::{
    DetectError, DetectReport, Evidence, MakerEngine, ScriptEntry, ScriptKind, ScriptPack,
    ScriptsError, detect_game_root, load_scripts, strip_rgss_comments,
};
pub use window::{WindowPlayError, play_game_windowed};

/// 确认 `path` 是受支持的游戏根。
pub fn validate_game_root(path: &Path) -> Result<DetectReport, String> {
    detect_game_root(path).map_err(|err| format!("{} {}", err.code(), err.explain()))
}
