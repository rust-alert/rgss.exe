#![warn(missing_docs)]
//! RPG Maker 游戏根检测。
//!
//! 识别 2000 / 2003 / XP / VX / VX Ace。MV / MZ 返回错误。
//! XP / VX / Ace 的脚本是 RGSS 特殊 Ruby。2000 / 2003 只有事件指令，本 crate 不执行。

mod detect;

pub use detect::{
    DetectError, DetectReport, Evidence, MakerEngine, ScriptKind, detect_game_root,
};
