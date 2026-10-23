#![allow(deprecated)]

mod app;
mod config;
mod digger;
mod spotify;
mod tui;
mod ui;
mod worker;

use anyhow::Result;
use clap::{Parser, Subcommand};
use config::AppConfig;
use rspotify::model::Id;

#[derive(Parser, Debug)]
#[command(name = "sub1k")]
#[command(author = "Sub-1K Radio Contributors")]
#[command(version = "0.1.0")]
#[command(about = "Terminal player & discovery engine rescuing indie tracks under Spotify's 1000 stream threshold", long_about = None)]
struct Cli {
    #[command(subcommand)]
    command: Option<Commands>,
}

#[derive(Subcommand, Debug)]
enum Commands {
    /// Launch the interactive Sub-1K Radio Terminal UI (Default)
    Radio {
        /// Target genre (defaults to config value)
        #[arg(short, long)]
        genre: Option<String>,

        /// Disable injection of creator seed tracks
        #[arg(long)]
        no_featured: bool,
    },

    /// Configure Spotify Client ID and display setup instructions
    Setup {
        /// Directly provide your Spotify Client ID (e.g. sub1k setup d6ccbb...)
        #[arg(value_name = "CLIENT_ID")]
        client_id: Option<String>,

        /// Optional alternative flag: --client-id <ID> or -c <ID>
        #[arg(short, long)]
        id: Option<String>,
    },

    /// Authenticate with Spotify via PKCE OAuth (opens browser)
    Auth,

    /// Show current configuration, login status, and active playback
    Status,

    /// List all available Spotify Connect devices
    Devices,

    /// Dig up obscure underground tracks (< 1000 streams) for a genre
    Dig {
        /// Target genre (e.g. synthwave, dark ambient, post-punk, industrial)
        #[arg(short, long)]
        genre: Option<String>,

        /// Maximum number of tracks to discover
        #[arg(short, long, default_value_t = 10)]
        limit: usize,

        /// Automatically play the first discovered track on the active Spotify player
        #[arg(short, long)]
        play: bool,

        /// Disable injection of creator seed tracks
        #[arg(long)]
        no_featured: bool,
    },

    /// Play a specific track ID or Spotify URI
    Play {
        /// Spotify track ID (e.g. 6fmKTHiDrUBVV8ZRudWEws) or URI
        track: String,
        /// Optional target Spotify Connect device ID
        #[arg(short, long)]
        device: Option<String>,
    },

    /// Save a track to your Liked Songs ('Rescue' the track)
    Rescue {
        /// Spotify track ID or URI
        track: String,
    },
}

