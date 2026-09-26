// Copyright 2019-2023 Tauri Programme within The Commons Conservancy
// SPDX-License-Identifier: Apache-2.0
// SPDX-License-Identifier: MIT

use std::path::{Path, PathBuf};

use tauri::{
    ipc::{CommandScope, GlobalScope},
    AppHandle, Runtime,
};

use crate::{scope::Scope, Error, OpenerExt};

#[tauri::command]
pub async fn open_url<R: Runtime>(
    app: AppHandle<R>,
    command_scope: CommandScope<crate::scope::Entry>,
    global_scope: GlobalScope<crate::scope::Entry>,
    url: String,
    with: Option<String>,
) -> crate::Result<()> {
    let scope = Scope::new(
        &app,
        command_scope
            .allows()
            .iter()
            .chain(global_scope.allows())
            .collect(),
        command_scope
            .denies()
            .iter()
            .chain(global_scope.denies())
            .collect(),
    );

    if scope.is_url_allowed(&url, with.as_deref()) {
        #[cfg(target_env = "ohos")]
        return tauri::async_runtime::spawn_blocking(move || app.opener().open_url(url, with))
            .await
            .map_err(Error::from)?;
        #[cfg(not(target_env = "ohos"))]
        app.opener().open_url(url, with)
    } else {
        Err(Error::ForbiddenUrl { url, with })
    }
}

#[tauri::command]
pub async fn open_path<R: Runtime>(
    app: AppHandle<R>,
    command_scope: CommandScope<crate::scope::Entry>,
    global_scope: GlobalScope<crate::scope::Entry>,
    path: String,
    with: Option<String>,
) -> crate::Result<()> {
    let scope = Scope::new(
        &app,
        command_scope
            .allows()
            .iter()
            .chain(global_scope.allows())
            .collect(),
        command_scope
            .denies()
            .iter()
            .chain(global_scope.denies())
            .collect(),
    );

    if scope.is_path_allowed(Path::new(&path), with.as_deref())? {
        #[cfg(target_env = "ohos")]
        return tauri::async_runtime::spawn_blocking(move || app.opener().open_path(path, with))
            .await
            .map_err(Error::from)?;
        #[cfg(not(target_env = "ohos"))]
        app.opener().open_path(path, with)
    } else {
        Err(Error::ForbiddenPath { path, with })
    }
}

/// TODO: in the next major version, rename to `reveal_items_in_dir`
#[tauri::command]
pub async fn reveal_item_in_dir(paths: Vec<PathBuf>) -> crate::Result<()> {
    crate::reveal_items_in_dir(&paths)
}
