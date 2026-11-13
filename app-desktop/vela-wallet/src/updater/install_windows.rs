//! The Windows install hand-off: a verified download is the Inno Setup
//! installer, and the installer upgrades the app in place.
//!
//! The manifest's SHA-256 already proves the file is the one the owner
//! signed. On top of it, Authenticode: when THIS app is signed
//! (`WinVerifyTrust` passes on our own executable), the installer must pass
//! too — a signed app never hands over to an unsigned or tampered installer.
//! The Windows installer ships unsigned by ruling today (README "Build a
//! Windows installer"), so while the app is unsigned the signed hash is the
//! whole proof, exactly as it is for the person's first download.
//!
//! The installer opens as a person would start it — its own wizard, never
//! `/SILENT` — and the app quits so Inno Setup (`CloseApplications=yes`)
//! can replace it.
//!
//! std and windows-sys only, so `scripts/check-windows.sh` can lift this
//! file into a crate that cross-checks (the app itself cannot be).

use std::path::Path;

/// Whether Windows trusts `path`'s Authenticode signature (a valid chain,
/// not revoked offline, the file unchanged since it was signed). No UI.
#[must_use]
pub fn authenticode_trusted(path: &Path) -> bool {
    use std::os::windows::ffi::OsStrExt as _;
    use windows_sys::Win32::Security::WinTrust::{
        WINTRUST_ACTION_GENERIC_VERIFY_V2, WINTRUST_DATA, WINTRUST_DATA_0, WINTRUST_FILE_INFO,
        WTD_CHOICE_FILE, WTD_REVOKE_NONE, WTD_STATEACTION_CLOSE, WTD_STATEACTION_VERIFY,
        WTD_UI_NONE, WinVerifyTrust,
    };

    let wide: Vec<u16> = path.as_os_str().encode_wide().chain(Some(0)).collect();
    let mut file = WINTRUST_FILE_INFO {
        cbStruct: size_of::<WINTRUST_FILE_INFO>() as u32,
        pcwszFilePath: wide.as_ptr(),
        hFile: std::ptr::null_mut(),
        pgKnownSubject: std::ptr::null_mut(),
    };
    let mut data = WINTRUST_DATA {
        cbStruct: size_of::<WINTRUST_DATA>() as u32,
        pPolicyCallbackData: std::ptr::null_mut(),
        pSIPClientData: std::ptr::null_mut(),
        dwUIChoice: WTD_UI_NONE,
        fdwRevocationChecks: WTD_REVOKE_NONE,
        dwUnionChoice: WTD_CHOICE_FILE,
        Anonymous: WINTRUST_DATA_0 { pFile: &mut file },
        dwStateAction: WTD_STATEACTION_VERIFY,
        hWVTStateData: std::ptr::null_mut(),
        pwszURLReference: std::ptr::null_mut(),
        dwProvFlags: 0,
        dwUIContext: 0,
        pSignatureSettings: std::ptr::null_mut(),
    };
    let mut action = WINTRUST_ACTION_GENERIC_VERIFY_V2;
    // SAFETY: `data` and `file` outlive both calls; `wide` is NUL-terminated
    // and outlives them too. The second call releases the state the first
    // allocated, as the API requires.
    let status = unsafe {
        let status = WinVerifyTrust(
            std::ptr::null_mut(),
            &mut action,
            (&raw mut data).cast::<core::ffi::c_void>(),
        );
        data.dwStateAction = WTD_STATEACTION_CLOSE;
        WinVerifyTrust(
            std::ptr::null_mut(),
            &mut action,
            (&raw mut data).cast::<core::ffi::c_void>(),
        );
        status
    };
    status == 0
}

/// Whether the installer may run: always when this app is unsigned (the
/// manifest's hash is the proof), and only when it verifies too when this
/// app is signed.
#[must_use]
pub fn installer_acceptable(app_signed: bool, installer_signed: bool) -> bool {
    !app_signed || installer_signed
}

/// Start the installer, detached, with its own wizard. `/SP-` only skips
/// Inno's "This will install… continue?" question the person already
/// answered with Install.
pub fn hand_off(installer: &Path) -> std::io::Result<()> {
    std::process::Command::new(installer)
        .arg("/SP-")
        .spawn()
        .map(drop)
}
