use std::fs::{self, File, OpenOptions};
use std::io;
use std::path::{Path, PathBuf};
use std::sync::{Condvar, LazyLock, Mutex};

const LOCK_FILE_NAME: &str = ".pkv-sync-storage.lock";
static LOCAL_LOCK: LazyLock<(Mutex<LocalLockState>, Condvar)> =
    LazyLock::new(|| (Mutex::new(LocalLockState::default()), Condvar::new()));

#[derive(Debug)]
pub struct StorageLock {
    _file: File,
    _local_guard: LocalGuard,
}

#[derive(Debug, Default)]
struct LocalLockState {
    readers: usize,
    writer: bool,
    waiting_writers: usize,
}

#[derive(Debug)]
struct LocalGuard {
    mode: LockMode,
}

pub fn acquire_storage_write_lock(data_dir: &Path) -> io::Result<StorageLock> {
    acquire_lock(data_dir, LockMode::Exclusive)
}

pub fn acquire_shared_storage_lock(data_dir: &Path) -> io::Result<StorageLock> {
    acquire_lock(data_dir, LockMode::Shared)
}

pub async fn acquire_shared_storage_lock_async(data_dir: PathBuf) -> io::Result<StorageLock> {
    tokio::task::spawn_blocking(move || acquire_shared_storage_lock(&data_dir))
        .await
        .map_err(|err| io::Error::other(format!("storage lock task failed: {err}")))?
}

#[derive(Debug, Clone, Copy)]
enum LockMode {
    Exclusive,
    Shared,
}

fn acquire_lock(data_dir: &Path, mode: LockMode) -> io::Result<StorageLock> {
    fs::create_dir_all(data_dir)?;
    let local_guard = acquire_local_lock(mode);
    let path = data_dir.join(LOCK_FILE_NAME);
    platform::open_locked(&path, mode).map(|file| StorageLock {
        _file: file,
        _local_guard: local_guard,
    })
}

fn acquire_local_lock(mode: LockMode) -> LocalGuard {
    let (state, ready) = &*LOCAL_LOCK;
    let mut state = state.lock().expect("storage lock mutex poisoned");
    match mode {
        LockMode::Shared => {
            while state.writer || state.waiting_writers > 0 {
                state = ready.wait(state).expect("storage lock mutex poisoned");
            }
            state.readers += 1;
        }
        LockMode::Exclusive => {
            state.waiting_writers += 1;
            while state.writer || state.readers > 0 {
                state = ready.wait(state).expect("storage lock mutex poisoned");
            }
            state.waiting_writers -= 1;
            state.writer = true;
        }
    }
    LocalGuard { mode }
}

impl Drop for LocalGuard {
    fn drop(&mut self) {
        let (state, ready) = &*LOCAL_LOCK;
        let mut state = state.lock().expect("storage lock mutex poisoned");
        match self.mode {
            LockMode::Shared => {
                state.readers = state.readers.saturating_sub(1);
            }
            LockMode::Exclusive => {
                state.writer = false;
            }
        }
        ready.notify_all();
    }
}

#[cfg(unix)]
impl Drop for StorageLock {
    fn drop(&mut self) {
        let _ = platform::unlock(&self._file);
    }
}

#[cfg(not(unix))]
impl Drop for StorageLock {
    fn drop(&mut self) {}
}

#[cfg(unix)]
mod platform {
    use super::{File, LockMode, OpenOptions};
    use std::io;
    use std::os::fd::AsRawFd;
    use std::os::raw::c_int;
    use std::path::Path;

    const LOCK_EX: c_int = 2;
    const LOCK_SH: c_int = 1;
    const LOCK_UN: c_int = 8;

    extern "C" {
        fn flock(fd: c_int, operation: c_int) -> c_int;
    }

    pub fn open_locked(path: &Path, mode: LockMode) -> io::Result<File> {
        let file = OpenOptions::new()
            .read(true)
            .write(true)
            .create(true)
            .truncate(false)
            .open(path)?;
        let operation = match mode {
            LockMode::Exclusive => LOCK_EX,
            LockMode::Shared => LOCK_SH,
        };
        lock(&file, operation)?;
        Ok(file)
    }

    pub fn unlock(file: &File) -> io::Result<()> {
        lock(file, LOCK_UN)
    }

    fn lock(file: &File, operation: c_int) -> io::Result<()> {
        let result = unsafe { flock(file.as_raw_fd(), operation) };
        if result == 0 {
            Ok(())
        } else {
            Err(io::Error::last_os_error())
        }
    }
}

#[cfg(windows)]
mod platform {
    use super::{File, LockMode, OpenOptions};
    use std::io;
    use std::os::windows::fs::OpenOptionsExt;
    use std::path::Path;
    use std::thread;
    use std::time::Duration;

    const FILE_SHARE_READ: u32 = 0x0000_0001;
    const FILE_SHARE_WRITE: u32 = 0x0000_0002;
    const RETRY_DELAY: Duration = Duration::from_millis(10);

    pub fn open_locked(path: &Path, mode: LockMode) -> io::Result<File> {
        loop {
            match open_once(path, &mode) {
                Ok(file) => return Ok(file),
                Err(err) if is_lock_contention(&err) => thread::sleep(RETRY_DELAY),
                Err(err) => return Err(err),
            }
        }
    }

    fn open_once(path: &Path, mode: &LockMode) -> io::Result<File> {
        match mode {
            LockMode::Exclusive => OpenOptions::new()
                .read(true)
                .write(true)
                .create(true)
                .truncate(false)
                .share_mode(0)
                .open(path),
            LockMode::Shared => OpenOptions::new()
                .read(true)
                .write(true)
                .create(true)
                .truncate(false)
                .share_mode(FILE_SHARE_READ | FILE_SHARE_WRITE)
                .open(path),
        }
    }

    fn is_lock_contention(err: &io::Error) -> bool {
        matches!(
            err.kind(),
            io::ErrorKind::PermissionDenied | io::ErrorKind::WouldBlock
        )
    }
}

#[cfg(not(any(unix, windows)))]
mod platform {
    use super::{File, LockMode, OpenOptions};
    use std::io;
    use std::path::Path;

    pub fn open_locked(path: &Path, _mode: LockMode) -> io::Result<File> {
        OpenOptions::new()
            .read(true)
            .write(true)
            .create(true)
            .truncate(false)
            .open(path)
    }
}
