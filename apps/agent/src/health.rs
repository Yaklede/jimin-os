use std::fs::{self, File, OpenOptions};
use std::io::{self, Write};
use std::path::{Path, PathBuf};

const MAX_MARKER_BYTES: u64 = 32;
const MAX_MARKER_NAME_BYTES: usize = 128;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum HealthState {
    Ready,
    AuthRequired,
    Incompatible,
    UnsupportedAccount,
}

impl HealthState {
    pub(crate) fn as_str(self) -> &'static str {
        match self {
            Self::Ready => "ready",
            Self::AuthRequired => "authRequired",
            Self::Incompatible => "incompatible",
            Self::UnsupportedAccount => "unsupportedAccount",
        }
    }

    fn marker_bytes(self) -> &'static [u8] {
        match self {
            Self::Ready => b"ready\n",
            Self::AuthRequired => b"authRequired\n",
            Self::Incompatible => b"incompatible\n",
            Self::UnsupportedAccount => b"unsupportedAccount\n",
        }
    }

    pub(crate) fn is_acceptable_container_health(self) -> bool {
        matches!(self, Self::Ready | Self::AuthRequired)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum HealthMarkerError {
    NotReady,
    UnsafePath,
    Unavailable,
}

impl HealthMarkerError {
    pub(crate) fn code(self) -> &'static str {
        match self {
            Self::NotReady => "agent_not_ready",
            Self::UnsafePath => "health_marker_unsafe",
            Self::Unavailable => "health_marker_unavailable",
        }
    }
}

pub(crate) struct HealthMarker {
    path: PathBuf,
    temporary_path: PathBuf,
}

impl HealthMarker {
    pub(crate) fn resolve(configured: &Path) -> Result<Self, HealthMarkerError> {
        if !configured.is_absolute() {
            return Err(HealthMarkerError::UnsafePath);
        }
        let parent = configured.parent().ok_or(HealthMarkerError::UnsafePath)?;
        let file_name = configured
            .file_name()
            .and_then(|name| name.to_str())
            .ok_or(HealthMarkerError::UnsafePath)?;
        if file_name.is_empty()
            || matches!(file_name, "." | "..")
            || file_name.len() > MAX_MARKER_NAME_BYTES
            || file_name.chars().any(char::is_control)
        {
            return Err(HealthMarkerError::UnsafePath);
        }
        let canonical_parent = parent
            .canonicalize()
            .map_err(|_| HealthMarkerError::Unavailable)?;
        if !canonical_parent.is_dir() {
            return Err(HealthMarkerError::UnsafePath);
        }
        let path = canonical_parent.join(file_name);
        let temporary_path =
            canonical_parent.join(format!(".{file_name}.tmp-{}", std::process::id()));
        Ok(Self {
            path,
            temporary_path,
        })
    }

    pub(crate) fn remove(&self) -> Result<(), HealthMarkerError> {
        remove_non_directory(&self.path)
    }

    pub(crate) fn write(&self, state: HealthState) -> Result<(), HealthMarkerError> {
        self.remove()?;
        remove_non_directory(&self.temporary_path)?;

        let mut file = create_new_private_file(&self.temporary_path)
            .map_err(|_| HealthMarkerError::Unavailable)?;
        let result = (|| -> io::Result<()> {
            file.write_all(state.marker_bytes())?;
            file.sync_all()?;
            drop(file);
            fs::rename(&self.temporary_path, &self.path)
        })();
        if result.is_err() {
            let _ = fs::remove_file(&self.temporary_path);
            return Err(HealthMarkerError::Unavailable);
        }
        Ok(())
    }

    pub(crate) fn read(&self) -> Result<HealthState, HealthMarkerError> {
        let metadata = fs::symlink_metadata(&self.path).map_err(|error| {
            if error.kind() == io::ErrorKind::NotFound {
                HealthMarkerError::NotReady
            } else {
                HealthMarkerError::Unavailable
            }
        })?;
        if metadata.file_type().is_symlink() || !metadata.is_file() {
            return Err(HealthMarkerError::UnsafePath);
        }
        if metadata.len() > MAX_MARKER_BYTES {
            return Err(HealthMarkerError::UnsafePath);
        }
        let bytes = fs::read(&self.path).map_err(|_| HealthMarkerError::Unavailable)?;
        match bytes.as_slice() {
            b"ready\n" => Ok(HealthState::Ready),
            b"authRequired\n" => Ok(HealthState::AuthRequired),
            b"incompatible\n" => Ok(HealthState::Incompatible),
            b"unsupportedAccount\n" => Ok(HealthState::UnsupportedAccount),
            _ => Err(HealthMarkerError::NotReady),
        }
    }
}

