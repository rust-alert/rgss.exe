/**
 * Build `rgss-napi` and install the `.node` into `projects/platforms/native/rgss-<short>/`.
 *
 * Usage: node scripts/build/napi.mjs [--release]
 */

import { spawnSync } from "node:child_process";
import {
    copyFileSync,
    existsSync,
    mkdirSync,
    readdirSync,
    writeFileSync,
} from "node:fs";
import path from "node:path";
import { fileURLToPath } from "node:url";

const root = path.resolve(
    path.dirname(fileURLToPath(import.meta.url)),
    "../..",
);
const release = process.argv.includes("--release");
const profile = release ? "release" : "debug";

/** @returns {{ triple: string, short: string, os: string[], cpu: string[] }} */
function platformInfo() {
    const { platform, arch } = process;
    if (platform === "win32" && arch === "x64") {
        return {
            triple: "win32-x64-msvc",
            short: "win32-x64",
            os: ["win32"],
            cpu: ["x64"],
        };
    }
    if (platform === "win32" && arch === "arm64") {
        return {
            triple: "win32-arm64-msvc",
            short: "win32-arm64",
            os: ["win32"],
            cpu: ["arm64"],
        };
    }
    if (platform === "darwin" && arch === "arm64") {
        return {
            triple: "darwin-arm64",
            short: "darwin-arm64",
            os: ["darwin"],
            cpu: ["arm64"],
        };
    }
    if (platform === "darwin" && arch === "x64") {
        return {
            triple: "darwin-x64",
            short: "darwin-x64",
            os: ["darwin"],
            cpu: ["x64"],
        };
    }
    if (platform === "linux" && arch === "x64") {
        return {
            triple: "linux-x64-gnu",
            short: "linux-x64",
            os: ["linux"],
            cpu: ["x64"],
        };
    }
    if (platform === "linux" && arch === "arm64") {
        return {
            triple: "linux-arm64-gnu",
            short: "linux-arm64",
            os: ["linux"],
            cpu: ["arm64"],
        };
    }
    const triple = `${platform}-${arch}`;
    return { triple, short: triple, os: [platform], cpu: [arch] };
}

const cargoArgs = [
    "build",
    "--manifest-path",
    path.join(root, "Cargo.toml"),
    "-p",
    "rgss-napi",
    "--features",
    "node",
];
if (release) cargoArgs.push("--release");

console.log(`cargo ${cargoArgs.join(" ")}`);
const build = spawnSync("cargo", cargoArgs, {
    cwd: root,
    stdio: "inherit",
    shell: false,
    windowsHide: true,
});
if (build.error) {
    console.error(build.error);
    process.exit(1);
}
if (build.status !== 0) {
    process.exit(build.status ?? 1);
}

const targetDir = path.join(root, "target", profile);
const stem = "rgss_napi";
const candidates = [];
if (process.platform === "win32") {
    candidates.push(`${stem}.dll`, `${stem}.node`);
} else if (process.platform === "darwin") {
    candidates.push(`lib${stem}.dylib`, `${stem}.dylib`, `${stem}.node`);
} else {
    candidates.push(`lib${stem}.so`, `${stem}.so`, `${stem}.node`);
}

let artifact = null;
for (const name of candidates) {
    const p = path.join(targetDir, name);
    if (existsSync(p)) {
        artifact = p;
        break;
    }
}

if (!artifact) {
    const deps = path.join(targetDir, "deps");
    if (existsSync(deps)) {
        for (const name of readdirSync(deps)) {
            const base = name.replace(/^lib/, "");
            if (
                (name === stem ||
                    name.startsWith(`${stem}.`) ||
                    base.startsWith(`${stem}.`) ||
                    name.startsWith(`lib${stem}.`)) &&
                (name.endsWith(".dll") ||
                    name.endsWith(".so") ||
                    name.endsWith(".dylib") ||
                    name.endsWith(".node"))
            ) {
                artifact = path.join(deps, name);
                break;
            }
        }
    }
}

if (!artifact) {
    console.error(`找不到 ${stem} 产物于 ${targetDir}`);
    process.exit(1);
}

const plat = platformInfo();
const outDir = path.join(
    root,
    "projects",
    "platforms",
    "native",
    `rgss-${plat.short}`,
);
mkdirSync(outDir, { recursive: true });

const binaryName = `rgss.${plat.triple}.node`;
const pkgJsonPath = path.join(outDir, "package.json");
writeFileSync(
    pkgJsonPath,
    `${JSON.stringify(
        {
            name: `@game-gpt/rgss-${plat.short}`,
            version: "0.0.0",
            private: true,
            description: `RGSS native N-API addon (${plat.short})`,
            license: "Apache-2.0",
            os: plat.os,
            cpu: plat.cpu,
            main: binaryName,
            files: [binaryName, "README.md"],
            preferUnplugged: true,
        },
        null,
        4,
    )}\n`,
);

const readmePath = path.join(outDir, "README.md");
if (!existsSync(readmePath)) {
    writeFileSync(
        readmePath,
        `# \`@game-gpt/rgss-${plat.short}\`\n\n` +
            `原生 N-API 平台包。\`main\` 为 \`${binaryName}\`（由 \`node scripts/build/napi.mjs\` 写入）。\n` +
            `元包 \`@game-gpt/rgss\` 经 optionalDependencies 加载。\n`,
    );
}

const dest = path.join(outDir, binaryName);
copyFileSync(artifact, dest);
console.log(`napi: ${artifact} → ${dest}`);
