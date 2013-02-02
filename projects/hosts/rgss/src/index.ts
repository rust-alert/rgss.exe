import { createRequire } from "node:module";

import {
    detectNativePlatformPackage,
    listPlatformPackages,
    resolveNativePath,
    WASM_PLATFORM_PACKAGE,
    type RgssPlatformPackage,
} from "./platform";

export {
    detectNativePlatformPackage,
    listPlatformPackages,
    nativePackageName,
    platformShort,
    platformTriple,
    resolveNativePath,
    WASM_PLATFORM_PACKAGE,
    type PlatformInfo,
    type RgssNativeShort,
    type RgssPlatformPackage,
} from "./platform";

const nodeRequire = createRequire(__filename);

/** 与 `rgss-napi`（`JsRgssHost`）对齐。 */
export interface RgssHostBindings {
    info(): { name: string; version: string; npmPackage: string };
    detect(path: string): DetectBindings;
}

export interface DetectBindings {
    engine: string;
    script: string;
    library: string;
    scriptsPath: string;
    title: string;
    summary: string;
}

interface RgssAddon {
    JsRgssHost: new () => {
        info(): { name: string; version: string; npm_package: string };
        detect(path: string): {
            engine: string;
            script: string;
            library: string;
            scripts_path: string;
            title: string;
            summary: string;
        };
    };
}

export interface LoadOptions {
    platformPackage?: RgssPlatformPackage;
}

let cached: RgssHostBindings | undefined;

/**
 * 加载当前平台原生绑定。
 * 游戏根识别走 `host.detect(gameRoot)`。
 */
export function loadRgss(options: LoadOptions = {}): RgssHostBindings {
    if (cached) return cached;
    if (options.platformPackage === WASM_PLATFORM_PACKAGE) {
        throw new Error(
            "`loadRgss` 只加载原生 `.node`。Wasm 请使用 `@game-gpt/rgss-unknown-wasm32`。",
        );
    }
    const addonPath = resolveNativePath();
    const addon = nodeRequire(addonPath) as RgssAddon;
    if (typeof addon.JsRgssHost !== "function") {
        throw new Error(
            `原生插件缺少 JsRgssHost：${addonPath}。请运行 node scripts/build/napi.mjs`,
        );
    }
    const host = new addon.JsRgssHost();
    cached = {
        info: () => {
            const info = host.info();
            return {
                name: info.name,
                version: info.version,
                npmPackage: info.npm_package,
            };
        },
        detect: (gameRoot) => {
            const report = host.detect(gameRoot);
            return {
                engine: report.engine,
                script: report.script,
                library: report.library,
                scriptsPath: report.scripts_path,
                title: report.title,
                summary: report.summary,
            };
        },
    };
    return cached;
}

export function currentNativePackage() {
    return detectNativePlatformPackage();
}
