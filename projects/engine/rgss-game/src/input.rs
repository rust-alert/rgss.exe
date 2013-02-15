//! RGSS `Input` 键位快照。窗口线程采样 raw 态，脚本经 `Input.update` 提交边沿。
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
///
/// 窗口每帧 [`InputPad::sample`] 写入 pending；脚本 [`InputPad::update`] 才把
/// 按下态与 `trigger?` 边沿提交给脚本可读快照。
#[derive(Debug, Default)]
pub struct InputPad {
    /// 窗口最近采样的按下掩码。
    pending_down: AtomicU32,
    /// 自上次 `update` 以来累积的按下边沿（含 `key_pressed`）。
    pending_pressed: AtomicU32,
    /// 上次 `update` 提交的按下掩码。
    prev_down: AtomicU32,
    /// 脚本可见：当前按下。
    down: AtomicU32,
    /// 脚本可见：本周期触发边沿。
    trigger: AtomicU32,
}

impl InputPad {
    /// 新建。
    pub fn new() -> Arc<Self> {
        Arc::new(Self::default())
    }

    /// 窗口线程：写入 pending，不立刻改脚本可见的 `trigger?`。
    pub fn sample(&self, input: &spark_renderer::Input) {
        let mut down = 0u32;
        let mut pressed = 0u32;
        let mut mark = |code: u32, key: Key| {
            if code >= 31 {
                return;
            }
            let bit = 1u32 << code;
            if input.key_down(key) {
                down |= bit;
            }
            if input.key_pressed(key) {
                pressed |= bit;
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
            pressed |= 1 << KEY_DOWN;
        }
        if input.key_down(Key::A) {
            down |= 1 << KEY_LEFT;
        }
        if input.key_pressed(Key::A) {
            pressed |= 1 << KEY_LEFT;
        }
        if input.key_down(Key::D) {
            down |= 1 << KEY_RIGHT;
        }
        if input.key_pressed(Key::D) {
            pressed |= 1 << KEY_RIGHT;
        }
        if input.key_down(Key::W) {
            down |= 1 << KEY_UP;
        }
        if input.key_pressed(Key::W) {
            pressed |= 1 << KEY_UP;
        }
        self.pending_down.store(down, Ordering::SeqCst);
        let _ = self
            .pending_pressed
            .fetch_or(pressed, Ordering::SeqCst);
    }

    /// `Input.update`：提交按下态，并计算相对上一拍的触发边沿。
    pub fn update(&self) {
        let pending = self.pending_down.load(Ordering::SeqCst);
        let pressed = self.pending_pressed.swap(0, Ordering::SeqCst);
        let prev = self.prev_down.load(Ordering::SeqCst);
        let rising = pending & !prev;
        let trigger = pressed | rising;
        self.down.store(pending, Ordering::SeqCst);
        self.trigger.store(trigger, Ordering::SeqCst);
        self.prev_down.store(pending, Ordering::SeqCst);
    }

    /// 测试或回放：直接写入脚本可见掩码（并同步 prev，避免下次 `update` 误触发）。
    pub fn set_masks(&self, down: u32, trigger: u32) {
        self.pending_down.store(down, Ordering::SeqCst);
        self.pending_pressed.store(0, Ordering::SeqCst);
        self.prev_down.store(down, Ordering::SeqCst);
        self.down.store(down, Ordering::SeqCst);
        self.trigger.store(trigger, Ordering::SeqCst);
    }

    /// 测试：只写 pending，供 `update` 提交。
    pub fn set_pending(&self, down: u32, pressed: u32) {
        self.pending_down.store(down, Ordering::SeqCst);
        self.pending_pressed.store(pressed, Ordering::SeqCst);
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
    let dir = pad.clone();
    vm.register_native("Input_dir4", move |_ctx, _args| {
        Ok(spark_gc::Value::Number(dir.dir4() as f64))
    });
    vm.register_native("Input_update", move |_ctx, _args| {
        pad.update();
        Ok(spark_gc::Value::Null)
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

    #[test]
    fn update_latches_rising_edge() {
        let pad = InputPad::new();
        pad.set_pending(1 << KEY_C, 0);
        assert!(!pad.trigger(KEY_C));
        assert!(!pad.press(KEY_C));
        pad.update();
        assert!(pad.press(KEY_C));
        assert!(pad.trigger(KEY_C));
        // 仍按住：再次 update 不应再 trigger。
        pad.set_pending(1 << KEY_C, 0);
        pad.update();
        assert!(pad.press(KEY_C));
        assert!(!pad.trigger(KEY_C));
        // 松开再按下。
        pad.set_pending(0, 0);
        pad.update();
        assert!(!pad.press(KEY_C));
        pad.set_pending(1 << KEY_C, 1 << KEY_C);
        pad.update();
        assert!(pad.trigger(KEY_C));
    }
}
