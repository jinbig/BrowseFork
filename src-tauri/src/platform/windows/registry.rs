use super::{expand_environment, split_command, wide};
use crate::platform::windows_model::{
    executable_id, is_browser_executable, unique_executables, BROWSER_PROG_ID, CAPABILITIES_KEY,
    REGISTRATION_NAME,
};
use std::path::Path;
use windows_sys::Win32::UI::Shell::{
    AssocQueryStringW, SHChangeNotify, SHLoadIndirectString, ASSOCF_IS_PROTOCOL, ASSOCSTR_PROGID,
    SHCNE_ASSOCCHANGED, SHCNF_IDLIST,
};
use winreg::{enums::*, RegKey};

pub(super) struct Registration {
    pub identity: String,
    pub executable: String,
    pub arguments: Vec<String>,
    pub name: String,
    pub icon: String,
}

struct Root {
    key: RegKey,
    view: u32,
}
impl Root {
    fn open(&self, path: &str) -> std::io::Result<RegKey> {
        self.key.open_subkey_with_flags(path, KEY_READ | self.view)
    }
}

fn roots() -> Vec<Root> {
    // Per-user registration wins, then native and 32-bit machine registrations.
    [HKEY_CURRENT_USER, HKEY_LOCAL_MACHINE]
        .into_iter()
        .flat_map(|hive| {
            [KEY_WOW64_64KEY, KEY_WOW64_32KEY]
                .into_iter()
                .map(move |view| Root {
                    key: RegKey::predef(hive),
                    view,
                })
        })
        .collect()
}

fn value(key: &RegKey, name: &str) -> Option<String> {
    key.get_value::<String, _>(name)
        .ok()
        .filter(|value| !value.trim().is_empty())
}

fn friendly_name(value: &str) -> Option<String> {
    if !value.starts_with('@') {
        return Some(value.to_owned());
    }
    let input = wide(value);
    let mut output = vec![0u16; 1024];
    let status = unsafe {
        SHLoadIndirectString(
            input.as_ptr(),
            output.as_mut_ptr(),
            output.len() as u32,
            std::ptr::null(),
        )
    };
    (status >= 0).then(|| {
        String::from_utf16_lossy(
            &output[..output.iter().position(|c| *c == 0).unwrap_or(output.len())],
        )
    })
}

fn class_value(roots: &[Root], prog_id: &str, suffix: &str) -> Option<String> {
    roots.iter().find_map(|root| {
        root.open(&format!(r"Software\Classes\{prog_id}\{suffix}"))
            .ok()
            .and_then(|key| value(&key, ""))
    })
}

fn registration(
    roots: &[Root],
    name: &str,
    capabilities: Option<&RegKey>,
    fallback: Option<&RegKey>,
    own: &str,
) -> Option<Registration> {
    let prog_id = capabilities
        .and_then(|key| key.open_subkey("URLAssociations").ok())
        .and_then(|key| value(&key, "https").or_else(|| value(&key, "http")));
    let command = prog_id
        .as_deref()
        .and_then(|id| class_value(roots, id, r"shell\open\command"))
        .or_else(|| {
            fallback
                .and_then(|key| key.open_subkey(r"shell\open\command").ok())
                .and_then(|key| value(&key, ""))
        })?;
    let mut words = split_command(&expand_environment(&command))?.into_iter();
    let executable = words.next()?;
    if !is_browser_executable(&executable, own) || !Path::new(&executable).is_file() {
        return None;
    }
    let display_name = capabilities
        .and_then(|key| value(key, "ApplicationName"))
        .or_else(|| fallback.and_then(|key| value(key, "")))
        .and_then(|name| friendly_name(&name))
        .unwrap_or_else(|| name.to_owned());
    let icon = capabilities
        .and_then(|key| value(key, "ApplicationIcon"))
        .or_else(|| {
            prog_id
                .as_deref()
                .and_then(|id| class_value(roots, id, "DefaultIcon"))
        })
        .or_else(|| {
            fallback
                .and_then(|key| key.open_subkey("DefaultIcon").ok())
                .and_then(|key| value(&key, ""))
        })
        .unwrap_or_else(|| executable.clone());
    Some(Registration {
        identity: super::normalize_source_app_id(&executable),
        executable,
        arguments: words.collect(),
        name: display_name,
        icon: expand_environment(&icon),
    })
}

