//! Pin and verify runtime archives before Cargo embeds them; incomplete downloads never become cache hits.
use std::{
    fs,
    io::{self, Read, Write},
    path::{Path, PathBuf},
    time::Duration,
};

use miette::{Context, IntoDiagnostic, Result, miette};
use serde::Deserialize;
use sha2::{Digest, Sha512};

#[derive(Deserialize)]
struct Lock {
    version: String,
    runtimes: Vec<Runtime>,
}

#[derive(Deserialize)]
struct Runtime {
    target: String,
    rid: String,
    archives: Vec<Archive>,
}

#[derive(Deserialize)]
struct Archive {
    format: String,
    url: String,
    sha512: String,
}

pub struct Generated {
    pub source: String,
    pub archives: Vec<PathBuf>,
}

pub fn prepare(manifest: &str, target: &str, output: &Path, offline: bool) -> Result<Generated> {
    let lock: Lock = toml_edit::de::from_str(manifest).into_diagnostic()?;
    let runtime = lock
        .runtimes
        .iter()
        .find(|runtime| runtime.target == target)
        .ok_or_else(|| miette!("No bundled .NET runtime for Cargo target {target}"))?;
    if runtime.archives.is_empty() {
        return Err(miette!("Runtime lock has no archives"));
    }
    let cache = output.join("runtime-cache");
    fs::create_dir_all(&cache).into_diagnostic()?;
    let hash = format!("{:x}", Sha512::digest(format!("{manifest}\n{target}")));
    let identity = format!(
        "{}-{}-{}",
        lock.version.replace('.', "_"),
        runtime.rid,
        hash.chars().take(24).collect::<String>()
    );
    let mut source = format!(
        "pub const ID: &str = {identity:?};\npub const RID: &str = {:?};\npub const VERSION: &str = {:?};\npub static ARCHIVES: &[EmbeddedArchive] = &[\n",
        runtime.rid, lock.version
    );
    let mut archives = Vec::new();
    for archive in &runtime.archives {
        let format = match archive.format.as_str() {
            "zip" => "ArchiveFormat::Zip",
            "tar_gz" => "ArchiveFormat::TarGz",
            other => return Err(miette!("Unknown runtime archive format: {other}")),
        };
        let path = cached_archive(archive, &cache, offline)?;
        source.push_str(&format!(
            "EmbeddedArchive {{ format: {format}, bytes: include_bytes!({:?}) }},\n",
            path.to_string_lossy()
        ));
        archives.push(path);
    }
    source.push_str("];\n");
    let (executable, hostfxr, coreclr) = if runtime.rid.starts_with("win-") {
        ("dotnet.exe", "hostfxr.dll", "coreclr.dll")
    } else {
        ("dotnet", "libhostfxr.so", "libcoreclr.so")
    };
    let mut required = vec![
        executable.to_owned(),
        format!("host/fxr/{}/{hostfxr}", lock.version),
        format!("shared/Microsoft.NETCore.App/{}/{coreclr}", lock.version),
        format!(
            "shared/Microsoft.NETCore.App/{}/System.Private.CoreLib.dll",
            lock.version
        ),
    ];
    if runtime.rid.starts_with("win-") {
        required.push(format!(
            "shared/Microsoft.WindowsDesktop.App/{}/System.Windows.Forms.dll",
            lock.version
        ));
    }
    source.push_str(&format!("pub static REQUIRED: &[&str] = &{required:?};\n"));
    Ok(Generated { source, archives })
}

fn digest(mut input: impl Read) -> Result<String> {
    let mut hash = Sha512::new();
    let mut buffer = [0_u8; 64 * 1024];
    loop {
        let count = input.read(&mut buffer).into_diagnostic()?;
        if count == 0 {
            break;
        }
        hash.update(
            buffer
                .get(..count)
                .ok_or_else(|| miette!("Invalid read length"))?,
        );
    }
    Ok(format!("{:x}", hash.finalize()))
}

