// Copyright 2019-2026 Tauri Programme within The Commons Conservancy
// SPDX-License-Identifier: Apache-2.0
// SPDX-License-Identifier: MIT

//! OHOS UIAbility adapter. The host installs a thread-safe dispatcher from ArkTS
//! during onCreate and clears it during onDestroy. Never use Tauri's currently
//! unimplemented OHOS run_mobile_plugin transport, or Linux xdg-open/DBus.
//!
//! Synchronous Rust open calls must run off the ArkTS registration thread.
//! Success means UIAbility accepted the request, not that a viewer rendered it.

use std::{
    collections::HashMap,
    io,
    path::Path,
    sync::{mpsc, Arc, Mutex, OnceLock},
    thread::{self, ThreadId},
    time::Duration,
};

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum OpenRequest {
    Url(String),
    Path(String),
}

type Dispatcher = Arc<dyn Fn(u32, OpenRequest) -> io::Result<()> + Send + Sync>;
type Reply = mpsc::SyncSender<io::Result<()>>;

#[derive(Default)]
struct State {
    dispatcher: Option<(ThreadId, Dispatcher)>,
    next_id: u32,
    pending: HashMap<u32, Reply>,
}

#[derive(Default)]
struct Bridge(Mutex<State>);

impl Bridge {
    fn install(&self, dispatcher: Dispatcher) {
        self.clear();
        self.0.lock().unwrap().dispatcher = Some((thread::current().id(), dispatcher));
    }

    fn clear(&self) {
        let mut state = self.0.lock().unwrap();
        state.dispatcher = None;
        for (_, reply) in state.pending.drain() {
            let _ = reply.try_send(Err(io::Error::new(
                io::ErrorKind::NotConnected,
                "OHOS UIAbility was destroyed or replaced",
            )));
        }
    }

    fn complete(&self, id: u32, result: io::Result<()>) {
        if let Some(reply) = self.0.lock().unwrap().pending.remove(&id) {
            let _ = reply.try_send(result);
        }
    }

    fn invoke(&self, request: OpenRequest, timeout: Duration) -> io::Result<()> {
        let (reply, receive) = mpsc::sync_channel(1);
        let (id, dispatch) = {
            let mut state = self.0.lock().unwrap();
            let (ui_thread, dispatch) = state.dispatcher.as_ref().ok_or_else(|| {
                io::Error::new(
                    io::ErrorKind::NotConnected,
                    "OHOS opener UIAbility adapter not registered",
                )
            })?;
            if *ui_thread == thread::current().id() {
                return Err(io::Error::new(
                    io::ErrorKind::WouldBlock,
                    "OHOS opener cannot wait on the ArkTS UI thread",
                ));
            }
            let dispatch = dispatch.clone();
            if state.pending.len() >= 64 {
                return Err(io::Error::new(
                    io::ErrorKind::WouldBlock,
                    "OHOS opener request limit reached",
                ));
            }
            // Never reuse an ID: a late ArkTS reply must not complete a new request.
            state.next_id = state
                .next_id
                .checked_add(1)
                .ok_or_else(|| io::Error::other("OHOS opener request IDs exhausted"))?;
            let id = state.next_id;
            state.pending.insert(id, reply);
            (id, dispatch)
        };
        let result = dispatch(id, request).and_then(|()| {
            receive.recv_timeout(timeout).map_err(|e| match e {
                mpsc::RecvTimeoutError::Timeout => {
                    io::Error::new(io::ErrorKind::TimedOut, "OHOS opener response timed out")
                }
                mpsc::RecvTimeoutError::Disconnected => io::Error::new(
                    io::ErrorKind::NotConnected,
                    "OHOS opener response disconnected",
                ),
            })?
        });
        self.0.lock().unwrap().pending.remove(&id);
        result
    }
}

fn bridge() -> &'static Bridge {
    static BRIDGE: OnceLock<Bridge> = OnceLock::new();
    BRIDGE.get_or_init(Bridge::default)
}

/// Install from the UI thread. Dispatcher must enqueue without blocking and
/// call `complete` after UIAbility resolves/rejects, not merely after enqueueing.
pub fn register(dispatcher: impl Fn(u32, OpenRequest) -> io::Result<()> + Send + Sync + 'static) {
    bridge().install(Arc::new(dispatcher));
}

/// Release the UIAbility reference and fail in-flight calls during onDestroy.
pub fn unregister() {
    bridge().clear();
}

/// Complete an accepted request. Unknown/expired IDs are safely ignored.
pub fn complete(id: u32, error: Option<String>) {
    bridge().complete(id, error.map_or(Ok(()), |e| Err(io::Error::other(e))));
}

/// ArkTS should skip queued requests that expired before it could handle them.
pub fn is_pending(id: u32) -> bool {
    bridge().0.lock().unwrap().pending.contains_key(&id)
}

