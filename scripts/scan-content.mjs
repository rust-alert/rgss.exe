/**
 * 仓库内容扫描：拒绝提交游戏素材与安装派生产物。
 *
 * 禁止常见图片/音频、字体，以及 RPG Maker / 旧管线数据包。
 */

import fs from "node:fs";
import path from "node:path";
import { fileURLToPath } from "node:url";

const root = path.resolve(path.dirname(fileURLToPath(import.meta.url)), "..");

const SKIP_DIRS = new Set([
    ".git",
    "target",
    "node_modules",
    "dist",
    ".idea",
]);

const FORBIDDEN_DIRS = new Set([
    "Content",
    "content-cache",
    "metadata-cache",
    "derived-content",
]);

const FORBIDDEN_EXT = new Set([
    ".xnb",
    ".wld",
    ".plr",
    ".png",
    ".jpg",
    ".jpeg",
    ".gif",
    ".bmp",
    ".tga",
    ".wav",
    ".mp3",
    ".ogg",
    ".flac",
    ".xwb",
    ".xsb",
    ".xgs",
    ".ttf",
    ".otf",
    ".fx",
    ".fxc",
    ".rgssad",
    ".rgss2a",
    ".rgss3a",
    ".rxdata",
    ".rvdata",
    ".rvdata2",
    ".ldb",
    ".lmu",
    ".lmt",
    ".lsd",
]);

/** @type {string[]} */
const hits = [];

/**
 * @param {string} abs
 * @param {string} rel
 */
function walk(abs, rel) {
    let entries;
    try {
        entries = fs.readdirSync(abs, { withFileTypes: true });
    } catch (err) {
        console.error(`无法读取 ${rel || "."}：${err}`);
        process.exit(2);
    }
    for (const ent of entries) {
        const name = ent.name;
        const childRel = rel ? `${rel}/${name}` : name;
        if (ent.isDirectory()) {
            if (SKIP_DIRS.has(name)) {
                continue;
            }
            if (FORBIDDEN_DIRS.has(name)) {
                hits.push(`${childRel}/`);
                continue;
            }
            walk(path.join(abs, name), childRel);
            continue;
        }
        if (!ent.isFile()) {
            continue;
        }
        const ext = path.extname(name).toLowerCase();
        if (FORBIDDEN_EXT.has(ext)) {
            hits.push(childRel);
        }
    }
}

walk(root, "");

if (hits.length > 0) {
    console.error("仓库内容扫描失败。下列路径不得进入开源树：");
    for (const h of hits.slice(0, 50)) {
        console.error(`  ${h}`);
    }
    if (hits.length > 50) {
        console.error(`  …另有 ${hits.length - 50} 项`);
    }
    process.exit(1);
}

console.log("仓库内容扫描通过：未发现不得入库的素材包。");
