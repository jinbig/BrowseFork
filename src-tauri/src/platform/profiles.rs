//! Desktop Chrome profiles. Read metadata only; never edit Chrome's files or
//! inspect cookies/passwords. The same discovery and stable IDs serve each OS.
use super::{Browser, BrowserProfile};
use base64::{engine::general_purpose::STANDARD, Engine};
use serde::{Deserialize, Serialize};
use std::{
    collections::BTreeMap,
    fs::File,
    io::Read,
    path::{Path, PathBuf},
};

const PROFILE_ID_PREFIX: &str = "browsefork-profile-v1:";
const MAX_LOCAL_STATE_BYTES: u64 = 8 * 1024 * 1024;
const MAX_AVATAR_BYTES: u64 = 1024 * 1024;

#[derive(Clone, Debug, Serialize)]
pub struct ChromeProfileTarget {
    pub browser_id: String,
    pub user_data_dir: PathBuf,
    pub directory: String,
}

impl ChromeProfileTarget {
    pub fn arguments(&self) -> Result<Vec<String>, String> {
        if !self.user_data_dir.join(&self.directory).is_dir() {
            return Err("Selected Chrome profile is no longer available".into());
        }
        Ok(vec![
            format!("--user-data-dir={}", self.user_data_dir.display()),
            format!("--profile-directory={}", self.directory),
        ])
    }
}

#[derive(Default, Deserialize)]
#[serde(default)]
struct LocalState {
    profile: ProfileCache,
}
#[derive(Default, Deserialize)]
#[serde(default)]
struct ProfileCache {
    info_cache: BTreeMap<String, ProfileEntry>,
}
#[derive(Default, Deserialize)]
#[serde(default)]
struct ProfileEntry {
    name: String,
    enterprise_label: String,
    is_managed: Option<i32>,
    user_accepted_account_management: bool,
    gaia_given_name: String,
    gaia_name: String,
    is_using_default_name: bool,
    is_using_default_avatar: bool,
    use_gaia_picture: bool,
    is_omitted: bool,
    gaia_picture_file_name: String,
}

fn component(value: &str) -> bool {
    !value.is_empty()
        && !matches!(value, "." | "..")
        && !value
            .chars()
            .any(|c| matches!(c, '/' | '\\' | ':' | '\0') || c.is_control())
}

fn bounded_read(path: &Path, limit: u64) -> Option<Vec<u8>> {
    let file = File::open(path).ok()?;
    if file.metadata().ok()?.len() > limit {
        return None;
    }
    let mut bytes = Vec::new();
    file.take(limit + 1).read_to_end(&mut bytes).ok()?;
    (bytes.len() as u64 <= limit).then_some(bytes)
}

fn avatar(root: &Path, folder: &str, entry: &ProfileEntry) -> Option<String> {
    if !(entry.use_gaia_picture || entry.is_using_default_avatar)
        || !component(&entry.gaia_picture_file_name)
    {
        return None;
    }
    let root = root.canonicalize().ok()?;
    let file = root
        .join(folder)
        .join(&entry.gaia_picture_file_name)
        .canonicalize()
        .ok()?;
    if !file.starts_with(&root) {
        return None;
    }
    let bytes = bounded_read(&file, MAX_AVATAR_BYTES)?;
    let mime = if bytes.starts_with(b"\x89PNG\r\n\x1a\n") {
        "image/png"
    } else if bytes.starts_with(b"\xff\xd8\xff") {
        "image/jpeg"
    } else {
        return None;
    };
    Some(format!("data:{mime};base64,{}", STANDARD.encode(bytes)))
}

fn display_name<'a>(entry: &'a ProfileEntry, folder: &'a str) -> &'a str {
    // Chrome's toolbar gives the enterprise label priority over the Google
    // account name. Only infer Work for an explicitly accepted managed account;
    // never classify a profile from its email address or display name.
    if !entry.enterprise_label.trim().is_empty() {
        return entry.enterprise_label.trim();
    }
    if entry.is_managed == Some(1) && entry.user_accepted_account_management {
        return "Work";
    }
    let gaia = if entry.gaia_given_name.trim().is_empty() {
        &entry.gaia_name
    } else {
        &entry.gaia_given_name
    };
    if entry.is_using_default_name && !gaia.trim().is_empty() {
        return gaia.trim();
    }
    if !entry.name.trim().is_empty() {
        return entry.name.trim();
    }
    if !gaia.trim().is_empty() {
        return gaia.trim();
    }
    folder
}

