use anyhow::Result;
use crossterm::{
    event::{Event, EventStream, KeyCode, KeyEventKind, KeyModifiers},
    execute,
    terminal::{disable_raw_mode, enable_raw_mode, EnterAlternateScreen, LeaveAlternateScreen},
};
use futures::StreamExt;
use ratatui::{backend::CrosstermBackend, Terminal};
use rspotify::model::Id;
use std::{io, time::Duration};
use tokio::sync::mpsc;

use crate::{
    app::{Action, App, InputMode, Update},
    config::AppConfig,
    ui,
    worker::run_worker,
};

pub async fn run_tui(config: AppConfig) -> Result<()> {
    enable_raw_mode()?;
    let mut stdout = io::stdout();
    execute!(stdout, EnterAlternateScreen)?;
    let backend = CrosstermBackend::new(stdout);
    let mut terminal = Terminal::new(backend)?;

    let (action_tx, action_rx) = mpsc::channel::<Action>(32);
    let (update_tx, mut update_rx) = mpsc::channel::<Update>(32);

    let worker_config = config.clone();
    tokio::spawn(async move {
        run_worker(worker_config, action_rx, update_tx).await;
    });

    let mut app = App::new(config);
    app.is_loading = true;

    let mut event_stream = EventStream::new();
    let mut tick_interval = tokio::time::interval(Duration::from_millis(100));

    while !app.should_quit {
        tokio::select! {
            _ = tick_interval.tick() => {
                let mut should_advance = false;
                let mut advance_reason = "";

                if let Some(pb) = &mut app.playback {
                    if pb.is_playing && pb.duration_ms > 0 {
                        pb.progress_ms = (pb.progress_ms + 100).min(pb.duration_ms);

                        let current_id = pb.track_id.as_deref().unwrap_or("").to_string();
                        let already_handled = app.last_handled_track_id.as_deref() == Some(&current_id);

                        // 1. Smart Boost Trigger: auto-skip after 35-45s
                        if app.boost_mode && !already_handled && pb.progress_ms >= app.target_boost_skip * 1000 {
                            should_advance = true;
                            advance_reason = "⚡ Smart Boost";
                            app.last_handled_track_id = Some(current_id.clone());
                        }

                        // 2. Track Ended Trigger: auto-advance when reaching the end of the song
                        if !already_handled && pb.progress_ms >= pb.duration_ms.saturating_sub(1200) {
                            should_advance = true;
                            advance_reason = "Track afgelopen";
                            app.last_handled_track_id = Some(current_id);
                        }
                    }
                }

                if should_advance && !app.is_transitioning {
                    if let Some(next) = app.next_queue_item() {
                        app.is_transitioning = true;
                        app.reset_boost_jitter();
                        let remaining_ids = app.get_remaining_track_ids();
                        if !remaining_ids.is_empty() {
                            let _ = action_tx.send(Action::PlayQueue(remaining_ids)).await;
                            app.set_status(format!("{} ➔ Fade naar: {}", advance_reason, next.track.name));
                        }
                    }
                }

                app.clear_expired_status();

                // Auto-replenish queue when running low (less than 4 tracks remaining)
                if !app.queue.is_empty() && app.queue_index + 4 >= app.queue.len() && !app.is_replenishing {
                    app.is_replenishing = true;
                    let _ = action_tx.send(Action::ReplenishQueue(app.genre.clone())).await;
                }

                terminal.draw(|f| ui::render(f, &app))?;
            }

            Some(update) = update_rx.recv() => {
                match update {
                    Update::PlaybackChanged(pb) => {
                        if let Some(info) = &pb {
                            if let Some(new_id) = &info.track_id {
                                if app.last_handled_track_id.as_ref() != Some(new_id) {
                                    app.is_transitioning = false;
                                }

                                // Sync queue index with active playing track
                                if let Some(idx) = app.queue.iter().position(|t| {
                                    t.track.id.as_ref().map(|id| id.id()) == Some(new_id.as_str())
                                }) {
                                    app.queue_index = idx;
                                }
                            }
                        }
                        app.playback = pb;
                    }
                    Update::TracksDiscovered(tracks) => {
                        app.is_loading = false;
                        app.is_replenishing = false;
                        if app.queue.is_empty() {
                            app.queue = tracks;
                            app.queue_index = 0;
                        } else {
                            for t in tracks {
                                if !app.queue.iter().any(|existing| existing.track.id == t.track.id) {
                                    app.queue.push(t);
                                }
                            }
                        }
                    }
                    Update::DevicesLoaded(devices) => {
                        app.available_devices = devices;
                    }
                    Update::Status(msg) => {
                        app.set_status(msg);
                    }
                    Update::Error(err) => {
                        app.set_status(format!("✗ {}", err));
                    }
                    Update::TrackRescued(id) => {
                        let mut stats = crate::config::RescuedStats::load();
                        stats.record_rescue(id);
                        app.rescued_count = stats.total_rescued;
                        app.set_status("🌟 Track gered & toegevoegd aan Liked Songs!");
                    }
                }
            }

            Some(Ok(event)) = event_stream.next() => {
                if let Event::Key(key) = event {
                    if key.kind == KeyEventKind::Press {
                        match app.input_mode {
                            InputMode::Normal => {
                                match key.code {
                                    KeyCode::Char('q') => {
                                        app.should_quit = true;
                                        let _ = action_tx.send(Action::Quit).await;
                                    }
                                    KeyCode::Char('c') if key.modifiers.contains(KeyModifiers::CONTROL) => {
                                        app.should_quit = true;
                                        let _ = action_tx.send(Action::Quit).await;
                                    }
                                    KeyCode::Char(' ') => {
                                        let _ = action_tx.send(Action::PlayPause).await;
                                    }
                                    KeyCode::Char('n') => {
                                        if let Some(next) = app.next_queue_item() {
                                            let remaining_ids = app.get_remaining_track_ids();
                                            if !remaining_ids.is_empty() {
                                                let _ = action_tx.send(Action::PlayQueue(remaining_ids)).await;
                                                app.set_status(format!("▶ Crossfade naar: {}", next.track.name));
                                            }
                                        }
                                    }
                                    KeyCode::Char('s') => {
                                        let track_id_to_rescue = app.playback.as_ref()
                                            .and_then(|p| p.track_id.clone())
                                            .or_else(|| {
                                                app.queue.get(app.queue_index)
                                                    .and_then(|item| item.track.id.as_ref().map(|id| id.id().to_string()))
                                            });

                                        if let Some(id) = track_id_to_rescue {
                                            let _ = action_tx.send(Action::RescueTrack(id)).await;
                                        } else {
                                            app.set_status("Geen actieve track om te redden.");
                                        }
                                    }
                                    KeyCode::Char('f') => {
                                        let artist_id_to_follow = app.playback.as_ref()
                                            .and_then(|p| p.artist_id.clone())
                                            .or_else(|| {
                                                app.queue.get(app.queue_index)
                                                    .and_then(|item| item.track.artists.first())
                                                    .and_then(|a| a.id.as_ref().map(|id| id.id().to_string()))
                                            });

                                        if let Some(id) = artist_id_to_follow {
                                            let _ = action_tx.send(Action::FollowArtist(id)).await;
                                        } else {
                                            app.set_status("Geen artiest om te volgen.");
                                        }
                                    }
                                    KeyCode::Char('b') => {
                                        app.boost_mode = !app.boost_mode;
                                        app.reset_boost_jitter();
                                        if app.boost_mode {
                                            app.set_status(format!("⚡ Smart Boost geactiveerd! (Skip tussen 35-45s)"));
                                        } else {
                                            app.set_status("Smart Boost gedeactiveerd.");
                                        }
                                    }
                                    KeyCode::Char('g') => {
                                        app.input_mode = InputMode::EnteringGenre;
                                        app.genre_input = app.genre.clone();
                                    }
                                    KeyCode::Char('d') => {
                                        app.input_mode = InputMode::SelectingDevice;
                                        let _ = action_tx.send(Action::RefreshDevices).await;
                                    }
                                    KeyCode::Up => {
                                        if app.queue_index > 0 {
                                            app.queue_index -= 1;
                                        }
                                    }
                                    KeyCode::Down => {
                                        if !app.queue.is_empty() && app.queue_index + 1 < app.queue.len() {
                                            app.queue_index += 1;
                                        }
                                    }
                                    KeyCode::Enter => {
                                        if let Some(selected) = app.queue.get(app.queue_index) {
                                            let remaining_ids = app.get_remaining_track_ids();
                                            if !remaining_ids.is_empty() {
                                                let _ = action_tx.send(Action::PlayQueue(remaining_ids)).await;
                                                app.set_status(format!("▶ Crossfade naar: {}", selected.track.name));
                                            }
                                        }
                                    }
                                    _ => {}
                                }
                            }
                            InputMode::EnteringGenre => {
                                match key.code {
                                    KeyCode::Enter => {
                                        let new_genre = app.genre_input.trim().to_string();
                                        if !new_genre.is_empty() {
                                            app.genre = new_genre.clone();
                                            app.is_loading = true;
                                            app.queue.clear();
                                            let _ = action_tx.send(Action::DigGenre(new_genre)).await;
                                        }
                                        app.input_mode = InputMode::Normal;
                                    }
                                    KeyCode::Esc => {
                                        app.input_mode = InputMode::Normal;
                                    }
                                    KeyCode::Backspace => {
                                        app.genre_input.pop();
                                    }
                                    KeyCode::Char(c) => {
                                        app.genre_input.push(c);
                                    }
                                    _ => {}
                                }
                            }
                            InputMode::SelectingDevice => {
                                match key.code {
                                    KeyCode::Up => {
                                        if app.device_index > 0 {
                                            app.device_index -= 1;
                                        }
                                    }
                                    KeyCode::Down => {
                                        if !app.available_devices.is_empty() && app.device_index + 1 < app.available_devices.len() {
                                            app.device_index += 1;
                                        }
                                    }
                                    KeyCode::Enter => {
                                        if let Some(device) = app.available_devices.get(app.device_index) {
                                            if let Some(id) = &device.id {
                                                let _ = action_tx.send(Action::SelectDevice(id.clone())).await;
                                                app.set_status(format!("Verbonden met: {}", device.name));
                                            }
                                        }
                                        app.input_mode = InputMode::Normal;
                                    }
                                    KeyCode::Esc => {
                                        app.input_mode = InputMode::Normal;
                                    }
                                    _ => {}
                                }
                            }
                        }
                    }
                }
            }
        }
    }

    disable_raw_mode()?;
    execute!(terminal.backend_mut(), LeaveAlternateScreen)?;
    terminal.show_cursor()?;

    Ok(())
}
