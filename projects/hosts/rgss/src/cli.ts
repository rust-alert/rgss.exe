#!/usr/bin/env node
/**
 * 产品入口：
 *
 *   rgss detect --path <游戏根>
 *
 * 识别 RPG Maker 2000 / 2003 / XP / VX / VX Ace。
 * MV / MZ 会失败。不打开窗口。
 */

import { loadRgss } from "./index";

function usage(): never {
    console.error(`用法：
  rgss detect --path <游戏根>

识别 RPG Maker 2000、2003、XP、VX、VX Ace。
XP / VX / VX Ace 的脚本是 RGSS 特殊 Ruby。2000 / 2003 只报告事件指令。
MV / MZ 会失败。
本仓库不附带运行时或素材。不能用 cargo run。`);
    process.exitCode = 2;
    throw new Error("usage");
}

function takeValue(argv: string[], i: number, flag: string): string {
    const value = argv[i];
    if (!value || value.startsWith("--")) {
        console.error(`${flag} 缺少值。`);
        usage();
    }
    return value;
}

function main() {
    const [command, ...rest] = process.argv.slice(2);
    if (command !== "detect") {
        usage();
    }
    let gameRoot: string | undefined;
    for (let i = 0; i < rest.length; i++) {
        const arg = rest[i];
        if (arg === "--path" || arg === "-p") {
            gameRoot = takeValue(rest, ++i, arg);
            continue;
        }
        if (arg.startsWith("--path=")) {
            gameRoot = arg.slice("--path=".length);
            continue;
        }
        console.error(`未知参数：${arg}`);
        usage();
    }
    if (!gameRoot || !gameRoot.trim()) {
        console.error("缺少 --path。");
        usage();
    }
    try {
        const report = loadRgss().detect(gameRoot.trim());
        console.log(report.summary);
    } catch (err) {
        const message = err instanceof Error ? err.message : String(err);
        console.error(message);
        process.exitCode = 1;
    }
}

try {
    main();
} catch (err) {
    if (process.exitCode === undefined) {
        console.error(err instanceof Error ? err.message : err);
        process.exitCode = 1;
    }
}
