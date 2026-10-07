use regex::Regex;
use serde::{Deserialize, Serialize};
use std::fs;
use std::path::Path;
use url::Url;

mod routing;
pub use routing::{
    dispatch_link, dispatch_link_for_source, route_link, route_link_for_source, DispatchOutcome,
    LinkRoute, PendingLinks,
};

pub const SETTINGS_VERSION: u32 = 1;

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum MatchMode {
    Host,
    Contains,
    Regex,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct Rule {
    pub id: u64,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub source_app_id: Option<String>,
    pub mode: MatchMode,
    pub pattern: String,
    pub browser_id: String,
    pub enabled: bool,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct Settings {
    pub version: u32,
    pub rules: Vec<Rule>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub preferences_window_size: Option<WindowSize>,
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct WindowSize {
    pub width: u32,
    pub height: u32,
}

impl Default for Settings {
    fn default() -> Self {
        Self {
            version: SETTINGS_VERSION,
            rules: Vec::new(),
            preferences_window_size: None,
        }
    }
}

pub fn parse_web_url(value: &str) -> Result<Url, String> {
    let url = Url::parse(value).map_err(|error| format!("Invalid URL: {error}"))?;
    if !matches!(url.scheme(), "http" | "https") || url.host_str().is_none() {
        return Err("Only absolute http and https URLs are supported".into());
    }
    Ok(url)
}

pub fn parse_openable_url(value: &str) -> Result<Url, String> {
    let url = Url::parse(value).map_err(|error| format!("Invalid URL: {error}"))?;
    if matches!(url.scheme(), "http" | "https") && url.host_str().is_some() {
        return Ok(url);
    }
    if url.scheme() == "file" {
        let path = url
            .to_file_path()
            .map_err(|_| "Only local HTML files can be opened".to_string())?;
        let extension = path.extension().and_then(|value| value.to_str());
        if extension.is_some_and(|value| {
            matches!(
                value.to_ascii_lowercase().as_str(),
                "html" | "htm" | "xhtml"
            )
        }) {
            return Ok(url);
        }
    }
    Err("Only http and https links or local HTML files are supported".into())
}

pub fn suggested_pattern(url: &Url) -> String {
    if url.scheme() == "file" {
        return url
            .join("./")
            .map(|folder| folder.to_string())
            .unwrap_or_default();
    }
    url.host_str().unwrap_or_default().to_owned()
}

pub fn validate_rule(rule: &Rule) -> Result<(), String> {
    if rule.browser_id.trim().is_empty() {
        return Err("Choose a browser".into());
    }
    let pattern = rule.pattern.trim();
    let source_app_id = rule.source_app_id.as_deref().map(str::trim).unwrap_or("");
    if pattern.is_empty() && source_app_id.is_empty() {
        return Err("Choose a source app or enter a URL pattern".into());
    }
    if source_app_id.len() > 512 || source_app_id.chars().any(char::is_whitespace) {
        return Err("Enter a valid source app ID".into());
    }
    if pattern.len() > 2_048 {
        return Err("Match pattern is too long".into());
    }
    if pattern.is_empty() {
        return Ok(());
    }
    match rule.mode {
        MatchMode::Host => {
            let host = pattern.trim_start_matches("*.");
            if host.is_empty()
                || host.contains('/')
                || host.contains(':')
                || host.chars().any(char::is_whitespace)
                || Url::parse(&format!("https://{host}/"))
                    .ok()
                    .and_then(|url| url.host_str().map(str::to_owned))
                    .as_deref()
                    != Some(host.to_ascii_lowercase().as_str())
            {
                return Err("Enter a hostname such as example.com".into());
            }
        }
        MatchMode::Contains => {}
        MatchMode::Regex => {
            Regex::new(pattern).map_err(|error| format!("Invalid regular expression: {error}"))?;
        }
    }
    Ok(())
}

pub fn matching_rule<'a>(settings: &'a Settings, url: &Url) -> Option<&'a Rule> {
    matching_rule_for_source(settings, url, None)
}

pub fn matching_rule_for_source<'a>(
    settings: &'a Settings,
    url: &Url,
    source_app_id: Option<&str>,
) -> Option<&'a Rule> {
    let full_url = url.as_str();
    let host = url.host_str().unwrap_or_default();
    settings.rules.iter().find(|rule| {
        if !rule.enabled {
            return false;
        }
        if let Some(required) = rule
            .source_app_id
            .as_deref()
            .filter(|id| !id.trim().is_empty())
        {
            if !source_app_id.is_some_and(|source| source.eq_ignore_ascii_case(required)) {
                return false;
            }
        }
        let pattern = rule.pattern.trim();
        if pattern.is_empty() {
            return true;
        }
        match rule.mode {
            MatchMode::Host => {
                let pattern = pattern.trim_start_matches("*.");
                host.eq_ignore_ascii_case(pattern)
                    || host
                        .to_ascii_lowercase()
                        .ends_with(&format!(".{}", pattern.to_ascii_lowercase()))
            }
            MatchMode::Contains => full_url
                .to_ascii_lowercase()
                .contains(&pattern.to_ascii_lowercase()),
            MatchMode::Regex => Regex::new(pattern)
                .map(|regex| regex.is_match(full_url))
                .unwrap_or(false),
        }
    })
}

