#![allow(deprecated)]

use anyhow::{Context, Result};
use clap::{Parser, Subcommand};
use rand::{seq::SliceRandom, Rng};
use rspotify::{
    clients::{BaseClient, OAuthClient},
    model::{FullTrack, PlayableId, PlaylistId, SearchResult, SearchType},
    prelude::Id,
    scopes, AuthCodeSpotify, Config, Credentials, OAuth, Token,
};
use serde::{Deserialize, Serialize};
use std::{
    collections::{HashMap, HashSet},
    env, fs,
    path::Path,
};

#[derive(Parser, Debug)]
#[command(name = "sub1k-sync")]
#[command(author = "THE SILICON RITE / sub1k contributors")]
#[command(version = "0.1.0")]
#[command(
    about = "Headless Spotify auto-curator for underdog indie tracks (<1K streams)",
    long_about = "Automatically curates and refreshes Spotify playlists with tracks under 1,000 streams while pinning anchor artist tracks in the top slots. Runs headless in GitHub Actions or locally."
)]
struct Cli {
    #[command(subcommand)]
    command: Option<Commands>,

    /// Path to playlists.toml configuration file
    #[arg(short, long, default_value = "playlists.toml")]
    config: String,

    /// Dry run mode: discover tracks and show playlist changes without modifying Spotify
    #[arg(long)]
    dry_run: bool,

    /// Directory to save track archives and website data (e.g. docs/data)
    #[arg(long, default_value = "docs/data")]
    archive_dir: String,
}

#[derive(Subcommand, Debug)]
enum Commands {
    /// Authorize Spotify via browser to generate SPOTIFY_REFRESH_TOKEN for GitHub Secrets
    Auth {
        /// Spotify Client ID (can also be read from RSPOTIFY_CLIENT_ID or SPOTIFY_CLIENT_ID)
        #[arg(long, env = "RSPOTIFY_CLIENT_ID")]
        client_id: Option<String>,

        /// Spotify Client Secret (can also be read from RSPOTIFY_CLIENT_SECRET or SPOTIFY_CLIENT_SECRET)
        #[arg(long, env = "RSPOTIFY_CLIENT_SECRET")]
        client_secret: Option<String>,

        /// Redirect URI configured in Spotify Developer Dashboard
        #[arg(long, default_value = "http://127.0.0.1:8888/callback")]
        redirect_uri: String,
    },

    /// Run the playlist synchronization (Default when run in GitHub Actions)
    Sync {
        /// Path to playlists.toml configuration file
        #[arg(short, long, default_value = "playlists.toml")]
        config: String,

        /// Dry run mode: test harvesting without mutating playlists
        #[arg(long)]
        dry_run: bool,

        /// Directory to save track archives and website data (e.g. docs/data)
        #[arg(long, default_value = "docs/data")]
        archive_dir: String,
    },
}

#[derive(Deserialize, Debug)]
struct AppConfig {
    settings: Settings,
    playlists: Vec<PlaylistConfig>,
}

#[derive(Deserialize, Debug)]
struct Settings {
    max_popularity: u32,
    playlist_size: usize,
    anchor_artist: String,
    anchor_max_position: usize,
    /// Optional webhook URL (e.g. n8n) to trigger when playlists are synced
    webhook_url: Option<String>,
}

#[derive(Deserialize, Debug)]
struct PlaylistConfig {
    name: String,
    spotify_playlist_id: String,
    genres: Vec<String>,
    anchor_track_query: Option<String>,
    #[serde(default)]
    description: Option<String>,
}

#[derive(Serialize, Deserialize, Clone, Debug)]
struct ArchivedTrack {
    id: String,
    title: String,
    artist: String,
    album: String,
    album_art: Option<String>,
    duration_ms: u64,
    popularity: u32,
    spotify_url: String,
    preview_url: Option<String>,
    added_at: String,
    playlist_id: String,
    playlist_name: String,
}

