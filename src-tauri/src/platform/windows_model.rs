//! Windows command/identity rules, also tested on macOS without a Windows runtime.
use std::collections::HashSet;

#[cfg(target_os = "windows")]
pub const REGISTRATION_NAME: &str = "BrowseFork";
#[cfg(target_os = "windows")]
pub const BROWSER_PROG_ID: &str = "BrowseFork.HTML";
#[cfg(target_os = "windows")]
pub const CAPABILITIES_KEY: &str = r"Software\Clients\StartMenuInternet\BrowseFork\Capabilities";

pub fn executable_id(path: &str) -> String {
    let normalized = path.replace('/', "\\");
    let normalized = normalized
        .strip_prefix(r"\\?\UNC\")
        .map(|rest| format!(r"\\{rest}"))
        .unwrap_or_else(|| {
            normalized
                .strip_prefix(r"\\?\")
                .unwrap_or(&normalized)
                .to_owned()
        });
    normalized.to_lowercase()
}

pub fn executable_name(path: &str) -> &str {
    path.rsplit(['\\', '/']).next().unwrap_or(path)
}

pub fn is_browser_executable(path: &str, own_path: &str) -> bool {
    let bytes = path.as_bytes();
    let absolute = path.starts_with(r"\\")
        || (bytes.len() > 3
            && bytes[0].is_ascii_alphabetic()
            && bytes[1] == b':'
            && matches!(bytes[2], b'\\' | b'/'));
    absolute
        && path.to_ascii_lowercase().ends_with(".exe")
        && executable_id(path) != executable_id(own_path)
        && !matches!(
            executable_name(path).to_ascii_lowercase().as_str(),
            "browsefork.exe"
                | "cmd.exe"
                | "powershell.exe"
                | "pwsh.exe"
                | "wscript.exe"
                | "cscript.exe"
                | "explorer.exe"
                | "rundll32.exe"
                | "mshta.exe"
        )
}

/// Build literal process arguments. Never pass URLs through cmd.exe or a shell.
pub fn launch_arguments(template: &[String], url: &str) -> Vec<String> {
    let mut replaced = false;
    let mut arguments = Vec::new();
    for argument in template {
        let mut value = argument.clone();
        // A single registered target is supported; do not expand shell wildcard
        // placeholders or substitute URL contents recursively.
        for placeholder in ["%1", "%l", "%L", "%u", "%U"] {
            if value.contains(placeholder) {
                // Split the original template rather than repeatedly replacing
                // the result: the URL may itself contain another placeholder.
                value = argument.replace(placeholder, url);
                replaced = true;
                break;
            }
        }
        if matches!(value.as_str(), "%*" | "%2" | "%3" | "%4") {
            continue;
        }
        arguments.push(value);
    }
    if !replaced {
        arguments.push(url.to_owned());
    }
    arguments
}

/// A profile choice overrides registration defaults without treating the URL as flags.
pub fn profile_launch_arguments(
    template: &[String],
    url: &str,
    profile: Vec<String>,
) -> Vec<String> {
    let mut filtered = Vec::new();
    let mut skip_value = false;
    for argument in template {
        if skip_value {
            skip_value = false;
            continue;
        }
        if matches!(argument.as_str(), "--profile-directory" | "--user-data-dir") {
            skip_value = true;
            continue;
        }
        if argument.starts_with("--profile-directory=") || argument.starts_with("--user-data-dir=")
        {
            continue;
        }
        filtered.push(argument.clone());
    }
    profile
        .into_iter()
        .chain(launch_arguments(&filtered, url))
        .collect()
}

pub fn unique_executables<T>(
    items: impl IntoIterator<Item = T>,
    path: impl Fn(&T) -> &str,
) -> Vec<T> {
    let mut seen = HashSet::new();
    items
        .into_iter()
        .filter(|item| seen.insert(executable_id(path(item))))
        .collect()
}

pub fn is_expected_browser_activation(expected: &str, foreground: &str) -> bool {
    executable_id(expected) == executable_id(foreground)
}

/// Shell registrations and explicit paths may include an icon resource index.
pub fn icon_location(value: &str) -> (String, i32) {
    let (path, index) = value
        .rsplit_once(',')
        .and_then(|(path, index)| index.trim().parse().ok().map(|index| (path, index)))
        .unwrap_or((value, 0));
    (path.trim().trim_matches('"').to_owned(), index)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn identities_ignore_case_slashes_and_extended_path_prefixes() {
        assert_eq!(
            executable_id(r"C:\Apps\Chrome.EXE"),
            executable_id("c:/apps/chrome.exe")
        );
        assert_eq!(
            executable_id(r"\\?\C:\Apps\Chrome.exe"),
            executable_id(r"c:\apps\chrome.exe")
        );
        assert_eq!(
            executable_id(r"\\?\UNC\server\Apps\Firefox.exe"),
            executable_id(r"\\server\apps\firefox.exe")
        );
    }

    #[test]
    fn duplicates_keep_the_first_user_registration_and_distinct_editions() {
        let paths = [
            r"C:\Apps\Chrome.exe",
            r"c:\apps\CHROME.EXE",
            r"C:\Apps\Chrome Beta\chrome.exe",
        ];
        assert_eq!(
            unique_executables(paths, |path| path),
            vec![paths[0], paths[2]]
        );
    }

    #[test]
    fn own_copies_shell_wrappers_and_relative_commands_are_excluded() {
        for path in [
            r"C:\Apps\BrowseFork.exe",
            r"D:\Old\browsefork.exe",
            r"C:\Windows\cmd.exe",
            r"C:\Windows\explorer.exe",
            r"C:\Windows\System32\rundll32.exe",
            "chrome.exe",
            "relative\\chrome.exe",
            r"C:\Apps\browser.bat",
        ] {
            assert!(
                !is_browser_executable(path, r"C:\Apps\BrowseFork.exe"),
                "{path}"
            );
        }
        assert!(is_browser_executable(
            r"C:\Apps\Brave\brave.exe",
            r"C:\Apps\BrowseFork.exe"
        ));
        assert!(is_browser_executable(r"\\server\Apps\firefox.exe", ""));
    }

    #[test]
    fn registered_flags_and_entire_url_are_preserved_as_literal_arguments() {
        let url = "https://example.invalid/path?q=a&token=%1#fragment\"quoted";
        assert_eq!(
            launch_arguments(&["--single-argument".into(), "%1".into()], url),
            vec!["--single-argument", url]
        );
        assert_eq!(
            launch_arguments(&["-osint".into(), "-url".into(), "%L".into()], url),
            vec!["-osint", "-url", url]
        );
        assert_eq!(
            launch_arguments(&["--url=%1".into()], url),
            vec![format!("--url={url}")]
        );
        assert_eq!(
            launch_arguments(&["--new-window".into()], url),
            vec!["--new-window", url]
        );
    }

    #[test]
    fn icon_indices_support_quotes_spaces_commas_and_resource_ids() {
        assert_eq!(
            icon_location(r#""C:\Apps, Inc\browser.exe",-101"#),
            (r"C:\Apps, Inc\browser.exe".into(), -101)
        );
        assert_eq!(
            icon_location(r"C:\Apps\browser.exe"),
            (r"C:\Apps\browser.exe".into(), 0)
        );
    }

    #[test]
    fn only_the_browser_being_launched_can_suppress_a_focus_loss() {
        assert!(is_expected_browser_activation(
            r"C:\Apps\Chrome.exe",
            r"c:\apps\CHROME.EXE"
        ));
        assert!(!is_expected_browser_activation(
            r"C:\Apps\Chrome.exe",
            r"C:\Apps\Slack.exe"
        ));
    }

    #[test]
    fn explicit_profile_overrides_registration_flags_and_keeps_literal_url_last() {
        let template = [
            "--profile-directory=Default",
            "--user-data-dir",
            "C:\\Old Root",
            "--single-argument",
            "%1",
        ]
        .map(str::to_owned);
        let url = "https://example.invalid/?q=--profile-directory=wrong&x=%1#literal|value";
        let arguments = profile_launch_arguments(
            &template,
            url,
            vec![
                "--user-data-dir=C:\\New Root".into(),
                "--profile-directory=Profile 2".into(),
            ],
        );
        assert_eq!(
            arguments,
            vec![
                "--user-data-dir=C:\\New Root",
                "--profile-directory=Profile 2",
                "--single-argument",
                url
            ]
        );
    }
}