fn default_app_only(with: Option<&str>) -> io::Result<()> {
    if with.is_some() {
        return Err(io::Error::new(
            io::ErrorKind::Unsupported,
            "OHOS named applications and inAppBrowser are not supported",
        ));
    }
    Ok(())
}

fn url_request(url: &str, with: Option<&str>) -> io::Result<OpenRequest> {
    default_app_only(with)?;
    // This adapter deliberately supports the plugin's default safe URL schemes.
    // File access must go through open_path and its separate path ACL.
    let scheme = url.split_once(':').map(|(s, _)| s.to_ascii_lowercase());
    if !matches!(scheme.as_deref(), Some("http" | "https" | "mailto" | "tel"))
        || url.chars().any(char::is_control)
    {
        return Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            "OHOS opener supports only http, https, mailto and tel URLs",
        ));
    }
    Ok(OpenRequest::Url(url.to_owned()))
}

fn path_request(path: &Path, with: Option<&str>) -> io::Result<OpenRequest> {
    default_app_only(with)?;
    let path = path.canonicalize()?;
    if !path.is_file() {
        return Err(io::Error::new(
            io::ErrorKind::Unsupported,
            "OHOS opener supports files, not directories",
        ));
    }
    let path = path
        .to_str()
        .ok_or_else(|| io::Error::new(io::ErrorKind::InvalidInput, "OHOS path must be UTF-8"))?;
    Ok(OpenRequest::Path(path.to_owned()))
}

pub fn open_url(url: &str, with: Option<&str>) -> io::Result<()> {
    bridge().invoke(url_request(url, with)?, Duration::from_secs(10))
}

pub fn open_path(path: &Path, with: Option<&str>) -> io::Result<()> {
    bridge().invoke(path_request(path, with)?, Duration::from_secs(10))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn validates_request_without_dispatch() {
        for url in [
            "javascript:alert(1)",
            "data:text/html,test",
            "file:///private",
            "https://a\n",
        ] {
            assert!(url_request(url, None).is_err());
        }
        for url in [
            "https://example.com",
            "http://localhost:3000",
            "mailto:a@b.com",
            "tel:+123",
        ] {
            assert!(url_request(url, None).is_ok());
        }
        assert!(url_request("https://example.com", Some("inAppBrowser")).is_err());
        assert!(path_request(Path::new("/"), None).is_err());
        assert!(path_request(Path::new("/nonexistent-ohos-opener-test"), None).is_err());
        assert!(path_request(&std::env::current_exe().unwrap(), None).is_ok());
    }

    #[test]
    fn missing_adapter_and_ui_thread_fail_immediately() {
        let bridge = Bridge::default();
        let request = OpenRequest::Url("https://example.com".into());
        assert_eq!(
            bridge
                .invoke(request.clone(), Duration::ZERO)
                .unwrap_err()
                .kind(),
            io::ErrorKind::NotConnected
        );
        bridge.install(Arc::new(|_, _| panic!("UI thread must not dispatch")));
        assert_eq!(
            bridge.invoke(request, Duration::ZERO).unwrap_err().kind(),
            io::ErrorKind::WouldBlock
        );
    }

    #[test]
    fn returns_native_success_and_errors_and_reclaims_requests() {
        for error in [false, true] {
            let bridge = Arc::new(Bridge::default());
            let weak = Arc::downgrade(&bridge);
            bridge.install(Arc::new(move |id, _| {
                weak.upgrade().unwrap().complete(
                    id,
                    if error {
                        Err(io::Error::other("no matching Ability"))
                    } else {
                        Ok(())
                    },
                );
                Ok(())
            }));
            let worker = bridge.clone();
            let result = thread::spawn(move || {
                worker.invoke(
                    OpenRequest::Path("/test.pdf".into()),
                    Duration::from_secs(1),
                )
            })
            .join()
            .unwrap();
            assert_eq!(result.is_err(), error);
            assert!(bridge.0.lock().unwrap().pending.is_empty());
        }
    }

    #[test]
    fn timeout_dispatch_failure_and_destruction_clean_up() {
        for mode in 0..3 {
            let bridge = Arc::new(Bridge::default());
            let weak = Arc::downgrade(&bridge);
            bridge.install(Arc::new(move |_, _| match mode {
                0 => Ok(()),
                1 => Err(io::Error::other("queue closed")),
                _ => {
                    weak.upgrade().unwrap().clear();
                    Ok(())
                }
            }));
            let worker = bridge.clone();
            let result = thread::spawn(move || {
                worker.invoke(
                    OpenRequest::Url("https://example.com".into()),
                    Duration::ZERO,
                )
            })
            .join()
            .unwrap();
            assert!(result.is_err());
            assert!(bridge.0.lock().unwrap().pending.is_empty());
            bridge.complete(1, Ok(())); // Late response is harmless.
        }
    }
}
