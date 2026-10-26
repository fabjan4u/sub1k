#![allow(dead_code)]
#![allow(deprecated)]

use anyhow::Result;
use rand::{seq::SliceRandom, Rng};
use rspotify::{
    clients::BaseClient,
    model::{FullTrack, Id, SearchResult, SearchType},
    AuthCodePkceSpotify,
};
use std::collections::{HashMap, HashSet};

use crate::config::AppConfig;
use crate::spotify;

/// Spotify Web API enforces a strict maximum of 10 items per search call for developer apps
const SPOTIFY_SEARCH_PAGE_LIMIT: u32 = 10;

#[derive(Debug, Clone)]
pub struct DiscoveredTrack {
    pub track: FullTrack,
    pub is_featured: bool,
    pub artist_followers: Option<u32>,
}

/// Filters out spam, lo-fi study beats, ASMR, karaoke, and non-musical fillers
pub fn is_clean_underdog_track(track: &FullTrack) -> bool {
    let blocked_terms = [
        "lofi",
        "lo-fi",
        "study beats",
        "sleep",
        "meditation",
        "relaxing",
        "bdsm",
        "erotic",
        "karaoke",
        "ringtone",
        "8-bit",
        "chiptune",
        "tribute to",
        "made famous by",
        "white noise",
        "rain sounds",
        "asmr",
        "lullaby",
        "yoga",
    ];

    let title_lower = track.name.to_lowercase();
    let album_lower = track.album.name.to_lowercase();
    let artist_lower = track
        .artists
        .first()
        .map(|a| a.name.to_lowercase())
        .unwrap_or_default();

    for term in &blocked_terms {
        if title_lower.contains(term) || album_lower.contains(term) || artist_lower.contains(term) {
            return false;
        }
    }

    if track.duration.num_seconds() < 45 {
        return false;
    }

    true
}

/// Expand a single genre into closely related Spotify genre tags when the primary genre has a small pool
fn expand_related_genres(genre: &str) -> Vec<String> {
    let g = genre.trim().to_lowercase();
    match g.as_str() {
        "synthwave" | "darksynth" | "cyberpunk" => vec![
            g,
            "dark electro".to_string(),
            "electro-industrial".to_string(),
            "ebm".to_string(),
            "cyberpunk".to_string(),
            "darkwave".to_string(),
        ],
        "industrial" | "ebm" | "aggrotech" | "dark electro" => vec![
            g,
            "industrial".to_string(),
            "electro-industrial".to_string(),
            "dark electro".to_string(),
            "ebm".to_string(),
            "power noise".to_string(),
            "industrial metal".to_string(),
        ],
        "deathcore" | "beatdown" | "slam" | "slam deathcore" => vec![
            g,
            "deathcore".to_string(),
            "beatdown".to_string(),
            "blackened deathcore".to_string(),
            "technical deathcore".to_string(),
            "metallic hardcore".to_string(),
            "brutal death metal".to_string(),
            "uk beatdown".to_string(),
        ],
        _ => vec![g],
    }
}

#[derive(Clone)]
pub struct CrateDigger {
    spotify: AuthCodePkceSpotify,
    config: AppConfig,
}

impl CrateDigger {
    pub fn new(spotify: AuthCodePkceSpotify, config: AppConfig) -> Self {
        Self { spotify, config }
    }

    /// Check if the selected genre matches the creator's music style
    pub fn is_matching_featured_genre(&self, genre: &str) -> bool {
        let g = genre.to_lowercase();
        self.config
            .featured_genres
            .iter()
            .any(|target| g.contains(&target.to_lowercase()))
    }

    /// Fetch one of the featured creator tracks directly by ID
    pub async fn fetch_featured_track(&self) -> Result<Option<DiscoveredTrack>> {
        if self.config.featured_track_ids.is_empty() {
            return Ok(None);
        }

        let track_id = {
            let mut rng = rand::thread_rng();
            self.config
                .featured_track_ids
                .choose(&mut rng)
                .cloned()
                .unwrap()
        };

        match spotify::get_track(&self.spotify, &track_id).await {
            Ok(track) => Ok(Some(DiscoveredTrack {
                track,
                is_featured: true,
                artist_followers: None,
            })),
            Err(e) => {
                eprintln!("Waarschuwing: Kon featured track niet ophalen: {}", e);
                Ok(None)
            }
        }
    }

