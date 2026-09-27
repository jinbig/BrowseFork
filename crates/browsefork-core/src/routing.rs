use crate::{matching_rule, parse_openable_url, Settings};
use std::collections::VecDeque;

#[derive(Debug, Eq, PartialEq)]
pub enum LinkRoute {
    OpenWithRule { url: String, browser_id: String },
    Choose { url: String },
}

#[derive(Debug, Eq, PartialEq)]
pub enum DispatchOutcome {
    Opened,
    NeedsChoice { url: String },
}

pub fn route_link(raw: &str, settings: &Settings) -> LinkRoute {
    let Ok(url) = parse_openable_url(raw) else {
        return LinkRoute::Choose {
            url: raw.to_owned(),
        };
    };
    let url_string = url.to_string();
    if let Some(rule) = matching_rule(settings, &url) {
        LinkRoute::OpenWithRule {
            url: url_string,
            browser_id: rule.browser_id.clone(),
        }
    } else {
        LinkRoute::Choose { url: url_string }
    }
}

pub fn dispatch_link(
    raw: &str,
    settings: &Settings,
    open_with_rule: impl FnOnce(&str, &str) -> Result<(), String>,
) -> DispatchOutcome {
    match route_link(raw, settings) {
        LinkRoute::OpenWithRule { url, browser_id } => {
            if open_with_rule(&browser_id, &url).is_ok() {
                DispatchOutcome::Opened
            } else {
                DispatchOutcome::NeedsChoice { url }
            }
        }
        LinkRoute::Choose { url } => DispatchOutcome::NeedsChoice { url },
    }
}

#[derive(Default)]
pub struct PendingLinks(VecDeque<String>);

impl PendingLinks {
    pub fn push(&mut self, url: String) {
        self.0.retain(|waiting| waiting != &url);
        self.0.push_front(url);
    }

    pub fn front(&self) -> Option<&str> {
        self.0.front().map(String::as_str)
    }

    pub fn pop_front(&mut self) -> Option<String> {
        self.0.pop_front()
    }

    pub fn len(&self) -> usize {
        self.0.len()
    }

    pub fn is_empty(&self) -> bool {
        self.0.is_empty()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::MatchMode;

    #[test]
    fn matching_web_and_local_html_links_route_silently() {
        let mut settings = Settings::default();
        settings
            .add_rule(MatchMode::Host, "example.com".into(), "web-browser".into())
            .unwrap();
        settings
            .add_rule(
                MatchMode::Contains,
                "file:///private/tmp/vpn/".into(),
                "vpn-browser".into(),
            )
            .unwrap();
        assert_eq!(
            route_link("https://example.com/login", &settings),
            LinkRoute::OpenWithRule {
                url: "https://example.com/login".into(),
                browser_id: "web-browser".into()
            }
        );
        assert_eq!(
            route_link("file:///private/tmp/vpn/login.html", &settings),
            LinkRoute::OpenWithRule {
                url: "file:///private/tmp/vpn/login.html".into(),
                browser_id: "vpn-browser".into()
            }
        );
    }

    #[test]
    fn unmatched_and_unsupported_links_reach_the_chooser() {
        let settings = Settings::default();
        for value in [
            "https://ya.ru/",
            "file:///private/tmp/document.txt",
            "custom-auth://callback?code=sample",
        ] {
            assert_eq!(
                route_link(value, &settings),
                LinkRoute::Choose { url: value.into() }
            );
        }
    }

    #[test]
    fn pending_links_keep_distinct_links_and_promote_repeated_link() {
        let mut pending = PendingLinks::default();
        pending.push("first".into());
        pending.push("second".into());
        pending.push("first".into());
        assert_eq!(pending.len(), 2);
        assert_eq!(pending.front(), Some("first"));
        assert_eq!(pending.pop_front().as_deref(), Some("first"));
        assert_eq!(pending.pop_front().as_deref(), Some("second"));
        assert!(pending.is_empty());
    }

    #[test]
    fn failed_or_missing_rule_browser_falls_back_to_choice() {
        let mut settings = Settings::default();
        settings
            .add_rule(
                MatchMode::Host,
                "example.com".into(),
                "removed-browser".into(),
            )
            .unwrap();
        let url = "https://example.com/login";
        assert_eq!(
            dispatch_link(url, &settings, |browser_id, actual_url| {
                assert_eq!(browser_id, "removed-browser");
                assert_eq!(actual_url, url);
                Err("Browser is no longer installed".into())
            }),
            DispatchOutcome::NeedsChoice { url: url.into() }
        );
        assert_eq!(
            dispatch_link(url, &settings, |_, _| Ok(())),
            DispatchOutcome::Opened
        );
    }

    #[test]
    fn unmatched_links_do_not_call_the_browser_launcher() {
        let settings = Settings::default();
        assert_eq!(
            dispatch_link("custom-auth://callback", &settings, |_, _| {
                panic!("unmatched links must use the chooser")
            }),
            DispatchOutcome::NeedsChoice {
                url: "custom-auth://callback".into()
            }
        );
    }
}
