#![allow(deprecated)]

use rspotify::{model::Id, AuthCodePkceSpotify};
use std::time::Duration;
use tokio::sync::mpsc;
use tokio::time::sleep;

use crate::app::{Action, PlaybackInfo, Update};
use crate::config::AppConfig;
use crate::digger::CrateDigger;
use crate::spotify;

pub async fn run_worker(
    config: AppConfig,
    mut action_rx: mpsc::Receiver<Action>,
    update_tx: mpsc::Sender<Update>,
) {
    let mut client = match spotify::build_client(&config) {
        Ok(c) => c,
        Err(e) => {
            let _ = update_tx
                .send(Update::Error(format!("Configuratiefout: {}", e)))
                .await;
            return;
        }
    };

    if let Err(e) = spotify::authenticate(&mut client).await {
        let _ = update_tx
            .send(Update::Error(format!("Authenticatiefout: {}", e)))
            .await;
        return;
    }

    let digger = CrateDigger::new(client.clone(), config.clone());

    // 1. Initial background fetch: devices & playback
    if let Ok(devices) = spotify::get_devices(&client).await {
        let _ = update_tx.send(Update::DevicesLoaded(devices)).await;
    }

    if let Some(pb) = extract_playback_info(&client, &config.featured_track_ids).await {
        let _ = update_tx.send(Update::PlaybackChanged(Some(pb))).await;
    }

    // 2. Initial background dig on default genre
    let initial_genre = config.default_genre.clone();
    let initial_limit = config.queue_initial_limit;
    let initial_digger = digger.clone();
    let initial_tx = update_tx.clone();
    tokio::spawn(async move {
        let _ = initial_tx
            .send(Update::Status(format!(
                "🔍 Obscure tracks opgraven voor '{}'...",
                initial_genre
            )))
            .await;
        match initial_digger
            .dig_tracks(&initial_genre, initial_limit)
            .await
        {
            Ok(tracks) => {
                let count = tracks.len();
                let _ = initial_tx.send(Update::TracksDiscovered(tracks)).await;
                let _ = initial_tx
                    .send(Update::Status(format!(
                        "✓ {} underdog tracks opgegraven!",
                        count
                    )))
                    .await;
            }
            Err(e) => {
                let _ = initial_tx
                    .send(Update::Error(format!("Fout bij opgraven: {}", e)))
                    .await;
            }
        }
    });

    // 3. Worker loop: listening for UI actions and periodic polling
    let mut poll_interval = tokio::time::interval(Duration::from_millis(1500));
    let mut current_volume: u8 = 80;

    loop {
        tokio::select! {
            _ = poll_interval.tick() => {
                let pb = extract_playback_info(&client, &config.featured_track_ids).await;
                if let Some(info) = &pb {
                    if let Some(vol) = info.device_volume {
                        if vol > 0 {
                            current_volume = vol as u8;
                        }
                    }
                }
                let _ = update_tx.send(Update::PlaybackChanged(pb)).await;
            }

            action = action_rx.recv() => {
                match action {
                    Some(Action::PlayPause) => {
                        let is_currently_playing = extract_playback_info(&client, &config.featured_track_ids)
                            .await
                            .map(|p| p.is_playing)
                            .unwrap_or(false);

                        if is_currently_playing {
                            if let Err(e) = spotify::pause(&client, None).await {
                                let _ = update_tx.send(Update::Error(format!("Kon niet pauzeren: {}", e))).await;
                            }
                        } else {
                            if let Err(e) = spotify::resume(&client, None).await {
                                let _ = update_tx.send(Update::Error(format!("Kon niet hervatten: {}", e))).await;
                            }
                        }

                        sleep(Duration::from_millis(200)).await;
                        let pb = extract_playback_info(&client, &config.featured_track_ids).await;
                        let _ = update_tx.send(Update::PlaybackChanged(pb)).await;
                    }

                    Some(Action::PlayTrack(track_id)) => {
                        let client_clone = client.clone();
                        let vol = current_volume;
                        let ids = vec![track_id];
                        tokio::spawn(async move {
                            let _ = spotify::fade_transition_to_queue(&client_clone, &ids, vol, None).await;
                        });

                        sleep(Duration::from_millis(600)).await;
                        let pb = extract_playback_info(&client, &config.featured_track_ids).await;
                        let _ = update_tx.send(Update::PlaybackChanged(pb)).await;
                    }

                    Some(Action::PlayQueue(track_ids)) => {
                        let client_clone = client.clone();
                        let vol = current_volume;
                        tokio::spawn(async move {
                            let _ = spotify::fade_transition_to_queue(&client_clone, &track_ids, vol, None).await;
                        });

                        sleep(Duration::from_millis(600)).await;
                        let pb = extract_playback_info(&client, &config.featured_track_ids).await;
                        let _ = update_tx.send(Update::PlaybackChanged(pb)).await;
                    }

                    Some(Action::RescueTrack(track_id)) => {
                        match spotify::rescue_track(&client, &track_id).await {
                            Ok(_) => {
                                let _ = update_tx.send(Update::TrackRescued(track_id)).await;
                                let _ = update_tx.send(Update::Status("✓ Track gered en toegevoegd aan Liked Songs!".to_string())).await;
                            }
                            Err(e) => {
                                let _ = update_tx.send(Update::Error(format!("Kon track niet redden: {}", e))).await;
                            }
                        }
                    }

                    Some(Action::FollowArtist(artist_id)) => {
                        match spotify::follow_artist(&client, &artist_id).await {
                            Ok(_) => {
                                let _ = update_tx.send(Update::Status("✓ Artiest gevolgd op Spotify!".to_string())).await;
                            }
                            Err(e) => {
                                let _ = update_tx.send(Update::Error(format!("Kon artiest niet volgen: {}", e))).await;
                            }
                        }
                    }

                    Some(Action::DigGenre(genre)) => {
                        let digger_clone = digger.clone();
                        let limit = config.queue_initial_limit;
                        let tx = update_tx.clone();
                        tokio::spawn(async move {
                            let _ = tx.send(Update::Status(format!("🔍 Tracks opgraven voor '{}'...", genre))).await;
                            match digger_clone.dig_tracks(&genre, limit).await {
                                Ok(tracks) => {
                                    let count = tracks.len();
                                    let _ = tx.send(Update::TracksDiscovered(tracks)).await;
                                    let _ = tx.send(Update::Status(format!("✓ {} tracks gevonden voor '{}'!", count, genre))).await;
                                }
                                Err(e) => {
                                    let _ = tx.send(Update::Error(format!("Fout bij opgraven: {}", e))).await;
                                }
                            }
                        });
                    }

                    Some(Action::ReplenishQueue(genre)) => {
                        let digger_clone = digger.clone();
                        let replenish_limit = config.queue_replenish_limit;
                        let tx = update_tx.clone();
                        tokio::spawn(async move {
                            if let Ok(tracks) = digger_clone.dig_tracks(&genre, replenish_limit).await {
                                let _ = tx.send(Update::TracksDiscovered(tracks)).await;
                            }
                        });
                    }

                    Some(Action::SelectDevice(device_id)) => {
                        match spotify::transfer_device(&client, &device_id).await {
                            Ok(_) => {
                                let _ = update_tx.send(Update::Status("✓ Spotify apparaat geselecteerd!".to_string())).await;
                                sleep(Duration::from_millis(300)).await;
                                if let Ok(devices) = spotify::get_devices(&client).await {
                                    let _ = update_tx.send(Update::DevicesLoaded(devices)).await;
                                }
                            }
                            Err(e) => {
                                let _ = update_tx.send(Update::Error(format!("Fout bij apparaatselectie: {}", e))).await;
                            }
                        }
                    }

                    Some(Action::RefreshDevices) => {
                        if let Ok(devices) = spotify::get_devices(&client).await {
                            let _ = update_tx.send(Update::DevicesLoaded(devices)).await;
                        }
                    }

                    Some(Action::RefreshPlayback) => {
                        let pb = extract_playback_info(&client, &config.featured_track_ids).await;
                        let _ = update_tx.send(Update::PlaybackChanged(pb)).await;
                    }

                    Some(Action::NextTrack) => {
                        // Handled in main / app loop by picking next track and issuing Action::PlayTrack
                    }

                    Some(Action::Quit) | None => {
                        let _ = spotify::pause(&client, None).await;
                        break;
                    }
                }
            }
        }
    }
}