#[derive(Serialize, Deserialize, Clone, Debug)]
struct PlaylistMeta {
    id: String,
    name: String,
    spotify_url: String,
    genres: Vec<String>,
    has_anchor: bool,
    track_count: usize,
    updated_at: String,
}

#[derive(Serialize, Deserialize, Clone, Debug)]
struct WebsiteState {
    updated_at: String,
    anchor_artist: String,
    playlists: Vec<PlaylistMeta>,
    latest_tracks: Vec<ArchivedTrack>,
    total_archived: usize,
}

/// Helper function to retrieve credentials from env variables
fn get_credentials() -> Result<Credentials> {
    let client_id = env::var("RSPOTIFY_CLIENT_ID")
        .or_else(|_| env::var("SPOTIFY_CLIENT_ID"))
        .context("RSPOTIFY_CLIENT_ID of SPOTIFY_CLIENT_ID ontbreekt in environment")?;

    let client_secret = env::var("RSPOTIFY_CLIENT_SECRET")
        .or_else(|_| env::var("SPOTIFY_CLIENT_SECRET"))
        .context("RSPOTIFY_CLIENT_SECRET of SPOTIFY_CLIENT_SECRET ontbreekt in environment")?;

    Ok(Credentials::new(&client_id, &client_secret))
}

/// Interactive OAuth command to generate the SPOTIFY_REFRESH_TOKEN for GitHub Secrets
async fn run_auth(
    client_id: Option<String>,
    client_secret: Option<String>,
    redirect_uri: String,
) -> Result<()> {
    println!("=== Sub-1K Auto-Curator Spotify Authentication ===");
    println!("Dit proces genereert een Refresh Token voor je GitHub Actions repository secrets.\n");

    let creds = match (client_id, client_secret) {
        (Some(id), Some(secret)) => Credentials::new(&id, &secret),
        _ => get_credentials()
            .context("Geef --client-id en --client-secret mee of zet ze in je .env")?,
    };

    let oauth = OAuth {
        redirect_uri,
        scopes: scopes!(
            "playlist-modify-public",
            "playlist-modify-private",
            "playlist-read-private"
        ),
        ..Default::default()
    };

    let config = Config {
        token_cached: true,
        cache_path: std::path::PathBuf::from(".sub1k_sync_token.json"),
        ..Default::default()
    };
    let spotify = AuthCodeSpotify::with_config(creds, oauth, config);
    let url = spotify.get_authorize_url(false)?;

    println!("Open de volgende link in je browser om toestemming te geven:\n");
    println!("  {}\n", url);

    spotify.prompt_for_token(&url).await?;

    let token = spotify.get_token().lock().await.unwrap().clone();
    if let Some(t) = token {
        if let Some(refresh_token) = t.refresh_token {
            println!("========================================================");
            println!("🎉 Authenticatie geslaagd!");
            println!("Voeg deze geheimen toe aan je GitHub Repo:");
            println!("Settings -> Secrets and variables -> Actions -> New repository secret\n");
            println!("1. RSPOTIFY_CLIENT_ID     = (jouw client ID)");
            println!("2. RSPOTIFY_CLIENT_SECRET = (jouw client secret)");
            println!("3. SPOTIFY_REFRESH_TOKEN  = {}\n", refresh_token);
            println!("========================================================");

            // Automatisch wegschrijven naar .env zodat lokale sync ook direct werkt
            let env_content = fs::read_to_string(".env").unwrap_or_default();
            if !env_content.contains("SPOTIFY_REFRESH_TOKEN") {
                use std::io::Write;
                let mut f = fs::OpenOptions::new().append(true).open(".env")?;
                writeln!(f, "SPOTIFY_REFRESH_TOKEN={}", refresh_token)?;
            }

            return Ok(());
        }
    }

    anyhow::bail!("Kon geen refresh token uitlezen na autorisatie.");
}