    /// Main discovery method: digs up obscure tracks under the popularity threshold using strict `tag:hipster`
    pub async fn dig_tracks(&self, genre: &str, limit: usize) -> Result<Vec<DiscoveredTrack>> {
        let mut obscure_tracks: Vec<DiscoveredTrack> = Vec::new();
        let mut seen_artists: HashSet<String> = HashSet::new();
        let mut seen_track_ids: HashSet<String> = HashSet::new();
        let mut genre_totals: HashMap<String, u32> = HashMap::new();
        let alphabet: Vec<char> = ('a'..='z').collect();
        let candidate_genres = expand_related_genres(genre);

        if self.config.include_featured {
            seen_artists.insert(self.config.featured_artist_name.to_lowercase());
        }

        let mut attempts = 0;
        let max_attempts = (limit * 2).max(30);

        while obscure_tracks.len() < limit && attempts < max_attempts {
            attempts += 1;

            // Kies het hoofdgenre in de eerste pogingen, daarna ook verwante subgenres
            let active_genre = if attempts <= 2 {
                &candidate_genres[0]
            } else {
                candidate_genres.choose(&mut rand::thread_rng()).unwrap()
            };

            let (query, offset) = match genre_totals.get(active_genre) {
                None => (format!("genre:\"{}\" tag:hipster", active_genre), 0),
                Some(&0) => {
                    let off = rand::thread_rng().gen_range(0..15);
                    (format!("\"{}\" tag:hipster", active_genre), off)
                }
                Some(&total) if total >= 100 => {
                    if rand::thread_rng().gen_bool(0.45) {
                        let letter = alphabet.choose(&mut rand::thread_rng()).unwrap();
                        (
                            format!("genre:\"{}\" tag:hipster {}*", active_genre, letter),
                            0,
                        )
                    } else {
                        let off = rand::thread_rng().gen_range(0..90);
                        (format!("genre:\"{}\" tag:hipster", active_genre), off)
                    }
                }
                Some(&total) if total > 10 => {
                    let max_off = total.saturating_sub(5).min(90);
                    let off = rand::thread_rng().gen_range(0..=max_off);
                    (format!("genre:\"{}\" tag:hipster", active_genre), off)
                }
                Some(_) => {
                    if candidate_genres.len() == 1 {
                        let off = rand::thread_rng().gen_range(0..15);
                        (format!("\"{}\" tag:hipster", active_genre), off)
                    } else {
                        continue;
                    }
                }
            };

            let result = self
                .spotify
                .search(
                    &query,
                    SearchType::Track,
                    None,
                    None,
                    Some(SPOTIFY_SEARCH_PAGE_LIMIT),
                    Some(offset),
                )
                .await;

            let mut batch_tracks = Vec::new();
            if let Ok(SearchResult::Tracks(page)) = result {
                genre_totals
                    .entry(active_genre.clone())
                    .or_insert(page.total);
                batch_tracks = page.items;
            } else {
                genre_totals.entry(active_genre.clone()).or_insert(0);
            }

            if batch_tracks.is_empty() {
                continue;
            }

            batch_tracks.shuffle(&mut rand::thread_rng());

            for track in batch_tracks {
                if track.popularity <= self.config.max_popularity && is_clean_underdog_track(&track)
                {
                    let track_id_key = track
                        .id
                        .as_ref()
                        .map(|id| id.id().to_string())
                        .unwrap_or_else(|| track.name.clone());

                    if !seen_track_ids.insert(track_id_key) {
                        continue;
                    }

                    if let Some(first_artist) = track.artists.first() {
                        if seen_artists.insert(first_artist.name.to_lowercase()) {
                            obscure_tracks.push(DiscoveredTrack {
                                track,
                                is_featured: false,
                                artist_followers: None,
                            });
                            if obscure_tracks.len() >= limit {
                                break;
                            }
                        }
                    }
                }
            }
        }

        obscure_tracks.shuffle(&mut rand::thread_rng());

        // Inject Featured Seed Track (e.g. THE SILICON RITE) if enabled and genre matches
        if self.config.include_featured && self.is_matching_featured_genre(genre) {
            if let Ok(Some(featured)) = self.fetch_featured_track().await {
                let already_present = obscure_tracks
                    .iter()
                    .any(|t| t.track.id == featured.track.id);

                if !already_present {
                    let insert_pos = if obscure_tracks.is_empty() {
                        0
                    } else {
                        let max_pos = obscure_tracks.len().min(4);
                        rand::thread_rng().gen_range(0..=max_pos)
                    };
                    obscure_tracks.insert(insert_pos, featured);
                }
            }
        }

        Ok(obscure_tracks)
    }
}