async fn extract_playback_info(
    spotify: &AuthCodePkceSpotify,
    featured_track_ids: &[String],
) -> Option<PlaybackInfo> {
    let pb = spotify::get_playback(spotify).await.ok().flatten()?;
    let item = pb.item?;

    match item {
        rspotify::model::PlayableItem::Track(track) => {
            let artist_names = track
                .artists
                .iter()
                .map(|a| a.name.clone())
                .collect::<Vec<_>>()
                .join(", ");

            let artist_id = track
                .artists
                .first()
                .and_then(|a| a.id.as_ref().map(|id| id.id().to_string()));

            let track_id_str = track.id.as_ref().map(|id| id.id().to_string());
            let is_featured = track_id_str
                .as_ref()
                .map(|id| featured_track_ids.contains(id))
                .unwrap_or(false);

            Some(PlaybackInfo {
                track_title: track.name,
                artist_names,
                album_name: track.album.name,
                track_id: track_id_str,
                artist_id,
                is_playing: pb.is_playing,
                progress_ms: pb
                    .progress
                    .map(|d| d.num_milliseconds().max(0) as u32)
                    .unwrap_or(0),
                duration_ms: track.duration.num_milliseconds().max(0) as u32,
                popularity: track.popularity,
                is_featured,
                device_name: pb.device.name,
                device_volume: pb.device.volume_percent,
            })
        }
        _ => None,
    }
}
