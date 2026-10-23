#![allow(deprecated)]

use anyhow::{Context, Result};
use clap::{Parser, Subcommand};
use rand::{seq::SliceRandom, Rng};
use rspotify::{
    clients::{BaseClient, OAuthClient},
    model::{FullTrack, PlayableId, PlaylistId, SearchResult, SearchType},
    scopes, AuthCodeSpotify, Config, Credentials, OAuth, Token,
};
use serde::Deserialize;
use std::{collections::HashSet, env, fs, path::Path};

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
}

#[derive(Deserialize, Debug)]
struct PlaylistConfig {
    name: String,
    spotify_playlist_id: String,
    genres: Vec<String>,
    anchor_track_query: String,
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
        _ => get_credentials().context("Geef --client-id en --client-secret mee of zet ze in je .env")?,
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

/// Graaft obscure underground tracks op voor de opgegeven genres
async fn harvest_underdog_tracks(
    spotify: &AuthCodeSpotify,
    genres: &[String],
    max_pop: u32,
    target_count: usize,
) -> Result<Vec<FullTrack>> {
    let mut tracks = Vec::new();
    let mut seen_artists = HashSet::new();
    let alphabet: Vec<char> = ('a'..='z').collect();

    let mut attempts = 0;
    while tracks.len() < target_count && attempts < 40 {
        attempts += 1;
        let genre = genres.choose(&mut rand::thread_rng()).unwrap();
        let letter = alphabet.choose(&mut rand::thread_rng()).unwrap();
        let random_offset = rand::thread_rng().gen_range(0..20);

        let queries = [
            format!("genre:\"{}\" tag:hipster {}*", genre, letter),
            format!("\"{}\" tag:hipster {}*", genre, letter),
            format!("\"{}\" tag:hipster", genre),
            format!("genre:\"{}\" tag:hipster", genre),
            format!("\"{}\" {}*", genre, letter),
        ];

        let mut batch_tracks = Vec::new();
        for query in &queries {
            let res = spotify
                .search(
                    query,
                    SearchType::Track,
                    None,
                    None,
                    Some(10), // Spotify Web API ondersteunt betrouwbaar 10 items
                    Some(random_offset),
                )
                .await;

            if let Ok(SearchResult::Tracks(page)) = res {
                if !page.items.is_empty() {
                    batch_tracks = page.items;
                    break;
                }
            }
        }

        for track in batch_tracks {
            if track.popularity <= max_pop {
                if let Some(first_artist) = track.artists.first() {
                    // Maximaal 1 track per artiest voor een diverse selectie
                    if seen_artists.insert(first_artist.name.clone()) {
                        tracks.push(track);
                        if tracks.len() >= target_count {
                            break;
                        }
                    }
                }
            }
        }
    }

    Ok(tracks)
}

async fn run_sync(config_path: &str, dry_run: bool) -> Result<()> {
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

    let mut summary_lines = Vec::new();
    summary_lines.push(format!("# 📻 Sub-1K Auto-Curator Sync Report"));
    summary_lines.push(format!("**Anchor Artist:** `{}`\n", config.settings.anchor_artist));

    for pl in &config.playlists {
        println!("\n📂 Playlist: {} ({})", pl.name, pl.spotify_playlist_id);

        if pl.spotify_playlist_id.starts_with("YOUR_PLAYLIST_ID") {
            println!("  ⚠️ Sla over: playlist ID is nog niet geconfigureerd in {}", config_path);
            continue;
        }

        println!("  🔍 Graven naar tracks met populariteit <= {}...", config.settings.max_popularity);
        let mut tracks = harvest_underdog_tracks(
            &spotify,
            &pl.genres,
            config.settings.max_popularity,
            config.settings.playlist_size.saturating_sub(1),
        )
        .await?;

        println!("  ✨ {} obscure tracks gevonden.", tracks.len());

        // Zoek de anchor track op en plaats deze in de top posities
        if let Ok(Some(anchor_track)) = fetch_anchor_track(&spotify, &pl.anchor_track_query).await {
            tracks.retain(|t| t.id != anchor_track.id);

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

        // Beschrijving bijwerken met datum en open-source credit
        let today = chrono::Utc::now().format("%Y-%m-%d").to_string();
        let description = format!(
            "Daily underdog discovery ({}). Under 1K streams. Curated by {} // Powered by open-source sub1k-sync",
            today, config.settings.anchor_artist
        );

        let _ = spotify
            .playlist_change_detail(
                playlist_id,
                None,
                None,
                Some(&description),
                None,
            )
            .await;

        println!("  ✅ Playlist succesvol bijgewerkt ({} nummers).", tracks.len());

        summary_lines.push(format!("### {}", pl.name));
        summary_lines.push(format!("Bijgewerkt met **{}** tracks.", tracks.len()));
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
        Some(Commands::Sync { config, dry_run }) => run_sync(&config, dry_run).await,
        None => run_sync(&cli.config, cli.dry_run).await,
    }
}
