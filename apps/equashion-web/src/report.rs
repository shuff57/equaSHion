//! The "Report an issue" form's data: the three kinds, the limits, validation and the JSON the
//! page sends to `POST /api/issue-reports`. Modelled on raSHio's issue report, minus the
//! sign-in: anyone can send one. Plain data, so it is tested natively; the form itself is in
//! `showcase.rs` and the network call in `web.rs`.

/// Longest title, characters.
pub const TITLE_MAX: usize = 100;
/// Longest description, characters.
pub const DESC_MAX: usize = 2000;
/// Longest screenshot, as a `data:` URL, characters (about 1 MB of image).
pub const SHOT_MAX_CHARS: usize = 1_400_000;
/// The same limit in words for the reporter.
pub const SHOT_MAX_TEXT: &str = "1 MB";

/// What kind of report this is. Three kinds instead of a category and a severity: severity is a
/// judgement the reporter is badly placed to make.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Kind {
    Bug,
    Quirk,
    Improve,
}

impl Kind {
    pub const ALL: [Kind; 3] = [Kind::Bug, Kind::Quirk, Kind::Improve];

    /// The value the API stores.
    pub fn value(self) -> &'static str {
        match self {
            Kind::Bug => "bug",
            Kind::Quirk => "quirk",
            Kind::Improve => "enhancement",
        }
    }

    pub fn label(self) -> &'static str {
        match self {
            Kind::Bug => "Bug",
            Kind::Quirk => "Quirk",
            Kind::Improve => "Improve",
        }
    }

    /// The gloss under the label: "quirk" is our word, not a reporter's.
    pub fn hint(self) -> &'static str {
        match self {
            Kind::Bug => "doesn't work",
            Kind::Quirk => "works, but picky",
            Kind::Improve => "wish it had",
        }
    }

    /// Example title, shown as the field's hint text.
    pub fn title_hint(self) -> &'static str {
        match self {
            Kind::Bug => "Matrix cells lose the caret after Tab",
            Kind::Quirk => "Typing sqrt needs a space before the argument",
            Kind::Improve => "Let me export the equation as a Word file",
        }
    }

    /// Example description, shown as the field's hint text.
    pub fn body_hint(self) -> &'static str {
        match self {
            Kind::Bug => "What did you try, and what happened instead?",
            Kind::Quirk => "What feels off or picky about how this works?",
            Kind::Improve => "What would you like to be able to do?",
        }
    }
}

/// An attached screenshot.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Shot {
    pub name: String,
    /// `data:image/...;base64,...`
    pub data_url: String,
}

/// Why a screenshot was refused, in words (None when it is fine).
pub fn shot_problem(data_url: &str) -> Option<&'static str> {
    let ok = ["data:image/png;base64,", "data:image/jpeg;base64,", "data:image/gif;base64,", "data:image/webp;base64,"];
    if !ok.iter().any(|p| data_url.starts_with(p)) {
        Some("Screenshots must be images (PNG, JPEG, GIF or WebP).")
    } else if data_url.len() > SHOT_MAX_CHARS {
        Some("That image is over 1 MB. Try a smaller screenshot.")
    } else {
        None
    }
}

/// What the page sends: the form plus context that helps to reproduce it.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Submission {
    pub kind: Kind,
    pub title: String,
    pub description: String,
    pub screenshot: Option<String>,
    /// The equation in the editor, as LaTeX.
    pub latex: String,
    pub browser: String,
    pub page_url: String,
    pub app_version: String,
}

/// The form is not ready to send, in words (None when it is).
pub fn problem(title: &str, description: &str) -> Option<&'static str> {
    if title.trim().is_empty() {
        Some("Add a short title.")
    } else if description.trim().is_empty() {
        Some("Add a description.")
    } else {
        None
    }
}

impl Submission {
    /// The request body: the fields the API reads (`title`, `description`, `kind`,
    /// `screenshot`) and the context (`latex`, `browser`, `pageUrl`, `appVersion`).
    pub fn to_json(&self) -> String {
        let mut o = String::from("{");
        let mut field = |k: &str, v: &str| {
            if o.len() > 1 {
                o.push(',');
            }
            o.push_str(&format!("\"{k}\":{}", json_string(v)));
        };
        field("title", self.title.trim());
        field("description", self.description.trim());
        field("kind", self.kind.value());
        if let Some(s) = &self.screenshot {
            field("screenshot", s);
        }
        field("latex", &self.latex);
        field("browser", &self.browser);
        field("pageUrl", &self.page_url);
        field("appVersion", &self.app_version);
        o.push('}');
        o
    }
}

/// A JSON string literal.
pub fn json_string(s: &str) -> String {
    let mut o = String::with_capacity(s.len() + 2);
    o.push('"');
    for c in s.chars() {
        match c {
            '"' => o.push_str("\\\""),
            '\\' => o.push_str("\\\\"),
            '\n' => o.push_str("\\n"),
            '\r' => o.push_str("\\r"),
            '\t' => o.push_str("\\t"),
            c if (c as u32) < 0x20 => o.push_str(&format!("\\u{:04x}", c as u32)),
            c => o.push(c),
        }
    }
    o.push('"');
    o
}

#[cfg(test)]
mod tests {
    use super::*;

    fn sample() -> Submission {
        Submission {
            kind: Kind::Quirk,
            title: "  A \"quoted\" title ".into(),
            description: "line one\nline two \\ backslash".into(),
            screenshot: None,
            latex: r"\frac{a}{b}".into(),
            browser: "UA".into(),
            page_url: "http://x/".into(),
            app_version: "0.1.0".into(),
        }
    }

    #[test]
    fn the_three_kinds_use_the_api_values() {
        assert_eq!(Kind::ALL.map(Kind::value), ["bug", "quirk", "enhancement"]);
        assert_eq!(Kind::ALL.map(Kind::label), ["Bug", "Quirk", "Improve"]);
    }

    #[test]
    fn the_body_is_json_with_every_field_escaped() {
        let j = sample().to_json();
        assert!(
            j.starts_with("{\"title\":\"A \\\"quoted\\\" title\",\"description\":\"line one\\nline two \\\\ backslash\",\"kind\":\"quirk\""),
            "{j}"
        );
        assert!(j.contains("\"latex\":\"\\\\frac{a}{b}\""), "{j}");
        assert!(!j.contains("screenshot"));
        let mut s = sample();
        s.screenshot = Some("data:image/png;base64,AAAA".into());
        assert!(s.to_json().contains("\"screenshot\":\"data:image/png;base64,AAAA\""));
    }

    #[test]
    fn control_characters_are_escaped() {
        assert_eq!(json_string("a\u{1}b\tc"), "\"a\\u0001b\\tc\"");
    }

    #[test]
    fn an_empty_title_or_description_is_refused() {
        assert_eq!(problem("  ", "x"), Some("Add a short title."));
        assert_eq!(problem("t", ""), Some("Add a description."));
        assert_eq!(problem("t", "d"), None);
    }

    #[test]
    fn only_small_images_are_accepted() {
        assert_eq!(shot_problem("data:image/png;base64,AAAA"), None);
        assert!(shot_problem("data:text/html;base64,AAAA").is_some());
        assert!(shot_problem(&format!("data:image/png;base64,{}", "A".repeat(SHOT_MAX_CHARS))).is_some());
    }
}
