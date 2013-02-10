//! Windows 主线程栈由 PE `/STACK` 决定，`RUST_MIN_STACK` 只作用于 `thread::spawn`。
//! RGSS 编译与窗口泵都在主线程，默认 1MB 会在罪途箱庭这种大脚本上溢出。

fn main() {
    if std::env::var("CARGO_CFG_TARGET_OS").ok().as_deref() == Some("windows") {
        println!("cargo:rustc-link-arg-bins=/STACK:33554432");
    }
}
