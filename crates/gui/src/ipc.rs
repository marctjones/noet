//! Single-instance IPC over a Unix socket (Linux + macOS). A second launch — e.g. a
//! GNOME custom keyboard shortcut bound to `noet --new-meeting` — forwards its action
//! to the already-running instance instead of opening a second window. This is the
//! clean Wayland/GNOME path: the desktop owns the keybinding, Noet just exposes the
//! action over the socket (no global key grab, no tray). No-op on Windows, where the
//! tray + global hotkey cover the same ground.

#[cfg(unix)]
mod imp {
    use std::io::{Read, Write};
    use std::os::unix::net::{UnixListener, UnixStream};
    use std::path::{Path, PathBuf};

    /// `$XDG_RUNTIME_DIR/noet.sock` (per-user), falling back to a uid-namespaced path
    /// in the temp dir if the runtime dir isn't set.
    fn sock_path() -> PathBuf {
        if let Some(rt) = std::env::var_os("XDG_RUNTIME_DIR") {
            return PathBuf::from(rt).join("noet.sock");
        }
        // No runtime dir → uid-namespace the temp path so users don't collide.
        // SAFETY: getuid() always succeeds and is thread-safe.
        let uid = unsafe { getuid() };
        std::env::temp_dir().join(format!("noet-{uid}.sock"))
    }

    extern "C" {
        fn getuid() -> u32;
    }

    /// Removes the socket file when the primary instance exits.
    pub struct Server {
        path: PathBuf,
    }
    impl Drop for Server {
        fn drop(&mut self) {
            let _ = std::fs::remove_file(&self.path);
        }
    }

    fn forward_at(path: &Path, cmd: &str) -> bool {
        match UnixStream::connect(path) {
            Ok(mut s) => s.write_all(cmd.as_bytes()).is_ok(),
            Err(_) => false,
        }
    }

    fn serve_at(path: PathBuf, on_cmd: impl Fn(String) + Send + 'static) -> Option<Server> {
        // A stale socket from a previous crash refuses connections; clear it so bind works.
        let _ = std::fs::remove_file(&path);
        let listener = UnixListener::bind(&path).ok()?;
        std::thread::spawn(move || {
            for stream in listener.incoming() {
                let Ok(mut s) = stream else { continue };
                let mut buf = String::new();
                if s.read_to_string(&mut buf).is_ok() {
                    let cmd = buf.trim().to_string();
                    if !cmd.is_empty() {
                        on_cmd(cmd);
                    }
                }
            }
        });
        Some(Server { path })
    }

    /// If another instance is listening, send `cmd` and return true (caller should
    /// then exit). False if we're the primary (nothing listening).
    pub fn forward_if_running(cmd: &str) -> bool {
        forward_at(&sock_path(), cmd)
    }

    /// Become the primary instance: bind the socket and call `on_cmd` (on a worker
    /// thread) for each forwarded command. Keep the returned guard alive. Best-effort
    /// — `None` if the socket can't be bound (then Noet just runs without IPC).
    pub fn serve(on_cmd: impl Fn(String) + Send + 'static) -> Option<Server> {
        serve_at(sock_path(), on_cmd)
    }

    #[cfg(test)]
    mod tests {
        use super::*;
        use std::sync::mpsc;
        use std::time::Duration;

        #[test]
        fn forwards_command_to_the_primary() {
            let path =
                std::env::temp_dir().join(format!("noet-ipc-test-{}.sock", std::process::id()));
            let (tx, rx) = mpsc::channel();
            let server = serve_at(path.clone(), move |cmd| {
                let _ = tx.send(cmd);
            })
            .expect("primary binds the socket");

            // A "second instance" forwards its action; the primary receives it.
            assert!(forward_at(&path, "new-meeting"), "forward succeeds to a live instance");
            let got = rx.recv_timeout(Duration::from_secs(2)).expect("command delivered");
            assert_eq!(got, "new-meeting");

            // With nothing listening, forward reports false (caller becomes primary).
            let dead = std::env::temp_dir().join(format!("noet-ipc-none-{}.sock", std::process::id()));
            let _ = std::fs::remove_file(&dead);
            assert!(!forward_at(&dead, "x"), "forward fails when no instance is running");

            drop(server);
            assert!(!path.exists(), "socket file removed on drop");
        }
    }
}

#[cfg(not(unix))]
mod imp {
    pub struct Server;
    pub fn forward_if_running(_cmd: &str) -> bool {
        false
    }
    pub fn serve(_on_cmd: impl Fn(String) + Send + 'static) -> Option<Server> {
        None
    }
}

pub use imp::{forward_if_running, serve, Server};
