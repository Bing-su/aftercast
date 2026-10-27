use camino::Utf8Path;
use std::{
    io::Write,
    sync::atomic::{AtomicU8, Ordering},
};

/// Cancellation wins before publication begins. Once committing, the explicit save may finish on Stop.
#[derive(Default, Debug)]
pub(super) struct SaveControl(AtomicU8);
impl SaveControl {
    pub(super) fn cancel(&self) -> bool {
        self.0
            .compare_exchange(0, 2, Ordering::AcqRel, Ordering::Acquire)
            .is_ok()
    }
}
#[derive(Clone, Debug, PartialEq, Eq)]
pub(super) enum Saved {
    Written,
    Canceled,
}
pub(super) fn save_new(
    path: &Utf8Path,
    text: &str,
    control: &SaveControl,
) -> Result<Saved, String> {
    save_with(
        path,
        text,
        control,
        |file, bytes| {
            file.write_all(bytes)?;
            file.flush()?;
            file.sync_all()
        },
        || {},
    )
}
fn save_with(
    path: &Utf8Path,
    text: &str,
    control: &SaveControl,
    write: impl FnOnce(&mut std::fs::File, &[u8]) -> std::io::Result<()>,
    before_commit: impl FnOnce(),
) -> Result<Saved, String> {
    let parent = path
        .parent()
        .ok_or("Missing report destination directory")?;
    let mut temporary =
        tempfile::NamedTempFile::new_in(parent).map_err(|error| error.to_string())?;
    // Same-directory temporary publication refuses a destination that exists, including one created during writing.
    write(temporary.as_file_mut(), text.as_bytes()).map_err(|error| error.to_string())?;
    before_commit();
    if control
        .0
        .compare_exchange(0, 1, Ordering::AcqRel, Ordering::Acquire)
        .is_err()
    {
        return Ok(Saved::Canceled);
    }
    match temporary.persist_noclobber(path) {
        Ok(_) => {
            control.0.store(3, Ordering::Release);
            Ok(Saved::Written)
        }
        Err(error) => {
            control.0.store(4, Ordering::Release);
            Err(error.error.to_string())
        }
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    fn fail_write(file: &mut std::fs::File, bytes: &[u8], stage: &str) -> std::io::Result<()> {
        if stage == "flush" {
            file.write_all(bytes)?;
        }
        Err(std::io::Error::other(stage))
    }
    #[test]
    fn new_file_unicode_cancel_failure_and_raced_destination() {
        let directory = tempfile::tempdir().expect("temporary");
        let root = Utf8Path::from_path(directory.path()).expect("UTF8");
        let path = root.join("report.txt");
        let text = "한글日本😀\n";
        assert_eq!(
            save_new(&path, text, &SaveControl::default()).expect("save"),
            Saved::Written
        );
        assert_eq!(std::fs::read(&path).expect("bytes"), text.as_bytes());
        save_new(&path, "replacement", &SaveControl::default()).expect_err("no overwrite");
        assert_eq!(std::fs::read_to_string(&path).expect("original"), text);
        let canceled = SaveControl::default();
        assert!(canceled.cancel());
        let other = root.join("canceled.txt");
        assert_eq!(
            save_new(&other, text, &canceled).expect("cancel"),
            Saved::Canceled
        );
        assert!(!other.exists());
        for stage in ["write", "flush"] {
            let failed = root.join(format!("{stage}.txt"));
            save_with(
                &failed,
                text,
                &SaveControl::default(),
                |file, bytes| fail_write(file, bytes, stage),
                || {},
            )
            .expect_err("injected write/flush failure");
            assert!(!failed.exists());
        }
        let raced = root.join("raced.txt");
        save_with(
            &raced,
            text,
            &SaveControl::default(),
            |file, bytes| file.write_all(bytes),
            || std::fs::write(&raced, "winner").expect("racer"),
        )
        .expect_err("raced destination");
        assert_eq!(std::fs::read_to_string(raced).expect("winner"), "winner");
        let late = root.join("late.txt");
        let control = SaveControl::default();
        assert_eq!(
            save_with(
                &late,
                text,
                &control,
                |file, bytes| file.write_all(bytes),
                || {
                    assert!(control.cancel());
                }
            )
            .expect("late cancel"),
            Saved::Canceled
        );
        assert!(!late.exists());
        save_new(
            &root.join("missing/report.txt"),
            text,
            &SaveControl::default(),
        )
        .expect_err("missing directory");
        assert_eq!(
            std::fs::read_dir(root).expect("list").count(),
            2,
            "no abandoned temporary files"
        );
        assert!(
            !SaveControl(AtomicU8::new(1)).cancel(),
            "publication already admitted"
        );
    }
}
