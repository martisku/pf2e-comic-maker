use serde::{Deserialize, Serialize};

pub const MAX_IMAGE_BYTES: usize = 20_000_000;
pub const ORIGINAL_PROJECT_ID: &str = "00000000-0000-4000-8000-000000000001";

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct PanelImage {
    pub id: String,
    pub url: String,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Panel {
    pub id: u32,
    pub title: String,
    pub action: String,
    pub dialogue: String,
    pub images: Vec<PanelImage>,
    pub selected_image: Option<String>,
    pub fill: bool,
    pub crop_x: u8,
    pub crop_y: u8,
}

impl Panel {
    pub fn image_url(&self) -> Option<&str> {
        self.images
            .iter()
            .find(|image| Some(&image.id) == self.selected_image.as_ref())
            .map(|image| image.url.as_str())
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
pub enum Layout {
    Full,
    Grid,
    Wide,
}

impl Layout {
    pub fn capacity(self) -> usize {
        match self {
            Self::Full => 1,
            Self::Grid => 4,
            Self::Wide => 3,
        }
    }
    pub fn css_class(self) -> &'static str {
        match self {
            Self::Full => "full",
            Self::Grid => "grid",
            Self::Wide => "wide",
        }
    }
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Workspace {
    pub title: String,
    pub layout: Layout,
    pub slots: Vec<Option<u32>>,
    pub panels: Vec<Panel>,
}

impl Default for Workspace {
    fn default() -> Self {
        Self {
            title: "The forest encounter".into(),
            layout: Layout::Grid,
            slots: vec![Some(1), Some(2), Some(3), Some(4)],
            panels: (1..=4)
                .map(|id| Panel {
                    id,
                    title: format!("Panel {id}"),
                    action: [
                        "Wick enters the forest.",
                        "A sound interrupts the silence.",
                        "Something moves between the trees.",
                        "Wick reaches for his sword.",
                    ][id as usize - 1]
                        .into(),
                    dialogue: String::new(),
                    images: vec![],
                    selected_image: None,
                    fill: true,
                    crop_x: 50,
                    crop_y: 50,
                })
                .collect(),
        }
    }
}

impl Workspace {
    pub fn blank(title: String) -> Self {
        let mut workspace = Self { title, ..Self::default() };
        for panel in &mut workspace.panels {
            panel.action.clear();
        }
        workspace
    }

    pub fn change_layout(&mut self, layout: Layout) {
        self.layout = layout;
        self.slots.resize(layout.capacity(), None);
    }

    /// Swap with an occupied position; removal leaves the panel unplaced.
    pub fn place(&mut self, panel_id: u32, target: Option<usize>) {
        let source = self.slots.iter().position(|id| *id == Some(panel_id));
        if let Some(target) = target {
            if target >= self.slots.len() {
                return;
            }
            let displaced = self.slots[target];
            if let Some(source) = source {
                self.slots[source] = displaced;
            }
            self.slots[target] = Some(panel_id);
        } else if let Some(source) = source {
            self.slots[source] = None;
        }
    }

    pub fn validate(&self) -> Result<(), String> {
        if self.panels.len() != 4 || self.slots.len() != self.layout.capacity() {
            return Err("The prototype requires four panels and a valid layout.".into());
        }
        if self.title.len() > 500 {
            return Err("Project title is too long.".into());
        }
        for (i, panel) in self.panels.iter().enumerate() {
            if panel.id != i as u32 + 1 || panel.crop_x > 100 || panel.crop_y > 100 {
                return Err("Invalid panel identity or crop position.".into());
            }
            if panel.title.len() > 500
                || panel.action.len() > 20_000
                || panel.dialogue.len() > 20_000
            {
                return Err("Panel text is too long.".into());
            }
            if panel
                .selected_image
                .as_ref()
                .is_some_and(|selected| !panel.images.iter().any(|image| &image.id == selected))
            {
                return Err("Selected image does not belong to this panel.".into());
            }
        }
        let mut seen = Vec::new();
        for id in self.slots.iter().flatten() {
            if !self.panels.iter().any(|p| p.id == *id) || seen.contains(id) {
                return Err("Invalid or duplicate panel placement.".into());
            }
            seen.push(*id);
        }
        Ok(())
    }
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Snapshot {
    pub revision: u64,
    pub workspace: Workspace,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct SaveRequest {
    pub revision: u64,
    pub workspace: Workspace,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct ProjectSnapshot {
    pub project_id: String,
    pub description: String,
    pub revision: u64,
    pub workspace: Workspace,
}

impl ProjectSnapshot {
    pub fn legacy(self) -> Snapshot {
        Snapshot { revision: self.revision, workspace: self.workspace }
    }
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct ProjectCreate {
    pub project_id: String,
    pub name: String,
    pub description: String,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct ProjectSummary {
    pub project_id: String,
    pub name: String,
}

pub fn validate_project_metadata(name: &str, description: &str) -> Result<(), &'static str> {
    if name.trim().is_empty() || name.len() > 500 {
        return Err("Project name must contain text and be no longer than 500 bytes.");
    }
    if description.len() > 20_000 {
        return Err("Project description must be no longer than 20,000 bytes.");
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn blank_project_has_no_sample_story_or_images() {
        let workspace = Workspace::blank("My project".into());
        assert_eq!(workspace.title, "My project");
        assert!(workspace.panels.iter().all(|panel| panel.action.is_empty()
            && panel.dialogue.is_empty() && panel.images.is_empty()));
        workspace.validate().unwrap();
    }

    #[test]
    fn metadata_requires_a_name_and_enforces_utf8_byte_limits() {
        assert!(validate_project_metadata("  \n", "").is_err());
        assert!(validate_project_metadata(&"é".repeat(250), &"é".repeat(10_000)).is_ok());
        assert!(validate_project_metadata(&"é".repeat(251), "").is_err());
        assert!(validate_project_metadata("Project", &"é".repeat(10_001)).is_err());
    }

    #[test]
    fn smaller_layout_preserves_panels_and_their_images() {
        let mut workspace = Workspace::default();
        let before = workspace.panels.clone();
        workspace.change_layout(Layout::Full);
        assert_eq!(workspace.panels, before);
        assert_eq!(workspace.slots, vec![Some(1)]);
        workspace.validate().unwrap();
    }

    #[test]
    fn swapping_slots_preserves_unique_placement() {
        let mut workspace = Workspace::default();
        workspace.place(1, Some(2));
        assert_eq!(workspace.slots, vec![Some(3), Some(2), Some(1), Some(4)]);
        workspace.place(1, None);
        assert_eq!(workspace.slots[2], None);
        workspace.validate().unwrap();
    }
}
