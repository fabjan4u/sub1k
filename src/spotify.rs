#![allow(dead_code)]
#![allow(deprecated)]

use anyhow::{bail, Context, Result};
use rspotify::{
    clients::{BaseClient, OAuthClient},
    model::{ArtistId, CurrentPlaybackContext, Device, FullArtist, FullTrack, PlayableId, TrackId},
    scopes, AuthCodePkceSpotify, Config, Credentials, OAuth,
};

use crate::config::AppConfig;

/// Builds an instance of AuthCodePkceSpotify configured for sub1k
pub fn build_client(config: &AppConfig) -> Result<AuthCodePkceSpotify> {
    if !config.is_client_id_configured() {
        bail!(
            "Spotify Client ID is nog niet geconfigureerd in {:?}.\nVoer `sub1k setup` uit voor instructies.",
            AppConfig::config_file_path()
        );
    }

    let creds = Credentials::new_pkce(&config.client_id);
    let scopes = scopes!(
        "user-read-playback-state",
        "user-modify-playback-state",
        "user-read-currently-playing",
        "user-library-modify",
        "user-library-read",
        "user-follow-modify"
    );

    let oauth = OAuth {
        redirect_uri: config.redirect_uri.clone(),
        scopes,
        ..Default::default()
    };

    let rspotify_config = Config {
        token_cached: true,
        cache_path: AppConfig::token_cache_path(),
        ..Default::default()
    };

    Ok(AuthCodePkceSpotify::with_config(
        creds,
        oauth,
        rspotify_config,
    ))
}

/// Check if a cached token exists without popping open the browser
pub async fn has_cached_token(spotify: &AuthCodePkceSpotify) -> bool {
    matches!(spotify.read_token_cache(true).await, Ok(Some(_)))
}

/// Runs PKCE authentication flow. If cached token exists and is valid, loads/refreshes it.
/// If not, opens browser and completes OAuth callback.
pub async fn authenticate(spotify: &mut AuthCodePkceSpotify) -> Result<()> {
    let url = spotify
        .get_authorize_url(None)
        .context("Kon autorisatie-URL niet genereren")?;

    spotify
        .prompt_for_token(&url)
        .await
        .context("Fout tijdens Spotify authenticatie (PKCE)")?;

    Ok(())
}

/// Returns currently logged in Spotify user display name or ID
pub async fn get_current_user_name(spotify: &AuthCodePkceSpotify) -> Result<String> {
    let user = spotify
        .current_user()
        .await
        .context("Kon profiel van huidige gebruiker niet ophalen")?;
    Ok(user.display_name.unwrap_or(user.id.to_string()))
}

/// Fetch list of all Spotify Connect devices
pub async fn get_devices(spotify: &AuthCodePkceSpotify) -> Result<Vec<Device>> {
    let devices = spotify
        .device()
        .await
        .context("Kon lijst met Spotify Connect apparaten niet ophalen")?;
    Ok(devices)
}

/// Returns the currently active device, or the first available device
pub async fn get_active_or_first_device(spotify: &AuthCodePkceSpotify) -> Result<Option<Device>> {
    let devices = get_devices(spotify).await?;
    let active = devices.iter().find(|d| d.is_active).cloned();
    Ok(active.or_else(|| devices.into_iter().next()))
}

/// Get current playback state (now playing, progress, device)
pub async fn get_playback(spotify: &AuthCodePkceSpotify) -> Result<Option<CurrentPlaybackContext>> {
    let playback = spotify
        .current_playback(None, None::<Vec<_>>)
        .await
        .context("Kon huidige afspeelstatus niet ophalen")?;
    Ok(playback)
}

/// Play a specific track on a device (or current active device if None)
pub async fn play_track(
    spotify: &AuthCodePkceSpotify,
    track_id: &str,
    device_id: Option<&str>,
) -> Result<()> {
    let track = TrackId::from_id_or_uri(track_id)
        .with_context(|| format!("Ongeldig Track ID of URI: {}", track_id))?;
    let playable = PlayableId::Track(track);

    spotify
        .start_uris_playback([playable], device_id, None, None)
        .await
        .context("Kon afspelen niet starten. Controleer of Spotify open staat en een actief apparaat is geselecteerd.")?;
    Ok(())
}

/// Start playing a list/queue of tracks starting from the first one
pub async fn play_queue_from(
    spotify: &AuthCodePkceSpotify,
    track_ids: &[String],
    device_id: Option<&str>,
) -> Result<()> {
    let mut playables = Vec::new();
    for id in track_ids {
        if let Ok(track) = TrackId::from_id_or_uri(id) {
            playables.push(PlayableId::Track(track));
        }
    }

    if playables.is_empty() {
        bail!("Geen geldige tracks om af te spelen.");
    }

    spotify
        .start_uris_playback(playables, device_id, None, None)
        .await
        .context("Kon afspeellijst niet starten op Spotify Connect apparaat.")?;
    Ok(())
}

