//! Where the trial/license state record is kept.
//!
//! The same authenticated record is written to several places so that
//! removing one (uninstalling, deleting the shop data, restoring an old
//! backup, clearing one folder) does not reset the free trial:
//!
//! | Copy | Survives |
//! |---|---|
//! | `HKCU\Software\CODE X MART Licensing` (registry) | uninstall, data folder deletion |
//! | `%APPDATA%\CODE X MART Licensing\state.dat` | uninstall |
//! | `%LOCALAPPDATA%\CODE X MART Licensing\state.dat` | uninstall |
//! | `%ProgramData%\CODE X MART Licensing\state.dat` | uninstall, other Windows users |
//! | `X:\.codexmart-licensing\state.dat` on other local disks | uninstall, Windows reinstall |
//! | the shop database (`app_meta`) and therefore its backups | restore on the same computer |
//!
//! None of these locations are removed by the uninstaller, including its
//! "delete the application data" option (that removes only the
//! `com.codexmart.pos` folders). The records hold no personal or shop data.

use std::path::{Path, PathBuf};

pub trait Anchor: Send + Sync {
    fn describe(&self) -> String;
    fn read(&self) -> Option<String>;
    fn write(&self, text: &str) -> std::io::Result<()>;
}

const README: &str = "This folder is used by CODE X MART to remember its free trial and license on this computer.\r\n\
It contains no shop data and no personal information. Deleting it does not delete any shop data,\r\n\
and it does not restart the free trial.\r\n";

pub struct FileAnchor {
    path: PathBuf,
    with_readme: bool,
}

impl FileAnchor {
    pub fn new(path: PathBuf, with_readme: bool) -> Self {
        FileAnchor { path, with_readme }
    }
}

impl Anchor for FileAnchor {
    fn describe(&self) -> String {
        self.path.display().to_string()
    }

    fn read(&self) -> Option<String> {
        std::fs::read_to_string(&self.path).ok()
    }

    fn write(&self, text: &str) -> std::io::Result<()> {
        let dir = self.path.parent().unwrap_or(Path::new("."));
        std::fs::create_dir_all(dir)?;
        if self.with_readme {
            let readme = dir.join("README.txt");
            if !readme.exists() {
                let _ = std::fs::write(readme, README);
            }
        }
        // Write a temporary file first so a power cut never leaves half a record.
        let tmp = self.path.with_extension("tmp");
        std::fs::write(&tmp, text)?;
        std::fs::rename(&tmp, &self.path)
    }
}

#[cfg(windows)]
pub struct RegistryAnchor;

#[cfg(windows)]
impl RegistryAnchor {
    const KEY: &'static str = r"Software\CODE X MART Licensing";
    const VALUE: &'static str = "State";
}

#[cfg(windows)]
impl Anchor for RegistryAnchor {
    fn describe(&self) -> String {
        format!(r"HKEY_CURRENT_USER\{}", Self::KEY)
    }

    fn read(&self) -> Option<String> {
        use windows_sys::Win32::System::Registry::HKEY_CURRENT_USER;
        super::fingerprint::sys::reg_string(HKEY_CURRENT_USER, Self::KEY, Self::VALUE, 0)
    }

    fn write(&self, text: &str) -> std::io::Result<()> {
        use super::fingerprint::sys::wide;
        use windows_sys::Win32::System::Registry::{RegSetKeyValueW, HKEY_CURRENT_USER, REG_SZ};
        let (k, v, data) = (wide(Self::KEY), wide(Self::VALUE), wide(text));
        let status = unsafe {
            RegSetKeyValueW(
                HKEY_CURRENT_USER,
                k.as_ptr(),
                v.as_ptr(),
                REG_SZ,
                data.as_ptr().cast(),
                (data.len() * 2) as u32,
            )
        };
        if status == 0 {
            Ok(())
        } else {
            Err(std::io::Error::from_raw_os_error(status as i32))
        }
    }
}

