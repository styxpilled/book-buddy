//! The `.epub` file association, under `HKCU\Software\Classes`, so no admin
//! rights are necessary. Only `.epub` keys and the ProgID are written, and
//! nothing is deleted.

use anyhow::{Result, bail};
use windows::Win32::System::Registry::{
    HKEY_CURRENT_USER, REG_SZ, RRF_RT_REG_SZ, RegGetValueW, RegSetKeyValueW,
};
use windows::Win32::UI::Shell::{SHCNE_ASSOCCHANGED, SHCNF_IDLIST, SHChangeNotify};
use windows::core::{HSTRING, PCWSTR};

const PROG_ID: &str = "BookBuddy.epub";
const CLASSES: &str = r"Software\Classes";

/// Writes the ProgID with the path of this exe, and adds it to the apps that
/// open `.epub` files. The default app of `.epub` changes only when there is
/// none. Windows keeps the choice of the user elsewhere, so the user still
/// picks the app once in "Open with".
pub fn register() -> Result<()> {
    let exe = std::env::current_exe()?;
    let command = format!("\"{}\" \"%1\"", exe.display());
    set(
        &format!(r"{CLASSES}\{PROG_ID}\shell\open\command"),
        None,
        &command,
    )?;
    set(
        &format!(r"{CLASSES}\.epub\OpenWithProgids"),
        Some(PROG_ID),
        "",
    )?;
    let extension = format!(r"{CLASSES}\.epub");
    if get(&extension, None).is_none_or(|app| app.is_empty() || app == PROG_ID) {
        set(&extension, None, PROG_ID)?;
    }
    // SAFETY: plain arguments.
    unsafe { SHChangeNotify(SHCNE_ASSOCCHANGED, SHCNF_IDLIST, None, None) };
    Ok(())
}

/// The ProgID opens this exe.
pub fn registered() -> bool {
    let Ok(exe) = std::env::current_exe() else {
        return false;
    };
    let command = format!("\"{}\" \"%1\"", exe.display());
    get(&format!(r"{CLASSES}\{PROG_ID}\shell\open\command"), None)
        .is_some_and(|value| value.eq_ignore_ascii_case(&command))
}

/// `None` as the name is the default value of the key. Creates the key.
fn set(key: &str, name: Option<&str>, value: &str) -> Result<()> {
    let data: Vec<u16> = value.encode_utf16().chain([0]).collect();
    let name = name.map(HSTRING::from);
    // SAFETY: the strings and the data live until the call returns.
    let status = unsafe {
        RegSetKeyValueW(
            HKEY_CURRENT_USER,
            &HSTRING::from(key),
            name.as_ref()
                .map_or(PCWSTR::null(), |name| PCWSTR(name.as_ptr())),
            REG_SZ.0,
            Some(data.as_ptr().cast()),
            (data.len() * 2) as u32,
        )
    };
    if status.is_err() {
        bail!(
            "cannot write HKCU\\{key}: {}",
            status.to_hresult().message()
        );
    }
    Ok(())
}

fn get(key: &str, name: Option<&str>) -> Option<String> {
    let key = HSTRING::from(key);
    let name = name.map(HSTRING::from);
    let name = name
        .as_ref()
        .map_or(PCWSTR::null(), |name| PCWSTR(name.as_ptr()));
    let mut size = 0u32;
    // SAFETY: the first call only gets the size.
    let status = unsafe {
        RegGetValueW(
            HKEY_CURRENT_USER,
            &key,
            name,
            RRF_RT_REG_SZ,
            None,
            None,
            Some(&mut size),
        )
    };
    if status.is_err() {
        return None;
    }
    let mut data = vec![0u16; (size as usize).div_ceil(2)];
    // SAFETY: the buffer has the size that the first call gave.
    let status = unsafe {
        RegGetValueW(
            HKEY_CURRENT_USER,
            &key,
            name,
            RRF_RT_REG_SZ,
            None,
            Some(data.as_mut_ptr().cast()),
            Some(&mut size),
        )
    };
    if status.is_err() {
        return None;
    }
    let len = data.iter().position(|&c| c == 0).unwrap_or(data.len());
    Some(String::from_utf16_lossy(&data[..len]))
}