fn expand_at(browser: Browser, root: &Path) -> Vec<Browser> {
    let Some(state) = bounded_read(&root.join("Local State"), MAX_LOCAL_STATE_BYTES)
        .and_then(|bytes| serde_json::from_slice::<LocalState>(&bytes).ok())
    else {
        return vec![browser];
    };
    let Some(canonical_root) = root.canonicalize().ok() else {
        return vec![browser];
    };
    let mut profiles = Vec::new();
    for (directory, entry) in state.profile.info_cache {
        if entry.is_omitted
            || !component(&directory)
            || matches!(directory.as_str(), "Guest Profile" | "System Profile")
        {
            continue;
        }
        let path = root.join(&directory);
        if !path.is_dir()
            || !path
                .canonicalize()
                .is_ok_and(|path| path.starts_with(&canonical_root))
        {
            continue;
        }
        let name = display_name(&entry, &directory).to_owned();
        let initials: String = name
            .chars()
            .next()
            .into_iter()
            .flat_map(char::to_uppercase)
            .take(2)
            .collect();
        let target = ChromeProfileTarget {
            browser_id: browser.id.clone(),
            user_data_dir: root.to_owned(),
            directory: directory.clone(),
        };
        // Identity uses the installation/data directory/profile directory, never
        // the editable display name or Google email address.
        let id = format!(
            "{PROFILE_ID_PREFIX}{}",
            serde_json::to_string(&target).expect("profile target serialization")
        );
        profiles.push(Browser {
            id,
            name: format!("{} · {name}", browser.name),
            profile: Some(BrowserProfile {
                name,
                initials,
                avatar_data_url: avatar(root, &directory, &entry),
                browser_id: browser.id.clone(),
                browser_name: browser.name.clone(),
            }),
            launch_profile: Some(target),
            ..browser.clone()
        });
    }
    profiles.sort_by(|a, b| {
        a.name
            .to_lowercase()
            .cmp(&b.name.to_lowercase())
            .then(a.id.cmp(&b.id))
    });
    // Equal names are legitimate. Keep both, with directory labels to tell them apart.
    let mut counts = BTreeMap::new();
    for profile in &profiles {
        *counts.entry(profile.name.clone()).or_insert(0) += 1;
    }
    for profile in &mut profiles {
        if counts[&profile.name] > 1 {
            let directory = &profile.launch_profile.as_ref().unwrap().directory;
            profile.name = format!("{} ({directory})", profile.name);
            let short_directory = directory.strip_prefix("Profile ").unwrap_or(directory);
            let info = profile.profile.as_mut().unwrap();
            // Keep the disambiguator first so a narrow tile cannot truncate it.
            info.name = format!("{short_directory} · {}", info.name);
        }
    }
    if profiles.is_empty() {
        vec![browser]
    } else {
        profiles
    }
}

fn chrome_channel(browser: &Browser) -> Option<&'static str> {
    let name = browser.name.to_ascii_lowercase();
    let id = browser.id.to_ascii_lowercase();
    if name == "chromium" || id.ends_with("/chromium.app") || id.ends_with("\\chromium.exe") {
        return Some("Chromium");
    }
    if !(name.contains("chrome")
        || id.ends_with("/google chrome.app")
        || id.ends_with("\\chrome.exe"))
    {
        return None;
    }
    if name.contains("canary") || id.contains("chrome sxs") {
        Some("Chrome Canary")
    } else if name.contains("beta") || id.contains("chrome beta") {
        Some("Chrome Beta")
    } else if name.contains("dev") || id.contains("chrome dev") {
        Some("Chrome Dev")
    } else {
        Some("Chrome")
    }
}