fn remove_non_directory(path: &Path) -> Result<(), HealthMarkerError> {
    match fs::symlink_metadata(path) {
        Ok(metadata) if metadata.is_dir() => Err(HealthMarkerError::UnsafePath),
        Ok(_) => fs::remove_file(path).map_err(|_| HealthMarkerError::Unavailable),
        Err(error) if error.kind() == io::ErrorKind::NotFound => Ok(()),
        Err(_) => Err(HealthMarkerError::Unavailable),
    }
}

fn create_new_private_file(path: &Path) -> io::Result<File> {
    let mut options = OpenOptions::new();
    options.write(true).create_new(true);
    #[cfg(unix)]
    {
        use std::os::unix::fs::OpenOptionsExt as _;
        options.mode(0o600);
    }
    options.open(path)
}

#[cfg(test)]
mod tests {
    use std::sync::atomic::{AtomicU64, Ordering};
    use std::time::Duration;

    use super::{HealthMarker, HealthMarkerError, HealthState};

    static NEXT_FIXTURE: AtomicU64 = AtomicU64::new(1);

    fn fixture_path(name: &str) -> (std::path::PathBuf, std::path::PathBuf) {
        let id = NEXT_FIXTURE.fetch_add(1, Ordering::Relaxed);
        let directory =
            std::env::temp_dir().join(format!("jimin-agent-health-{}-{id}", std::process::id()));
        std::fs::create_dir(&directory).expect("health fixture directory");
        let marker = directory.join(name);
        (directory, marker)
    }

    #[tokio::test]
    async fn marker_stays_absent_during_delayed_startup_then_becomes_exact() {
        let (directory, path) = fixture_path("health");
        let marker = HealthMarker::resolve(&path).expect("resolved marker");
        marker.remove().expect("initial cleanup");

        tokio::time::sleep(Duration::from_millis(5)).await;
        assert_eq!(marker.read(), Err(HealthMarkerError::NotReady));
        marker.write(HealthState::Ready).expect("atomic marker");
        assert_eq!(marker.read(), Ok(HealthState::Ready));
        assert_eq!(std::fs::read(&path).expect("marker bytes"), b"ready\n");

        std::fs::write(&path, b"ready \n").expect("invalid exact-state fixture");
        assert_eq!(marker.read(), Err(HealthMarkerError::NotReady));

        marker.remove().expect("marker cleanup");
        assert_eq!(marker.read(), Err(HealthMarkerError::NotReady));
        std::fs::remove_dir(directory).expect("fixture cleanup");
    }

    #[test]
    fn terminal_states_are_exact_and_queryable() {
        let (directory, path) = fixture_path("health");
        let marker = HealthMarker::resolve(&path).expect("resolved marker");

        for (state, expected) in [
            (HealthState::Incompatible, b"incompatible\n".as_slice()),
            (
                HealthState::UnsupportedAccount,
                b"unsupportedAccount\n".as_slice(),
            ),
        ] {
            marker.write(state).expect("terminal marker");
            assert_eq!(marker.read(), Ok(state));
            assert_eq!(std::fs::read(&path).expect("marker bytes"), expected);
            assert!(!state.is_acceptable_container_health());
        }

        assert!(HealthState::Ready.is_acceptable_container_health());
        assert!(HealthState::AuthRequired.is_acceptable_container_health());

        marker.remove().expect("marker cleanup");
        std::fs::remove_dir(directory).expect("fixture cleanup");
    }

    #[cfg(unix)]
    #[test]
    fn health_check_never_follows_a_marker_symlink() {
        use std::os::unix::fs::symlink;

        let (directory, path) = fixture_path("health");
        let target = directory.join("target");
        std::fs::write(&target, b"ready\n").expect("target fixture");
        symlink(&target, &path).expect("marker symlink");
        let marker = HealthMarker::resolve(&path).expect("resolved marker");

        assert_eq!(marker.read(), Err(HealthMarkerError::UnsafePath));
        marker.remove().expect("safe symlink removal");
        assert_eq!(
            std::fs::read(&target).expect("target preserved"),
            b"ready\n"
        );
        std::fs::remove_file(target).expect("target cleanup");
        std::fs::remove_dir(directory).expect("fixture cleanup");
    }
}
