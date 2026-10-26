pub fn application() -> camino::Utf8PathBuf {
    // Integration harnesses have a generated main; this fixture implements the real application entry point.
    let executable = std::env::current_exe().expect("test harness path");
    let debug = executable
        .parent()
        .and_then(std::path::Path::parent)
        .expect("Cargo target directory");
    camino::Utf8PathBuf::try_from(
        debug
            .join("examples")
            .join(format!("test-application{}", std::env::consts::EXE_SUFFIX)),
    )
    .expect("UTF-8 app path")
}
