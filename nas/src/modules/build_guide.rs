use std::path::PathBuf;
use std::fs;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::error::Error;

#[derive(Serialize, Deserialize, Clone)]
pub struct GroupData {
    pub buildings: Vec<String>,
    pub units: Vec<String>,
    pub description: String,
}

#[derive(Serialize, Deserialize, Clone)]
pub struct BuildGuide {
    pub clan: String,
    pub lore_order: Vec<String>,
    pub groups: HashMap<String, GroupData>,
}

pub struct BuildGuideManager {
    guides: HashMap<String, BuildGuide>,
    guides_dir: PathBuf,
}

impl BuildGuideManager {
    pub fn new() -> Result<Self, Box<dyn Error>> {
        let mut guides_dir = dirs::document_dir()
            .ok_or("Could not find Documents directory")?;
        guides_dir.push("NgAssistant");
        guides_dir.push("Guides");

        // Create directories if they don't exist
        fs::create_dir_all(&guides_dir)?;

        let mut manager = Self {
            guides: HashMap::new(),
            guides_dir,
        };
        manager.load_guides()?;

        Ok(manager)
    }

    fn load_guides(&mut self) -> Result<(), Box<dyn Error>> {
        for entry in fs::read_dir(&self.guides_dir)? {
            let entry = entry?;
            let path = entry.path();
            
            if path.extension().and_then(|s| s.to_str()) == Some("json") {
                let file_name = path.file_stem()
                    .and_then(|s| s.to_str())
                    .ok_or("Invalid filename")?
                    .to_string();
                
                let content = fs::read_to_string(&path)?;
                let guide: BuildGuide = serde_json::from_str(&content)?;
                
                self.guides.insert(file_name, guide);
            }
        }
        Ok(())
    }

    pub fn get_guide_names(&self) -> Vec<String> {
        self.guides.keys().cloned().collect()
    }

    pub fn get_guide(&self, name: &str) -> Option<&BuildGuide> {
        self.guides.get(name)
    }
} 