fn data_root(browser: &Browser) -> Option<PathBuf> {
    let channel = chrome_channel(browser)?;
    #[cfg(target_os = "macos")]
    return Some(
        PathBuf::from(std::env::var_os("HOME")?)
            .join("Library/Application Support")
            .join(if channel == "Chromium" {
                PathBuf::from(channel)
            } else {
                PathBuf::from("Google").join(channel)
            }),
    );
    #[cfg(target_os = "windows")]
    return Some(
        PathBuf::from(std::env::var_os("LOCALAPPDATA")?)
            .join(if channel == "Chromium" {
                PathBuf::from(channel)
            } else {
                PathBuf::from("Google").join(if channel == "Chrome Canary" {
                    "Chrome SxS"
                } else {
                    channel
                })
            })
            .join("User Data"),
    );
    #[cfg(not(any(target_os = "macos", target_os = "windows")))]
    {
        let base = std::env::var_os("CHROME_CONFIG_HOME")
            .or_else(|| std::env::var_os("XDG_CONFIG_HOME"))
            .map(PathBuf::from)
            .or_else(|| std::env::var_os("HOME").map(|home| PathBuf::from(home).join(".config")))?;
        Some(base.join(match channel {
            "Chromium" => "chromium",
            "Chrome Beta" => "google-chrome-beta",
            "Chrome Dev" => "google-chrome-unstable",
            "Chrome Canary" => "google-chrome-canary",
            _ => "google-chrome",
        }))
    }
}

