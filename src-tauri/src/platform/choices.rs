//! Chooser visibility is independent of browser discovery and rule resolution.
use super::Browser;
use browsefork_core::{ProfileDisplayMode, ProfilePreferences};

pub fn chooser_ids(browsers: &[Browser], preferences: &ProfilePreferences) -> Vec<String> {
    let selected: std::collections::HashSet<_> = preferences.selected_profile_ids.iter().collect();
    let mut ids = Vec::new();
    for browser in browsers.iter().filter(|browser| browser.profile.is_none()) {
        let profiles: Vec<_> = browsers
            .iter()
            .filter(|candidate| {
                candidate
                    .profile
                    .as_ref()
                    .is_some_and(|profile| profile.browser_id == browser.id)
            })
            .collect();
        let choices: Vec<_> = match preferences.mode {
            ProfileDisplayMode::Single => Vec::new(),
            ProfileDisplayMode::All => profiles,
            ProfileDisplayMode::Selected => profiles
                .into_iter()
                .filter(|profile| selected.contains(&profile.id))
                .collect(),
        };
        if choices.is_empty() {
            // Unavailable/deleted selections never make the browser disappear.
            ids.push(browser.id.clone());
        } else {
            ids.extend(choices.into_iter().map(|profile| profile.id.clone()));
        }
    }
    ids
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::platform::BrowserProfile;
    fn catalog() -> Vec<Browser> {
        let chrome = Browser {
            id: "chrome".into(),
            name: "Chrome".into(),
            ..Default::default()
        };
        let mut browsers = vec![chrome.clone()];
        for name in ["A", "S"] {
            browsers.push(Browser {
                id: name.into(),
                name: format!("Chrome · {name}"),
                profile: Some(BrowserProfile {
                    name: name.into(),
                    initials: name.into(),
                    avatar_data_url: None,
                    browser_id: chrome.id.clone(),
                    browser_name: chrome.name.clone(),
                }),
                ..Default::default()
            });
        }
        browsers.push(Browser {
            id: "safari".into(),
            name: "Safari".into(),
            ..Default::default()
        });
        browsers
    }
    #[test]
    fn all_is_default_single_keeps_original_browser_and_selected_filters_only_picker() {
        let browsers = catalog();
        assert_eq!(
            chooser_ids(&browsers, &ProfilePreferences::default()),
            ["A", "S", "safari"]
        );
        assert_eq!(
            chooser_ids(
                &browsers,
                &ProfilePreferences {
                    mode: ProfileDisplayMode::Single,
                    ..Default::default()
                }
            ),
            ["chrome", "safari"]
        );
        let selected = ProfilePreferences {
            mode: ProfileDisplayMode::Selected,
            selected_profile_ids: vec!["S".into()],
        };
        assert_eq!(chooser_ids(&browsers, &selected), ["S", "safari"]);
        assert!(
            browsers.iter().any(|browser| browser.id == "A"),
            "Hidden targets remain available to existing rules"
        );
    }
    #[test]
    fn deleted_selections_and_browsers_without_profiles_keep_one_usable_entry() {
        let selected = ProfilePreferences {
            mode: ProfileDisplayMode::Selected,
            selected_profile_ids: vec!["deleted".into()],
        };
        assert_eq!(chooser_ids(&catalog(), &selected), ["chrome", "safari"]);
        assert_eq!(chooser_ids(&[], &selected), Vec::<String>::new());
    }
}
