//! RGSS 显示对象：Bitmap / Sprite / Color / Rect（最小可画标题）。

use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU32, Ordering};
use std::sync::{Arc, Mutex};

use spark_gc::{GcHandle, GcObject, Value};
use spark_vm::NativeCtx;

/// 一帧可画的 Sprite 快照。
#[derive(Debug, Clone)]
pub struct SpriteSnap {
    /// 位图 id（[`DisplayState`] 内）。
    pub bitmap_id: Option<u32>,
    pub x: f32,
    pub y: f32,
    pub z: i32,
    pub opacity: f32,
    pub zoom_x: f32,
    pub zoom_y: f32,
    /// 原点（位图像素），绘制时从 `x`/`y` 减去 `ox*zoom_x` / `oy*zoom_y`。
    pub ox: f32,
    pub oy: f32,
    /// RGSS `angle`（度，逆时针）。
    pub angle_deg: f32,
    /// 源矩形（像素）。宽或高为 0 时表示整张位图。
    pub src_x: f32,
    pub src_y: f32,
    pub src_w: f32,
    pub src_h: f32,
    pub visible: bool,
}

/// 把 `src_rect` 像素矩形转成归一化 UV，并返回绘制宽高（已乘 zoom）。
pub fn sprite_draw_uv(
    bmp_w: u32,
    bmp_h: u32,
    src_x: f32,
    src_y: f32,
    src_w: f32,
    src_h: f32,
    zoom_x: f32,
    zoom_y: f32,
) -> (f32, f32, f32, f32, f32, f32) {
    let bw = bmp_w.max(1) as f32;
    let bh = bmp_h.max(1) as f32;
    let use_full = src_w <= 0.0 || src_h <= 0.0;
    let (sx, sy, sw, sh) = if use_full {
        (0.0, 0.0, bw, bh)
    } else {
        (
            src_x.clamp(0.0, bw),
            src_y.clamp(0.0, bh),
            src_w.min(bw - src_x.max(0.0)).max(0.0),
            src_h.min(bh - src_y.max(0.0)).max(0.0),
        )
    };
    let u0 = sx / bw;
    let v0 = sy / bh;
    let uw = if bw > 0.0 { sw / bw } else { 1.0 };
    let vh = if bh > 0.0 { sh / bh } else { 1.0 };
    let dw = sw * zoom_x;
    let dh = sh * zoom_y;
    (u0, v0, uw, vh, dw, dh)
}

/// CPU 位图。
#[derive(Debug, Clone)]
pub struct BitmapData {
    pub width: u32,
    pub height: u32,
    pub rgba: Vec<u8>,
}

/// 跨 VM / 窗口线程的显示状态。
#[derive(Debug)]
pub struct DisplayState {
    game_root: PathBuf,
    next_id: AtomicU32,
    bitmaps: Mutex<HashMap<u32, BitmapData>>,
    /// Sprite 对象句柄（堆槽位），每帧从堆读字段。
    sprites: Mutex<Vec<GcHandle>>,
    frame_snap: Mutex<Vec<SpriteSnap>>,
}

impl DisplayState {
    /// 新建。
    pub fn new(game_root: PathBuf) -> Arc<Self> {
        Arc::new(Self {
            game_root,
            next_id: AtomicU32::new(1),
            bitmaps: Mutex::new(HashMap::new()),
            sprites: Mutex::new(Vec::new()),
            frame_snap: Mutex::new(Vec::new()),
        })
    }

    /// 游戏根。
    pub fn game_root(&self) -> &Path {
        &self.game_root
    }

    /// 分配位图，返回 id。
    pub fn alloc_bitmap(&self, width: u32, height: u32, rgba: Vec<u8>) -> u32 {
        let id = self.next_id.fetch_add(1, Ordering::Relaxed);
        if let Ok(mut map) = self.bitmaps.lock() {
            map.insert(
                id,
                BitmapData {
                    width,
                    height,
                    rgba,
                },
            );
        }
        id
    }

    /// 取位图。
    pub fn bitmap(&self, id: u32) -> Option<BitmapData> {
        self.bitmaps.lock().ok()?.get(&id).cloned()
    }

    /// 登记 Sprite 句柄。
    pub fn register_sprite(&self, handle: GcHandle) {
        if let Ok(mut list) = self.sprites.lock() {
            list.push(handle);
        }
    }

