//! RGSS `Input` 键位快照。窗口线程每帧采样，脚本经 native 只读。
//!
//! 键码沿用 RPG Maker XP 公开常量：方向 2/4/6/8，确认键 C=13。

use std::sync::atomic::{AtomicU32, Ordering};
use std::sync::Arc;

use spark_renderer::Key;

/// 下。
pub const KEY_DOWN: u32 = 2;
/// 左。
pub const KEY_LEFT: u32 = 4;
/// 右。
pub const KEY_RIGHT: u32 = 6;
/// 上。
pub const KEY_UP: u32 = 8;
/// A（Shift）。
pub const KEY_A: u32 = 11;
/// B（取消）。
pub const KEY_B: u32 = 12;
/// C（确认）。
pub const KEY_C: u32 = 13;

/// 跨线程键位。bit `1 << code` 对应 RGSS 键码（码须 < 31）。
#[derive(Debug, Default)]
pub struct InputPad {
    down: AtomicU32,
    trigger: AtomicU32,
}

impl InputPad {
    /// 新建。
    pub fn new() -> Arc<Self> {
        Arc::new(Self::default())
    }

    /// 用本帧输入覆盖按下与触发边沿。
    pub fn sample(&self, input: &spark_renderer::Input) {
        let mut down = 0u32;
        let mut trigger = 0u32;
        let mut mark = |code: u32, key: Key| {
            if code >= 31 {
                return;
            }
            let bit = 1u32 << code;
            if input.key_down(key) {
                down |= bit;
            }
            if input.key_pressed(key) {
                trigger |= bit;
            }
        };
        mark(KEY_DOWN, Key::Down);
        mark(KEY_LEFT, Key::Left);
        mark(KEY_RIGHT, Key::Right);
        mark(KEY_UP, Key::Up);
        mark(KEY_A, Key::LShift);
        mark(KEY_B, Key::X);
        mark(KEY_B, Key::Escape);
        mark(KEY_C, Key::Enter);
        mark(KEY_C, Key::Space);
        mark(KEY_C, Key::Z);
        // 方向也接受 WASD，便于键位不全的窗口后端。
        if input.key_down(Key::S) {
            down |= 1 << KEY_DOWN;
        }
        if input.key_pressed(Key::S) {
            trigger |= 1 << KEY_DOWN;
        }
        if input.key_down(Key::A) {
            down |= 1 << KEY_LEFT;
        }
        if input.key_pressed(Key::A) {
            trigger |= 1 << KEY_LEFT;
        }
        if input.key_down(Key::D) {
            down |= 1 << KEY_RIGHT;
        }
        if input.key_pressed(Key::D) {
            trigger |= 1 << KEY_RIGHT;
        }
        if input.key_down(Key::W) {
            down |= 1 << KEY_UP;
        }
        if input.key_pressed(Key::W) {
            trigger |= 1 << KEY_UP;
        }
        self.down.store(down, Ordering::SeqCst);
        self.trigger.store(trigger, Ordering::SeqCst);
    }

    /// 测试或回放直接写入掩码。
    pub fn set_masks(&self, down: u32, trigger: u32) {
        self.down.store(down, Ordering::SeqCst);
        self.trigger.store(trigger, Ordering::SeqCst);
    }

    /// `Input.press?`
    pub fn press(&self, code: u32) -> bool {
        if code >= 31 {
            return false;
        }
        self.down.load(Ordering::SeqCst) & (1 << code) != 0
    }

    /// `Input.trigger?`
    pub fn trigger(&self, code: u32) -> bool {
        if code >= 31 {
            return false;
        }
        self.trigger.load(Ordering::SeqCst) & (1 << code) != 0
    }

    /// `Input.dir4`：对角为 0。
    pub fn dir4(&self) -> i32 {
        let down = self.press(KEY_DOWN);
        let up = self.press(KEY_UP);
        let left = self.press(KEY_LEFT);
        let right = self.press(KEY_RIGHT);
        let vert = match (up, down) {
            (true, false) => 8,
            (false, true) => 2,
            _ => 0,
        };
        let horz = match (left, right) {
            (true, false) => 4,
            (false, true) => 6,
            _ => 0,
        };
        if vert != 0 && horz != 0 {
            0
        } else {
            vert + horz
        }
    }
}

/// 注册 `Input.press?` / `trigger?` / `dir4`。
pub fn register_input_natives(vm: &mut spark_vm::Vm, pad: Arc<InputPad>) {
    let press = pad.clone();
    vm.register_native("Input_press?", move |_ctx, args| {
        let code = args.first().and_then(|v| v.as_number()).unwrap_or(0.0) as u32;
        Ok(spark_gc::Value::Bool(press.press(code)))
    });
    let trig = pad.clone();
    vm.register_native("Input_trigger?", move |_ctx, args| {
        let code = args.first().and_then(|v| v.as_number()).unwrap_or(0.0) as u32;
        Ok(spark_gc::Value::Bool(trig.trigger(code)))
    });
    vm.register_native("Input_dir4", move |_ctx, _args| {
        Ok(spark_gc::Value::Number(pad.dir4() as f64))
    });
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn dir4_rejects_diagonal() {
        let pad = InputPad::new();
        pad.set_masks(1 << KEY_UP, 0);
        assert_eq!(pad.dir4(), 8);
        assert!(pad.press(KEY_UP));
        assert!(!pad.trigger(KEY_UP));
        pad.set_masks((1 << KEY_UP) | (1 << KEY_RIGHT), 1 << KEY_C);
        assert_eq!(pad.dir4(), 0);
        assert!(pad.trigger(KEY_C));
    }
}