impl Settings {
    pub fn add_rule(
        &mut self,
        mode: MatchMode,
        pattern: String,
        browser_id: String,
    ) -> Result<Rule, String> {
        self.add_rule_for_source(None, mode, pattern, browser_id)
    }

    pub fn add_rule_for_source(
        &mut self,
        source_app_id: Option<String>,
        mode: MatchMode,
        pattern: String,
        browser_id: String,
    ) -> Result<Rule, String> {
        let next_id = self
            .rules
            .iter()
            .map(|rule| rule.id)
            .max()
            .unwrap_or(0)
            .checked_add(1)
            .ok_or("Too many rules")?;
        let rule = Rule {
            id: next_id,
            source_app_id: source_app_id
                .map(|id| id.trim().to_owned())
                .filter(|id| !id.is_empty()),
            mode,
            pattern: pattern.trim().to_owned(),
            browser_id,
            enabled: true,
        };
        validate_rule(&rule)?;
        self.rules.insert(0, rule.clone());
        Ok(rule)
    }

    pub fn replace_rules(&mut self, rules: Vec<Rule>) -> Result<(), String> {
        let mut ids = std::collections::HashSet::new();
        for rule in &rules {
            validate_rule(rule)?;
            if !ids.insert(rule.id) {
                return Err("Rule IDs must be unique".into());
            }
        }
        self.rules = rules;
        Ok(())
    }
}

pub fn load_settings(path: &Path) -> Result<Settings, String> {
    let contents = match fs::read_to_string(path) {
        Ok(contents) => contents,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => {
            return Ok(Settings::default())
        }
        Err(error) => return Err(format!("Cannot read preferences: {error}")),
    };
    let settings: Settings = serde_json::from_str(&contents)
        .map_err(|error| format!("Cannot parse preferences: {error}"))?;
    if settings.version != SETTINGS_VERSION {
        return Err(format!(
            "Unsupported preferences version: {}",
            settings.version
        ));
    }
    let mut checked = Settings::default();
    checked.replace_rules(settings.rules)?;
    checked.preferences_window_size = settings.preferences_window_size;
    Ok(checked)
}