/// Initialiseert een headless Spotify client op basis van een opgeslagen Refresh Token
async fn get_headless_client() -> Result<AuthCodeSpotify> {
    dotenvy::dotenv().ok();

    let creds = get_credentials()?;
    let refresh_token = env::var("SPOTIFY_REFRESH_TOKEN")
        .context("SPOTIFY_REFRESH_TOKEN ontbreekt in environment (of GitHub Secrets)")?;

    let oauth = OAuth {
        redirect_uri: "http://127.0.0.1:8888/callback".to_string(),
        scopes: scopes!(
            "playlist-modify-public",
            "playlist-modify-private",
            "playlist-read-private"
        ),
        ..Default::default()
    };

    let token = Token {
        refresh_token: Some(refresh_token),
        ..Default::default()
    };

    let spotify = AuthCodeSpotify::from_token_with_config(
        token,
        creds,
        oauth,
        Config {
            token_refreshing: true,
            ..Default::default()
        },
    );

    // Refresh het access_token on-the-fly
    spotify.refresh_token().await.context(
        "Fout bij verversen van access token via SPOTIFY_REFRESH_TOKEN. Controleer geldigheid.",
    )?;

    Ok(spotify)
}

/// Zoekt de anchor track op (bijv. THE SILICON RITE) op basis van de geconfigureerde query
async fn fetch_anchor_track(spotify: &AuthCodeSpotify, query: &str) -> Result<Option<FullTrack>> {
    let res = spotify
        .search(query, SearchType::Track, None, None, Some(10), Some(0))
        .await?;

    if let SearchResult::Tracks(page) = res {
        let chosen = page.items.choose(&mut rand::thread_rng()).cloned();
        return Ok(chosen);
    }
    Ok(None)
}

/// Filtert spam, lo-fi study beats, ASMR, skits en interludes eruit voor maximaal luisterbehoud (anti-skip)
fn is_clean_underdog_track(track: &FullTrack) -> bool {
    let blocked_substrings = [
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
        "spoken word",
        "interview",
        "monologue",
    ];

    let blocked_keywords = [
        "intro",
        "outro",
        "interlude",
        "prelude",
        "skit",
        "transition",
    ];

    let title_lower = track.name.to_lowercase();
    let album_lower = track.album.name.to_lowercase();
    let artist_lower = track
        .artists
        .first()
        .map(|a| a.name.to_lowercase())
        .unwrap_or_default();

    for term in &blocked_substrings {
        if title_lower.contains(term) || album_lower.contains(term) || artist_lower.contains(term) {
            return false;
        }
    }

    // Skip-rate defense: filter expliciete intro/outro/skit tracks eruit
    for word in title_lower.split(|c: char| !c.is_alphanumeric()) {
        if blocked_keywords.contains(&word) {
            return false;
        }
    }

    // Anti-skip retention: filter tracks onder 75s (vaak audio-samples) of langer dan 10 min (drones)
    let duration_sec = track.duration.num_seconds();
    if !(75..=600).contains(&duration_sec) {
        return false;
    }

    true
}

