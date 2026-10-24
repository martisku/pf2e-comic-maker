use crate::Workspace;
use serde::{Deserialize, Serialize};
use std::collections::HashSet;

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum AssetKind {
    Character,
    Location,
    Equipment,
}

impl AssetKind {
    pub fn label(self) -> &'static str {
        match self {
            Self::Character => "Character",
            Self::Location => "Location",
            Self::Equipment => "Equipment",
        }
    }
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Asset {
    pub id: String,
    pub kind: AssetKind,
    pub name: String,
    pub description: String,
    pub design_notes: String,
    pub character_size: Option<String>,
}

#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct PanelAssets {
    pub characters: Vec<String>,
    pub location: Option<String>,
    pub equipment: Vec<String>,
}

impl PanelAssets {
    pub fn contains(&self, id: &str) -> bool {
        self.characters
            .iter()
            .chain(&self.equipment)
            .any(|value| value == id)
            || self.location.as_deref() == Some(id)
    }
}

fn valid_id(id: &str) -> bool {
    id.len() == 36
        && id.bytes().enumerate().all(|(index, byte)| {
            if [8, 13, 18, 23].contains(&index) {
                byte == b'-'
            } else {
                byte.is_ascii_digit() || (b'a'..=b'f').contains(&byte)
            }
        })
}

impl Workspace {
    pub fn asset_users(&self, id: &str) -> Vec<(u32, String)> {
        self.panels
            .iter()
            .filter(|panel| panel.assets.contains(id))
            .map(|panel| (panel.id, panel.title.clone()))
            .collect()
    }

    pub fn remove_asset(&mut self, id: &str) -> Result<(), String> {
        if !self.asset_users(id).is_empty() {
            return Err("Remove this asset from its panels before deleting it.".into());
        }
        if !self.assets.iter().any(|asset| asset.id == id) {
            return Err("This asset is no longer in the project.".into());
        }
        self.assets.retain(|asset| asset.id != id);
        Ok(())
    }

    pub fn validate_assets(&self) -> Result<(), String> {
        if self.assets.len() > 200 {
            return Err("A project supports up to 200 assets.".into());
        }
        let mut ids = HashSet::new();
        for asset in &self.assets {
            if !valid_id(&asset.id) || !ids.insert(asset.id.as_str()) {
                return Err("Invalid or duplicate asset identity.".into());
            }
            if asset.name.trim().is_empty()
                || asset.name.len() > 500
                || asset.description.len() > 20_000
                || asset.design_notes.len() > 20_000
                || asset
                    .character_size
                    .as_ref()
                    .is_some_and(|size| size.len() > 500)
                || (asset.kind != AssetKind::Character && asset.character_size.is_some())
            {
                return Err("Invalid asset name, text or character size.".into());
            }
        }
        for panel in &self.panels {
            let mut selected = HashSet::new();
            for (ids, kind) in [
                (panel.assets.characters.as_slice(), AssetKind::Character),
                (panel.assets.equipment.as_slice(), AssetKind::Equipment),
                (panel.assets.location.as_slice(), AssetKind::Location),
            ] {
                for id in ids {
                    if !selected.insert(id)
                        || !self
                            .assets
                            .iter()
                            .any(|asset| asset.id == *id && asset.kind == kind)
                    {
                        return Err(
                            "Panel assets must be unique and match an asset type in this project."
                                .into(),
                        );
                    }
                }
            }
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    const ID: &str = "4fd2c62e-c2ce-4c27-bb78-e527a9ff34cd";

    fn workspace() -> Workspace {
        let mut workspace = Workspace::default();
        workspace.assets.push(Asset {
            id: ID.into(),
            kind: AssetKind::Character,
            name: "Wick".into(),
            description: "A hero".into(),
            design_notes: "Green cloak".into(),
            character_size: Some("Small".into()),
        });
        workspace.panels[0].images = vec![
            crate::PanelImage {
                id: "first".into(),
                url: "/images/first".into(),
            },
            crate::PanelImage {
                id: "second".into(),
                url: "/images/second".into(),
            },
        ];
        workspace.panels[0].selected_image = Some("second".into());
        workspace.panels[0].assets.characters.push(ID.into());
        workspace.panels[1].assets.characters.push(ID.into());
        workspace
    }

    #[test]
    fn shared_edits_preserve_identity_associations_and_images() {
        let mut workspace = workspace();
        let panels = workspace.panels.clone();
        workspace.assets[0].design_notes = "Blue cloak".into();
        assert_eq!(workspace.panels, panels);
        assert_eq!(workspace.asset_users(ID).len(), 2);
        workspace.validate_assets().unwrap();
    }

    #[test]
    fn deletion_requires_removing_every_association() {
        let mut workspace = workspace();
        assert!(workspace.remove_asset(ID).is_err());
        workspace.panels[0].assets.characters.clear();
        assert_eq!(
            workspace.asset_users(ID),
            vec![(workspace.panels[1].id, workspace.panels[1].title.clone())]
        );
        assert!(workspace.remove_asset(ID).is_err());
        workspace.panels[1].assets.characters.clear();
        workspace.remove_asset(ID).unwrap();
        assert!(workspace.assets.is_empty());
    }

    #[test]
    fn dangling_foreign_wrong_kind_and_duplicate_references_are_rejected() {
        let workspace = workspace();
        let mut foreign = Workspace::default();
        foreign.panels[0].assets = workspace.panels[0].assets.clone();
        assert!(foreign.validate_assets().is_err());
        let mut wrong_kind = workspace.clone();
        wrong_kind.panels[0].assets.characters.clear();
        wrong_kind.panels[0].assets.location = Some(ID.into());
        assert!(wrong_kind.validate_assets().is_err());
        let mut duplicate = workspace.clone();
        duplicate.panels[0].assets.characters.push(ID.into());
        assert!(duplicate.validate_assets().is_err());
        let mut deleted = workspace;
        deleted.assets.clear();
        assert!(deleted.validate_assets().is_err());
    }

    #[test]
    fn rejects_bad_id_duplicate_assets_and_invalid_text() {
        let original = workspace();
        let mut workspace = original.clone();
        workspace.assets[0].id = "not-an-id".into();
        assert!(workspace.validate_assets().is_err());
        let mut workspace = original.clone();
        workspace.assets.push(workspace.assets[0].clone());
        assert!(workspace.validate_assets().is_err());
        let mut workspace = original.clone();
        workspace.assets[0].name = "  ".into();
        assert!(workspace.validate_assets().is_err());
        let mut workspace = original;
        workspace.assets[0].character_size = None;
        workspace.validate_assets().unwrap();
        workspace.assets[0].description = "x".repeat(20_001);
        assert!(workspace.validate_assets().is_err());
    }
}
