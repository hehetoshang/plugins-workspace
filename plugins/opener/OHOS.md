# OpenHarmony adapter

This adapter changes shell/opener; the fork also supplies the OHOS standard
filesystem backend documented in `../fs/OHOS.md`. Use the official Tauri
`feat/open-harmony` branch. Patch **all three crates from this same revision** in the application's root
Cargo manifest, including when other libraries also depend on them.

Upstream Tauri's OHOS `run_mobile_plugin` transport is not implemented. A host
UIAbility adapter is therefore required; changing only the platform cfg is not
sufficient. No Tauri/Wry patch is needed by this adapter.

## Host integration

On the ArkTS UI thread, install `tauri_plugin_opener::ohos::register(dispatcher)`
via an app-owned NAPI export. The dispatcher receives `(u32, OpenRequest)` and
must enqueue work on that same UI thread using a NAPI ThreadsafeFunction. It must
return a queueing error if enqueueing fails. Do not block the dispatcher.

The ArkTS callback should check `ohos::is_pending(id)` before dispatch and use:

- URL `http`/`https`: `UIAbilityContext.openLink`.
- URL `mailto`/`tel`: `UIAbilityContext.startAbility` with the appropriate Want.
- File: `fileUri.getUriFromPath(path)` and `startAbility` with
  `ohos.want.action.viewData`, its MIME type and **read-only** URI permission.

Only after the native promise settles, call `ohos::complete(id, None)` on success
or `ohos::complete(id, Some(error))` on failure. An absent viewer/permission error
is a failure, not success. `ohos::unregister()` must run in `onDestroy` to release
the callback and fail pending requests. These are host-native exports, never
WebView IPC commands, and must not be exposed to publication HTML.

Moke's `src-tauri/src/ohos_opener.rs` and `scripts/ohos-opener/` provide the host
implementation. The application retains ownership of the UIAbility lifecycle.

## Support boundary

- JS calls still pass Tauri ACL, opener URL/path allow/deny scopes or shell's
  configured open regex before dispatch. Rust calls remain trusted APIs.
- Supports only `http`, `https`, `mailto`, `tel` URLs and existing UTF-8 files.
  `file:` URLs cannot bypass the file-path ACL. The OS controls file sharing.
- Named desktop programs, `inAppBrowser`, directories, and reveal-in-file-manager
  return errors; they are not silently routed to Linux xdg-open/DBus.
- Shell process APIs retain their existing permission checks and POSIX backend.
  The app sandbox still limits available executables; this is not privileged
  access to OHOS system tools and does not guarantee bundled sidecar support.
- Rust open APIs are synchronous: call them off the ArkTS UI thread (enforced).
  JS commands use the blocking worker pool. Calls time out after 10 seconds and
  at most 64 may be pending. A timeout cannot undo an OS request already started.
- Registration errors, native rejection, shutdown and timeouts propagate to the
  caller. Success means the OS accepted opening, not that a viewer rendered it.

## Fast transport tests

The transport/validation module uses only std; it can be tested on the host
without building a desktop WebView or cross-compiling the entire application:

```sh
rustc --edition 2021 --test plugins/opener/src/ohos.rs -o /tmp/ohos-opener-tests
/tmp/ohos-opener-tests
```

These unit tests do not replace an OHOS target check or a device test.
