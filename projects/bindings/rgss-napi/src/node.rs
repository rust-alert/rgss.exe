//! napi 导出（feature = `node`）。

use napi::bindgen_prelude::*;
use napi_derive::napi;

use crate::host::RgssHost;

#[napi(object)]
pub struct JsHostInfo {
    pub name: String,
    pub version: String,
    pub npm_package: String,
}

#[napi(object)]
pub struct JsDetectReport {
    pub engine: String,
    pub script: String,
    pub library: String,
    pub scripts_path: String,
    pub title: String,
    pub summary: String,
}

#[napi]
pub struct JsRgssHost {
    inner: RgssHost,
}

#[napi]
impl JsRgssHost {
    #[napi(constructor)]
    pub fn new() -> Self {
        Self {
            inner: RgssHost::new(),
        }
    }

    #[napi]
    pub fn info(&self) -> JsHostInfo {
        let info = self.inner.info();
        JsHostInfo {
            name: info.name.into(),
            version: info.version.into(),
            npm_package: info.npm_package.into(),
        }
    }

    /// `rgss detect --path`。
    #[napi]
    pub fn detect(&self, path: String) -> Result<JsDetectReport> {
        let report = self.inner.detect(&path).map_err(Error::from_reason)?;
        Ok(JsDetectReport {
            engine: report.engine.label().into(),
            script: report.script.label().into(),
            library: report.library.unwrap_or_default(),
            scripts_path: report.scripts_path.unwrap_or_default(),
            title: report.title.unwrap_or_default(),
            summary: report.summary_line(),
        })
    }
}