/// Set volume percent (0 - 100) on active device
pub async fn set_volume(
    spotify: &AuthCodePkceSpotify,
    volume_percent: u8,
    device_id: Option<&str>,
) -> Result<()> {
    spotify
        .volume(volume_percent, device_id)
        .await
        .context("Kon volume niet instellen")?;
    Ok(())
}

/// Smooth crossfade transition: ramps volume down, switches track queue, and ramps volume back up
pub async fn fade_transition_to_queue(
    spotify: &AuthCodePkceSpotify,
    track_ids: &[String],
    initial_volume: u8,
    device_id: Option<&str>,
) -> Result<()> {
    let target_vol = initial_volume.clamp(15, 100);

    // 1. Smooth fade out (3 quick steps over ~300ms)
    for step in [60, 25, 0] {
        let vol = ((target_vol as u32 * step) / 100) as u8;
        let _ = spotify.volume(vol, device_id).await;
        tokio::time::sleep(std::time::Duration::from_millis(90)).await;
    }

    // 2. Start new track queue
    play_queue_from(spotify, track_ids, device_id).await?;

    // 3. Smooth fade in (4 quick steps over ~400ms)
    for step in [25, 55, 80, 100] {
        tokio::time::sleep(std::time::Duration::from_millis(90)).await;
        let vol = ((target_vol as u32 * step) / 100) as u8;
        let _ = spotify.volume(vol, device_id).await;
    }

    Ok(())
}

/// Pause playback
pub async fn pause(spotify: &AuthCodePkceSpotify, device_id: Option<&str>) -> Result<()> {
    spotify
        .pause_playback(device_id)
        .await
        .context("Kon afspelen niet pauzeren")?;
    Ok(())
}

/// Resume playback
pub async fn resume(spotify: &AuthCodePkceSpotify, device_id: Option<&str>) -> Result<()> {
    spotify
        .resume_playback(device_id, None)
        .await
        .context("Kon afspelen niet hervatten")?;
    Ok(())
}

/// Transfer playback to a different device
pub async fn transfer_device(spotify: &AuthCodePkceSpotify, device_id: &str) -> Result<()> {
    spotify
        .transfer_playback(device_id, Some(true))
        .await
        .context("Kon afspelen niet overdragen naar apparaat")?;
    Ok(())
}

/// Rescue a track: adds it to user's 'Liked Songs' (crucial algorithmic signal for Spotify)
pub async fn rescue_track(spotify: &AuthCodePkceSpotify, track_id: &str) -> Result<()> {
    let track = TrackId::from_id_or_uri(track_id)
        .with_context(|| format!("Ongeldig Track ID of URI: {}", track_id))?;
    spotify
        .current_user_saved_tracks_add([track])
        .await
        .context("Kon track niet opslaan in Liked Songs")?;
    Ok(())
}

/// Follow an artist directly
pub async fn follow_artist(spotify: &AuthCodePkceSpotify, artist_id: &str) -> Result<()> {
    let artist = ArtistId::from_id_or_uri(artist_id)
        .with_context(|| format!("Ongeldig Artist ID of URI: {}", artist_id))?;
    spotify
        .user_follow_artists([artist])
        .await
        .context("Kon artiest niet volgen")?;
    Ok(())
}

/// Fetch full details for a track
pub async fn get_track(spotify: &AuthCodePkceSpotify, track_id: &str) -> Result<FullTrack> {
    let track = TrackId::from_id_or_uri(track_id)
        .with_context(|| format!("Ongeldig Track ID of URI: {}", track_id))?;
    let full = spotify
        .track(track, None)
        .await
        .with_context(|| format!("Kon track details niet ophalen voor {}", track_id))?;
    Ok(full)
}

/// Fetch full details for an artist
pub async fn get_artist(spotify: &AuthCodePkceSpotify, artist_id: &str) -> Result<FullArtist> {
    let artist = ArtistId::from_id_or_uri(artist_id)
        .with_context(|| format!("Ongeldig Artist ID of URI: {}", artist_id))?;
    let full = spotify
        .artist(artist)
        .await
        .with_context(|| format!("Kon artiest details niet ophalen voor {}", artist_id))?;
    Ok(full)
}

/// Bulk fetch details for up to 50 artists at once (prevents 429 rate limit errors!)
pub async fn get_artists_bulk(
    spotify: &AuthCodePkceSpotify,
    artist_ids: &[&str],
) -> Result<Vec<FullArtist>> {
    let mut parsed_ids = Vec::with_capacity(artist_ids.len());
    for id in artist_ids {
        if let Ok(parsed) = ArtistId::from_id_or_uri(id) {
            parsed_ids.push(parsed);
        }
    }

    if parsed_ids.is_empty() {
        return Ok(Vec::new());
    }

    let artists = spotify
        .artists(parsed_ids)
        .await
        .context("Kon artiesten niet ophalen via bulk API")?;
    Ok(artists)
}