pub fn save_settings(path: &Path, settings: &Settings) -> Result<(), String> {
    let parent = path.parent().ok_or("Preferences path has no parent")?;
    fs::create_dir_all(parent)
        .map_err(|error| format!("Cannot create preferences folder: {error}"))?;
    let data = serde_json::to_vec_pretty(settings)
        .map_err(|error| format!("Cannot serialize preferences: {error}"))?;
    let temporary = path.with_extension("json.tmp");
    fs::write(&temporary, data).map_err(|error| format!("Cannot write preferences: {error}"))?;
    fs::rename(&temporary, path).map_err(|error| format!("Cannot save preferences: {error}"))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn host_rule_matches_only_domain_and_subdomains() {
        let mut settings = Settings::default();
        settings
            .add_rule(MatchMode::Host, "example.com".into(), "browser".into())
            .unwrap();
        for value in ["https://example.com/a", "https://work.example.com/b"] {
            assert!(matching_rule(&settings, &parse_web_url(value).unwrap()).is_some());
        }
        for value in [
            "https://evil-example.com/",
            "https://example.com.evil.test/",
        ] {
            assert!(matching_rule(&settings, &parse_web_url(value).unwrap()).is_none());
        }
    }

    #[test]
    fn ordered_rules_support_substrings_and_regex() {
        let mut settings = Settings::default();
        settings
            .add_rule(MatchMode::Contains, "/work/".into(), "work".into())
            .unwrap();
        settings
            .add_rule(
                MatchMode::Regex,
                r"^https://example\.com/private/".into(),
                "private".into(),
            )
            .unwrap();
        let url = parse_web_url("https://example.com/private/work/").unwrap();
        assert_eq!(
            matching_rule(&settings, &url).unwrap().browser_id,
            "private"
        );
    }

    #[test]
    fn rejects_invalid_regex_and_non_web_urls() {
        let mut settings = Settings::default();
        assert!(settings
            .add_rule(MatchMode::Regex, "[".into(), "browser".into())
            .is_err());
        assert!(parse_web_url("file:///tmp/a").is_err());
    }

    #[test]
    fn local_html_pages_can_be_routed_by_folder() {
        let url = parse_openable_url("file:///private/tmp/vpn%20auth/saml.HTML").unwrap();
        assert_eq!(suggested_pattern(&url), "file:///private/tmp/vpn%20auth/");
        let mut settings = Settings::default();
        settings
            .add_rule(
                MatchMode::Contains,
                suggested_pattern(&url),
                "browser".into(),
            )
            .unwrap();
        assert_eq!(
            matching_rule(&settings, &url).unwrap().browser_id,
            "browser"
        );
        assert!(parse_openable_url("file:///private/tmp/document.pdf").is_err());
        assert!(parse_openable_url("file://remote-host/private/tmp/saml.html").is_err());
    }

    #[test]
    fn disabled_rules_are_skipped_and_reordering_changes_priority() {
        let mut settings = Settings::default();
        let broad = settings
            .add_rule(MatchMode::Host, "example.com".into(), "broad".into())
            .unwrap();
        let specific = settings
            .add_rule(MatchMode::Contains, "/private/".into(), "specific".into())
            .unwrap();
        let url = parse_web_url("https://example.com/private/page").unwrap();
        assert_eq!(
            matching_rule(&settings, &url).unwrap().browser_id,
            "specific"
        );

        settings
            .replace_rules(vec![broad.clone(), specific.clone()])
            .unwrap();
        assert_eq!(matching_rule(&settings, &url).unwrap().browser_id, "broad");

        let mut disabled = broad;
        disabled.enabled = false;
        settings.replace_rules(vec![disabled, specific]).unwrap();
        assert_eq!(
            matching_rule(&settings, &url).unwrap().browser_id,
            "specific"
        );
    }

    #[test]
    fn invalid_rule_updates_leave_existing_rules_intact() {
        let mut settings = Settings::default();
        let valid = settings
            .add_rule(MatchMode::Host, "example.com".into(), "browser".into())
            .unwrap();
        assert!(settings
            .replace_rules(vec![valid.clone(), valid.clone()])
            .is_err());
        assert_eq!(settings.rules, vec![valid.clone()]);

        let mut invalid = valid.clone();
        invalid.pattern = "https://example.com/path".into();
        assert!(settings.replace_rules(vec![invalid]).is_err());
        assert_eq!(settings.rules, vec![valid]);
    }

    #[test]
    fn contains_is_case_insensitive_while_regex_keeps_its_flags() {
        let url = parse_web_url("https://example.com/Private/Document").unwrap();
        let mut settings = Settings::default();
        settings
            .add_rule(MatchMode::Contains, "/private/".into(), "contains".into())
            .unwrap();
        assert_eq!(
            matching_rule(&settings, &url).unwrap().browser_id,
            "contains"
        );

        settings
            .add_rule(MatchMode::Regex, "/private/".into(), "regex".into())
            .unwrap();
        assert_eq!(
            matching_rule(&settings, &url).unwrap().browser_id,
            "contains"
        );
        settings
            .add_rule(MatchMode::Regex, "(?i)/private/".into(), "flagged".into())
            .unwrap();
        assert_eq!(
            matching_rule(&settings, &url).unwrap().browser_id,
            "flagged"
        );
    }

    #[test]
    fn source_app_and_url_conditions_match_in_rule_order() {
        let mut settings = Settings::default();
        settings
            .add_rule(MatchMode::Host, "example.com".into(), "fallback".into())
            .unwrap();
        settings
            .add_rule_for_source(
                Some("com.tinyspeck.slackmacgap".into()),
                MatchMode::Contains,
                "/work/".into(),
                "slack-work".into(),
            )
            .unwrap();
        settings
            .add_rule_for_source(
                Some("com.apple.mail".into()),
                MatchMode::Host,
                String::new(),
                "mail-all".into(),
            )
            .unwrap();
        let work = parse_web_url("https://example.com/work/page").unwrap();
        assert_eq!(
            matching_rule_for_source(&settings, &work, Some("com.tinyspeck.slackmacgap"))
                .unwrap()
                .browser_id,
            "slack-work"
        );
        assert_eq!(
            matching_rule_for_source(&settings, &work, Some("com.apple.mail"))
                .unwrap()
                .browser_id,
            "mail-all"
        );
        assert_eq!(
            matching_rule_for_source(&settings, &work, None)
                .unwrap()
                .browser_id,
            "fallback"
        );
        assert_eq!(
            matching_rule_for_source(&settings, &work, Some("com.other.app"))
                .unwrap()
                .browser_id,
            "fallback"
        );
        let other = parse_web_url("https://other.test/").unwrap();
        assert!(matching_rule_for_source(&settings, &other, None).is_none());
    }

    #[test]
    fn rejects_rule_with_any_app_and_any_url() {
        let mut settings = Settings::default();
        assert!(settings
            .add_rule_for_source(None, MatchMode::Host, String::new(), "browser".into())
            .unwrap_err()
            .contains("source app or enter a URL pattern"));
        assert!(settings
            .add_rule_for_source(
                Some("  ".into()),
                MatchMode::Host,
                String::new(),
                "browser".into()
            )
            .is_err());
        assert!(settings.rules.is_empty());
    }

    #[test]
    fn legacy_rules_load_as_any_app() {
        let rule: Rule = serde_json::from_str(
            r#"{"id":1,"mode":"host","pattern":"example.com","browser_id":"browser","enabled":true}"#
        ).unwrap();
        assert_eq!(rule.source_app_id, None);
        validate_rule(&rule).unwrap();
    }

    #[test]
    fn preferences_round_trip_and_reject_corruption() {
        let folder = std::env::temp_dir().join(format!(
            "browsefork-core-test-{}-{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        let path = folder.join("nested/preferences.json");
        let defaults = load_settings(&path).unwrap();
        assert!(defaults.rules.is_empty());
        assert_eq!(defaults.preferences_window_size, None);

        let mut settings = Settings::default();
        settings
            .add_rule(MatchMode::Host, "example.com".into(), "browser".into())
            .unwrap();
        settings.preferences_window_size = Some(WindowSize {
            width: 780,
            height: 640,
        });
        save_settings(&path, &settings).unwrap();
        let loaded = load_settings(&path).unwrap();
        assert_eq!(loaded.version, SETTINGS_VERSION);
        assert_eq!(loaded.rules, settings.rules);
        assert_eq!(
            loaded.preferences_window_size,
            settings.preferences_window_size
        );

        // Old chooser preferences are ignored without losing rules or window size.
        for old_style in ["list", "switcher"] {
            let mut legacy = serde_json::to_value(&settings).unwrap();
            legacy["picker_style"] = serde_json::json!(old_style);
            std::fs::write(&path, serde_json::to_vec(&legacy).unwrap()).unwrap();
            let migrated = load_settings(&path).unwrap();
            assert_eq!(migrated.rules, settings.rules);
            assert_eq!(
                migrated.preferences_window_size,
                settings.preferences_window_size
            );
            save_settings(&path, &migrated).unwrap();
            let written: serde_json::Value =
                serde_json::from_slice(&std::fs::read(&path).unwrap()).unwrap();
            assert!(written.get("picker_style").is_none());
        }

        std::fs::write(&path, r#"{"version":1,"rules":[]}"#).unwrap();
        assert!(load_settings(&path).unwrap().rules.is_empty());
        assert_eq!(load_settings(&path).unwrap().preferences_window_size, None);

        std::fs::write(&path, r#"{"version":999,"rules":[]}"#).unwrap();
        assert!(load_settings(&path)
            .unwrap_err()
            .contains("Unsupported preferences version"));
        std::fs::write(&path, "not json").unwrap();
        assert!(load_settings(&path)
            .unwrap_err()
            .contains("Cannot parse preferences"));
        std::fs::remove_dir_all(folder).unwrap();
    }
}
