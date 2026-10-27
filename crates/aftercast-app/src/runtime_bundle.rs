//! Deploy the embedded runtime beside the executable so end users need neither a .NET installation nor network access.
use std::{
    fs,
    io::{self, Read, Write},
    path::{Component, Path, PathBuf},
};

use camino::{Utf8Path, Utf8PathBuf};
use miette::{Context, IntoDiagnostic, Result, miette};

enum ArchiveFormat {
    #[cfg(any(windows, test))]
    Zip,
    #[cfg(any(target_os = "linux", test))]
    TarGz,
}

struct EmbeddedArchive<'a> {
    format: ArchiveFormat,
    bytes: &'a [u8],
}

mod embedded {
    use super::{ArchiveFormat, EmbeddedArchive};
    include!(concat!(env!("OUT_DIR"), "/runtime.rs"));
}

pub fn ensure(root: &Utf8Path) -> Result<Utf8PathBuf> {
    let directory = install(
        root.as_std_path(),
        embedded::ID,
        embedded::ARCHIVES,
        embedded::REQUIRED,
    )
    .wrap_err_with(|| {
        format!(
            "Deploy embedded .NET {} ({}) in {}",
            embedded::VERSION,
            embedded::RID,
            root.join("dotnet")
        )
    })?;
    Utf8PathBuf::from_path_buf(directory)
        .map_err(|path| miette!("Runtime path is not UTF-8: {}", path.display()))
}

fn install(
    root: &Path,
    id: &str,
    archives: &[EmbeddedArchive<'_>],
    required: &[&str],
) -> Result<PathBuf> {
    if id.is_empty()
        || !id
            .bytes()
            .all(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b'-' | b'_'))
    {
        // Version dots are normalized for a single directory component, e.g. 10_0_14-win-x64-<hash>.
        return Err(miette!("Invalid runtime bundle identity"));
    }
    let cache = root.join("dotnet");
    fs::create_dir_all(&cache).into_diagnostic()?;
    let lock = fs::OpenOptions::new()
        .create(true)
        .truncate(false)
        .read(true)
        .write(true)
        .open(cache.join(".extract.lock"))
        .into_diagnostic()?;
    // Serialize publication across app processes; extraction stays on the existing blocking executor.
    lock.lock().into_diagnostic()?;
    let destination = cache.join(id);
    if destination.exists() {
        validate(&destination, id, required).wrap_err_with(|| {
            format!(
                "Bundled runtime cache is damaged: {}",
                destination.display()
            )
        })?;
        return Ok(destination);
    }
    let stage = tempfile::Builder::new()
        .prefix(".extract-")
        .tempdir_in(&cache)
        .into_diagnostic()?;
    let mut remaining = 1024 * 1024 * 1024;
    for archive in archives {
        unpack(archive, stage.path(), &mut remaining)?;
    }
    let mut marker = fs::File::create(stage.path().join(".complete.toml")).into_diagnostic()?;
    marker
        .write_all(format!("id = \"{id}\"\n").as_bytes())
        .into_diagnostic()?;
    marker.sync_all().into_diagnostic()?;
    drop(marker);
    validate(stage.path(), id, required)?;
    // Publish only a complete directory; a killed extraction cannot become a cache hit.
    fs::rename(stage.path(), &destination).into_diagnostic()?;
    Ok(destination)
}

fn relative(path: &Path) -> Result<()> {
    if path.as_os_str().is_empty()
        || !path
            .components()
            .all(|part| matches!(part, Component::Normal(_) | Component::CurDir))
        || path.to_string_lossy().contains(['\\', ':'])
    {
        return Err(miette!("Unsafe runtime archive path: {}", path.display()));
    }
    Ok(())
}

