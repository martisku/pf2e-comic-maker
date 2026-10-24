pub mod assets;
pub use assets::{Asset, AssetKind, PanelAssets};

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
    #[serde(default)]
    pub expression: String,
    #[serde(default)]
    pub assets: PanelAssets,
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

pub const WORKSPACE_SCHEMA_VERSION: u32 = 2;

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Page {
    pub id: u32,
    pub layout: Layout,
    pub slots: Vec<Option<u32>>,
}

impl Page {
    pub fn change_layout(&mut self, layout: Layout) {
        self.layout = layout;
        self.slots.resize(layout.capacity(), None);
    }

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
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Chapter {
    pub id: u32,
    pub title: String,
    pub description: String,
    pub panel_ids: Vec<u32>,
    pub pages: Vec<Page>,
}

#[derive(Clone, Debug, PartialEq, Serialize)]
pub struct Workspace {
    pub schema_version: u32,
    pub title: String,
    pub chapters: Vec<Chapter>,
    pub panels: Vec<Panel>,
    pub assets: Vec<Asset>,
    pub next_panel_id: u32,
}

// Stored legacy documents are readable, but retain their original version until
// the database read boundary explicitly upgrades them. HTTP saves must reject it.
impl<'de> Deserialize<'de> for Workspace {
    fn deserialize<D: serde::Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        #[derive(Deserialize)]
        struct Wire {
            schema_version: Option<u32>,
            title: String,
            chapters: Option<Vec<Chapter>>,
            panels: Vec<Panel>,
            #[serde(default)]
            assets: Vec<Asset>,
            next_panel_id: Option<u32>,
            layout: Option<Layout>,
            slots: Option<Vec<Option<u32>>>,
        }
        let wire = Wire::deserialize(deserializer)?;
        let version = wire.schema_version.unwrap_or(1);
        if version != 1 && version != WORKSPACE_SCHEMA_VERSION {
            return Err(serde::de::Error::custom(
                "Unsupported workspace schema version.",
            ));
        }
        let (chapters, next_panel_id) = if version == WORKSPACE_SCHEMA_VERSION {
            (
                wire.chapters
                    .ok_or_else(|| serde::de::Error::custom("Missing chapters."))?,
                wire.next_panel_id
                    .ok_or_else(|| serde::de::Error::custom("Missing panel identity counter."))?,
            )
        } else {
            if wire.chapters.is_some() {
                return Err(serde::de::Error::custom(
                    "Chapter documents require the current schema version.",
                ));
            }
            let next = wire
                .panels
                .iter()
                .map(|p| p.id)
                .max()
                .unwrap_or(0)
                .checked_add(1)
                .ok_or_else(|| serde::de::Error::custom("Invalid panel identity."))?;
            (
                vec![Chapter {
                    id: 1,
                    title: "Chapter 1".into(),
                    description: String::new(),
                    panel_ids: wire.panels.iter().map(|p| p.id).collect(),
                    pages: vec![Page {
                        id: 1,
                        layout: wire
                            .layout
                            .ok_or_else(|| serde::de::Error::custom("Missing legacy layout."))?,
                        slots: wire
                            .slots
                            .ok_or_else(|| serde::de::Error::custom("Missing legacy slots."))?,
                    }],
                }],
                next,
            )
        };
        Ok(Self {
            schema_version: version,
            title: wire.title,
            chapters,
            panels: wire.panels,
            assets: wire.assets,
            next_panel_id,
        })
    }
}

impl Default for Workspace {
    fn default() -> Self {
        let panels = (1..=4)
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
                expression: String::new(),
                assets: PanelAssets::default(),
                images: vec![],
                selected_image: None,
                fill: true,
                crop_x: 50,
                crop_y: 50,
            })
            .collect();
        Self {
            schema_version: WORKSPACE_SCHEMA_VERSION,
            title: "The forest encounter".into(),
            chapters: vec![Chapter {
                id: 1,
                title: "Chapter 1".into(),
                description: String::new(),
                panel_ids: vec![1, 2, 3, 4],
                pages: vec![Page {
                    id: 1,
                    layout: Layout::Grid,
                    slots: vec![Some(1), Some(2), Some(3), Some(4)],
                }],
            }],
            panels,
            assets: vec![],
            next_panel_id: 5,
        }
    }
}