fn discover_in(roots: &[Root], own: &str) -> Vec<Registration> {
    let mut registrations = Vec::new();
    for root in roots {
        if let Ok(registered) = root.open(r"Software\RegisteredApplications") {
            for (name, _) in registered.enum_values().flatten() {
                let Some(path) = value(&registered, &name) else {
                    continue;
                };
                let Ok(capabilities) = root.open(&path) else {
                    continue;
                };
                if let Some(browser) = registration(roots, &name, Some(&capabilities), None, own) {
                    registrations.push(browser);
                }
            }
        }
        if let Ok(clients) = root.open(r"Software\Clients\StartMenuInternet") {
            for name in clients.enum_keys().flatten() {
                let Ok(client) = clients.open_subkey(&name) else {
                    continue;
                };
                let capabilities = client.open_subkey("Capabilities").ok();
                if let Some(browser) =
                    registration(roots, &name, capabilities.as_ref(), Some(&client), own)
                {
                    registrations.push(browser);
                }
            }
        }
    }
    let mut browsers = unique_executables(registrations, |registration| &registration.identity);
    browsers.sort_by_key(|browser| browser.name.to_lowercase());
    browsers
}

pub(super) fn discover() -> Result<Vec<Registration>, String> {
    let own = std::env::current_exe().map_err(|error| error.to_string())?;
    Ok(discover_in(&roots(), &own.to_string_lossy()))
}

fn set(root: &RegKey, path: &str, name: &str, value: &str) -> std::io::Result<()> {
    root.create_subkey(path)?.0.set_value(name, &value)
}

pub(super) fn write_registration(root: &RegKey, executable: &str) -> std::io::Result<()> {
    let command = format!("\"{executable}\" \"%1\"");
    let icon = format!("\"{executable}\",0");
    let client = r"Software\Clients\StartMenuInternet\BrowseFork";
    set(root, client, "", REGISTRATION_NAME)?;
    set(root, &format!(r"{client}\DefaultIcon"), "", &icon)?;
    set(
        root,
        &format!(r"{client}\shell\open\command"),
        "",
        &format!("\"{executable}\""),
    )?;
    set(root, CAPABILITIES_KEY, "ApplicationName", REGISTRATION_NAME)?;
    set(
        root,
        CAPABILITIES_KEY,
        "ApplicationDescription",
        "Choose a browser for each link, with optional routing rules.",
    )?;
    set(root, CAPABILITIES_KEY, "ApplicationIcon", &icon)?;
    set(
        root,
        &format!(r"{CAPABILITIES_KEY}\Startmenu"),
        "StartMenuInternet",
        REGISTRATION_NAME,
    )?;
    for scheme in ["http", "https"] {
        set(
            root,
            &format!(r"{CAPABILITIES_KEY}\URLAssociations"),
            scheme,
            BROWSER_PROG_ID,
        )?;
    }
    for extension in [".htm", ".html", ".xhtml"] {
        set(
            root,
            &format!(r"{CAPABILITIES_KEY}\FileAssociations"),
            extension,
            BROWSER_PROG_ID,
        )?;
    }
    let class = format!(r"Software\Classes\{BROWSER_PROG_ID}");
    set(root, &class, "", "BrowseFork web link or HTML document")?;
    set(root, &class, "URL Protocol", "")?;
    set(root, &format!(r"{class}\DefaultIcon"), "", &icon)?;
    set(root, &format!(r"{class}\shell\open\command"), "", &command)?;
    // This private scheme only opens Settings; HTTP/HTTPS defaults remain user-controlled.
    set(
        root,
        r"Software\Classes\browsefork",
        "",
        "URL:BrowseFork Settings",
    )?;
    set(root, r"Software\Classes\browsefork", "URL Protocol", "")?;
    set(
        root,
        r"Software\Classes\browsefork\shell\open\command",
        "",
        &command,
    )?;
    set(
        root,
        r"Software\RegisteredApplications",
        REGISTRATION_NAME,
        CAPABILITIES_KEY,
    )
}

fn remove_registration(root: &RegKey, executable: &str) -> std::io::Result<()> {
    let key = format!(r"Software\Classes\{BROWSER_PROG_ID}\shell\open\command");
    let registered = root
        .open_subkey(&key)
        .ok()
        .and_then(|key| value(&key, ""))
        .and_then(|command| split_command(&command))
        .and_then(|words| words.into_iter().next());
    // An old uninstaller must not unregister an installation in a new location.
    if registered.as_deref().map(executable_id) != Some(executable_id(executable)) {
        return Ok(());
    }
    for key in [
        r"Software\Clients\StartMenuInternet\BrowseFork".to_owned(),
        format!(r"Software\Classes\{BROWSER_PROG_ID}"),
        r"Software\Classes\browsefork".into(),
    ] {
        match root.delete_subkey_all(key) {
            Err(error) if error.kind() != std::io::ErrorKind::NotFound => return Err(error),
            _ => {}
        }
    }
    if let Ok(key) = root.open_subkey_with_flags(r"Software\RegisteredApplications", KEY_SET_VALUE)
    {
        match key.delete_value(REGISTRATION_NAME) {
            Err(error) if error.kind() != std::io::ErrorKind::NotFound => return Err(error),
            _ => {}
        }
    }
    Ok(())
}