/// Graaft obscure underground tracks (<1K streams via `tag:hipster`) op voor de opgegeven genres.
/// Gebruikt strikte `genre:"..." tag:hipster` queries met dynamische offsets op basis van Spotify's
/// werkelijke `page.total`, zodat we nooit terugvallen op ongerelateerde populaire tracks.
async fn harvest_underdog_tracks(
    spotify: &AuthCodeSpotify,
    genres: &[String],
    max_pop: u32,
    target_count: usize,
    exclude_artist: Option<&str>,
) -> Result<Vec<FullTrack>> {
    let mut tracks = Vec::new();
    let mut seen_artists = HashSet::new();
    let mut seen_track_ids = HashSet::new();
    let mut genre_totals: HashMap<String, u32> = HashMap::new();
    let alphabet: Vec<char> = ('a'..='z').collect();

    if let Some(ex) = exclude_artist {
        seen_artists.insert(ex.to_lowercase());
    }

    let mut attempts = 0;
    let max_attempts = 75;
    let mut skip_spins = 0;

    while tracks.len() < target_count && attempts < max_attempts && skip_spins < 300 {
        let genre = genres.choose(&mut rand::thread_rng()).unwrap();

        let (query, offset) = match genre_totals.get(genre) {
            None => {
                // Eerste keer dat we dit genre bevragen: haal offset 0 op om `page.total` te leren kennen
                (format!("genre:\"{}\" tag:hipster", genre), 0)
            }
            Some(&0) => {
                // Dit genre bestaat niet als strikte Spotify genre-tag; gebruik keyword + verplichte tag:hipster
                let off = rand::thread_rng().gen_range(0..15);
                (format!("\"{}\" tag:hipster", genre), off)
            }
            Some(&total) if total >= 100 => {
                // Grote pool (>=100 tracks): wissel af tussen random offset en letter-filter binnen genre + tag:hipster
                if rand::thread_rng().gen_bool(0.45) {
                    let letter = alphabet.choose(&mut rand::thread_rng()).unwrap();
                    (format!("genre:\"{}\" tag:hipster {}*", genre, letter), 0)
                } else {
                    let off = rand::thread_rng().gen_range(0..90);
                    (format!("genre:\"{}\" tag:hipster", genre), off)
                }
            }
            Some(&total) if total > 10 => {
                let max_off = total.saturating_sub(5).min(90);
                let off = rand::thread_rng().gen_range(0..=max_off);
                (format!("genre:\"{}\" tag:hipster", genre), off)
            }
            Some(_) => {
                // Kleine pool (<= 10 tracks), offset 0 is al opgehaald; sla over zonder API-attempt te verspillen
                skip_spins += 1;
                continue;
            }
        };

        attempts += 1;

        let res = spotify
            .search(
                &query,
                SearchType::Track,
                None,
                None,
                Some(10), // Spotify Web API ondersteunt max 10 items per call
                Some(offset),
            )
            .await;

        let mut batch_tracks = Vec::new();
        if let Ok(SearchResult::Tracks(page)) = res {
            genre_totals.entry(genre.clone()).or_insert(page.total);
            batch_tracks = page.items;
        } else {
            genre_totals.entry(genre.clone()).or_insert(0);
        }

        // Schud de batch zodat we niet telkens dezelfde top-positie uit een kleine pool pakken
        batch_tracks.shuffle(&mut rand::thread_rng());

        for track in batch_tracks {
            if track.popularity <= max_pop && is_clean_underdog_track(&track) {
                let track_id_key = track
                    .id
                    .as_ref()
                    .map(|id| id.id().to_string())
                    .unwrap_or_else(|| track.name.clone());

                if !seen_track_ids.insert(track_id_key) {
                    continue;
                }

                if let Some(first_artist) = track.artists.first() {
                    // Maximaal 1 track per artiest (case-insensitive) voor maximale diversiteit
                    if seen_artists.insert(first_artist.name.to_lowercase()) {
                        tracks.push(track);
                        if tracks.len() >= target_count {
                            break;
                        }
                    }
                }
            }
        }
    }

    // Schud de uiteindelijke lijst zodat genres mooi door elkaar lopen
    tracks.shuffle(&mut rand::thread_rng());

    Ok(tracks)
}

