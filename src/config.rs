#![allow(dead_code)]

use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};
use std::{
    fs,
    path::{Path, PathBuf},
};

const DEFAULT_REDIRECT_URI: &str = "http://127.0.0.1:8888/callback";
const DEFAULT_CLIENT_ID_PLACEHOLDER: &str = "PASTE_YOUR_SPOTIFY_CLIENT_ID_HERE";

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AppConfig {
    /// Spotify Developer Client ID (PKCE flow, no secret required)
    pub client_id: String,

    /// OAuth Redirect URI (Must match the redirect URI registered in Spotify Dashboard)
    #[serde(default = "default_redirect_uri")]
    pub redirect_uri: String,

    /// Maximum track popularity threshold (0-100; usually <= 3 for <1000 streams)
    #[serde(default = "default_max_popularity")]
    pub max_popularity: u32,

    /// Default genre when none is specified
    #[serde(default = "default_genre")]
    pub default_genre: String,

    /// Minimum seconds in Smart Boost mode before skipping (Spotify requires > 30s)
    #[serde(default = "default_boost_min")]
    pub boost_min_seconds: u32,

    /// Maximum seconds in Smart Boost mode
    #[serde(default = "default_boost_max")]
    pub boost_max_seconds: u32,

    /// Initial number of tracks to dig up when loading a genre (default: 15)
    #[serde(default = "default_initial_limit")]
    pub queue_initial_limit: usize,

    /// Number of tracks to dig up when auto-replenishing the queue (default: 15)
    #[serde(default = "default_replenish_limit")]
    pub queue_replenish_limit: usize,

    /// Include featured creator/seed track in matching genres
    #[serde(default = "default_true")]
    pub include_featured: bool,

    /// Featured artist metadata
    #[serde(default = "default_featured_artist_name")]
    pub featured_artist_name: String,

    #[serde(default = "default_featured_artist_id")]
    pub featured_artist_id: String,

    #[serde(default = "default_featured_track_ids")]
    pub featured_track_ids: Vec<String>,

    #[serde(default = "default_featured_genres")]
    pub featured_genres: Vec<String>,
}

fn default_redirect_uri() -> String {
    DEFAULT_REDIRECT_URI.to_string()
}

fn default_max_popularity() -> u32 {
    3
}

fn default_genre() -> String {
    "synthwave".to_string()
}

fn default_boost_min() -> u32 {
    40
}

fn default_boost_max() -> u32 {
    60
}

fn default_initial_limit() -> usize {
    15
}

fn default_replenish_limit() -> usize {
    15
}

fn default_true() -> bool {
    true
}

fn default_featured_artist_name() -> String {
    "THE SILICON RITE".to_string()
}

fn default_featured_artist_id() -> String {
    "7p709xnrWUct3WiKHuS12f".to_string()
}

fn default_featured_track_ids() -> Vec<String> {
    vec![
        "6fmKTHiDrUBVV8ZRudWEws".to_string(),
        "20hpDOQzkq9oruMyIjun0u".to_string(),
    ]
}

fn default_featured_genres() -> Vec<String> {
    vec![
        "dark ambient".to_string(),
        "synthwave".to_string(),
        "darksynth".to_string(),
        "industrial".to_string(),
        "ebm".to_string(),
        "cyberpunk".to_string(),
        "phonk".to_string(),
        "hardcore".to_string(),
        "electronic".to_string(),
        "ritual ambient".to_string(),
        "witch house".to_string(),
    ]
}

impl Default for AppConfig {
    fn default() -> Self {
        Self {
            client_id: DEFAULT_CLIENT_ID_PLACEHOLDER.to_string(),
            redirect_uri: default_redirect_uri(),
            max_popularity: default_max_popularity(),
            default_genre: default_genre(),
            boost_min_seconds: default_boost_min(),
            boost_max_seconds: default_boost_max(),
            queue_initial_limit: default_initial_limit(),
            queue_replenish_limit: default_replenish_limit(),
            include_featured: true,
            featured_artist_name: default_featured_artist_name(),
            featured_artist_id: default_featured_artist_id(),
            featured_track_ids: default_featured_track_ids(),
            featured_genres: default_featured_genres(),
        }
    }
}

impl AppConfig {
    pub fn config_dir() -> PathBuf {
        dirs::config_dir()
            .map(|d| d.join("sub1k"))
            .unwrap_or_else(|| PathBuf::from(".sub1k"))
    }

    pub fn config_file_path() -> PathBuf {
        Self::config_dir().join("config.toml")
    }

    pub fn token_cache_path() -> PathBuf {
        Self::config_dir().join(".spotify_token_cache.json")
    }

    pub fn stats_file_path() -> PathBuf {
        Self::config_dir().join("rescued_stats.json")
    }

    pub fn is_client_id_configured(&self) -> bool {
        let trimmed = self.client_id.trim();
        !trimmed.is_empty() && trimmed != DEFAULT_CLIENT_ID_PLACEHOLDER
    }

