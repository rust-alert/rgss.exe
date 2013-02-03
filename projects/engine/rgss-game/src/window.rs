//! Spark 窗口宿主：与 VM 线程上的 `Graphics.update` 同频。

use std::path::Path;
use std::sync::atomic::{AtomicU32, Ordering};
use std::sync::Arc;

use spark_core::Color;
use spark_engine::run_game;
use spark_renderer::{DrawList, FrameCtx, GameHost, Key, WindowConfig};

use crate::play::{prepare_game_root, run_session_threaded, FrameSync, PlayError};

/// 窗口 play 错误。
#[derive(Debug)]
pub enum WindowPlayError {
    /// 脚本准备失败。
    Play(PlayError),
    /// 窗口 / GPU。
    Window(String),
}

impl std::fmt::Display for WindowPlayError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Play(e) => write!(f, "{e}"),
            Self::Window(e) => write!(f, "{e}"),
        }
    }
}

impl std::error::Error for WindowPlayError {}

/// 打开 Spark 窗口并跑 RGSS 标题主循环（Esc 退出）。
pub fn play_game_windowed(path: &Path) -> Result<(), WindowPlayError> {
    let session = prepare_game_root(path).map_err(WindowPlayError::Play)?;
    let title = session.title.clone();
    let frames = Arc::new(AtomicU32::new(0));
    let sync = Arc::new(FrameSync::new());
    let max_frames = std::env::var("RGSS_MAX_FRAMES")
        .ok()
        .and_then(|s| s.parse().ok())
        .unwrap_or(u32::MAX);
    let join = run_session_threaded(session, frames.clone(), sync.clone(), max_frames);

    let host = RgssWindowHost {
        title: title.clone(),
        frames,
        sync: sync.clone(),
        exit: false,
        vm_done: false,
        join: Some(join),
        status: "正在编译 / 启动脚本…".into(),
    };

    run_game(
        WindowConfig {
            title,
            width: 640,
            height: 480,
            clear_color: [0.08, 0.05, 0.12, 1.0],
        },
        host,
    )
    .map_err(|e| WindowPlayError::Window(e.to_string()))?;

    Ok(())
}

struct RgssWindowHost {
    title: String,
    frames: Arc<AtomicU32>,
    sync: Arc<FrameSync>,
    exit: bool,
    vm_done: bool,
    join: Option<std::thread::JoinHandle<Result<(spark_gc::Value, u32, usize), spark_vm::VmError>>>,
    status: String,
}

impl GameHost for RgssWindowHost {
    fn update(&mut self, frame: &FrameCtx<'_>) {
        if frame.input.key_pressed(Key::Escape) {
            self.sync.request_exit();
            self.exit = true;
        }

        // 放行一帧：标题循环里的 Graphics.update 与窗口同频。
        if !self.vm_done {
            self.sync.pump_frame();
        }

        if let Some(handle) = self.join.as_ref() {
            if handle.is_finished() {
                if let Some(h) = self.join.take() {
                    match h.join() {
                        Ok(Ok((_, f, _))) => {
                            self.status = format!("脚本结束 frames={f}");
                        }
                        Ok(Err(e)) => {
                            self.status = format!("VM 错误: {e}");
                        }
                        Err(_) => {
                            self.status = "VM 线程 panic".into();
                        }
                    }
                    self.vm_done = true;
                    self.sync.request_exit();
                    self.exit = true;
                }
            } else {
                let f = self.frames.load(Ordering::SeqCst);
                if f > 0 {
                    self.status = format!("标题循环 frames={f}（Esc 退出）");
                }
                // 有限帧烟测：RGSS_MAX_FRAMES 到齐后自动关窗。
                if let Ok(max) = std::env::var("RGSS_MAX_FRAMES") {
                    if let Ok(max) = max.parse::<u32>() {
                        if max > 0 && f >= max {
                            self.sync.request_exit();
                            self.exit = true;
                        }
                    }
                }
            }
        }
    }

    fn draw(&mut self, draw: &mut DrawList) {
        draw.clear = Color::rgba(0.08, 0.05, 0.12, 1.0);
        draw.begin_hud();
        // 深色底板，模拟标题氛围（尚无 Bitmap 上传）。
        draw.fill_rect(
            spark_core::Rect {
                x: 0.0,
                y: 0.0,
                w: 640.0,
                h: 480.0,
            },
            Color::rgba(0.12, 0.08, 0.18, 1.0),
        );
        draw.text(
            24.0,
            40.0,
            28.0,
            Color::rgba(0.95, 0.9, 0.85, 1.0),
            &self.title,
        );
        draw.text(
            24.0,
            90.0,
            18.0,
            Color::rgba(0.75, 0.7, 0.8, 1.0),
            &self.status,
        );
        let f = self.frames.load(Ordering::SeqCst);
        draw.text(
            24.0,
            130.0,
            16.0,
            Color::rgba(0.55, 0.85, 0.65, 1.0),
            format!("Graphics.update = {f}"),
        );
    }

    fn should_exit(&self) -> bool {
        self.exit
    }
}

impl Drop for RgssWindowHost {
    fn drop(&mut self) {
        self.sync.request_exit();
        if let Some(h) = self.join.take() {
            let _ = h.join();
        }
    }
}
