use crate::{Layout, Page, Workspace};

impl Workspace {
    pub fn reading_order(&self, chapter: u32) -> Vec<u32> {
        self.chapter(chapter)
            .map(|chapter| {
                chapter
                    .pages
                    .iter()
                    .flat_map(|p| p.slots.iter().flatten().copied())
                    .collect()
            })
            .unwrap_or_default()
    }

    pub fn add_page(&mut self, chapter: u32) -> Result<u32, String> {
        let chapter = self.chapter_mut(chapter).ok_or("Choose a chapter first.")?;
        let id = chapter
            .pages
            .iter()
            .map(|p| p.id)
            .max()
            .unwrap_or(0)
            .checked_add(1)
            .ok_or("No page identities remain.")?;
        chapter.pages.push(Page {
            id,
            layout: Layout::Grid,
            slots: vec![None; 4],
        });
        Ok(id)
    }

    pub fn remove_page(&mut self, chapter: u32, page: u32) -> bool {
        let Some(chapter) = self.chapter_mut(chapter) else {
            return false;
        };
        let count = chapter.pages.len();
        chapter.pages.retain(|p| p.id != page);
        chapter.pages.len() != count
    }

    pub fn move_page(&mut self, chapter: u32, page: u32, offset: isize) {
        if let Some(chapter) = self.chapter_mut(chapter)
            && let Some(index) = chapter.pages.iter().position(|p| p.id == page)
            && let Some(target) = index.checked_add_signed(offset)
            && target < chapter.pages.len()
        {
            chapter.pages.swap(index, target);
        }
    }

    pub fn remove_panel(&mut self, panel: u32) -> bool {
        let count = self.panels.len();
        self.panels.retain(|p| p.id != panel);
        for chapter in &mut self.chapters {
            chapter.panel_ids.retain(|id| *id != panel);
            for page in &mut chapter.pages {
                for slot in &mut page.slots {
                    if *slot == Some(panel) {
                        *slot = None;
                    }
                }
            }
        }
        self.panels.len() != count
    }

    pub fn move_panel(&mut self, panel: u32, destination: u32) -> Result<(), String> {
        if self.chapter(destination).is_none() {
            return Err("Choose a destination chapter.".into());
        }
        let source = self
            .chapters
            .iter()
            .position(|c| c.panel_ids.contains(&panel))
            .ok_or("Panel does not belong to this project.")?;
        if self.chapters[source].id == destination {
            return Ok(());
        }
        let source = &mut self.chapters[source];
        source.panel_ids.retain(|id| *id != panel);
        for page in &mut source.pages {
            for slot in &mut page.slots {
                if *slot == Some(panel) {
                    *slot = None;
                }
            }
        }
        self.chapter_mut(destination)
            .expect("checked destination")
            .panel_ids
            .push(panel);
        Ok(())
    }

