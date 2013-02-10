#!/usr/bin/env node
/**
 * 产品入口：
 *
 *   rgss --path <游戏根>           # 开窗口跑（默认）
 *   rgss detect --path <游戏根>    # 仅检测
 *   rgss play --path <游戏根>      # 无头编译执行
 */

import { spawnSync } from "node:child_process";
import fs from "node:fs";
import path from "node:path";

import { loadRgss } from "./index";

// 本包是 CommonJS，用 `__filename`（与 `index.ts` 一致），勿用 `import.meta`。
const hostRoot = path.resolve(path.dirname(__filename), "..");
const repoRoot = path.resolve(hostRoot, "..", "..", "..");

function usage(): never {
    console.error(`用法：
  rgss --path <游戏根>
  rgss detect --path <游戏根>
  rgss play --path <游戏根>

默认（仅 --path）：打开 Spark 窗口跑 RGSS。
detect：识别 RPG Maker 版本。
play：无头编译 Scripts（oak-ruby → spark-vm），不打开窗口。
也可用 -p / --path=。`);
    process.exitCode = 2;
    throw new Error("usage");
}

function takeValue(argv: string[], i: number, flag: string): string {
    const value = argv[i];
    if (!value || value.startsWith("-")) {
        console.error(`${flag} 缺少值。`);
        usage();
    }
    return value;
}

function parsePath(rest: string[]): string {
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
    return gameRoot.trim();
}

function findRgssBinary(): string | undefined {
    const exe = process.platform === "win32" ? "rgss.exe" : "rgss";
    const candidates = [
        path.join(repoRoot, "target", "release", exe),
        path.join(repoRoot, "target", "debug", exe),
        path.join(hostRoot, "bin", exe),
    ];
    return candidates.find((p) => fs.existsSync(p));
}

function runWindowed(gameRoot: string): void {
    const bin = findRgssBinary();
    if (!bin) {
        console.error(
            "找不到 rgss 二进制。请先：cargo build -p rgss-game --bin rgss --release",
        );
        process.exitCode = 1;
        return;
    }
    const result = spawnSync(bin, ["--path", gameRoot], {
        cwd: repoRoot,
        stdio: "inherit",
        windowsHide: false,
        env: {
            ...process.env,
            RUST_MIN_STACK: process.env.RUST_MIN_STACK ?? "16777216",
        },
    });
    if (result.error) {
        console.error(result.error.message);
        process.exitCode = 1;
        return;
    }
    process.exitCode = result.status ?? 1;
}

function main() {
    const argv = process.argv.slice(2);
    if (argv.length === 0) {
        usage();
    }

    const first = argv[0];
    let command: "window" | "detect" | "play";
    let rest: string[];

    if (first === "detect" || first === "play") {
        command = first;
        rest = argv.slice(1);
    } else if (
        first === "--path" ||
        first === "-p" ||
        first.startsWith("--path=") ||
        first === "--help" ||
        first === "-h"
    ) {
        if (first === "--help" || first === "-h") {
            usage();
        }
        command = "window";
        rest = argv;
    } else {
        console.error(`未知命令：${first}`);
        usage();
    }

    const gameRoot = parsePath(rest);

    if (command === "window") {
        runWindowed(gameRoot);
        return;
    }

    try {
        const host = loadRgss();
        if (command === "detect") {
            const report = host.detect(gameRoot);
            console.log(report.summary);
            return;
        }
        const report = host.play(gameRoot);
        console.log(report.summary);
        if (report.failures && report.failures.length > 0) {
            for (const line of report.failures.slice(0, 40)) {
                console.error(line);
            }
            if (report.failures.length > 40) {
                console.error(`… 另有 ${report.failures.length - 40} 条失败`);
            }
        }
        if (!report.ranEntry) {
            process.exitCode = 1;
        }
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
