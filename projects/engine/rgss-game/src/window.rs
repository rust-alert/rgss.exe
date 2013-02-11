//! Spark 窗口宿主：与 VM 线程上的 `Graphics.update` 同频。

use std::collections::HashMap;
use std::path::Path;
use std::sync::atomic::{AtomicU32, Ordering};
use std::sync::Arc;

use spark_core::{Color, Rect};
use spark_engine::run_game;
use spark_renderer::{DrawList, FrameCtx, GameHost, Key, TextureId, WindowConfig};

use crate::display::DisplayState;
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
    let display = DisplayState::new(session.game_root.clone());
    let input = crate::input::InputPad::new();
    let frames = Arc::new(AtomicU32::new(0));
    let sync = Arc::new(FrameSync::new());
    let max_frames = std::env::var("RGSS_MAX_FRAMES")
        .ok()
        .and_then(|s| s.parse().ok())
        .unwrap_or(u32::MAX);
    let join = run_session_threaded(
        session,
        frames.clone(),
        sync.clone(),
        max_frames,
        Some(display.clone()),
        input.clone(),
    );

    let host = RgssWindowHost {
        title: title.clone(),
        frames,
        sync: sync.clone(),
        display,
        input,
        textures: HashMap::new(),
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
    display: Arc<DisplayState>,
    input: Arc<crate::input::InputPad>,
    textures: HashMap<u32, TextureId>,
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
        self.input.sample(frame.input);

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
        draw.clear = Color::rgba(0.05, 0.04, 0.08, 1.0);
        draw.begin_world();
        draw.fill_rect(
            Rect {
                x: 0.0,
                y: 0.0,
                w: 640.0,
                h: 480.0,
            },
            Color::rgba(0.08, 0.06, 0.12, 1.0),
        );

        let snaps = self.display.take_frame_snap();
        for snap in &snaps {
            let Some(bid) = snap.bitmap_id else {
                continue;
            };
            let Some(bmp) = self.display.bitmap(bid) else {
                continue;
            };
            // 位图可能被 fill_rect / blt 改写，每帧重传像素。
            let tex = if let Some(id) = self.textures.get(&bid).copied() {
                let _ = draw.update_texture(id, bmp.width, bmp.height, bmp.rgba.clone());
                id
            } else {
                match draw.create_texture(bmp.width, bmp.height, bmp.rgba.clone()) {
                    Ok(id) => {
                        self.textures.insert(bid, id);
                        id
                    }
                    Err(_) => continue,
                }
            };
            let (u0, v0, uw, vh, w, h) = crate::display::sprite_draw_uv(
                bmp.width,
                bmp.height,
                snap.src_x,
                snap.src_y,
                snap.src_w,
                snap.src_h,
                snap.zoom_x,
                snap.zoom_y,
            );
            let a = snap.opacity.clamp(0.0, 1.0);
            draw.tex_rect(
                tex,
                Rect {
                    x: snap.x,
                    y: snap.y,
                    w,
                    h,
                },
                Rect {
                    x: u0,
                    y: v0,
                    w: uw,
                    h: vh,
                },
                Color::rgba(1.0, 1.0, 1.0, a),
            );
        }

        draw.begin_hud();
        draw.text(
            12.0,
            12.0,
            18.0,
            Color::rgba(0.95, 0.9, 0.85, 0.85),
            &self.title,
        );
        draw.text(
            12.0,
            36.0,
            14.0,
            Color::rgba(0.7, 0.75, 0.8, 0.8),
            &self.status,
        );
        let f = self.frames.load(Ordering::SeqCst);
        draw.text(
            12.0,
            56.0,
            13.0,
            Color::rgba(0.5, 0.85, 0.6, 0.8),
            format!("Graphics.update = {f}  sprites = {}", snaps.len()),
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
