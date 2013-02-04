/**
 * `pnpm launch`：编译原生与 TypeScript，再 `rgss --path <游戏根>` 开窗口。
 *
 *   pnpm launch -- --path <游戏根>
 *   pnpm launch -- --path <游戏根> --release
 */

import { spawnSync } from "node:child_process";
import path from "node:path";
import { fileURLToPath } from "node:url";

const root = path.resolve(path.dirname(fileURLToPath(import.meta.url)), "..");

function fail(message) {
    console.error(message);
    process.exit(1);
}

function run(command, args) {
    const result = spawnSync(command, args, {
        cwd: root,
        stdio: "inherit",
        windowsHide: false,
        shell: process.platform === "win32" && command === "cargo",
    });
    if (result.error) {
        fail(result.error.message);
    }
    if ((result.status ?? 1) !== 0) {
        process.exit(result.status ?? 1);
    }
}

function parseArgs(argv) {
    let gameRoot;
    let release = false;
    for (let i = 0; i < argv.length; i++) {
        const arg = argv[i];
        if (arg === "--") {
            continue;
        }
        if (arg === "--release") {
            release = true;
            continue;
        }
        if (arg === "--path" || arg === "-p") {
            gameRoot = argv[++i];
            continue;
        }
        if (arg.startsWith("--path=")) {
            gameRoot = arg.slice("--path=".length);
            continue;
        }
        fail(`未知参数：${arg}`);
    }
    if (!gameRoot || !gameRoot.trim()) {
        fail("缺少 --path <游戏根>。");
    }
    return { gameRoot: gameRoot.trim(), release };
}

function resolveTypescript() {
    return path.join(root, "node_modules", "typescript", "lib", "tsc.js");
}

function main() {
    const { gameRoot, release } = parseArgs(process.argv.slice(2));
    const napiArgs = [path.join(root, "scripts", "build", "napi.mjs")];
    const profile = release ? "release" : "dev";
    if (release) {
        napiArgs.push("--release");
    }
    run(process.execPath, napiArgs);
    const cargoArgs = ["build", "-p", "rgss-game", "--bin", "rgss"];
    if (release) {
        cargoArgs.push("--release");
    }
    run("cargo", cargoArgs);
    run(process.execPath, [
        resolveTypescript(),
        "-p",
        path.join(root, "projects", "hosts", "rgss", "tsconfig.json"),
    ]);
    run(process.execPath, [
        path.join(root, "projects", "hosts", "rgss", "dist", "cli.js"),
        "--path",
        gameRoot,
    ]);
    void profile;
}

main();