    /// 从堆快照 Sprite 列表供窗口绘制。
    pub fn snapshot_from_heap(&self, heap: &spark_gc::Heap) {
        let handles = self.sprites.lock().map(|g| g.clone()).unwrap_or_default();
        let mut snaps = Vec::new();
        for h in handles {
            let Ok(GcObject::Table(map)) = heap.get(h) else {
                continue;
            };
            let visible = map
                .get("visible")
                .map(|v| v.truthy())
                .unwrap_or(true);
            if !visible {
                continue;
            }
            let bitmap_id = map
                .get("bitmap")
                .and_then(|v| match v {
                    Value::Handle(bh) => match heap.get(*bh) {
                        Ok(GcObject::Table(bm)) => bm
                            .get("__bitmap_id")
                            .and_then(|id| id.as_number())
                            .map(|n| n as u32),
                        _ => None,
                    },
                    _ => None,
                });
            snaps.push(SpriteSnap {
                bitmap_id,
                x: num_field(map, "x"),
                y: num_field(map, "y"),
                z: num_field(map, "z") as i32,
                opacity: num_field(map, "opacity").clamp(0.0, 255.0) / 255.0,
                zoom_x: {
                    let z = num_field(map, "zoom_x");
                    if z == 0.0 { 1.0 } else { z }
                },
                zoom_y: {
                    let z = num_field(map, "zoom_y");
                    if z == 0.0 { 1.0 } else { z }
                },
                ox: num_field(map, "ox"),
                oy: num_field(map, "oy"),
                angle_deg: num_field(map, "angle"),
                src_x: 0.0,
                src_y: 0.0,
                src_w: 0.0,
                src_h: 0.0,
                visible: true,
            });
            if let Some(last) = snaps.last_mut() {
                if let Some((sx, sy, sw, sh)) = read_src_rect(heap, map) {
                    last.src_x = sx;
                    last.src_y = sy;
                    last.src_w = sw;
                    last.src_h = sh;
                }
            }
        }
        snaps.sort_by_key(|s| s.z);
        if let Ok(mut slot) = self.frame_snap.lock() {
            *slot = snaps;
        }
    }

    /// 窗口线程取上一帧快照。
    pub fn take_frame_snap(&self) -> Vec<SpriteSnap> {
        self.frame_snap
            .lock()
            .map(|g| g.clone())
            .unwrap_or_default()
    }
}

fn num_field(map: &HashMap<String, Value>, key: &str) -> f32 {
    map.get(key)
        .and_then(|v| v.as_number())
        .unwrap_or(0.0) as f32
}

/// 从 Sprite 表读 `src_rect`（Rect 表：x/y/width/height）。
fn read_src_rect(heap: &spark_gc::Heap, map: &HashMap<String, Value>) -> Option<(f32, f32, f32, f32)> {
    let Value::Handle(h) = map.get("src_rect")? else {
        return None;
    };
    let Ok(GcObject::Table(rect)) = heap.get(*h) else {
        return None;
    };
    Some((
        num_field(rect, "x"),
        num_field(rect, "y"),
        num_field(rect, "width"),
        num_field(rect, "height"),
    ))
}

fn table_insert(ctx: &mut NativeCtx<'_>, recv: &Value, key: &str, val: Value) {
    let Value::Handle(h) = recv else {
        return;
    };
    if let Ok(GcObject::Table(map)) = ctx.heap.get_mut(*h) {
        map.insert(key.into(), val);
    }
}

fn set_class(ctx: &mut NativeCtx<'_>, recv: &Value, class: &str) {
    let s = ctx.heap.alloc_string(class);
    table_insert(ctx, recv, "__class", s);
}

fn table_get<'a>(ctx: &'a NativeCtx<'_>, recv: &Value, key: &str) -> Value {
    let Value::Handle(h) = recv else {
        return Value::Null;
    };
    match ctx.heap.get(*h) {
        Ok(GcObject::Table(map)) => map.get(key).cloned().unwrap_or(Value::Null),
        _ => Value::Null,
    }
}

fn as_handle(v: &Value) -> Option<GcHandle> {
    match v {
        Value::Handle(h) => Some(*h),
        _ => None,
    }
}