    pub fn place_panel(
        &mut self,
        chapter: u32,
        panel: u32,
        target: Option<(u32, usize)>,
    ) -> Result<(), String> {
        let chapter = self.chapter_mut(chapter).ok_or("Choose a chapter first.")?;
        if !chapter.panel_ids.contains(&panel) {
            return Err("Panel does not belong to this chapter.".into());
        }
        let destination = match target {
            Some((id, slot)) => {
                let index = chapter
                    .pages
                    .iter()
                    .position(|p| p.id == id)
                    .ok_or("Choose a page.")?;
                if slot >= chapter.pages[index].slots.len() {
                    return Err("Choose a valid page position.".into());
                }
                Some((index, slot))
            }
            None => None,
        };
        let source = chapter.pages.iter().enumerate().find_map(|(page, p)| {
            p.slots
                .iter()
                .position(|id| *id == Some(panel))
                .map(|slot| (page, slot))
        });
        if source == destination {
            return Ok(());
        }
        let displaced = destination.and_then(|(page, slot)| chapter.pages[page].slots[slot]);
        if let Some((page, slot)) = source {
            chapter.pages[page].slots[slot] = displaced;
        }
        if let Some((page, slot)) = destination {
            chapter.pages[page].slots[slot] = Some(panel);
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{PanelImage, WORKSPACE_SCHEMA_VERSION};

    #[test]
    fn panel_move_and_deletion_preserve_other_data_and_identity_counter() {
        let mut workspace = Workspace::default();
        let destination = workspace.add_chapter().unwrap();
        let panel = &mut workspace.panels[0];
        panel.framing = "Close-up".into();
        panel.mood = "Tense".into();
        panel.production_notes = "Keep cloak visible".into();
        panel.images.push(PanelImage {
            id: "original".into(),
            url: "/image".into(),
        });
        panel.selected_image = Some("original".into());
        let retained = panel.clone();
        let counter = workspace.next_panel_id;
        let before = workspace.clone();
        assert!(workspace.move_panel(1, 999).is_err());
        assert_eq!(workspace, before);
        workspace.move_panel(1, destination).unwrap();
        assert_eq!(workspace.panels[0], retained);
        assert_eq!(workspace.next_panel_id, counter);
        assert!(!workspace.reading_order(1).contains(&1));
        assert!(workspace.reading_order(destination).is_empty());
        workspace.validate().unwrap();
        assert!(workspace.remove_panel(1));
        assert_eq!(workspace.next_panel_id, counter);
        assert_eq!(workspace.panels.len(), 3);
        workspace.validate().unwrap();
    }

    #[test]
    fn page_order_cross_page_swaps_and_last_page_removal_are_lossless() {
        let mut workspace = Workspace::default();
        let second = workspace.add_page(1).unwrap();
        workspace.place_panel(1, 1, Some((second, 0))).unwrap();
        workspace.place_panel(1, 2, Some((second, 0))).unwrap();
        assert_eq!(workspace.chapters[0].pages[0].slots[1], Some(1));
        assert_eq!(workspace.reading_order(1), vec![1, 3, 4, 2]);
        workspace.move_page(1, second, -1);
        assert_eq!(workspace.reading_order(1), vec![2, 1, 3, 4]);
        let unchanged = workspace.clone();
        assert!(workspace.place_panel(1, 2, Some((second, 99))).is_err());
        assert_eq!(workspace, unchanged);
        let draft = workspace.add_panel(1).unwrap();
        workspace.place_panel(1, draft, Some((second, 0))).unwrap();
        assert!(!workspace.reading_order(1).contains(&2));
        assert!(workspace.reading_order(1).contains(&draft));
        workspace.place_panel(1, draft, None).unwrap();
        assert!(!workspace.reading_order(1).contains(&draft));
        let panels = workspace.panels.clone();
        workspace.remove_page(1, second);
        workspace.remove_page(1, 1);
        assert_eq!(workspace.panels, panels);
        assert!(workspace.reading_order(1).is_empty());
        workspace.validate().unwrap();
        workspace.add_page(1).unwrap();
        workspace.place_panel(1, 2, Some((1, 0))).unwrap();
        assert_eq!(workspace.reading_order(1), vec![2]);
    }

    #[test]
    fn planning_fields_and_legacy_v2_read_without_silent_write_upgrade() {
        let mut workspace = Workspace::default();
        workspace.panels[0].framing = "Wide".into();
        workspace.panels[0].mood = "Quiet".into();
        workspace.panels[0].production_notes = "No lettering".into();
        let value = serde_json::to_value(&workspace).unwrap();
        assert_eq!(
            serde_json::from_value::<Workspace>(value.clone()).unwrap(),
            workspace
        );
        let mut legacy = value;
        legacy["schema_version"] = 2.into();
        for panel in legacy["panels"].as_array_mut().unwrap() {
            let panel = panel.as_object_mut().unwrap();
            panel.remove("framing");
            panel.remove("mood");
            panel.remove("production_notes");
        }
        let mut decoded: Workspace = serde_json::from_value(legacy).unwrap();
        assert_eq!(decoded.schema_version, 2);
        assert!(decoded.validate().is_err());
        assert!(decoded.panels[0].framing.is_empty());
        decoded.schema_version = WORKSPACE_SCHEMA_VERSION;
        decoded.validate().unwrap();
        decoded.panels[0].mood = "x".repeat(20_001);
        assert!(decoded.validate().is_err());
    }
}