fn cached_archive(archive: &Archive, cache: &Path, offline: bool) -> Result<PathBuf> {
    if archive.sha512.len() != 128 || !archive.sha512.bytes().all(|byte| byte.is_ascii_hexdigit()) {
        return Err(miette!("Invalid SHA-512 in runtime lock"));
    }
    let path = cache.join(&archive.sha512);
    if path.exists() {
        if digest(fs::File::open(&path).into_diagnostic()?)? == archive.sha512 {
            return Ok(path);
        }
        if offline {
            return Err(miette!(
                "Corrupt cached runtime archive: {}",
                path.display()
            ));
        }
    } else if offline {
        return Err(miette!(
            "Runtime archive missing in offline mode: {}",
            path.display()
        ));
    }
    // Stream into a private temporary file; interrupted or rejected downloads never become cache hits.
    let agent: ureq::Agent = ureq::Agent::config_builder()
        .timeout_global(Some(Duration::from_secs(300)))
        .build()
        .into();
    let mut response = agent
        .get(&archive.url)
        .call()
        .into_diagnostic()
        .wrap_err_with(|| format!("Download {}", archive.url))?;
    let mut temporary = tempfile::NamedTempFile::new_in(cache).into_diagnostic()?;
    const MAX_BYTES: u64 = 512 * 1024 * 1024;
    let size = io::copy(
        &mut response.body_mut().as_reader().take(MAX_BYTES + 1),
        &mut temporary,
    )
    .into_diagnostic()?;
    if size > MAX_BYTES {
        return Err(miette!("Runtime archive exceeds download limit"));
    }
    temporary.flush().into_diagnostic()?;
    if digest(fs::File::open(temporary.path()).into_diagnostic()?)? != archive.sha512 {
        return Err(miette!("SHA-512 mismatch for {}", archive.url));
    }
    temporary.as_file().sync_all().into_diagnostic()?;
    temporary
        .persist(&path)
        .map_err(|error| error.error)
        .into_diagnostic()?;
    Ok(path)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::{net::TcpListener, thread};

    fn fixture(url: &str, hash: &str) -> String {
        format!(
            "version = '10.0.14'\n[[runtimes]]\ntarget = 'test-target'\nrid = 'test-rid'\n[[runtimes.archives]]\nformat = 'zip'\nurl = '{url}'\nsha512 = '{hash}'\n"
        )
    }

    #[test]
    fn cache_supports_offline_builds_and_rejects_corruption() {
        let temp = tempfile::tempdir().expect("cache");
        let cache = temp.path().join("runtime-cache");
        fs::create_dir(&cache).expect("cache directory");
        let bytes = b"verified archive fixture";
        let hash = digest(bytes.as_slice()).expect("digest");
        let lock = fixture("https://invalid.example/never-requested", &hash);
        let path = cache.join(&hash);
        fs::write(&path, bytes).expect("seed cache");
        let generated = prepare(&lock, "test-target", temp.path(), true).expect("offline cache");
        assert!(generated.source.contains("test-rid"));
        assert_eq!(generated.archives, vec![path.clone()]);
        fs::write(&path, b"corrupt").expect("corrupt cache");
        prepare(&lock, "test-target", temp.path(), true)
            .err()
            .expect("reject corrupt cache");
        prepare(&lock, "wrong-target", temp.path(), true)
            .err()
            .expect("reject unsupported target");
        fs::remove_file(path).expect("remove fixture");
        prepare(&lock, "test-target", temp.path(), true)
            .err()
            .expect("reject missing cache");
    }

    #[test]
    fn failed_download_hash_never_publishes_an_archive() {
        let listener = TcpListener::bind("127.0.0.1:0").expect("test server");
        let url = format!("http://{}", listener.local_addr().expect("address"));
        let server = thread::spawn(move || {
            let (mut stream, _) = listener.accept().expect("request");
            let mut request = [0; 4096];
            assert!(stream.read(&mut request).expect("read request") > 0);
            stream
                .write_all(b"HTTP/1.1 200 OK\r\nContent-Length: 3\r\nConnection: close\r\n\r\nbad")
                .expect("response");
        });
        let temp = tempfile::tempdir().expect("cache");
        let hash = digest(b"expected".as_slice()).expect("digest");
        prepare(&fixture(&url, &hash), "test-target", temp.path(), false)
            .err()
            .expect("reject hash mismatch");
        server.join().expect("server");
        assert_eq!(
            fs::read_dir(temp.path().join("runtime-cache"))
                .expect("cache")
                .count(),
            0
        );
    }

    #[test]
    fn locked_platforms_select_the_requested_target_without_network() {
        let lock: Lock = toml_edit::de::from_str(include_str!("runtime-lock.toml")).expect("lock");
        assert_eq!(lock.runtimes.len(), 4);
        for runtime in &lock.runtimes {
            let temp = tempfile::tempdir().expect("cache");
            let error = prepare(
                include_str!("runtime-lock.toml"),
                &runtime.target,
                temp.path(),
                true,
            )
            .err()
            .expect("empty offline cache");
            assert!(error.to_string().contains("missing in offline mode"));
        }
    }
}