fn color_rgba(ctx: &NativeCtx<'_>, color: &Value) -> [u8; 4] {
    let Some(h) = as_handle(color) else {
        return [255, 255, 255, 255];
    };
    let Ok(GcObject::Table(map)) = ctx.heap.get(h) else {
        return [255, 255, 255, 255];
    };
    let r = map.get("red").and_then(|v| v.as_number()).unwrap_or(255.0) as u8;
    let g = map.get("green").and_then(|v| v.as_number()).unwrap_or(255.0) as u8;
    let b = map.get("blue").and_then(|v| v.as_number()).unwrap_or(255.0) as u8;
    let a = map
        .get("alpha")
        .and_then(|v| v.as_number())
        .unwrap_or(255.0) as u8;
    [r, g, b, a]
}

fn load_image_file(path: &Path) -> Option<BitmapData> {
    let img = image::open(path).ok()?.to_rgba8();
    let (width, height) = img.dimensions();
    Some(BitmapData {
        width,
        height,
        rgba: img.into_raw(),
    })
}

fn resolve_graphic(root: &Path, rel: &str) -> Option<PathBuf> {
    let direct = root.join(rel);
    if direct.is_file() {
        return Some(direct);
    }
    // 尝试常见扩展。
    for ext in ["png", "PNG", "jpg", "JPG", "jpeg", "bmp"] {
        let p = if rel.contains('.') {
            direct.clone()
        } else {
            root.join(format!("{rel}.{ext}"))
        };
        if p.is_file() {
            return Some(p);
        }
        if !rel.contains('.') {
            // 也试 Graphics 相对
            continue;
        }
    }
    None
}

fn title_path(root: &Path, name: &str) -> Option<PathBuf> {
    let base = root.join("Graphics").join("Titles");
    for ext in ["png", "PNG", "jpg", "JPG", "jpeg"] {
        let p = base.join(format!("{name}.{ext}"));
        if p.is_file() {
            return Some(p);
        }
    }
    // 名称可能已带扩展或是完整相对路径。
    resolve_graphic(root, name).or_else(|| resolve_graphic(root, &format!("Graphics/Titles/{name}")))
}

fn value_as_path_string(ctx: &NativeCtx<'_>, v: &Value) -> Option<String> {
    match v {
        Value::Handle(h) => match ctx.heap.get(*h) {
            Ok(GcObject::String(s)) => Some(s.clone()),
            _ => None,
        },
        _ => None,
    }
}

