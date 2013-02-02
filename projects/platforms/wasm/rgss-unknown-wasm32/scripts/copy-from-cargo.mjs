/**
 * 将 `rgss-wasm` 的 release Wasm 拷到本包根目录 `rgss_bg.wasm`。
 */
import { copyFileSync, existsSync, mkdirSync } from "node:fs";
import { dirname, join, resolve } from "node:path";
import { fileURLToPath } from "node:url";

const here = dirname(fileURLToPath(import.meta.url));
const pkgRoot = resolve(here, "..");
const repoRoot = resolve(pkgRoot, "../../../..");
const src = join(
    repoRoot,
    "target/wasm32-unknown-unknown/release/rgss_wasm.wasm",
);
const dest = join(pkgRoot, "rgss_bg.wasm");

if (!existsSync(src)) {
    console.error(
        `未找到 ${src}\n请先：rustup target add wasm32-unknown-unknown\n然后：cargo build -p rgss-wasm --target wasm32-unknown-unknown --release`,
    );
    process.exit(1);
}

mkdirSync(pkgRoot, { recursive: true });
copyFileSync(src, dest);
console.log(`已拷贝 → ${dest}`);
