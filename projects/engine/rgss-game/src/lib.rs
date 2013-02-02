#![warn(missing_docs)]
//! 游戏根校验。产品入口是 `@game-gpt/rgss` 的 `rgss detect`。
//!
//! 不提供独立二进制。不打开窗口。

use std::path::Path;

pub use rgss_core::{
    DetectError, DetectReport, Evidence, MakerEngine, ScriptKind, detect_game_root,
};

/// 确认 `path` 是受支持的游戏根。失败信息含稳定错误码。
pub fn validate_game_root(path: &Path) -> Result<DetectReport, String> {
    detect_game_root(path).map_err(|err| format!("{} {}", err.code(), err.explain()))
}
