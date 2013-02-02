/**
 * `pnpm fmt`：用 Biome 格式化 TypeScript / JavaScript / JSON。缩进 4 空格。
 */

import { spawnSync } from "node:child_process";
import { createRequire } from "node:module";
import path from "node:path";
import { fileURLToPath } from "node:url";

const root = path.resolve(path.dirname(fileURLToPath(import.meta.url)), "..");
const require = createRequire(path.join(root, "package.json"));

let biomeBin;
try {
    const pkgDir = path.dirname(require.resolve("@biomejs/biome/package.json"));
    biomeBin = path.join(pkgDir, "bin", "biome");
} catch {
    console.error("未安装 @biomejs/biome。请在仓库根执行 pnpm install。");
    process.exit(1);
}

const result = spawnSync(
    process.execPath,
    [biomeBin, "format", "--write", root],
    {
        cwd: root,
        stdio: "inherit",
        windowsHide: true,
    },
);

if (result.error) {
    console.error(result.error);
    process.exit(1);
}

process.exit(result.status ?? 1);