impl Workspace {
    pub fn blank(title: String) -> Self {
        let mut workspace = Self {
            title,
            ..Self::default()
        };
        for panel in &mut workspace.panels {
            panel.action.clear();
        }
        workspace
    }

    pub fn chapter(&self, id: u32) -> Option<&Chapter> {
        self.chapters.iter().find(|chapter| chapter.id == id)
    }

    pub fn chapter_mut(&mut self, id: u32) -> Option<&mut Chapter> {
        self.chapters.iter_mut().find(|chapter| chapter.id == id)
    }

    pub fn add_chapter(&mut self) -> Result<u32, String> {
        let id = self
            .chapters
            .iter()
            .map(|c| c.id)
            .max()
            .unwrap_or(0)
            .checked_add(1)
            .ok_or("No chapter identities remain.")?;
        self.chapters.push(Chapter {
            id,
            title: format!("Chapter {}", self.chapters.len() + 1),
            description: String::new(),
            panel_ids: vec![],
            pages: vec![Page {
                id: 1,
                layout: Layout::Grid,
                slots: vec![None; 4],
            }],
        });
        Ok(id)
    }

    pub fn remove_chapter(&mut self, id: u32) -> bool {
        let Some(index) = self.chapters.iter().position(|c| c.id == id) else {
            return false;
        };
        let chapter = self.chapters.remove(index);
        self.panels.retain(|p| !chapter.panel_ids.contains(&p.id));
        true
    }

    pub fn move_chapter(&mut self, id: u32, offset: isize) {
        if let Some(index) = self.chapters.iter().position(|c| c.id == id)
            && let Some(target) = index.checked_add_signed(offset)
            && target < self.chapters.len()
        {
            self.chapters.swap(index, target);
        }
    }

    pub fn add_panel(&mut self, chapter_id: u32) -> Result<u32, String> {
        if self.chapter(chapter_id).is_none() {
            return Err("Choose a chapter first.".into());
        }
        let id = self.next_panel_id;
        if id == 0 || id > i32::MAX as u32 {
            return Err("No panel identities remain.".into());
        }
        self.next_panel_id += 1;
        self.panels.push(Panel {
            id,
            title: format!("Panel {id}"),
            action: String::new(),
            dialogue: String::new(),
            expression: String::new(),
            assets: PanelAssets::default(),
            images: vec![],
            selected_image: None,
            fill: true,
            crop_x: 50,
            crop_y: 50,
        });
        self.chapter_mut(chapter_id)
            .expect("checked chapter")
            .panel_ids
            .push(id);
        Ok(id)
    }

    // Compatibility helpers for existing single-page model callers.
    pub fn change_layout(&mut self, layout: Layout) {
        if let Some(page) = self.chapters.first_mut().and_then(|c| c.pages.first_mut()) {
            page.change_layout(layout);
        }
    }
    pub fn place(&mut self, panel_id: u32, target: Option<usize>) {
        if let Some(chapter) = self
            .chapters
            .iter_mut()
            .find(|c| c.panel_ids.contains(&panel_id))
            && let Some(page) = chapter.pages.first_mut()
        {
            page.place(panel_id, target);
        }
    }