    /// Load config from file or environment, creating default config file if missing
    pub fn load() -> Result<Self> {
        let config_dir = Self::config_dir();
        if !config_dir.exists() {
            fs::create_dir_all(&config_dir)
                .with_context(|| format!("Kon directory niet aanmaken: {:?}", config_dir))?;
        }

        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            let _ = fs::set_permissions(&config_dir, fs::Permissions::from_mode(0o700));
            let token_path = Self::token_cache_path();
            if token_path.exists() {
                let _ = fs::set_permissions(&token_path, fs::Permissions::from_mode(0o600));
            }
        }

        let config_path = Self::config_file_path();
        let mut config: AppConfig = if config_path.exists() {
            let content = fs::read_to_string(&config_path)
                .with_context(|| format!("Kon configuratie niet lezen van {:?}", config_path))?;
            toml::from_str(&content)
                .with_context(|| format!("Fout bij parsen van {:?}", config_path))?
        } else {
            let default_cfg = AppConfig::default();
            default_cfg.save_to(&config_path)?;
            default_cfg
        };

        // Environment overrides
        if let Ok(id) =
            std::env::var("SUB1K_CLIENT_ID").or_else(|_| std::env::var("RSPOTIFY_CLIENT_ID"))
        {
            if !id.trim().is_empty() {
                config.client_id = id.trim().to_string();
            }
        }

        if let Ok(uri) =
            std::env::var("SUB1K_REDIRECT_URI").or_else(|_| std::env::var("RSPOTIFY_REDIRECT_URI"))
        {
            if !uri.trim().is_empty() {
                config.redirect_uri = uri.trim().to_string();
            }
        }

        if let Ok(genre) = std::env::var("SUB1K_GENRE") {
            if !genre.trim().is_empty() {
                config.default_genre = genre.trim().to_string();
            }
        }

        if let Ok(pop) = std::env::var("SUB1K_MAX_POPULARITY") {
            if let Ok(parsed) = pop.parse::<u32>() {
                config.max_popularity = parsed;
            }
        }

        if std::env::var("SUB1K_NO_FEATURED").is_ok() {
            config.include_featured = false;
        }

        Ok(config)
    }

    pub fn save_to(&self, path: &Path) -> Result<()> {
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent)
                .with_context(|| format!("Kon directory niet aanmaken: {:?}", parent))?;
        }

        let toml_string =
            toml::to_string_pretty(self).context("Kon configuratie niet serialiseren naar TOML")?;

        fs::write(path, toml_string)
            .with_context(|| format!("Kon configuratiebestand niet schrijven naar {:?}", path))?;

        Ok(())
    }

    pub fn print_setup_instructions() {
        let path = Self::config_file_path();
        println!("\n╔══════════════════════════════════════════════════════════════════════╗");
        println!("║                 SUB-1K RADIO - SETUP VEREIST                         ║");
        println!("╠══════════════════════════════════════════════════════════════════════╣");
        println!("║ Om verbinding te maken met de officiële Spotify API via PKCE heb     ║");
        println!("║ je een eigen gratis Spotify 'Client ID' nodig.                       ║");
        println!("║                                                                      ║");
        println!("║ 1. Ga naar: https://developer.spotify.com/dashboard                  ║");
        println!("║ 2. Log in en klik op 'Create app'                                    ║");
        println!("║    - App name: Sub-1K Radio (of wat je wilt)                         ║");
        println!("║    - Redirect URI: http://127.0.0.1:8888/callback                    ║");
        println!("║    - APIs used: Web API (en eventueel Web Playback SDK)              ║");
        println!("║ 3. Kopieer je 'Client ID'                                            ║");
        println!("║ 4. Plak hem in je configuratiebestand:                               ║");
        println!("║    {:?}                                ║", path);
        println!("║    of zet de omgevingsvariabele: export SUB1K_CLIENT_ID=\"...\"       ║");
        println!("║                                                                      ║");
        println!("║ (Je hebt géén Client Secret nodig dankzij veilige PKCE OAuth!)       ║");
        println!("╚══════════════════════════════════════════════════════════════════════╝\n");
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct RescuedStats {
    pub total_rescued: u32,
    pub rescued_track_ids: Vec<String>,
}

impl RescuedStats {
    pub fn load() -> Self {
        let path = AppConfig::stats_file_path();
        if path.exists() {
            fs::read_to_string(&path)
                .ok()
                .and_then(|s| serde_json::from_str(&s).ok())
                .unwrap_or_default()
        } else {
            Self::default()
        }
    }

    pub fn record_rescue(&mut self, track_id: String) {
        if !self.rescued_track_ids.contains(&track_id) {
            self.rescued_track_ids.push(track_id);
            self.total_rescued = self.rescued_track_ids.len() as u32;
            let path = AppConfig::stats_file_path();
            if let Ok(json) = serde_json::to_string_pretty(self) {
                let _ = fs::write(path, json);
            }
        }
    }
}