#[tokio::main]
async fn main() -> Result<()> {
    let cli = Cli::parse();
    let config = AppConfig::load()?;

    match cli.command {
        Some(Commands::Radio { genre, no_featured }) => {
            let mut updated_config = config.clone();
            if let Some(g) = genre {
                updated_config.default_genre = g;
            }
            if no_featured {
                updated_config.include_featured = false;
            }

            if !updated_config.is_client_id_configured() {
                AppConfig::print_setup_instructions();
                return Ok(());
            }

            let client = spotify::build_client(&updated_config)?;
            if !spotify::has_cached_token(&client).await {
                println!("Nog niet ingelogd bij Spotify. Voer eerst `sub1k auth` uit om via je browser in te loggen.");
                return Ok(());
            }

            tui::run_tui(updated_config).await?;
        }

        Some(Commands::Setup { client_id, id }) => {
            let target_id = client_id.or(id);
            if let Some(new_id) = target_id {
                let mut updated_config = config;
                updated_config.client_id = new_id.trim().to_string();
                let config_path = AppConfig::config_file_path();
                updated_config.save_to(&config_path)?;
                println!(
                    "✓ Spotify Client ID succesvol opgeslagen in {:?}!",
                    config_path
                );
            } else {
                AppConfig::print_setup_instructions();
            }
        }

        Some(Commands::Auth) => {
            if !config.is_client_id_configured() {
                AppConfig::print_setup_instructions();
                return Ok(());
            }

            println!("Bezig met opstarten van Spotify PKCE authenticatie...");
            let mut client = spotify::build_client(&config)?;
            spotify::authenticate(&mut client).await?;
            let user = spotify::get_current_user_name(&client).await?;
            println!("\n✓ Succesvol ingelogd bij Spotify als: {}", user);
            println!(
                "  Token is veilig opgeslagen in {:?}",
                AppConfig::token_cache_path()
            );
        }

        Some(Commands::Status) => {
            println!("─── Sub-1K Radio Status ───");
            println!("Configuratiebestand: {:?}", AppConfig::config_file_path());
            println!("Token cache:        {:?}", AppConfig::token_cache_path());
            println!(
                "Client ID:          {}",
                if config.is_client_id_configured() {
                    &config.client_id
                } else {
                    "[NIET INGESTELD]"
                }
            );
            println!("Default genre:      {}", config.default_genre);
            println!("Max popularity:     <= {}", config.max_popularity);
            println!(
                "Featured seed:      {} (Track IDs: {})",
                config.featured_artist_name,
                config.featured_track_ids.join(", ")
            );

            if !config.is_client_id_configured() {
                println!("\nLet op: Spotify Client ID ontbreekt. Voer `sub1k setup` uit.");
                return Ok(());
            }

            println!("\nVerbinding maken met Spotify...");
            let mut client = match spotify::build_client(&config) {
                Ok(c) => c,
                Err(e) => {
                    eprintln!("Fout: {}", e);
                    return Ok(());
                }
            };

            if !spotify::has_cached_token(&client).await {
                println!("Inlogstatus:        ✗ Nog niet ingelogd.");
                println!("                    Voer `cargo run -- auth` uit om via je browser in te loggen.");
                return Ok(());
            }

            if let Err(e) = spotify::authenticate(&mut client).await {
                eprintln!("Authenticatiefout: {}", e);
                return Ok(());
            }

            match spotify::get_current_user_name(&client).await {
                Ok(user) => println!("Ingelogd als:       ✓ {}", user),
                Err(e) => println!("Inlogstatus:        ✗ Fout ({})", e),
            }

            match spotify::get_devices(&client).await {
                Ok(devices) => {
                    println!("\nBeschikbare apparaten (Spotify Connect):");
                    if devices.is_empty() {
                        println!("  (Geen actieve apparaten gevonden. Start de Spotify desktop app of web player)");
                    } else {
                        for d in &devices {
                            let mark = if d.is_active {
                                "▶ [ACTIEF]"
                            } else {
                                "  [INACTIEF]"
                            };
                            println!(
                                "  {} {} (ID: {}) - Type: {:?}",
                                mark,
                                d.name,
                                d.id.as_deref().unwrap_or("?"),
                                d._type
                            );
                        }
                    }
                }
                Err(e) => eprintln!("Fout bij ophalen apparaten: {}", e),
            }

            match spotify::get_playback(&client).await {
                Ok(Some(pb)) => {
                    println!("\nNu aan het afspelen op {}:", pb.device.name);
                    println!(
                        "  Status:     {}",
                        if pb.is_playing {
                            "Afspelen ▶"
                        } else {
                            "Gepauzeerd ⏸"
                        }
                    );
                    if let Some(item) = pb.item {
                        match item {
                            rspotify::model::PlayableItem::Track(track) => {
                                let artists = track
                                    .artists
                                    .iter()
                                    .map(|a| a.name.as_str())
                                    .collect::<Vec<_>>()
                                    .join(", ");
                                println!("  Nummer:     {} - {}", artists, track.name);
                                println!("  Populariteit: {}%", track.popularity);
                                println!("  Album:      {}", track.album.name);
                                if let Some(id) = track.id {
                                    println!("  Track ID:   {}", id);
                                }
                            }
                            rspotify::model::PlayableItem::Episode(ep) => {
                                println!("  Podcast:    {}", ep.name);
                            }
                            _ => {}
                        }
                    }
                }
                Ok(None) => println!("\nEr speelt momenteel niets af."),
                Err(e) => eprintln!("Fout bij ophalen afspeelstatus: {}", e),
            }
        }

        Some(Commands::Dig {
            genre,
            limit,
            play,
            no_featured,
        }) => {
            let mut updated_config = config.clone();
            if no_featured {
                updated_config.include_featured = false;
            }
            let target_genre = genre.unwrap_or_else(|| updated_config.default_genre.clone());

            let mut client = spotify::build_client(&updated_config)?;
            spotify::authenticate(&mut client).await?;

            println!(
                "🔍 Bezig met opgraven van tracks in genre '{}'...",
                target_genre
            );
            println!(
                "   (Filters: Populariteit <= {}%, tag:hipster)",
                updated_config.max_popularity
            );

            let digger = digger::CrateDigger::new(client.clone(), updated_config);
            let discovered = digger.dig_tracks(&target_genre, limit).await?;

            if discovered.is_empty() {
                println!("\nGeen nummers gevonden die aan de criteria voldeden. Probeer een ander genre.");
                return Ok(());
            }

            println!(
                "\n═══ Gevonden Underdog Tracks ({} resultaten) ═══",
                discovered.len()
            );
            for (idx, item) in discovered.iter().enumerate() {
                let track = &item.track;
                let artists = track
                    .artists
                    .iter()
                    .map(|a| a.name.as_str())
                    .collect::<Vec<_>>()
                    .join(", ");
                let seed_badge = if item.is_featured {
                    " 🌟 [CREATOR SEED]"
                } else {
                    ""
                };
                let followers_str = item
                    .artist_followers
                    .map(|f| format!("{} followers", f))
                    .unwrap_or_else(|| "Nieuw/onbekend".to_string());
                let track_id_str = track.id.as_ref().map(|id| id.id()).unwrap_or("geen ID");

                println!("{:2}. {} - {}{}", idx + 1, artists, track.name, seed_badge);
                println!(
                    "    Pop: {}% | Artiest: {} | ID: {}",
                    track.popularity, followers_str, track_id_str
                );
            }

            if play {
                if let Some(first) = discovered.first() {
                    if let Some(id) = &first.track.id {
                        println!("\n▶ Afspelen starten van: {}...", first.track.name);
                        match spotify::play_track(&client, id.id(), None).await {
                            Ok(_) => println!("✓ Spelen gestart op je actieve Spotify-client!"),
                            Err(e) => eprintln!("✗ Kon afspelen niet starten: {}", e),
                        }
                    }
                }
            } else {
                println!("\nTip: Voeg `--play` toe aan het commando om direct de eerste track af te spelen.");
            }
        }

        Some(Commands::Devices) => {
            let mut client = spotify::build_client(&config)?;
            spotify::authenticate(&mut client).await?;
            let devices = spotify::get_devices(&client).await?;
            println!("Spotify Connect apparaten ({} gevonden):", devices.len());
            for d in devices {
                let status = if d.is_active {
                    "* ACTIEF *"
                } else {
                    "inactief"
                };
                println!(
                    "- {} [{}] (ID: {}, Volume: {}%)",
                    d.name,
                    status,
                    d.id.as_deref().unwrap_or("?"),
                    d.volume_percent.unwrap_or(0)
                );
            }
        }

        Some(Commands::Play { track, device }) => {
            let mut client = spotify::build_client(&config)?;
            spotify::authenticate(&mut client).await?;
            println!("Track {} starten...", track);
            spotify::play_track(&client, &track, device.as_deref()).await?;
            println!("✓ Afspelen gestart!");
        }

        Some(Commands::Rescue { track }) => {
            let mut client = spotify::build_client(&config)?;
            spotify::authenticate(&mut client).await?;
            println!("Track {} redden (opslaan in Liked Songs)...", track);
            spotify::rescue_track(&client, &track).await?;
            let mut stats = config::RescuedStats::load();
            stats.record_rescue(track.clone());
            println!(
                "✓ Track gered en toegevoegd aan je Liked Songs! (Totaal gered: {})",
                stats.total_rescued
            );
        }

        None => {
            if !config.is_client_id_configured() {
                AppConfig::print_setup_instructions();
            } else {
                let client = spotify::build_client(&config)?;
                if spotify::has_cached_token(&client).await {
                    tui::run_tui(config).await?;
                } else {
                    println!("Sub-1K Radio v0.1.0");
                    println!("Je bent nog niet ingelogd. Voer `sub1k auth` uit om via je browser verbinding te maken.");
                }
            }
        }
    }

    Ok(())
}
