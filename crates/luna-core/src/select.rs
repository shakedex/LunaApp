//! Which cameras a run reports on.
//!
//! A selector matches a camera when it is the camera's name as the report
//! prints it (`A (ARRI ALEXA 35)`), ignoring case, or when it names a camera
//! letter (`A`, `A_CAM`, `b cam`, `CAM_B`) and the camera has that letter. So
//! a bare letter selects every body under it.

use crate::identity::camera_from_folder_name;

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct CameraFilter {
    selectors: Vec<String>,
}

impl CameraFilter {
    /// No selectors means every camera.
    pub fn new(selectors: &[String]) -> Self {
        Self {
            selectors: selectors
                .iter()
                .map(|s| s.trim().to_owned())
                .filter(|s| !s.is_empty())
                .collect(),
        }
    }

    pub fn is_all(&self) -> bool {
        self.selectors.is_empty()
    }

    pub fn selectors(&self) -> &[String] {
        &self.selectors
    }

    pub fn matches(&self, camera_name: &str) -> bool {
        if self.is_all() {
            return true;
        }
        let letter = camera_letter(camera_name);
        self.selectors.iter().any(|sel| {
            sel.to_lowercase() == camera_name.to_lowercase()
                || (letter.is_some() && selector_letter(sel) == letter)
        })
    }
}

/// The camera letter of a report camera name: the part before ` (` when it is
/// one letter, or the letter a camera folder name carries (`B_CAM`).
pub fn camera_letter(camera_name: &str) -> Option<String> {
    let base = camera_name
        .split_once(" (")
        .map_or(camera_name, |(b, _)| b)
        .trim();
    single_letter(base).or_else(|| camera_from_folder_name(base))
}

fn selector_letter(sel: &str) -> Option<String> {
    single_letter(sel).or_else(|| camera_from_folder_name(sel))
}

fn single_letter(s: &str) -> Option<String> {
    let mut chars = s.chars();
    match (chars.next(), chars.next()) {
        (Some(c), None) if c.is_ascii_alphabetic() => Some(c.to_ascii_uppercase().to_string()),
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn f(sel: &[&str]) -> CameraFilter {
        CameraFilter::new(&sel.iter().map(|s| s.to_string()).collect::<Vec<_>>())
    }

    #[test]
    fn selectors_match_case_insensitively() {
        assert!(f(&["a"]).matches("A"));
        assert!(f(&["b cam"]).matches("B"));
        assert!(f(&["CAM_B"]).matches("B"));
        assert!(f(&["a (arri alexa 35)"]).matches("A (ARRI ALEXA 35)"));
        assert!(!f(&["a (arri alexa 35)"]).matches("A (Canon EOS C70)"));
    }

    #[test]
    fn letter_selects_every_body() {
        let a = f(&["A"]);
        assert!(a.matches("A (ARRI ALEXA 35)"));
        assert!(a.matches("A (Canon EOS C70)"));
        assert!(!a.matches("B"));
    }

    #[test]
    fn empty_is_all() {
        let all = f(&[]);
        assert!(all.is_all());
        assert!(all.matches("Card 7"));
    }

    #[test]
    fn camera_letters() {
        assert_eq!(camera_letter("A (ARRI ALEXA 35)").as_deref(), Some("A"));
        assert_eq!(camera_letter("B_CAM").as_deref(), Some("B"));
        assert_eq!(camera_letter("Card 7"), None);
    }
}