    pub fn validate(&self) -> Result<(), String> {
        use std::collections::HashSet;
        if self.schema_version != WORKSPACE_SCHEMA_VERSION {
            return Err("Reload the current application before saving this project.".into());
        }
        if self.title.len() > 500 {
            return Err("Project title is too long.".into());
        }
        if self.next_panel_id == 0 || self.next_panel_id > i32::MAX as u32 + 1 {
            return Err("Invalid panel identity counter.".into());
        }
        let mut panel_ids = HashSet::new();
        for panel in &self.panels {
            if panel.id == 0
                || panel.id >= self.next_panel_id
                || panel.id > i32::MAX as u32
                || !panel_ids.insert(panel.id)
                || panel.crop_x > 100
                || panel.crop_y > 100
            {
                return Err("Invalid panel identity or crop position.".into());
            }
            if panel.title.len() > 500
                || panel.action.len() > 20_000
                || panel.dialogue.len() > 20_000
                || panel.expression.len() > 20_000
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
        let mut chapters = HashSet::new();
        let mut owned = HashSet::new();
        for chapter in &self.chapters {
            if chapter.id == 0
                || !chapters.insert(chapter.id)
                || chapter.title.trim().is_empty()
                || chapter.title.len() > 500
                || chapter.description.len() > 20_000
            {
                return Err("Invalid chapter identity or details.".into());
            }
            for id in &chapter.panel_ids {
                if !panel_ids.contains(id) || !owned.insert(*id) {
                    return Err("Invalid chapter panel ownership.".into());
                }
            }
            if chapter.pages.is_empty() {
                return Err("Each chapter must have a composition page.".into());
            }
            let mut pages = HashSet::new();
            let mut placed = HashSet::new();
            for page in &chapter.pages {
                if page.id == 0
                    || !pages.insert(page.id)
                    || page.slots.len() != page.layout.capacity()
                {
                    return Err("Invalid chapter page.".into());
                }
                for id in page.slots.iter().flatten() {
                    if !chapter.panel_ids.contains(id) || !placed.insert(*id) {
                        return Err("Invalid or duplicate panel placement.".into());
                    }
                }
            }
        }
        if owned != panel_ids {
            return Err("Every panel must belong to a chapter.".into());
        }
        self.validate_assets()?;
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
        Snapshot {
            revision: self.revision,
            workspace: self.workspace,
        }
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
    fn legacy_document_preserves_images_and_requires_explicit_read_upgrade() {
        let mut original = Workspace::default();
        original.panels[0].images.push(PanelImage {
            id: "image-id".into(),
            url: "/api/images/image-id".into(),
        });
        original.panels[0].selected_image = Some("image-id".into());
        let legacy = serde_json::json!({ "title": original.title, "panels": original.panels,
            "layout": "Grid", "slots": [1, 2, 3, 4] });
        let mut decoded: Workspace = serde_json::from_value(legacy).unwrap();
        assert_eq!(decoded.schema_version, 1);
        assert!(decoded.validate().is_err());
        decoded.schema_version = WORKSPACE_SCHEMA_VERSION;
        assert_eq!(decoded, original);
        decoded.validate().unwrap();
        let encoded = serde_json::to_value(&decoded).unwrap();
        assert!(encoded.get("layout").is_none());
        assert_eq!(
            serde_json::from_value::<Workspace>(encoded).unwrap(),
            decoded
        );
    }

    #[test]
    fn chapter_order_deletion_and_empty_roundtrip_preserve_identity() {
        let mut workspace = Workspace::default();
        workspace.add_panel(1).unwrap();
        let second = workspace.add_chapter().unwrap();
        let retained = workspace.add_panel(second).unwrap();
        workspace.chapter_mut(second).unwrap().title = "The return".into();
        workspace.chapter_mut(second).unwrap().description = "After the encounter".into();
        workspace.move_chapter(second, -1);
        assert_eq!(workspace.chapters[0].id, second);
        assert_eq!(workspace.chapter(1).unwrap().panel_ids.len(), 5);
        let before = workspace
            .panels
            .iter()
            .find(|p| p.id == retained)
            .unwrap()
            .clone();
        assert!(workspace.remove_chapter(1));
        assert_eq!(workspace.panels, vec![before]);
        assert!(workspace.add_panel(second).unwrap() > retained);
        workspace.validate().unwrap();
        assert_eq!(
            serde_json::from_str::<Workspace>(&serde_json::to_string(&workspace).unwrap()).unwrap(),
            workspace
        );
        workspace.remove_chapter(second);
        let empty: Workspace =
            serde_json::from_str(&serde_json::to_string(&workspace).unwrap()).unwrap();
        assert!(empty.chapters.is_empty());
        assert!(empty.panels.is_empty());
        empty.validate().unwrap();
        let new_chapter = workspace.add_chapter().unwrap();
        assert!(workspace.add_panel(new_chapter).unwrap() > retained);
    }

    #[test]
    fn chapter_deletion_preserves_shared_assets_and_other_panel_associations() {
        let mut workspace = Workspace::default();
        let asset_id = "4fd2c62e-c2ce-4c27-bb78-e527a9ff34cd".to_string();
        workspace.assets.push(Asset {
            id: asset_id.clone(),
            kind: AssetKind::Character,
            name: "Wick".into(),
            description: "Shared hero".into(),
            design_notes: "Green cloak".into(),
            character_size: None,
        });
        workspace.panels[0].assets.characters.push(asset_id.clone());
        let retained_chapter = workspace.add_chapter().unwrap();
        let retained_panel = workspace.add_panel(retained_chapter).unwrap();
        workspace
            .panels
            .iter_mut()
            .find(|p| p.id == retained_panel)
            .unwrap()
            .assets
            .characters
            .push(asset_id.clone());
        let assets = workspace.assets.clone();
        workspace.remove_chapter(1);
        assert_eq!(workspace.assets, assets);
        assert_eq!(
            workspace.asset_users(&asset_id),
            vec![(retained_panel, format!("Panel {retained_panel}"))]
        );
        assert!(workspace.remove_asset(&asset_id).is_err());
        workspace.validate().unwrap();
        workspace.remove_chapter(retained_chapter);
        assert_eq!(workspace.assets, assets);
        workspace.remove_asset(&asset_id).unwrap();
    }

    #[test]
    fn chapters_require_a_composition_page() {
        let mut workspace = Workspace::default();
        let page = workspace.chapters[0].pages.pop().unwrap();
        assert!(workspace.validate().is_err());
        workspace.chapters[0].pages.push(page.clone());
        workspace.validate().unwrap();
        workspace.chapters[0].pages.push(Page {
            id: 2,
            slots: vec![None; 4],
            ..page
        });
        workspace.validate().unwrap();
    }

    #[test]
    fn ownership_and_placement_are_checked_across_chapters_and_pages() {
        let mut workspace = Workspace::default();
        let second = workspace.add_chapter().unwrap();
        workspace.chapter_mut(second).unwrap().pages[0].slots[0] = Some(1);
        assert!(workspace.validate().is_err());
        workspace.chapter_mut(second).unwrap().pages[0].slots[0] = None;
        workspace.chapter_mut(second).unwrap().panel_ids.push(1);
        assert!(workspace.validate().is_err());
        workspace.chapter_mut(second).unwrap().panel_ids.clear();
        let mut page = workspace.chapters[0].pages[0].clone();
        page.id = 2;
        workspace.chapters[0].pages.push(page);
        assert!(workspace.validate().is_err());
        workspace.chapters[0].pages.pop();
        workspace.panels[0].id = 0;
        assert!(workspace.validate().is_err());
    }

    #[test]
    fn future_or_incomplete_schema_is_rejected_and_panel_identity_exhaustion_is_safe() {
        let mut encoded = serde_json::to_value(Workspace::default()).unwrap();
        encoded["schema_version"] = 99.into();
        assert!(serde_json::from_value::<Workspace>(encoded.clone()).is_err());
        encoded["schema_version"] = WORKSPACE_SCHEMA_VERSION.into();
        encoded.as_object_mut().unwrap().remove("chapters");
        assert!(serde_json::from_value::<Workspace>(encoded).is_err());
        let mut workspace = Workspace {
            next_panel_id: i32::MAX as u32,
            ..Workspace::default()
        };
        assert_eq!(workspace.add_panel(1).unwrap(), i32::MAX as u32);
        assert!(workspace.add_panel(1).is_err());
        workspace.validate().unwrap();
    }

    #[test]
    fn blank_project_has_no_sample_story_or_images() {
        let workspace = Workspace::blank("My project".into());
        assert_eq!(workspace.title, "My project");
        assert!(workspace.panels.iter().all(|panel| panel.action.is_empty()
            && panel.dialogue.is_empty()
            && panel.images.is_empty()));
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
        assert_eq!(workspace.chapters[0].pages[0].slots, vec![Some(1)]);
        workspace.validate().unwrap();
    }

    #[test]
    fn swapping_slots_preserves_unique_placement() {
        let mut workspace = Workspace::default();
        workspace.place(1, Some(2));
        assert_eq!(
            workspace.chapters[0].pages[0].slots,
            vec![Some(3), Some(2), Some(1), Some(4)]
        );
        workspace.place(1, None);
        assert_eq!(workspace.chapters[0].pages[0].slots[2], None);
        workspace.validate().unwrap();
    }
}