/// 注册显示相关 native。
pub fn register_display_natives(vm: &mut spark_vm::Vm, display: Arc<DisplayState>) {
    {
        let display = display.clone();
        vm.register_native("Bitmap_initialize", move |ctx, args| {
            let recv = args.first().cloned().unwrap_or(Value::Null);
            let a1 = args.get(1);
            let a2 = args.get(2);
            let (w, h, rgba, path_note) = match (a1, a2) {
                (Some(Value::Number(w)), Some(Value::Number(h))) => {
                    let w = (*w).max(1.0) as u32;
                    let h = (*h).max(1.0) as u32;
                    let rgba = vec![0u8; (w * h * 4) as usize];
                    (w, h, rgba, None)
                }
                (Some(path), _) => {
                    let path_s = value_as_path_string(ctx, path).unwrap_or_default();
                    if let Some(file) = resolve_graphic(display.game_root(), &path_s) {
                        if let Some(bmp) = load_image_file(&file) {
                            (bmp.width, bmp.height, bmp.rgba, Some(path_s))
                        } else {
                            (32, 32, vec![0u8; 32 * 32 * 4], Some(path_s))
                        }
                    } else {
                        (32, 32, vec![0u8; 32 * 32 * 4], Some(path_s))
                    }
                }
                _ => (32, 32, vec![0u8; 32 * 32 * 4], None),
            };
            let id = display.alloc_bitmap(w, h, rgba);
            set_class(ctx, &recv, "Bitmap");
            table_insert(ctx, &recv, "__bitmap_id", Value::Number(id as f64));
            table_insert(ctx, &recv, "width", Value::Number(w as f64));
            table_insert(ctx, &recv, "height", Value::Number(h as f64));
            if let Some(p) = path_note {
                let ps = ctx.heap.alloc_string(p);
            table_insert(ctx, &recv, "__path", ps);
            }
            Ok(Value::Null)
        });
    }

    {
        let display = display.clone();
        vm.register_native("Bitmap_fill_rect", move |ctx, args| {
            // fill_rect(x,y,w,h,color) 或 fill_rect(rect, color)
            let recv = args.first().cloned().unwrap_or(Value::Null);
            let id = match table_get(ctx, &recv, "__bitmap_id").as_number() {
                Some(n) => n as u32,
                None => return Ok(Value::Null),
            };
            let (x, y, w, h, color) = if args.len() >= 6 {
                (
                    args.get(1).and_then(|v| v.as_number()).unwrap_or(0.0) as i32,
                    args.get(2).and_then(|v| v.as_number()).unwrap_or(0.0) as i32,
                    args.get(3).and_then(|v| v.as_number()).unwrap_or(0.0) as i32,
                    args.get(4).and_then(|v| v.as_number()).unwrap_or(0.0) as i32,
                    args.get(5).cloned().unwrap_or(Value::Null),
                )
            } else {
                let rect = args.get(1).cloned().unwrap_or(Value::Null);
                let color = args.get(2).cloned().unwrap_or(Value::Null);
                let (rx, ry, rw, rh) = match as_handle(&rect).and_then(|h| {
                    let GcObject::Table(m) = ctx.heap.get(h).ok()? else {
                        return None;
                    };
                    Some((
                        m.get("x").and_then(|v| v.as_number()).unwrap_or(0.0) as i32,
                        m.get("y").and_then(|v| v.as_number()).unwrap_or(0.0) as i32,
                        m.get("width").and_then(|v| v.as_number()).unwrap_or(0.0) as i32,
                        m.get("height").and_then(|v| v.as_number()).unwrap_or(0.0) as i32,
                    ))
                }) {
                    Some(t) => t,
                    None => (0, 0, 0, 0),
                };
                (rx, ry, rw, rh, color)
            };
            let rgba = color_rgba(ctx, &color);
            if let Ok(mut map) = display.bitmaps.lock() {
                if let Some(bmp) = map.get_mut(&id) {
                    fill_rect_rgba(bmp, x, y, w, h, rgba);
                }
            }
            Ok(Value::Null)
        });
    }

    {
        let display = display.clone();
        vm.register_native("Bitmap_blt", move |ctx, args| {
            // blt(x, y, src_bitmap, src_rect)
            let recv = args.first().cloned().unwrap_or(Value::Null);
            let dst_id = match table_get(ctx, &recv, "__bitmap_id").as_number() {
                Some(n) => n as u32,
                None => return Ok(Value::Null),
            };
            let dx = args.get(1).and_then(|v| v.as_number()).unwrap_or(0.0) as i32;
            let dy = args.get(2).and_then(|v| v.as_number()).unwrap_or(0.0) as i32;
            let src = args.get(3).cloned().unwrap_or(Value::Null);
            let src_id = table_get(ctx, &src, "__bitmap_id")
                .as_number()
                .map(|n| n as u32);
            let rect = args.get(4).cloned().unwrap_or(Value::Null);
            let (sx, sy, sw, sh) = match as_handle(&rect).and_then(|h| {
                let GcObject::Table(m) = ctx.heap.get(h).ok()? else {
                    return None;
                };
                Some((
                    m.get("x").and_then(|v| v.as_number()).unwrap_or(0.0) as i32,
                    m.get("y").and_then(|v| v.as_number()).unwrap_or(0.0) as i32,
                    m.get("width").and_then(|v| v.as_number()).unwrap_or(0.0) as i32,
                    m.get("height").and_then(|v| v.as_number()).unwrap_or(0.0) as i32,
                ))
            }) {
                Some(t) => t,
                None => (0, 0, 0, 0),
            };
            let Some(src_id) = src_id else {
                return Ok(Value::Null);
            };
            if let Ok(mut map) = display.bitmaps.lock() {
                let src_bmp = map.get(&src_id).cloned();
                if let (Some(src_bmp), Some(dst)) = (src_bmp, map.get_mut(&dst_id)) {
                    blt_rgba(dst, dx, dy, &src_bmp, sx, sy, sw, sh);
                }
            }
            Ok(Value::Null)
        });
    }

    vm.register_native("Bitmap_dispose", |_ctx, _args| Ok(Value::Null));

    {
        let display = display.clone();
        vm.register_native("Sprite_initialize", move |ctx, args| {
            let recv = args.first().cloned().unwrap_or(Value::Null);
            set_class(ctx, &recv, "Sprite");
            table_insert(ctx, &recv, "x", Value::Number(0.0));
            table_insert(ctx, &recv, "y", Value::Number(0.0));
            table_insert(ctx, &recv, "z", Value::Number(0.0));
            table_insert(ctx, &recv, "ox", Value::Number(0.0));
            table_insert(ctx, &recv, "oy", Value::Number(0.0));
            table_insert(ctx, &recv, "angle", Value::Number(0.0));
            table_insert(ctx, &recv, "zoom_x", Value::Number(1.0));
            table_insert(ctx, &recv, "zoom_y", Value::Number(1.0));
            table_insert(ctx, &recv, "opacity", Value::Number(255.0));
            table_insert(ctx, &recv, "blend_type", Value::Number(0.0));
            table_insert(ctx, &recv, "visible", Value::Bool(true));
            table_insert(ctx, &recv, "bitmap", Value::Null);
            // 默认空 Rect：绘制时宽/高为 0 → 使用整张位图。
            let class_name = ctx.heap.alloc_string("Rect");
            let rh = ctx.heap.alloc(GcObject::Table(HashMap::new()));
            if let Ok(GcObject::Table(m)) = ctx.heap.get_mut(rh) {
                m.insert("__class".into(), class_name);
                m.insert("x".into(), Value::Number(0.0));
                m.insert("y".into(), Value::Number(0.0));
                m.insert("width".into(), Value::Number(0.0));
                m.insert("height".into(), Value::Number(0.0));
            }
            table_insert(ctx, &recv, "src_rect", Value::Handle(rh));
            if let Value::Handle(h) = &recv {
                display.register_sprite(*h);
            }
            Ok(Value::Null)
        });
    }

    vm.register_native("Sprite_dispose", |_ctx, _args| Ok(Value::Null));
    vm.register_native("Sprite_update", |_ctx, _args| Ok(Value::Null));

    vm.register_native("Color_initialize", |ctx, args| {
        let recv = args.first().cloned().unwrap_or(Value::Null);
        let r = args.get(1).and_then(|v| v.as_number()).unwrap_or(0.0);
        let g = args.get(2).and_then(|v| v.as_number()).unwrap_or(0.0);
        let b = args.get(3).and_then(|v| v.as_number()).unwrap_or(0.0);
        let a = args.get(4).and_then(|v| v.as_number()).unwrap_or(255.0);
        set_class(ctx, &recv, "Color");
        table_insert(ctx, &recv, "red", Value::Number(r));
        table_insert(ctx, &recv, "green", Value::Number(g));
        table_insert(ctx, &recv, "blue", Value::Number(b));
        table_insert(ctx, &recv, "alpha", Value::Number(a));
        Ok(Value::Null)
    });

    vm.register_native("Rect_initialize", |ctx, args| {
        let recv = args.first().cloned().unwrap_or(Value::Null);
        let x = args.get(1).and_then(|v| v.as_number()).unwrap_or(0.0);
        let y = args.get(2).and_then(|v| v.as_number()).unwrap_or(0.0);
        let w = args.get(3).and_then(|v| v.as_number()).unwrap_or(0.0);
        let h = args.get(4).and_then(|v| v.as_number()).unwrap_or(0.0);
        set_class(ctx, &recv, "Rect");
        table_insert(ctx, &recv, "x", Value::Number(x));
        table_insert(ctx, &recv, "y", Value::Number(y));
        table_insert(ctx, &recv, "width", Value::Number(w));
        table_insert(ctx, &recv, "height", Value::Number(h));
        Ok(Value::Null)
    });

    vm.register_native("Viewport_initialize", |ctx, args| {
        let recv = args.first().cloned().unwrap_or(Value::Null);
        set_class(ctx, &recv, "Viewport");
        table_insert(ctx, &recv, "z", Value::Number(0.0));
        table_insert(ctx, &recv, "visible", Value::Bool(true));
        Ok(Value::Null)
    });
    vm.register_native("Viewport_dispose", |_ctx, _args| Ok(Value::Null));

    {
        let display = display.clone();
        vm.register_native("RPG_Cache_title", move |ctx, args| {
            let name = args
                .first()
                .and_then(|v| value_as_path_string(ctx, v))
                .unwrap_or_default();
            let mut table = HashMap::new();
            table.insert("__class".into(), ctx.heap.alloc_string("Bitmap"));
            if let Some(path) = title_path(display.game_root(), &name) {
                if let Some(bmp) = load_image_file(&path) {
                    let id = display.alloc_bitmap(bmp.width, bmp.height, bmp.rgba);
                    table.insert("__bitmap_id".into(), Value::Number(id as f64));
                    table.insert("width".into(), Value::Number(bmp.width as f64));
                    table.insert("height".into(), Value::Number(bmp.height as f64));
                }
            }
            Ok(Value::Handle(ctx.heap.alloc(GcObject::Table(table))))
        });
    }
}

