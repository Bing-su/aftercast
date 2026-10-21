use aftercast::{HostConfig, extract_bridge};
use camino::{Utf8Path, Utf8PathBuf};
use rstest::fixture;

pub struct Deployment {
    temporary: tempfile::TempDir,
    root: Utf8PathBuf,
    pub bridge: Utf8PathBuf,
    pub state: Utf8PathBuf,
    #[cfg(windows)]
    _apartment: Apartment,
}

#[cfg(windows)]
struct Apartment(std::marker::PhantomData<std::rc::Rc<()>>);

#[cfg(windows)]
impl Apartment {
    fn new() -> Self {
        use windows_sys::Win32::System::Com::{COINIT_APARTMENTTHREADED, CoInitializeEx};

        // SAFETY: Initialize this test's owner thread; the guard balances success on the same thread.
        let status = unsafe { CoInitializeEx(std::ptr::null(), COINIT_APARTMENTTHREADED as u32) };
        assert!(status >= 0, "initialize Windows STA: {status:#x}");
        Self(std::marker::PhantomData)
    }
}

#[cfg(windows)]
impl Drop for Apartment {
    fn drop(&mut self) {
        // SAFETY: The non-Send guard owns one successful COM initialization on this thread.
        unsafe { windows_sys::Win32::System::Com::CoUninitialize() };
    }
}

impl Deployment {
    pub fn directory(&self) -> &Utf8Path {
        Utf8Path::from_path(self.temporary.path()).expect("UTF-8 fixture directory")
    }

    pub fn config(&self) -> HostConfig {
        HostConfig::builder()
            .dotnet_root(&self.root)
            .bridge_directory(&self.bridge)
            .data_directory(&self.state)
            .build()
    }
}

#[fixture]
pub fn deployment(#[default("state")] state_name: &str) -> Deployment {
    #[cfg(windows)]
    let apartment = Apartment::new();
    let root =
        std::env::var("DOTNET_ROOT").expect("set DOTNET_ROOT to a trusted .NET 10 installation");
    let temporary = tempfile::tempdir().expect("create isolated deployment");
    let directory = Utf8Path::from_path(temporary.path()).expect("UTF-8 fixture directory");
    let bridge = directory.join("bridge 한글");
    let state = directory.join(state_name);
    extract_bridge(&bridge).expect("extract prebuilt managed payload");
    // Own the directory, rather than a running Host, so restart tests retain their files until completion.
    Deployment {
        temporary,
        root: root.into(),
        bridge,
        state,
        #[cfg(windows)]
        _apartment: apartment,
    }
}