pub fn register_browser() -> Result<(), String> {
    let executable = std::env::current_exe().map_err(|error| error.to_string())?;
    write_registration(
        &RegKey::predef(HKEY_CURRENT_USER),
        &executable.to_string_lossy(),
    )
    .map_err(|error| error.to_string())?;
    unsafe {
        SHChangeNotify(
            SHCNE_ASSOCCHANGED as i32,
            SHCNF_IDLIST,
            std::ptr::null(),
            std::ptr::null(),
        )
    };
    Ok(())
}

pub fn registration_command(argument: &str) -> Option<Result<(), String>> {
    match argument {
        "--register-browser" => Some(register_browser()),
        "--unregister-browser" => Some((|| {
            let executable = std::env::current_exe().map_err(|error| error.to_string())?;
            remove_registration(
                &RegKey::predef(HKEY_CURRENT_USER),
                &executable.to_string_lossy(),
            )
            .map_err(|error| error.to_string())?;
            unsafe {
                SHChangeNotify(
                    SHCNE_ASSOCCHANGED as i32,
                    SHCNF_IDLIST,
                    std::ptr::null(),
                    std::ptr::null(),
                )
            };
            Ok(())
        })()),
        _ => None,
    }
}

pub fn is_default_browser() -> bool {
    ["http", "https"].into_iter().all(|scheme| {
        let scheme = wide(scheme);
        let mut count = 0;
        unsafe {
            AssocQueryStringW(
                ASSOCF_IS_PROTOCOL,
                ASSOCSTR_PROGID,
                scheme.as_ptr(),
                std::ptr::null(),
                std::ptr::null_mut(),
                &mut count,
            )
        };
        if count == 0 {
            return false;
        }
        let mut output = vec![0u16; count as usize];
        let status = unsafe {
            AssocQueryStringW(
                ASSOCF_IS_PROTOCOL,
                ASSOCSTR_PROGID,
                scheme.as_ptr(),
                std::ptr::null(),
                output.as_mut_ptr(),
                &mut count,
            )
        };
        status >= 0
            && String::from_utf16_lossy(
                &output[..output.iter().position(|c| *c == 0).unwrap_or(output.len())],
            )
            .eq_ignore_ascii_case(BROWSER_PROG_ID)
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::platform::windows_model::executable_name;
    #[test]
    fn registration_discovery_and_uninstall_are_isolated_and_do_not_change_defaults() {
        let hkcu = RegKey::predef(HKEY_CURRENT_USER);
        let path = format!(r"Software\BrowseFork.Tests.{}", std::process::id());
        let (fixture, _) = hkcu.create_subkey(&path).unwrap();
        let own = std::env::current_exe()
            .unwrap()
            .to_string_lossy()
            .into_owned();
        write_registration(&fixture, &own).unwrap();
        write_registration(&fixture, &own).unwrap();
        let class = format!(r"Software\Classes\{BROWSER_PROG_ID}\shell\open\command");
        assert_eq!(
            fixture
                .open_subkey(&class)
                .unwrap()
                .get_value::<String, _>("")
                .unwrap(),
            format!("\"{own}\" \"%1\"")
        );
        assert!(fixture.open_subkey(r"Software\Classes\http").is_err());
        assert!(fixture
            .open_subkey(r"Software\Microsoft\Windows\Shell\Associations\UrlAssociations")
            .is_err());
        remove_registration(&fixture, r"C:\Old\BrowseFork.exe").unwrap();
        assert!(fixture.open_subkey(&class).is_ok());
        // Discover actual registry data using this executable as a safe fake browser.
        let clients = fixture
            .open_subkey_with_flags(
                r"Software\Clients\StartMenuInternet\BrowseFork",
                KEY_ALL_ACCESS,
            )
            .unwrap();
        clients.set_value("", &"Fixture browser").unwrap();
        let discovered = discover_in(
            &[Root {
                key: fixture,
                view: 0,
            }],
            r"C:\Other\BrowseFork.exe",
        );
        assert_eq!(
            discovered.len(),
            usize::from(executable_name(&own).to_lowercase() != "browsefork.exe")
        );
        let fixture = hkcu.open_subkey_with_flags(&path, KEY_ALL_ACCESS).unwrap();
        remove_registration(&fixture, &own).unwrap();
        assert!(fixture.open_subkey(class).is_err());
        drop(fixture);
        hkcu.delete_subkey_all(path).unwrap();
    }
}
