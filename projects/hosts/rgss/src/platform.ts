import { existsSync } from "node:fs";
import { createRequire } from "node:module";
import path from "node:path";

const nodeRequire = createRequire(__filename);

export type RgssNativeShort =
    | "win32-x64"
    | "win32-arm64"
    | "darwin-x64"
    | "darwin-arm64"
    | "linux-x64"
    | "linux-arm64";

export type RgssPlatformPackage =
    | `@game-gpt/rgss-${RgssNativeShort}`
    | "@game-gpt/rgss-unknown-wasm32";

export const WASM_PLATFORM_PACKAGE =
    "@game-gpt/rgss-unknown-wasm32" as const;

export function nativePackageName(
    short: RgssNativeShort,
): `@game-gpt/rgss-${RgssNativeShort}` {
    return `@game-gpt/rgss-${short}`;
}

export interface PlatformInfo {
    packageName: RgssPlatformPackage;
    triple: string;
    rustTarget: string;
    kind: "native" | "wasm";
}

const TABLE: PlatformInfo[] = [
    {
        packageName: "@game-gpt/rgss-win32-x64",
        triple: "win32-x64-msvc",
        rustTarget: "x86_64-pc-windows-msvc",
        kind: "native",
    },
    {
        packageName: "@game-gpt/rgss-win32-arm64",
        triple: "win32-arm64-msvc",
        rustTarget: "aarch64-pc-windows-msvc",
        kind: "native",
    },
    {
        packageName: "@game-gpt/rgss-darwin-x64",
        triple: "darwin-x64",
        rustTarget: "x86_64-apple-darwin",
        kind: "native",
    },
    {
        packageName: "@game-gpt/rgss-darwin-arm64",
        triple: "darwin-arm64",
        rustTarget: "aarch64-apple-darwin",
        kind: "native",
    },
    {
        packageName: "@game-gpt/rgss-linux-x64",
        triple: "linux-x64-gnu",
        rustTarget: "x86_64-unknown-linux-gnu",
        kind: "native",
    },
    {
        packageName: "@game-gpt/rgss-linux-arm64",
        triple: "linux-arm64-gnu",
        rustTarget: "aarch64-unknown-linux-gnu",
        kind: "native",
    },
    {
        packageName: WASM_PLATFORM_PACKAGE,
        triple: "wasm32-unknown-unknown",
        rustTarget: "wasm32-unknown-unknown",
        kind: "wasm",
    },
];

export function listPlatformPackages(): readonly PlatformInfo[] {
    return TABLE;
}

export function platformTriple(
    platform: NodeJS.Platform = process.platform,
    arch: NodeJS.Architecture = process.arch,
): string {
    if (platform === "win32" && arch === "x64") return "win32-x64-msvc";
    if (platform === "win32" && arch === "arm64") return "win32-arm64-msvc";
    if (platform === "darwin" && arch === "x64") return "darwin-x64";
    if (platform === "darwin" && arch === "arm64") return "darwin-arm64";
    if (platform === "linux" && arch === "x64") return "linux-x64-gnu";
    if (platform === "linux" && arch === "arm64") return "linux-arm64-gnu";
    return `${platform}-${arch}`;
}

export function platformShort(
    triple: string = platformTriple(),
): RgssNativeShort {
    if (triple === "win32-x64-msvc") return "win32-x64";
    if (triple === "win32-arm64-msvc") return "win32-arm64";
    if (triple === "linux-x64-gnu") return "linux-x64";
    if (triple === "linux-arm64-gnu") return "linux-arm64";
    if (triple === "darwin-x64" || triple === "darwin-arm64") {
        return triple as RgssNativeShort;
    }
    throw new Error(
        `不受支持的 Node 平台 triple：${triple}。可改用 ${WASM_PLATFORM_PACKAGE}。`,
    );
}

export function detectNativePlatformPackage(
    platform: NodeJS.Platform = process.platform,
    arch: NodeJS.Architecture = process.arch,
): `@game-gpt/rgss-${RgssNativeShort}` {
    return nativePackageName(platformShort(platformTriple(platform, arch)));
}

export function resolveNativePath(): string {
    const envPath =
        (typeof process.env.RGSS_NATIVE_NODE === "string" &&
            process.env.RGSS_NATIVE_NODE.trim()) ||
        "";
    if (envPath) {
        const abs = path.resolve(envPath);
        if (!existsSync(abs)) {
            throw new Error(`RGSS_NATIVE_NODE 指向的文件不存在：${abs}`);
        }
        return abs;
    }

    const triple = platformTriple();
    const short = platformShort(triple);
    const name = nativePackageName(short);
    const folder = `rgss-${short}`;
    const binary = `rgss.${triple}.node`;
    const candidates: string[] = [];

    try {
        const resolved = nodeRequire.resolve(`${name}/package.json`);
        const dir = path.dirname(resolved);
        candidates.push(path.join(dir, binary), path.join(dir, "rgss.node"));
    } catch {
        /* optional dep 未安装 */
    }

    candidates.push(
        path.join(
            __dirname,
            "..",
            "node_modules",
            "@game-gpt",
            `rgss-${short}`,
            binary,
        ),
        path.join(
            __dirname,
            "..",
            "node_modules",
            "@game-gpt",
            `rgss-${short}`,
            "rgss.node",
        ),
    );

    candidates.push(
        path.join(
            __dirname,
            "..",
            "..",
            "..",
            "platforms",
            "native",
            folder,
            binary,
        ),
        path.join(
            __dirname,
            "..",
            "..",
            "..",
            "platforms",
            "native",
            folder,
            "rgss.node",
        ),
    );

    for (const p of candidates) {
        if (existsSync(p)) return p;
    }

    throw new Error(
        `找不到原生插件 ${name}（${binary}）。` +
            `请运行：node scripts/build/napi.mjs\n` +
            `已查找：\n${candidates.map((c) => `  - ${c}`).join("\n")}`,
    );
}