fn unpack(archive: &EmbeddedArchive<'_>, root: &Path, remaining: &mut u64) -> Result<()> {
    match archive.format {
        #[cfg(any(windows, test))]
        ArchiveFormat::Zip => {
            let mut zip = zip::ZipArchive::new(io::Cursor::new(archive.bytes)).into_diagnostic()?;
            for index in 0..zip.len() {
                let mut entry = zip.by_index(index).into_diagnostic()?;
                let path = entry
                    .enclosed_name()
                    .ok_or_else(|| miette!("Unsafe ZIP path"))?;
                relative(&path)?;
                let mode = entry.unix_mode().unwrap_or(0o644);
                if mode & 0o170000 != 0 && !matches!(mode & 0o170000, 0o100000 | 0o040000) {
                    return Err(miette!(
                        "Runtime archive links and special files are unsupported"
                    ));
                }
                let path = root.join(path);
                if entry.is_dir() {
                    fs::create_dir_all(path).into_diagnostic()?;
                } else {
                    let size = entry.size();
                    write_file(&path, &mut entry, size, mode, remaining)?;
                }
            }
        }
        #[cfg(any(target_os = "linux", test))]
        ArchiveFormat::TarGz => {
            let mut tar = tar::Archive::new(flate2::read::GzDecoder::new(archive.bytes));
            for entry in tar.entries().into_diagnostic()? {
                let mut entry = entry.into_diagnostic()?;
                let path = entry.path().into_diagnostic()?;
                relative(&path)?;
                let path = root.join(path);
                let kind = entry.header().entry_type();
                if kind.is_dir() {
                    fs::create_dir_all(path).into_diagnostic()?;
                } else if kind.is_file() {
                    let size = entry.size();
                    let mode = entry.header().mode().into_diagnostic()?;
                    write_file(&path, &mut entry, size, mode, remaining)?;
                } else {
                    return Err(miette!(
                        "Runtime archive links and special files are unsupported"
                    ));
                }
            }
        }
    }
    Ok(())
}

fn write_file(
    path: &Path,
    reader: impl Read,
    size: u64,
    mode: u32,
    remaining: &mut u64,
) -> Result<()> {
    *remaining = remaining
        .checked_sub(size)
        .ok_or_else(|| miette!("Expanded runtime exceeds size limit"))?;
    let parent = path
        .parent()
        .ok_or_else(|| miette!("Missing archive parent"))?;
    fs::create_dir_all(parent).into_diagnostic()?;
    let mut file = fs::File::create(path).into_diagnostic()?;
    if io::copy(&mut reader.take(size + 1), &mut file).into_diagnostic()? != size {
        return Err(miette!("Runtime file size mismatch: {}", path.display()));
    }
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        file.set_permissions(fs::Permissions::from_mode(mode & 0o777))
            .into_diagnostic()?;
    }
    #[cfg(not(unix))]
    let _mode = mode;
    Ok(())
}