pub fn expand_browser(browser: Browser) -> Vec<Browser> {
    match data_root(&browser) {
        Some(root) => expand_at(browser, &root),
        None => vec![browser],
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::atomic::{AtomicU64, Ordering};
    static NEXT_FIXTURE: AtomicU64 = AtomicU64::new(0);
    struct Fixture(PathBuf);
    impl Fixture {
        fn new() -> Self {
            let root = std::env::temp_dir().join(format!(
                "browsefork-profiles-{}-{}",
                std::process::id(),
                NEXT_FIXTURE.fetch_add(1, Ordering::Relaxed)
            ));
            std::fs::create_dir_all(&root).unwrap();
            Self(root)
        }
        fn state(&self, cache: serde_json::Value) {
            std::fs::write(
                self.0.join("Local State"),
                serde_json::to_vec(&serde_json::json!({"profile":{"info_cache":cache}})).unwrap(),
            )
            .unwrap();
        }
        fn profile(&self, folder: &str) {
            std::fs::create_dir_all(self.0.join(folder)).unwrap();
        }
        fn browsers(&self) -> Vec<Browser> {
            expand_at(
                Browser {
                    id: "chrome-install".into(),
                    name: "Chrome".into(),
                    ..Default::default()
                },
                &self.0,
            )
        }
    }
    impl Drop for Fixture {
        fn drop(&mut self) {
            let _ = std::fs::remove_dir_all(&self.0);
        }
    }

    #[test]
    fn arbitrary_personal_names_and_initials_are_separate_stable_targets() {
        let f = Fixture::new();
        f.profile("Default");
        f.profile("Profile 1");
        f.state(serde_json::json!({"Default":{"name":"A"},"Profile 1":{"name":"S"}}));
        let before = f.browsers();
        assert_eq!(
            before.iter().map(|b| b.name.as_str()).collect::<Vec<_>>(),
            vec!["Chrome · A", "Chrome · S"]
        );
        assert_eq!(before[0].profile.as_ref().unwrap().initials, "A");
        f.state(serde_json::json!({"Default":{"name":"Renamed"},"Profile 1":{"name":"S"}}));
        assert_eq!(before[0].id, f.browsers()[0].id);
        assert_ne!(before[0].id, before[1].id);
        assert!(!serde_json::to_string(&before[0])
            .unwrap()
            .contains("launch_profile"));
    }

    #[test]
    fn missing_corrupt_empty_and_oversized_metadata_leave_browser_usable() {
        let f = Fixture::new();
        assert_eq!(f.browsers()[0].id, "chrome-install");
        std::fs::write(f.0.join("Local State"), b"unfinished {").unwrap();
        assert_eq!(f.browsers()[0].id, "chrome-install");
        f.state(serde_json::json!({"Missing":{"name":"Deleted"}}));
        assert_eq!(f.browsers()[0].id, "chrome-install");
        File::create(f.0.join("Local State"))
            .unwrap()
            .set_len(MAX_LOCAL_STATE_BYTES + 1)
            .unwrap();
        assert_eq!(f.browsers()[0].id, "chrome-install");
    }

    #[test]
    fn deleted_guest_omitted_and_traversal_profiles_are_not_choices() {
        let f = Fixture::new();
        f.profile("Default");
        f.profile("Guest Profile");
        f.profile("Hidden");
        f.state(serde_json::json!({"Default":{"name":"A"},"Guest Profile":{"name":"Guest"},"Hidden":{"name":"H","is_omitted":true},"Missing":{"name":"M"},"../outside":{"name":"Bad"},"C:\\outside":{"name":"Bad"}}));
        let browsers = f.browsers();
        assert_eq!(browsers.len(), 1);
        let target = browsers[0].launch_profile.as_ref().unwrap();
        assert_eq!(
            target.arguments().unwrap()[1],
            "--profile-directory=Default"
        );
        std::fs::remove_dir(f.0.join("Default")).unwrap();
        assert!(target.arguments().is_err());
        assert_eq!(f.browsers()[0].id, "chrome-install");
    }

    #[test]
    fn duplicate_names_are_distinct_and_gaia_default_names_are_readable() {
        let f = Fixture::new();
        f.profile("Default");
        f.profile("Profile 1");
        f.profile("Profile 2");
        f.state(serde_json::json!({"Default":{"name":"A"},"Profile 1":{"name":"A"},"Profile 2":{"name":"Person 3","is_using_default_name":true,"gaia_given_name":"Света"}}));
        let browsers = f.browsers();
        assert_eq!(browsers[0].name, "Chrome · A (Default)");
        assert_eq!(browsers[1].name, "Chrome · A (Profile 1)");
        assert_eq!(browsers[2].profile.as_ref().unwrap().initials, "С");
        assert_eq!(browsers[2].name, "Chrome · Света");
    }

    #[test]
    fn toolbar_enterprise_labels_and_custom_profile_names_win_over_account_name() {
        let entry = |value| serde_json::from_value::<ProfileEntry>(value).unwrap();
        for (value, expected) in [
            (
                serde_json::json!({"name":"Personal","gaia_given_name":"Mikhail"}),
                "Personal",
            ),
            (
                serde_json::json!({"name":"Mikhail","enterprise_label":"Engineering","is_managed":1,"user_accepted_account_management":true}),
                "Engineering",
            ),
            (
                serde_json::json!({"name":"Mikhail","is_managed":1,"user_accepted_account_management":true}),
                "Work",
            ),
            (
                serde_json::json!({"name":"Mikhail","is_managed":1,"user_accepted_account_management":false}),
                "Mikhail",
            ),
            (serde_json::json!({"name":"A","is_managed":0}), "A"),
        ] {
            assert_eq!(display_name(&entry(value), "Default"), expected);
        }
        let f = Fixture::new();
        f.profile("Default");
        f.state(serde_json::json!({"Default":{"name":"Mikhail"}}));
        let before = f.browsers()[0].id.clone();
        f.state(serde_json::json!({"Default":{"name":"Personal","gaia_given_name":"Mikhail"}}));
        assert_eq!(f.browsers()[0].name, "Chrome · Personal");
        assert_eq!(f.browsers()[0].id, before);
    }

    #[test]
    fn chromium_is_distinct_from_chrome_channels() {
        let browser = |name: &str| Browser {
            name: name.into(),
            ..Default::default()
        };
        assert_eq!(chrome_channel(&browser("Chromium")), Some("Chromium"));
        assert_eq!(
            chrome_channel(&browser("Google Chrome Beta")),
            Some("Chrome Beta")
        );
        assert_eq!(chrome_channel(&browser("Brave Browser")), None);
    }

    #[test]
    fn avatar_is_local_bounded_image_only_and_never_an_external_url() {
        let f = Fixture::new();
        f.profile("Default");
        f.state(serde_json::json!({"Default":{"name":"A","use_gaia_picture":true,"gaia_picture_file_name":"photo.png"}}));
        std::fs::write(f.0.join("Default/photo.png"), b"\x89PNG\r\n\x1a\nfixture").unwrap();
        assert!(f.browsers()[0]
            .profile
            .as_ref()
            .unwrap()
            .avatar_data_url
            .as_ref()
            .unwrap()
            .starts_with("data:image/png;base64,"));
        std::fs::write(f.0.join("Default/photo.png"), b"<svg>not accepted</svg>").unwrap();
        assert!(f.browsers()[0]
            .profile
            .as_ref()
            .unwrap()
            .avatar_data_url
            .is_none());
        f.state(serde_json::json!({"Default":{"name":"A","use_gaia_picture":true,"gaia_picture_file_name":"../Local State"}}));
        assert!(f.browsers()[0]
            .profile
            .as_ref()
            .unwrap()
            .avatar_data_url
            .is_none());
    }
}