/// The copies to keep. `fast` copies are refreshed regularly while the app
/// runs; `extra` copies (other disks) only when the state changes.
pub struct Locations {
    pub fast: Vec<Box<dyn Anchor>>,
    pub extra: Vec<Box<dyn Anchor>>,
}

impl Locations {
    pub fn resolve(roaming: Option<PathBuf>, local: Option<PathBuf>) -> Self {
        #[cfg(feature = "e2e")]
        if let Ok(dir) = std::env::var("CODEXMART_LICENSE_DIR") {
            return Self::isolated(Path::new(&dir));
        }
        Self::system(roaming, local)
    }

    /// Everything inside one folder (automated tests only).
    #[cfg(any(test, feature = "e2e"))]
    pub fn isolated(dir: &Path) -> Self {
        let file = |name: &str| Box::new(FileAnchor::new(dir.join(name), false)) as Box<dyn Anchor>;
        Locations {
            fast: vec![file("registry.dat"), file("appdata.dat"), file("localappdata.dat"), file("programdata.dat")],
            extra: vec![file("other-disk.dat")],
        }
    }

    fn system(roaming: Option<PathBuf>, local: Option<PathBuf>) -> Self {
        const FOLDER: &str = "CODE X MART Licensing";
        let mut fast: Vec<Box<dyn Anchor>> = Vec::new();
        #[cfg(windows)]
        fast.push(Box::new(RegistryAnchor));
        for base in [roaming, local, program_data()].into_iter().flatten() {
            fast.push(Box::new(FileAnchor::new(base.join(FOLDER).join("state.dat"), true)));
        }
        let extra = other_fixed_disks()
            .into_iter()
            .map(|root| {
                Box::new(FileAnchor::new(root.join(".codexmart-licensing").join("state.dat"), true)) as Box<dyn Anchor>
            })
            .collect();
        Locations { fast, extra }
    }

    pub fn all(&self) -> impl Iterator<Item = &dyn Anchor> {
        self.fast.iter().chain(self.extra.iter()).map(|a| a.as_ref())
    }
}

#[cfg(windows)]
fn program_data() -> Option<PathBuf> {
    use windows_sys::Win32::System::Com::CoTaskMemFree;
    use windows_sys::Win32::UI::Shell::{FOLDERID_ProgramData, SHGetKnownFolderPath};
    unsafe {
        let mut path: windows_sys::core::PWSTR = std::ptr::null_mut();
        let hr = SHGetKnownFolderPath(&FOLDERID_ProgramData, 0, std::ptr::null_mut(), &mut path);
        let result = if hr == 0 && !path.is_null() {
            let len = (0..).take_while(|&i| *path.add(i) != 0).count();
            Some(PathBuf::from(String::from_utf16_lossy(std::slice::from_raw_parts(path, len))))
        } else {
            None
        };
        CoTaskMemFree(path.cast());
        result
    }
}

#[cfg(not(windows))]
fn program_data() -> Option<PathBuf> {
    None
}

/// Fixed (non-removable) local disks other than the Windows drive, at most four.
#[cfg(windows)]
fn other_fixed_disks() -> Vec<PathBuf> {
    use windows_sys::Win32::Storage::FileSystem::{GetDriveTypeW, GetLogicalDrives};
    const DRIVE_FIXED: u32 = 3;
    let system = super::fingerprint::sys::system_drive_root().to_ascii_uppercase();
    let mask = unsafe { GetLogicalDrives() };
    (0..26u8)
        .filter(|i| mask & (1 << i) != 0)
        .map(|i| format!("{}:\\", (b'A' + i) as char))
        .filter(|root| *root != system)
        .filter(|root| unsafe { GetDriveTypeW(super::fingerprint::sys::wide(root).as_ptr()) } == DRIVE_FIXED)
        .take(4)
        .map(PathBuf::from)
        .collect()
}

#[cfg(not(windows))]
fn other_fixed_disks() -> Vec<PathBuf> {
    Vec::new()
}
