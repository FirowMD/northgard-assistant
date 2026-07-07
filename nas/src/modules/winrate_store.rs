use std::collections::HashMap;
use std::fs;
use std::io::{self, Write};
use std::path::{Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};

use serde::{Deserialize, Serialize};

use crate::modules::winrate_tracker::EndGameKind;

#[derive(Debug, Serialize, Deserialize, Clone, Copy)]
pub enum Outcome {
    Win,
    Loss,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct WinrateEntry {
    pub outcome: Outcome,
    pub reason: Option<String>,
    pub timestamp: u64,
}

#[derive(Debug, Default, Serialize, Deserialize)]
pub struct WinrateStats {
    pub total_wins: u32,
    pub total_losses: u32,
    pub by_reason: HashMap<String, u32>,
    #[serde(default)]
    pub entries: Vec<WinrateEntry>,
}

pub struct WinrateStore {
    file_path: PathBuf,
}

impl WinrateStore {
    pub fn new_default_path() -> Self {
        let file_path: PathBuf = if let Ok(pd) = std::env::var("PROGRAMDATA") {
            PathBuf::from(pd).join("northgard-tracker").join("winrate.json")
        } else {
            let base = dirs::data_dir().unwrap_or_else(|| PathBuf::from("."));
            base.join("northgard-tracker").join("winrate.json")
        };
        Self { file_path }
    }

    fn ensure_parent_dir(&self) -> io::Result<()> {
        if let Some(parent) = self.file_path.parent() {
            fs::create_dir_all(parent)
        } else {
            Ok(())
        }
    }

    pub fn load(&self) -> WinrateStats {
        match fs::read_to_string(&self.file_path) {
            Ok(contents) => serde_json::from_str::<WinrateStats>(&contents).unwrap_or_default(),
            Err(_) => WinrateStats::default(),
        }
    }

    pub fn save(&self, stats: &WinrateStats) -> io::Result<()> {
        self.ensure_parent_dir()?;
        let mut f = fs::File::create(&self.file_path)?;
        let json = serde_json::to_string_pretty(stats).unwrap_or_else(|_| String::from("{}"));
        f.write_all(json.as_bytes())
    }

    pub fn update_from_kind(&self, kind: EndGameKind) -> io::Result<()> {
        let mut stats = self.load();

        let now_secs = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap_or_default()
            .as_secs();

        if matches!(kind, EndGameKind::Defeat) {
            stats.total_losses = stats.total_losses.saturating_add(1);
            stats.entries.push(WinrateEntry {
                outcome: Outcome::Loss,
                reason: None,
                timestamp: now_secs,
            });
        } else {
            stats.total_wins = stats.total_wins.saturating_add(1);
            let reason = reason_for_kind(kind).map(|s| s.to_string());
            if let Some(ref r) = reason {
                let entry = stats.by_reason.entry(r.clone()).or_insert(0);
                *entry = entry.saturating_add(1);
            }
            stats.entries.push(WinrateEntry {
                outcome: Outcome::Win,
                reason,
                timestamp: now_secs,
            });
        }

        self.save(&stats)
    }

    pub fn path(&self) -> &Path {
        &self.file_path
    }
}

fn reason_for_kind(kind: EndGameKind) -> Option<&'static str> {
    match kind {
        EndGameKind::Victory => Some("defaultVictory"),
        EndGameKind::Fame => Some("fameVictory"),
        EndGameKind::Helheim => Some("helheimVictory"),
        EndGameKind::Faith => Some("faithVictory"),
        EndGameKind::Lore => Some("loreVictory"),
        EndGameKind::Mealsquirrel => Some("mealSquirrelVictory"),
        EndGameKind::Odinsword => Some("odinSwordVictory"),
        EndGameKind::Money => Some("moneyVictory"),
        EndGameKind::Owltitan => Some("owlTitanVictory"),
        EndGameKind::Yggdrasil => Some("yggdrasilVictory"),
        _ => None,
    }
}