async fn run_sync(config_path: &str, dry_run: bool, archive_dir: &str) -> Result<()> {
    if !Path::new(config_path).exists() {
        anyhow::bail!(
            "Configuratiebestand '{}' niet gevonden. Maak dit bestand aan of kopieer het voorbeeld.",
            config_path
        );
    }

    let config_str = fs::read_to_string(config_path)
        .with_context(|| format!("Kon {} niet lezen", config_path))?;
    let config: AppConfig = toml::from_str(&config_str)
        .with_context(|| format!("Ongeldig TOML formaat in {}", config_path))?;

    println!("⚡ Initialiseren van Spotify headless client...");
    let spotify = get_headless_client().await?;

    let now_utc = chrono::Utc::now();
    let today = now_utc.format("%Y-%m-%d").to_string();
    let iso_now = now_utc.to_rfc3339();

    let mut summary_lines = Vec::new();
    summary_lines.push(format!("# 📻 Sub-1K Auto-Curator Sync Report"));
    summary_lines.push(format!(
        "**Anchor Artist:** `{}`\n",
        config.settings.anchor_artist
    ));

    let mut all_archived_tracks: Vec<ArchivedTrack> = Vec::new();
    let mut playlists_meta: Vec<PlaylistMeta> = Vec::new();

    for pl in &config.playlists {
        println!("\n📂 Playlist: {} ({})", pl.name, pl.spotify_playlist_id);

        if pl.spotify_playlist_id.starts_with("YOUR_PLAYLIST_ID") {
            println!(
                "  ⚠️ Sla over: playlist ID is nog niet geconfigureerd in {}",
                config_path
            );
            continue;
        }

        println!(
            "  🔍 Graven naar tracks met populariteit <= {}...",
            config.settings.max_popularity
        );
        let has_anchor = pl
            .anchor_track_query
            .as_ref()
            .map(|q| !q.trim().is_empty())
            .unwrap_or(false);
        let target_harvest = if has_anchor {
            config.settings.playlist_size.saturating_sub(1)
        } else {
            config.settings.playlist_size
        };

        let exclude_artist = if has_anchor {
            Some(config.settings.anchor_artist.as_str())
        } else {
            None
        };

        let mut tracks = harvest_underdog_tracks(
            &spotify,
            &pl.genres,
            config.settings.max_popularity,
            target_harvest,
            exclude_artist,
        )
        .await?;

        println!("  ✨ {} obscure tracks gevonden.", tracks.len());

        // Algoritme-optimalisatie (Anti-skip): zorg dat de openingsposities tracks zijn met ideale radiolengte (2m15s - 4m45s)
        if tracks.len() > 3 {
            if let Some(pos) = tracks.iter().position(|t| {
                let s = t.duration.num_seconds();
                (135..=285).contains(&s)
            }) {
                let opener = tracks.remove(pos);
                tracks.insert(0, opener);
            }
        }

        let mut anchor_track_name = String::new();
        // Zoek de anchor track op en plaats deze in de top posities (indien geconfigureerd)
        if let Some(ref query) = pl.anchor_track_query {
            if !query.trim().is_empty() {
                if let Ok(Some(anchor_track)) = fetch_anchor_track(&spotify, query).await {
                    tracks.retain(|t| t.id != anchor_track.id);
                    anchor_track_name = anchor_track.name.clone();

                    let max_pos = config.settings.anchor_max_position.min(tracks.len());
                    let insert_idx = if max_pos > 0 {
                        rand::thread_rng().gen_range(0..=max_pos)
                    } else {
                        0
                    };

                    println!(
                        "  ⚓ Geïnjecteerd op positie #{}: {} - {}",
                        insert_idx + 1,
                        config.settings.anchor_artist,
                        anchor_track.name
                    );
                    tracks.insert(insert_idx, anchor_track);
                }
            }
        }

        // Archiveer de gevonden tracks voor deze playlist
        let pl_archived: Vec<ArchivedTrack> = tracks
            .iter()
            .map(|t| {
                let artist_name = t
                    .artists
                    .first()
                    .map(|a| a.name.clone())
                    .unwrap_or_else(|| "Unknown Artist".to_string());
                let album_name = t.album.name.clone();
                let album_art = t.album.images.first().map(|img| img.url.clone());
                let track_id_str =
                    t.id.as_ref()
                        .map(|id| id.id().to_string())
                        .unwrap_or_default();
                let spotify_url =
                    t.external_urls.get("spotify").cloned().unwrap_or_else(|| {
                        format!("https://open.spotify.com/track/{}", track_id_str)
                    });

                ArchivedTrack {
                    id: track_id_str,
                    title: t.name.clone(),
                    artist: artist_name,
                    album: album_name,
                    album_art,
                    duration_ms: t.duration.num_milliseconds().max(0) as u64,
                    popularity: t.popularity,
                    spotify_url,
                    preview_url: None,
                    added_at: iso_now.clone(),
                    playlist_id: pl.spotify_playlist_id.clone(),
                    playlist_name: pl.name.clone(),
                }
            })
            .collect();

        all_archived_tracks.extend(pl_archived);

        playlists_meta.push(PlaylistMeta {
            id: pl.spotify_playlist_id.clone(),
            name: pl.name.clone(),
            spotify_url: format!(
                "https://open.spotify.com/playlist/{}",
                pl.spotify_playlist_id
            ),
            genres: pl.genres.clone(),
            has_anchor,
            track_count: tracks.len(),
            updated_at: iso_now.clone(),
        });

        if dry_run {
            println!("  [DRY RUN] Geen Spotify wijzigingen doorgevoerd.");
            continue;
        }

        // Converteer naar PlayableId's en vervang items in Spotify
        let track_ids: Vec<PlayableId> = tracks
            .iter()
            .filter_map(|t| t.id.as_ref().map(|id| PlayableId::Track(id.clone_static())))
            .collect();

        let playlist_id = PlaylistId::from_id(&pl.spotify_playlist_id)?;

        spotify
            .playlist_replace_items(playlist_id.clone(), track_ids)
            .await
            .with_context(|| format!("Kon tracks niet vervangen voor playlist {}", pl.name))?;

        // Beschrijving bijwerken met geoptimaliseerde SEO-tagline of dynamische fallback
        let description = pl.description.clone().unwrap_or_else(|| {
            format!(
                "Daily underdog discovery ({}). Under 1K streams. Curated by {} // Powered by open-source sub1k-sync",
                today, config.settings.anchor_artist
            )
        });

        let _ = spotify
            .playlist_change_detail(playlist_id, None, None, Some(&description), None)
            .await;

        println!(
            "  ✅ Playlist succesvol bijgewerkt ({} nummers).",
            tracks.len()
        );

        summary_lines.push(format!("### {}", pl.name));
        summary_lines.push(format!("Bijgewerkt met **{}** tracks.", tracks.len()));

        // Webhook afvuren naar n8n (indien geconfigureerd via env of config)
        let webhook_url = env::var("N8N_WEBHOOK_URL")
            .or_else(|_| env::var("WEBHOOK_URL"))
            .ok()
            .or_else(|| config.settings.webhook_url.clone());

        if let Some(target_url) = webhook_url {
            if !target_url.is_empty() && !dry_run {
                println!(
                    "  📡 Verzenden van sync-notificatie naar webhook: {}",
                    target_url
                );

                let artist_names: Vec<String> = tracks
                    .iter()
                    .filter_map(|t| t.artists.first().map(|a| a.name.clone()))
                    .collect();

                let featured_samples: Vec<String> = artist_names
                    .iter()
                    .filter(|a| a.as_str() != config.settings.anchor_artist)
                    .take(3)
                    .cloned()
                    .collect();

                let tweet_text = format!(
                    "🚨 Fresh <1K Underground Radio live on Spotify! Featuring {} + {}. Listen: https://open.spotify.com/playlist/{} #darksynth #cyberpunk #industrial #indie",
                    config.settings.anchor_artist,
                    featured_samples.join(", "),
                    pl.spotify_playlist_id
                );

                let payload_anchor_artist = if has_anchor {
                    config.settings.anchor_artist.as_str()
                } else {
                    ""
                };

                let payload = serde_json::json!({
                    "event": "playlist_updated",
                    "playlist_name": pl.name,
                    "playlist_id": pl.spotify_playlist_id,
                    "playlist_url": format!("https://open.spotify.com/playlist/{}", pl.spotify_playlist_id),
                    "anchor_artist": payload_anchor_artist,
                    "anchor_track": anchor_track_name,
                    "total_tracks": tracks.len(),
                    "artists": artist_names,
                    "tweet_text": tweet_text,
                    "timestamp": chrono::Utc::now().to_rfc3339()
                });

                let client = reqwest::Client::new();
                match client.post(&target_url).json(&payload).send().await {
                    Ok(resp) if resp.status().is_success() => {
                        println!("  ✅ Webhook succesvol afgeleverd aan n8n!");
                    }
                    Ok(resp) => {
                        eprintln!("  ⚠️ Webhook HTTP status: {}", resp.status());
                    }
                    Err(e) => {
                        eprintln!("  ⚠️ Kon webhook niet bereiken: {}", e);
                    }
                }
            }
        }
    }

    // 💾 Wegschrijven naar Archive & Website Data
    if !all_archived_tracks.is_empty() {
        let archive_path = Path::new(archive_dir);
        if let Err(e) = fs::create_dir_all(archive_path) {
            eprintln!("  ⚠️ Kon archive dir niet aanmaken: {}", e);
        } else {
            // 1. Lees bestaande geschiedenis in
            let history_file = archive_path.join("history.json");
            let existing_history: Vec<ArchivedTrack> = if history_file.exists() {
                fs::read_to_string(&history_file)
                    .ok()
                    .and_then(|data| serde_json::from_str(&data).ok())
                    .unwrap_or_default()
            } else {
                Vec::new()
            };

            // Voeg nieuwe toe vooraan (meest recente eerst), zonder duplicaten per playlist
            let mut seen_ids = HashSet::new();
            let mut merged_history = Vec::new();

            for track in all_archived_tracks.iter().chain(existing_history.iter()) {
                let key = format!("{}:{}", track.playlist_id, track.id);
                if seen_ids.insert(key) {
                    merged_history.push(track.clone());
                }
            }

            // Beperk history eventueel tot laatste 2.500 tracks voor snelle web laadtijd
            if merged_history.len() > 2500 {
                merged_history.truncate(2500);
            }

            let total_count = merged_history.len();

            // Sla history.json op
            if let Ok(json_str) = serde_json::to_string_pretty(&merged_history) {
                let _ = fs::write(&history_file, json_str);
            }

            // 2. Sla latest.json (enkel vandaag) op
            let latest_file = archive_path.join("latest.json");
            let site_state = WebsiteState {
                updated_at: iso_now,
                anchor_artist: config.settings.anchor_artist.clone(),
                playlists: playlists_meta,
                latest_tracks: all_archived_tracks,
                total_archived: total_count,
            };

            if let Ok(json_str) = serde_json::to_string_pretty(&site_state) {
                let _ = fs::write(&latest_file, json_str);
            }

            println!(
                "  📁 Archief bijgewerkt in {} (Totaal geregistreerde tracks: {})",
                archive_dir, total_count
            );
        }
    }

    // Schrijf een GitHub Action Step Summary indien beschikbaar
    if let Ok(summary_file) = env::var("GITHUB_STEP_SUMMARY") {
        let content = summary_lines.join("\n");
        let _ = fs::write(summary_file, content);
    }

    println!("\n🎉 Alles succesvol gesynchroniseerd!");
    Ok(())
}

#[tokio::main]
async fn main() -> Result<()> {
    dotenvy::dotenv().ok();
    let cli = Cli::parse();

    match cli.command {
        Some(Commands::Auth {
            client_id,
            client_secret,
            redirect_uri,
        }) => run_auth(client_id, client_secret, redirect_uri).await,
        Some(Commands::Sync {
            config,
            dry_run,
            archive_dir,
        }) => run_sync(&config, dry_run, &archive_dir).await,
        None => run_sync(&cli.config, cli.dry_run, &cli.archive_dir).await,
    }
}
