/**
 * `rgss-unknown-wasm32`：Wasm 目标的 TypeScript 加载器。
 * 桌面检测请使用 `@game-gpt/rgss` 的 `rgss detect --path`。
 */

export const rustTarget = "wasm32-unknown-unknown" as const;
export const platformPackage = "rgss-unknown-wasm32" as const;

export interface WasmInfo {
    name: string;
    version: string;
    npm_package: string;
    versionCode: number;
}

export interface RgssWasmBindings {
    info(): WasmInfo;
    vec2Length(x: number, y: number): number;
}

type WasmExports = {
    rgss_vec2_length: (x: number, y: number) => number;
    rgss_version_code: () => number;
};

function decodeVersion(code: number): string {
    const major = Math.floor(code / 1_000_000);
    const minor = Math.floor((code % 1_000_000) / 1_000);
    const patch = code % 1_000;
    return `${major}.${minor}.${patch}`;
}

export async function loadRgssWasm(): Promise<RgssWasmBindings> {
    const url = new URL("../rgss_bg.wasm", import.meta.url);
    let exports: Partial<WasmExports> | undefined;
    try {
        if (typeof fetch === "function") {
            const resp = await fetch(url);
            if (resp.ok) {
                const buf = await resp.arrayBuffer();
                const { instance } = await WebAssembly.instantiate(buf, {});
                exports = instance.exports as unknown as WasmExports;
            }
        }
    } catch {
        /* 回退纯 JS */
    }

    if (exports?.rgss_vec2_length && exports.rgss_version_code) {
        const code = exports.rgss_version_code();
        return {
            info: () => ({
                name: "RGSS",
                version: decodeVersion(code),
                npm_package: platformPackage,
                versionCode: code,
            }),
            vec2Length: (x, y) => exports!.rgss_vec2_length!(x, y),
        };
    }

    return {
        info: () => ({
            name: "RGSS",
            version: "0.0.0",
            npm_package: platformPackage,
            versionCode: 0,
        }),
        vec2Length: (x, y) => Math.hypot(x, y),
    };
}