fn fill_rect_rgba(bmp: &mut BitmapData, x: i32, y: i32, w: i32, h: i32, rgba: [u8; 4]) {
    if w <= 0 || h <= 0 {
        return;
    }
    let bw = bmp.width as i32;
    let bh = bmp.height as i32;
    for py in y.max(0)..(y + h).min(bh) {
        for px in x.max(0)..(x + w).min(bw) {
            let i = ((py as u32 * bmp.width + px as u32) * 4) as usize;
            if i + 3 < bmp.rgba.len() {
                bmp.rgba[i] = rgba[0];
                bmp.rgba[i + 1] = rgba[1];
                bmp.rgba[i + 2] = rgba[2];
                bmp.rgba[i + 3] = rgba[3];
            }
        }
    }
}

fn blt_rgba(
    dst: &mut BitmapData,
    dx: i32,
    dy: i32,
    src: &BitmapData,
    sx: i32,
    sy: i32,
    sw: i32,
    sh: i32,
) {
    if sw <= 0 || sh <= 0 {
        return;
    }
    for row in 0..sh {
        for col in 0..sw {
            let spx = sx + col;
            let spy = sy + row;
            let dpx = dx + col;
            let dpy = dy + row;
            if spx < 0 || spy < 0 || spx >= src.width as i32 || spy >= src.height as i32 {
                continue;
            }
            if dpx < 0 || dpy < 0 || dpx >= dst.width as i32 || dpy >= dst.height as i32 {
                continue;
            }
            let si = ((spy as u32 * src.width + spx as u32) * 4) as usize;
            let di = ((dpy as u32 * dst.width + dpx as u32) * 4) as usize;
            if si + 3 < src.rgba.len() && di + 3 < dst.rgba.len() {
                let sa = src.rgba[si + 3] as u32;
                if sa == 0 {
                    continue;
                }
                if sa == 255 {
                    dst.rgba[di..di + 4].copy_from_slice(&src.rgba[si..si + 4]);
                } else {
                    let inv = 255 - sa;
                    for c in 0..3 {
                        let s = src.rgba[si + c] as u32;
                        let d = dst.rgba[di + c] as u32;
                        dst.rgba[di + c] = ((s * sa + d * inv) / 255) as u8;
                    }
                    dst.rgba[di + 3] = dst.rgba[di + 3].max(src.rgba[si + 3]);
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::sprite_draw_uv;

    #[test]
    fn full_bitmap_when_src_size_zero() {
        let (u0, v0, uw, vh, dw, dh) = sprite_draw_uv(100, 50, 0.0, 0.0, 0.0, 0.0, 2.0, 1.0);
        assert_eq!((u0, v0, uw, vh), (0.0, 0.0, 1.0, 1.0));
        assert_eq!((dw, dh), (200.0, 50.0));
    }

    #[test]
    fn cropped_src_rect_uv() {
        let (u0, v0, uw, vh, dw, dh) =
            sprite_draw_uv(200, 100, 50.0, 25.0, 100.0, 50.0, 1.0, 1.0);
        assert!((u0 - 0.25).abs() < 1e-5);
        assert!((v0 - 0.25).abs() < 1e-5);
        assert!((uw - 0.5).abs() < 1e-5);
        assert!((vh - 0.5).abs() < 1e-5);
        assert_eq!((dw, dh), (100.0, 50.0));
    }

    #[test]
    fn origin_offset_matches_rgss() {
        // dest = (x - ox * zoom_x, y - oy * zoom_y)
        let ox = 16.0;
        let oy = 8.0;
        let zoom_x = 2.0;
        let zoom_y = 2.0;
        let x = 100.0;
        let y = 50.0;
        assert_eq!(x - ox * zoom_x, 68.0);
        assert_eq!(y - oy * zoom_y, 34.0);
    }

    #[test]
    fn angle_degrees_to_radians() {
        let deg = 90.0_f32;
        assert!((deg.to_radians() - std::f32::consts::FRAC_PI_2).abs() < 1e-5);
    }
}
