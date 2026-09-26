# OpenHarmony filesystem backend

OHOS is mobile in official Tauri, but is neither Android nor iOS. Enable the
existing standard filesystem backend explicitly for `target_env = "ohos"`:
its module, `Fs` export and managed state must all be present.

The JS `resolve_file` entry point also uses the standard checked resolver on
OHOS. In particular, **file URLs must not take the Android/iOS native-URI
shortcut**, which calls the trusted Rust `Fs::open` API directly. Both ordinary
paths and file URLs go through `resolve_path` and its existing command/global
allow/deny scopes, dynamic filesystem scope and path validation. No capability
files or permission defaults are widened.

The backend supports filesystem operations allowed by the OS app sandbox and
Tauri scopes. It does not add Android content URI handling, a document picker,
or native OHOS document-provider authorization. Rust Fs APIs remain trusted APIs,
as upstream. Tauri itself and the Reader repository do not need modifications.

Patch `tauri-plugin-fs` at the host application's Cargo root to unify transitive
Reader dependencies with the host plugin. In Moke this lives alongside the
shell/opener patches in `src-tauri/Cargo.toml`.

Run `node --test plugins/fs/ohos-routing.test.mjs` for routing regression checks.
These source guards protect backend selection and scope routing; they do not
replace a target build or device filesystem/permission tests.
