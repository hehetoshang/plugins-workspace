// Copyright 2019-2023 Tauri Programme within The Commons Conservancy
// SPDX-License-Identifier: Apache-2.0
// SPDX-License-Identifier: MIT

//! Types and functions related to shell.

#[cfg(not(target_env = "ohos"))]
use std::ffi::OsStr;
use std::path::Path;

#[cfg(not(target_env = "ohos"))]
pub(crate) fn open<P: AsRef<OsStr>, S: AsRef<str>>(path: P, with: Option<S>) -> crate::Result<()> {
    match with {
        Some(program) => ::open::with_detached(path, program.as_ref()),
        None => ::open::that_detached(path),
    }
    .map_err(Into::into)
}

/// Opens URL with the program specified in `with`, or system default if `None`.
///
/// ## Platform-specific:
///
/// - **Android / iOS**: Always opens using default program.
///
/// # Examples
///
/// ```rust,no_run
/// tauri::Builder::default()
///   .setup(|app| {
///     // open the given URL on the system default browser
///     tauri_plugin_opener::open_url("https://github.com/tauri-apps/tauri", None::<&str>)?;
///     Ok(())
///   });
/// ```
pub fn open_url<P: AsRef<str>, S: AsRef<str>>(url: P, with: Option<S>) -> crate::Result<()> {
    let url = url.as_ref();
    #[cfg(target_env = "ohos")]
    return crate::ohos::open_url(url, with.as_ref().map(AsRef::as_ref)).map_err(Into::into);
    #[cfg(not(target_env = "ohos"))]
    open(url, with)
}

/// Opens path with the program specified in `with`, or system default if `None`.
///
/// ## Platform-specific:
///
/// - **Android / iOS**: Always opens using default program.
///
/// # Examples
///
/// ```rust,no_run
/// tauri::Builder::default()
///   .setup(|app| {
///     // open the given URL on the system default explorer
///     tauri_plugin_opener::open_path("/path/to/file", None::<&str>)?;
///     Ok(())
///   });
/// ```
pub fn open_path<P: AsRef<Path>, S: AsRef<str>>(path: P, with: Option<S>) -> crate::Result<()> {
    let path = path.as_ref();
    if with.is_none() {
        // Returns an IO error if not exists, and besides `exists()` is a shorthand for `metadata()`
        _ = path.metadata()?;
    }
    #[cfg(target_env = "ohos")]
    return crate::ohos::open_path(path, with.as_ref().map(AsRef::as_ref)).map_err(Into::into);
    #[cfg(not(target_env = "ohos"))]
    open(path, with)
}
