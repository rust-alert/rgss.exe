#![warn(missing_docs)]
//! RPG Maker 游戏根检测与 RGSS 脚本包加载。
//!
//! 识别 2000 / 2003 / XP / VX / VX Ace。MV / MZ 返回错误。
//! XP / VX / Ace 的脚本是 RGSS 特殊 Ruby，经 `Scripts.*` 加载后交给 Spark。

mod detect;
mod marshal;
mod scripts;

pub use detect::{
    DetectError, DetectReport, Evidence, MakerEngine, ScriptKind, detect_game_root,
};
pub use marshal::{MarshalError, MarshalValue, load as load_marshal};
pub use scripts::{
    ScriptEntry, ScriptPack, ScriptsError, load_scripts, load_scripts_with_report,
    strip_rgss_comments,
};
