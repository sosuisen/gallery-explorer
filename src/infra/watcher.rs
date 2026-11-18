//! Watches one folder and reports when its items change.

use std::path::Path;
use std::sync::mpsc::{self, Receiver, RecvTimeoutError};
use std::sync::{Arc, Mutex};
use std::time::Duration;

use notify::{EventKind, RecommendedWatcher, RecursiveMode, Watcher};

/// A copy writes one file in many steps. The change is reported after this much quiet time, so
/// the folder is read once, after the steps end.
const QUIET: Duration = Duration::from_millis(200);
/// How often a waiting thread checks whether it was cancelled.
const POLL: Duration = Duration::from_millis(100);

/// Watches the items directly in one folder. Dropping it stops the watch.
pub struct FolderWatcher {
    _watcher: RecommendedWatcher,
    changes: Changes,
}

/// The changes of a watched folder. Another thread can wait for them.
#[derive(Clone)]
pub struct Changes(Arc<Mutex<Receiver<()>>>);

impl FolderWatcher {
    pub fn start(folder: &Path) -> notify::Result<Self> {
        let (sender, receiver) = mpsc::channel();
        let mut watcher =
            notify::recommended_watcher(move |event: notify::Result<notify::Event>| {
                // Reading a file is not a change of the folder.
                if event.is_ok_and(|event| !matches!(event.kind, EventKind::Access(_))) {
                    // The receiver is gone only when nobody waits any more.
                    _ = sender.send(());
                }
            })?;
        watcher.watch(folder, RecursiveMode::NonRecursive)?;
        Ok(Self {
            _watcher: watcher,
            changes: Changes(Arc::new(Mutex::new(receiver))),
        })
    }

    pub fn changes(&self) -> Changes {
        self.changes.clone()
    }
}

impl Changes {
    /// Blocks until the folder changes and then stays quiet for a short time.
    ///
    /// Returns `false` without a change when `cancelled` returns `true`, or when the watcher
    /// is dropped.
    pub fn wait(&self, cancelled: impl Fn() -> bool) -> bool {
        // A panic in a waiting task stops the whole app, so the lock is never poisoned.
        let receiver = self.0.lock().expect("no waiting thread panicked");
        loop {
            match receiver.recv_timeout(POLL) {
                Ok(()) => break,
                Err(RecvTimeoutError::Timeout) if cancelled() => return false,
                Err(RecvTimeoutError::Timeout) => {}
                Err(RecvTimeoutError::Disconnected) => return false,
            }
        }
        // A change was seen, so even a stopped watch reports it.
        while receiver.recv_timeout(QUIET).is_ok() {}
        true
    }
}

#[cfg(test)]
mod tests {
    use std::fs;
    use std::time::Instant;

    use super::*;

    /// Waits for a change, but gives up after a few seconds so that a broken watch fails the
    /// test instead of hanging it.
    fn wait_a_while(changes: &Changes) -> bool {
        let deadline = Instant::now() + Duration::from_secs(5);
        changes.wait(|| Instant::now() > deadline)
    }

    // フォルダにファイルが追加されると、変化が通知される。
    #[test]
    fn adding_a_file_is_reported() {
        let folder = tempfile::tempdir().unwrap();
        let watcher = FolderWatcher::start(folder.path()).unwrap();

        fs::write(folder.path().join("new.jpg"), "").unwrap();

        assert!(wait_a_while(&watcher.changes()));
    }

    // 待ち始めたあとの変化も通知される。
    #[test]
    fn a_change_after_the_wait_starts_is_reported() {
        let folder = tempfile::tempdir().unwrap();
        let watcher = FolderWatcher::start(folder.path()).unwrap();
        let changes = watcher.changes();
        let waiting = std::thread::spawn(move || wait_a_while(&changes));

        std::thread::sleep(Duration::from_millis(300));
        fs::write(folder.path().join("late.jpg"), "").unwrap();

        assert!(waiting.join().unwrap());
    }

    // 続けて起きた変化は、まとめて1回で通知される。
    #[test]
    fn changes_in_a_row_are_reported_once() {
        let folder = tempfile::tempdir().unwrap();
        let watcher = FolderWatcher::start(folder.path()).unwrap();
        for name in ["a.jpg", "b.jpg", "c.jpg"] {
            fs::write(folder.path().join(name), "data").unwrap();
        }

        assert!(wait_a_while(&watcher.changes()));
        assert!(!watcher.changes().wait(|| true));
    }

    // フォルダからファイルが削除されると、変化が通知される。
    #[test]
    fn removing_a_file_is_reported() {
        let folder = tempfile::tempdir().unwrap();
        fs::write(folder.path().join("old.jpg"), "").unwrap();
        let watcher = FolderWatcher::start(folder.path()).unwrap();

        fs::remove_file(folder.path().join("old.jpg")).unwrap();

        assert!(wait_a_while(&watcher.changes()));
    }

    // 変化がないまま取り消されると、待つのをやめる。
    #[test]
    fn waiting_ends_without_a_change_when_cancelled() {
        let folder = tempfile::tempdir().unwrap();
        let watcher = FolderWatcher::start(folder.path()).unwrap();

        assert!(!watcher.changes().wait(|| true));
    }

    // 監視をやめると、待つのをやめる。
    #[test]
    fn waiting_ends_when_the_watcher_is_dropped() {
        let folder = tempfile::tempdir().unwrap();
        let changes = FolderWatcher::start(folder.path()).unwrap().changes();

        assert!(!wait_a_while(&changes));
    }

    // 存在しないフォルダは監視できない。
    #[test]
    fn a_missing_folder_cannot_be_watched() {
        let folder = tempfile::tempdir().unwrap();

        assert!(FolderWatcher::start(&folder.path().join("missing")).is_err());
    }
}
