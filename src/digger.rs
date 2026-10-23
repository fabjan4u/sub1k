#![allow(dead_code)]
#![allow(deprecated)]

use anyhow::Result;
use rand::{seq::SliceRandom, Rng};
use rspotify::{
    clients::BaseClient,
    model::{FullTrack, SearchResult, SearchType},
    AuthCodePkceSpotify,
};

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

    /// Main discovery method: digs up obscure tracks under the popularity threshold
    pub async fn dig_tracks(&self, genre: &str, limit: usize) -> Result<Vec<DiscoveredTrack>> {
        let mut obscure_tracks: Vec<DiscoveredTrack> = Vec::new();
        let alphabet: Vec<char> = ('a'..='z').collect();

        let mut attempts = 0;
        let max_attempts = ((limit / 4) * 3).max(25);

        while obscure_tracks.len() < limit && attempts < max_attempts {
            attempts += 1;
            let random_char = {
                let mut rng = rand::thread_rng();
                *alphabet.choose(&mut rng).unwrap_or(&'a')
            };
            // Random offset within 0..40 to get fresh batches
            let random_offset = {
                let mut rng = rand::thread_rng();
                rng.gen_range(0..25)
            };

            // Construct fallback queries in order of specificity
            let queries = [
                format!("genre:\"{}\" tag:hipster {}*", genre, random_char),
                format!("\"{}\" tag:hipster {}*", genre, random_char),
                format!("\"{}\" tag:hipster", genre),
                format!("genre:\"{}\" tag:hipster", genre),
                format!("\"{}\" {}*", genre, random_char),
            ];

            let mut batch_tracks = Vec::new();

            for query in &queries {
                let result = self
                    .spotify
                    .search(
                        query,
                        SearchType::Track,
                        None,
                        None,
                        Some(SPOTIFY_SEARCH_PAGE_LIMIT),
                        Some(random_offset),
                    )
                    .await;

                if let Ok(SearchResult::Tracks(page)) = result {
                    if !page.items.is_empty() {
                        batch_tracks = page.items;
                        break;
                    }
                }
            }

            if batch_tracks.is_empty() {
                continue;
            }

            // Filter tracks:
            // Spotify popularity: if present, must be <= max_popularity.
            // If Spotify omitted popularity, it defaults to 0 (which matches).
            for track in batch_tracks {
                if obscure_tracks
                    .iter()
                    .any(|existing| existing.track.id == track.id)
                {
                    continue;
                }

                if track.popularity <= self.config.max_popularity {
                    obscure_tracks.push(DiscoveredTrack {
                        track,
                        is_featured: false,
                        artist_followers: None,
                    });
                }

                if obscure_tracks.len() >= limit {
                    break;
                }
            }
        }

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
