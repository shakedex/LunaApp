//! Editing state for the Settings window: a draft of [`Settings`] next to the
//! last saved copy, with custom-field add/remove/reorder and save validation.

use luna_core::settings::Settings;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SettingsEditor {
    saved: Settings,
    draft: Settings,
}

impl SettingsEditor {
    pub fn new(saved: Settings) -> Self {
        Self {
            draft: saved.clone(),
            saved,
        }
    }

    pub fn draft(&self) -> &Settings {
        &self.draft
    }

    pub fn draft_mut(&mut self) -> &mut Settings {
        &mut self.draft
    }

    pub fn is_dirty(&self) -> bool {
        self.draft != self.saved
    }

    /// Throws away unsaved edits.
    pub fn revert(&mut self) {
        self.draft = self.saved.clone();
    }

    pub fn add_field(&mut self) {
        self.draft.custom.push((String::new(), String::new()));
    }

    pub fn remove_field(&mut self, i: usize) {
        if i < self.draft.custom.len() {
            self.draft.custom.remove(i);
        }
    }

    pub fn move_up(&mut self, i: usize) {
        if i > 0 && i < self.draft.custom.len() {
            self.draft.custom.swap(i - 1, i);
        }
    }

    pub fn move_down(&mut self, i: usize) {
        if i + 1 < self.draft.custom.len() {
            self.draft.custom.swap(i, i + 1);
        }
    }

    pub fn set_label(&mut self, i: usize, label: &str) {
        if let Some(f) = self.draft.custom.get_mut(i) {
            f.0 = label.to_string();
        }
    }

    pub fn set_value(&mut self, i: usize, value: &str) {
        if let Some(f) = self.draft.custom.get_mut(i) {
            f.1 = value.to_string();
        }
    }

    /// The draft as it will be saved: text trimmed, fully empty custom rows
    /// dropped. Fails when a custom value has no label.
    pub fn validated(&self) -> Result<Settings, String> {
        let mut s = self.draft.clone();
        for t in [&mut s.production, &mut s.company, &mut s.author] {
            *t = t.trim().to_string();
        }
        s.custom = s
            .custom
            .into_iter()
            .map(|(l, v)| (l.trim().to_string(), v.trim().to_string()))
            .filter(|(l, v)| !(l.is_empty() && v.is_empty()))
            .collect();
        if s.custom.iter().any(|(l, _)| l.is_empty()) {
            return Err("Give every custom field a label, or remove it.".into());
        }
        Ok(s)
    }

    /// [`Self::validated`], with the settings that save the moment they're
    /// switched (theme, source-drive copy, problems, colour) taken from
    /// `current`, so saving the draft never undoes them.
    pub fn validated_over(&self, current: &Settings) -> Result<Settings, String> {
        self.validated().map(|s| Settings {
            report_theme: current.report_theme,
            copy_to_source: current.copy_to_source,
            show_problems: current.show_problems,
            colour: current.colour,
            lut: current.lut.clone(),
            camera_look: current.camera_look,
            recent_luts: current.recent_luts.clone(),
            ..s
        })
    }

    /// Records `saved` (what the store accepted) as the new baseline.
    pub fn mark_saved(&mut self, saved: Settings) {
        self.draft = saved.clone();
        self.saved = saved;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn save_keeps_what_was_switched_while_editing() {
        use luna_core::settings::{ReportTheme, ThumbnailColour};
        let mut ed = SettingsEditor::new(Settings::default());
        ed.draft_mut().production = "Night Shift".into();
        // Switched (and saved) while the draft was open.
        let current = Settings {
            report_theme: ReportTheme::Dark,
            copy_to_source: true,
            show_problems: true,
            colour: ThumbnailColour::Lut,
            lut: Some("D:/show.cube".into()),
            camera_look: true,
            recent_luts: vec!["D:/show.cube".into(), "D:/old.cube".into()],
            ..Settings::default()
        };
        let s = ed.validated_over(&current).unwrap();
        assert_eq!(s.production, "Night Shift");
        assert_eq!(s.report_theme, ReportTheme::Dark);
        assert!(s.copy_to_source && s.show_problems && s.camera_look);
        assert_eq!(s.colour, ThumbnailColour::Lut);
        assert_eq!(s.lut, current.lut);
        assert_eq!(s.recent_luts, current.recent_luts);
    }

    fn with_fields(fields: &[(&str, &str)]) -> SettingsEditor {
        SettingsEditor::new(Settings {
            custom: fields
                .iter()
                .map(|(l, v)| (l.to_string(), v.to_string()))
                .collect(),
            ..Settings::default()
        })
    }

    fn labels(e: &SettingsEditor) -> Vec<&str> {
        e.draft().custom.iter().map(|(l, _)| l.as_str()).collect()
    }

    #[test]
    fn a_fresh_editor_is_clean() {
        assert!(!with_fields(&[("Camera", "A")]).is_dirty());
    }

    #[test]
    fn adding_a_field_appends_an_empty_row_and_dirties() {
        let mut e = with_fields(&[("Camera", "A")]);
        e.add_field();
        assert_eq!(labels(&e), ["Camera", ""]);
        assert!(e.is_dirty());
    }

    #[test]
    fn removing_a_field_drops_that_row() {
        let mut e = with_fields(&[("Camera", "A"), ("Location", "Dock 4"), ("Episode", "3")]);
        e.remove_field(1);
        assert_eq!(labels(&e), ["Camera", "Episode"]);
        e.remove_field(9);
        assert_eq!(labels(&e), ["Camera", "Episode"]);
    }

    #[test]
    fn fields_move_up_and_down_within_bounds() {
        let mut e = with_fields(&[("A", ""), ("B", ""), ("C", "")]);
        e.move_up(2);
        assert_eq!(labels(&e), ["A", "C", "B"]);
        e.move_down(0);
        assert_eq!(labels(&e), ["C", "A", "B"]);
        e.move_up(0);
        e.move_down(2);
        assert_eq!(labels(&e), ["C", "A", "B"]);
    }

    #[test]
    fn editing_label_and_value_updates_the_draft_only() {
        let mut e = with_fields(&[("Camera", "A")]);
        e.set_label(0, "Unit");
        e.set_value(0, "Second unit");
        assert_eq!(
            e.draft().custom,
            vec![("Unit".to_string(), "Second unit".to_string())]
        );
        assert!(e.is_dirty());
        e.revert();
        assert!(!e.is_dirty());
    }

    #[test]
    fn validation_trims_and_drops_empty_rows() {
        let mut e = with_fields(&[("  Camera ", " A "), ("", "  ")]);
        e.draft_mut().production = "  Night Shoot ".into();
        let s = e.validated().unwrap();
        assert_eq!(s.production, "Night Shoot");
        assert_eq!(s.custom, vec![("Camera".to_string(), "A".to_string())]);
    }

    #[test]
    fn a_value_without_a_label_is_rejected() {
        let e = with_fields(&[("", "Dock 4")]);
        assert!(e.validated().unwrap_err().contains("label"));
    }

    #[test]
    fn marking_saved_makes_the_editor_clean_again() {
        let mut e = with_fields(&[]);
        e.add_field();
        e.set_label(0, "Camera");
        let s = e.validated().unwrap();
        e.mark_saved(s);
        assert!(!e.is_dirty());
        assert_eq!(labels(&e), ["Camera"]);
    }
}