fn validate(directory: &Path, id: &str, required: &[&str]) -> Result<()> {
    let marker = directory.join(".complete.toml");
    if fs::metadata(&marker).into_diagnostic()?.len() > 4 * 1024 * 1024 {
        return Err(miette!("Oversized runtime completion marker"));
    }
    // Accept the older marker's extra fields; only identity and essential files govern cache reuse.
    let marker = fs::read_to_string(marker)
        .into_diagnostic()?
        .parse::<toml_edit::DocumentMut>()
        .into_diagnostic()?;
    if marker.get("id").and_then(toml_edit::Item::as_str) != Some(id) {
        return Err(miette!("Runtime completion marker identity mismatch"));
    }
    for file in required {
        let path = directory.join(file);
        if !fs::symlink_metadata(&path).into_diagnostic()?.is_file() {
            return Err(miette!("Missing runtime file: {}", path.display()));
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use flate2::{Compression, write::GzEncoder};
    use std::{io::Cursor, process::Command, thread};

    fn zip_file(name: &str) -> Vec<u8> {
        let mut zip = zip::ZipWriter::new(Cursor::new(Vec::new()));
        zip.start_file(name, zip::write::SimpleFileOptions::default())
            .expect("zip entry");
        zip.write_all(b"runtime fixture").expect("zip body");
        zip.finish().expect("zip").into_inner()
    }

    #[test]
    fn extraction_reuses_completed_caches_and_checks_required_files() {
        let root = tempfile::tempdir().expect("private root");
        let bytes = zip_file("host/fxr/fixture.dll");
        let archives = [EmbeddedArchive {
            format: ArchiveFormat::Zip,
            bytes: &bytes,
        }];
        let required = ["host/fxr/fixture.dll"];
        let path = install(root.path(), "test-id", &archives, &required).expect("extract");
        // Older caches contain an inventory; keep them usable without reading or rehashing its files.
        fs::write(
            path.join(".complete.toml"),
            "id = 'test-id'\n[[files]]\npath = 'ignored.dll'\nsha512 = 'unused'\n",
        )
        .expect("old marker");
        assert_eq!(
            install(root.path(), "test-id", &archives, &required).expect("reuse"),
            path
        );
        fs::write(path.join(".complete.toml"), "id = 'other'").expect("wrong marker");
        install(root.path(), "test-id", &archives, &required).expect_err("reject wrong identity");
        fs::write(path.join(".complete.toml"), "id = 'test-id'").expect("restore marker");
        fs::remove_file(path.join("host/fxr/fixture.dll")).expect("remove required fixture");
        install(root.path(), "test-id", &archives, &required)
            .expect_err("reject missing required file");
        assert!(path.join(".complete.toml").is_file());
    }

    #[test]
    fn rejected_archives_never_publish_a_directory() {
        let root = tempfile::tempdir().expect("private root");
        let bytes = zip_file("../escape.dll");
        let archive = EmbeddedArchive {
            format: ArchiveFormat::Zip,
            bytes: &bytes,
        };
        install(root.path(), "unsafe", &[archive], &["dotnet"]).expect_err("reject traversal");
        assert!(!root.path().join("dotnet/unsafe").exists());
        assert!(!root.path().join("escape.dll").exists());
        let archive = EmbeddedArchive {
            format: ArchiveFormat::Zip,
            bytes: b"invalid ZIP data",
        };
        install(root.path(), "corrupt", &[archive], &["dotnet"])
            .expect_err("reject malformed archive");
        assert!(!root.path().join("dotnet/corrupt").exists());
        install(root.path(), "empty", &[], &["dotnet"]).expect_err("reject incomplete extraction");
        assert!(!root.path().join("dotnet/empty").exists());
        install(root.path(), "../escape", &[], &[]).expect_err("reject identity traversal");
    }

    #[test]
    fn tar_extraction_preserves_modes_and_rejects_links() {
        let root = tempfile::tempdir().expect("root");
        for link in [false, true] {
            let mut tar = tar::Builder::new(GzEncoder::new(Vec::new(), Compression::fast()));
            let mut header = tar::Header::new_gnu();
            header.set_mode(0o755);
            header.set_size(if link { 0 } else { 3 });
            if link {
                header.set_entry_type(tar::EntryType::Symlink);
                header.set_link_name("../outside").expect("link");
            }
            header.set_cksum();
            let body: &[u8] = if link { b"" } else { b"run" };
            tar.append_data(&mut header, "dotnet", body).expect("entry");
            let bytes = tar.into_inner().expect("tar").finish().expect("gzip");
            let archive = EmbeddedArchive {
                format: ArchiveFormat::TarGz,
                bytes: &bytes,
            };
            if link {
                install(root.path(), "link", &[archive], &["dotnet"]).expect_err("reject link");
                assert!(!root.path().join("dotnet/link").exists());
                continue;
            }
            let path =
                install(root.path(), "regular", &[archive], &["dotnet"]).expect("tar extraction");
            assert_eq!(fs::read(path.join("dotnet")).expect("file"), b"run");
            #[cfg(unix)]
            {
                use std::os::unix::fs::PermissionsExt;
                assert_eq!(
                    fs::metadata(path.join("dotnet"))
                        .expect("mode")
                        .permissions()
                        .mode()
                        & 0o777,
                    0o755
                );
            }
        }
    }

    #[test]
    fn embedded_runtime_survives_concurrent_extraction_and_runs_without_global_dotnet() {
        let root = tempfile::tempdir().expect("runtime root");
        let root = Utf8Path::from_path(root.path()).expect("UTF-8 root");
        let other = root.to_owned();
        let concurrent = thread::spawn(move || ensure(&other).expect("concurrent extraction"));
        let runtime = ensure(root).expect("embedded runtime");
        assert_eq!(concurrent.join().expect("worker"), runtime);
        let binary = runtime.join(if cfg!(windows) {
            "dotnet.exe"
        } else {
            "dotnet"
        });
        let result = Command::new(binary)
            .arg("--list-runtimes")
            .env("DOTNET_ROOT", root.join("does-not-exist"))
            .output()
            .expect("private runtime");
        assert!(
            result.status.success(),
            "{}",
            String::from_utf8_lossy(&result.stderr)
        );
        let output = String::from_utf8_lossy(&result.stdout);
        assert!(output.contains(&format!("Microsoft.NETCore.App {}", embedded::VERSION)));
        if embedded::RID.starts_with("win-") {
            assert!(output.contains("Microsoft.WindowsDesktop.App"));
        }
    }
}
