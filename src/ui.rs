use ratatui::{
    layout::{Alignment, Constraint, Direction, Layout, Rect},
    style::{Color, Style, Stylize},
    text::{Line, Span},
    widgets::{Block, BorderType, Borders, Clear, Gauge, List, ListItem, Paragraph, Wrap},
    Frame,
};

use crate::app::{App, InputMode};

pub fn render(frame: &mut Frame, app: &App) {
    let size = frame.area();

    // Vertical layout: Header (3), Main Body (Remaining), Footer (3)
    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(3),
            Constraint::Min(10),
            Constraint::Length(3),
        ])
        .split(size);

    render_header(frame, chunks[0], app);
    render_main_body(frame, chunks[1], app);
    render_footer(frame, chunks[2], app);

    // Modal overlays if active
    if app.input_mode == InputMode::EnteringGenre {
        render_genre_modal(frame, size, app);
    } else if app.input_mode == InputMode::SelectingDevice {
        render_device_modal(frame, size, app);
    }
}

fn render_header(frame: &mut Frame, area: Rect, app: &App) {
    let boost_badge = if app.boost_mode {
        let current_sec = app.current_track_played_ms / 1000;
        Span::styled(
            format!(" [⚡ BOOST: {}s/{}s] ", current_sec, app.target_boost_skip),
            Style::default().bg(Color::Magenta).fg(Color::White).bold(),
        )
    } else {
        Span::styled(" [BOOST OFF] ", Style::default().fg(Color::DarkGray))
    };

    let title_line = Line::from(vec![
        Span::styled(
            " ⚡ SUB-1K RADIO ",
            Style::default().fg(Color::Yellow).bold(),
        ),
        Span::raw(" │ "),
        Span::styled("Genre: ", Style::default().fg(Color::Gray)),
        Span::styled(&app.genre, Style::default().fg(Color::Cyan).bold()),
        Span::raw(" │ "),
        Span::styled("Filter: ", Style::default().fg(Color::Gray)),
        Span::styled(
            format!("pop <= {}%", app.config.max_popularity),
            Style::default().fg(Color::Green),
        ),
        Span::raw(" │ "),
        boost_badge,
        Span::raw(" │ "),
        Span::styled("Rescued: ", Style::default().fg(Color::Gray)),
        Span::styled(
            format!("{} tracks", app.rescued_count),
            Style::default().fg(Color::Yellow).bold(),
        ),
    ]);

    let block = Block::default()
        .borders(Borders::ALL)
        .border_type(BorderType::Rounded)
        .border_style(Style::default().fg(Color::Cyan));

    let header_widget = Paragraph::new(title_line)
        .block(block)
        .alignment(Alignment::Center);

    frame.render_widget(header_widget, area);
}

fn render_main_body(frame: &mut Frame, area: Rect, app: &App) {
    let chunks = Layout::default()
        .direction(Direction::Horizontal)
        .constraints([Constraint::Percentage(48), Constraint::Percentage(52)])
        .split(area);

    render_now_playing(frame, chunks[0], app);
    render_queue(frame, chunks[1], app);
}

fn render_now_playing(frame: &mut Frame, area: Rect, app: &App) {
    let block = Block::default()
        .title(" ▶ NOW PLAYING ")
        .title_alignment(Alignment::Left)
        .borders(Borders::ALL)
        .border_type(BorderType::Rounded)
        .border_style(Style::default().fg(Color::Green));

    let inner_area = block.inner(area);
    frame.render_widget(block, area);

    if let Some(playback) = &app.playback {
        let status_span = if playback.is_playing {
            Span::styled(
                " [PLAYING] ",
                Style::default().bg(Color::Green).fg(Color::Black).bold(),
            )
        } else {
            Span::styled(
                " [PAUSED] ",
                Style::default().bg(Color::Yellow).fg(Color::Black).bold(),
            )
        };

        let seed_badge = if playback.is_featured {
            Span::styled(
                " 🌟 CREATOR SEED (THE SILICON RITE) ",
                Style::default().bg(Color::Magenta).fg(Color::White).bold(),
            )
        } else {
            Span::styled(
                " 🛡 UNDERDOG TRACK (< 1,000 streams) ",
                Style::default().fg(Color::LightGreen),
            )
        };

        let lines = vec![
            Line::from(vec![status_span, Span::raw(" "), seed_badge]),
            Line::raw(""),
            Line::from(vec![
                Span::styled("Track:   ", Style::default().fg(Color::DarkGray)),
                Span::styled(
                    &playback.track_title,
                    Style::default().fg(Color::Cyan).bold(),
                ),
            ]),
            Line::from(vec![
                Span::styled("Artist:  ", Style::default().fg(Color::DarkGray)),
                Span::styled(
                    &playback.artist_names,
                    Style::default().fg(Color::White).bold(),
                ),
            ]),
            Line::from(vec![
                Span::styled("Album:   ", Style::default().fg(Color::DarkGray)),
                Span::styled(&playback.album_name, Style::default().fg(Color::Gray)),
            ]),
            Line::from(vec![
                Span::styled("Device:  ", Style::default().fg(Color::DarkGray)),
                Span::styled(
                    format!(
                        "{} (Volume: {}%)",
                        playback.device_name,
                        playback.device_volume.unwrap_or(100)
                    ),
                    Style::default().fg(Color::Yellow),
                ),
            ]),
            Line::raw(""),
            Line::from(vec![
                Span::styled("Est. Streams:  ", Style::default().fg(Color::DarkGray)),
                Span::styled(
                    "< 1,000 streams (Sub-Threshold)",
                    Style::default().fg(Color::LightGreen).bold(),
                ),
            ]),
            Line::raw(""),
        ];

        let text_layout = Layout::default()
            .direction(Direction::Vertical)
            .constraints([
                Constraint::Min(8),
                Constraint::Length(2),
                Constraint::Length(1),
            ])
            .split(inner_area);

        let info_paragraph = Paragraph::new(lines).wrap(Wrap { trim: true });
        frame.render_widget(info_paragraph, text_layout[0]);

        // Progress Gauge
        let progress_ratio = if playback.duration_ms > 0 {
            (playback.progress_ms as f64 / playback.duration_ms as f64).clamp(0.0, 1.0)
        } else {
            0.0
        };

        let progress_label = format!(
            "{}:{:02} / {}:{:02}",
            (playback.progress_ms / 1000) / 60,
            (playback.progress_ms / 1000) % 60,
            (playback.duration_ms / 1000) / 60,
            (playback.duration_ms / 1000) % 60,
        );

        let progress_gauge = Gauge::default()
            .block(Block::default().borders(Borders::NONE))
            .gauge_style(Style::default().fg(Color::Cyan).bg(Color::DarkGray))
            .ratio(progress_ratio)
            .label(progress_label);

        frame.render_widget(progress_gauge, text_layout[1]);
    } else {
        let no_playback_text = vec![
            Line::raw(""),
            Line::styled(
                "Geen actieve playback gedetecteerd.",
                Style::default().fg(Color::Yellow),
            ),
            Line::raw(""),
            Line::styled(
                "1. Zorg dat Spotify open staat (Desktop of Web Player).",
                Style::default().fg(Color::Gray),
            ),
            Line::styled(
                "2. Druk op [d] om je apparaat te selecteren.",
                Style::default().fg(Color::Gray),
            ),
            Line::styled(
                "3. Druk op [n] om een track uit de wachtrij te starten.",
                Style::default().fg(Color::Gray),
            ),
        ];

        let p = Paragraph::new(no_playback_text).alignment(Alignment::Center);
        frame.render_widget(p, inner_area);
    }
}

fn render_queue(frame: &mut Frame, area: Rect, app: &App) {
    let block = Block::default()
        .title(format!(
            " 📜 THE UNDERDOG QUEUE ({} tracks) ",
            app.queue.len()
        ))
        .title_alignment(Alignment::Left)
        .borders(Borders::ALL)
        .border_type(BorderType::Rounded)
        .border_style(Style::default().fg(Color::Magenta));

    let inner_area = block.inner(area);
    frame.render_widget(block, area);

    if app.is_loading && app.queue.is_empty() {
        let loading_text = vec![
            Line::raw(""),
            Line::styled(
                "🔍 Crate Digger is bezig met opgraven...",
                Style::default().fg(Color::Yellow).bold(),
            ),
            Line::styled(
                "Zoeken naar obscure tracks met tag:hipster...",
                Style::default().fg(Color::Gray),
            ),
        ];
        let p = Paragraph::new(loading_text).alignment(Alignment::Center);
        frame.render_widget(p, inner_area);
        return;
    }

    if app.queue.is_empty() {
        let empty_text = vec![
            Line::raw(""),
            Line::styled(
                "Geen tracks in wachtrij.",
                Style::default().fg(Color::DarkGray),
            ),
            Line::styled(
                "Druk op [g] om een ander genre te kiezen.",
                Style::default().fg(Color::Gray),
            ),
        ];
        let p = Paragraph::new(empty_text).alignment(Alignment::Center);
        frame.render_widget(p, inner_area);
        return;
    }

    let max_visible = inner_area.height.saturating_sub(1) as usize;
    let (start_idx, end_idx) = if app.queue.len() <= max_visible || max_visible == 0 {
        (0, app.queue.len())
    } else {
        let half = max_visible / 2;
        let start = app.queue_index.saturating_sub(half);
        let end = (start + max_visible).min(app.queue.len());
        let start = if end == app.queue.len() {
            end.saturating_sub(max_visible)
        } else {
            start
        };
        (start, end)
    };

    let items: Vec<ListItem> = app.queue[start_idx..end_idx]
        .iter()
        .enumerate()
        .map(|(offset, item)| {
            let actual_idx = start_idx + offset;
            let is_selected = actual_idx == app.queue_index;
            let marker = if is_selected { "▶ " } else { "  " };

            let artists = item
                .track
                .artists
                .iter()
                .map(|a| a.name.as_str())
                .collect::<Vec<_>>()
                .join(", ");

            let seed_badge = if item.is_featured { " [🌟 Seed]" } else { "" };

            let content = format!(
                "{}{}. {} - {}{}",
                marker,
                actual_idx + 1,
                artists,
                item.track.name,
                seed_badge
            );

            let style = if is_selected {
                Style::default().fg(Color::Cyan).bold()
            } else if item.is_featured {
                Style::default().fg(Color::Magenta)
            } else {
                Style::default().fg(Color::White)
            };

            ListItem::new(content).style(style)
        })
        .collect();

    let list = List::new(items);
    frame.render_widget(list, inner_area);
}

fn render_footer(frame: &mut Frame, area: Rect, app: &App) {
    let block = Block::default()
        .borders(Borders::ALL)
        .border_type(BorderType::Rounded)
        .border_style(Style::default().fg(Color::DarkGray));

    let content = if let Some((msg, _)) = &app.status_message {
        Line::from(vec![
            Span::styled(
                " STATUS: ",
                Style::default().bg(Color::Green).fg(Color::Black).bold(),
            ),
            Span::raw(" "),
            Span::styled(msg, Style::default().fg(Color::White).bold()),
        ])
    } else {
        Line::from(vec![
            Span::styled("[Space]", Style::default().fg(Color::Yellow).bold()),
            Span::raw(" Play/Pause  "),
            Span::styled("[n]", Style::default().fg(Color::Yellow).bold()),
            Span::raw(" Next Track  "),
            Span::styled("[s]", Style::default().fg(Color::Green).bold()),
            Span::raw(" Rescue (Like)  "),
            Span::styled("[f]", Style::default().fg(Color::Cyan).bold()),
            Span::raw(" Follow  "),
            Span::styled("[b]", Style::default().fg(Color::Magenta).bold()),
            Span::raw(" Boost  "),
            Span::styled("[g]", Style::default().fg(Color::Cyan).bold()),
            Span::raw(" Genre  "),
            Span::styled("[d]", Style::default().fg(Color::Blue).bold()),
            Span::raw(" Device  "),
            Span::styled("[q]", Style::default().fg(Color::Red).bold()),
            Span::raw(" Quit"),
        ])
    };

    let p = Paragraph::new(content)
        .block(block)
        .alignment(Alignment::Center);
    frame.render_widget(p, area);
}

fn render_genre_modal(frame: &mut Frame, area: Rect, app: &App) {
    let modal_area = centered_rect(60, 20, area);
    frame.render_widget(Clear, modal_area);

    let block = Block::default()
        .title(" 🎵 KIES GENRE ")
        .borders(Borders::ALL)
        .border_type(BorderType::Double)
        .border_style(Style::default().fg(Color::Cyan));

    let lines = vec![
        Line::raw(""),
        Line::styled(
            "Typ een muziekgenre en druk op [Enter]:",
            Style::default().fg(Color::Gray),
        ),
        Line::raw(""),
        Line::from(vec![
            Span::styled("> ", Style::default().fg(Color::Cyan).bold()),
            Span::styled(&app.genre_input, Style::default().fg(Color::White).bold()),
            Span::styled("█", Style::default().fg(Color::Yellow)),
        ]),
        Line::raw(""),
        Line::styled(
            "[Enter] Zoeken en wachtrij vullen  |  [Esc] Annuleren",
            Style::default().fg(Color::DarkGray),
        ),
    ];

    let p = Paragraph::new(lines)
        .block(block)
        .alignment(Alignment::Center);
    frame.render_widget(p, modal_area);
}

fn render_device_modal(frame: &mut Frame, area: Rect, app: &App) {
    let modal_area = centered_rect(60, 35, area);
    frame.render_widget(Clear, modal_area);

    let block = Block::default()
        .title(" 📱 KIES SPOTIFY CONNECT APPARAAT ")
        .borders(Borders::ALL)
        .border_type(BorderType::Double)
        .border_style(Style::default().fg(Color::Blue));

    let mut lines = vec![
        Line::raw(""),
        Line::styled(
            "Kies het actieve afspeelapparaat:",
            Style::default().fg(Color::Gray),
        ),
        Line::raw(""),
    ];

    if app.available_devices.is_empty() {
        lines.push(Line::styled(
            "Geen actieve apparaten gevonden! Open Spotify Desktop of Web.",
            Style::default().fg(Color::Red),
        ));
    } else {
        for (i, d) in app.available_devices.iter().enumerate() {
            let active = if d.is_active { " [ACTIEF]" } else { "" };
            let is_sel = i == app.device_index;
            let marker = if is_sel { "▶ " } else { "  " };
            lines.push(Line::from(vec![
                Span::styled(
                    format!("{}[{}] {} ", marker, i + 1, d.name),
                    if is_sel {
                        Style::default().fg(Color::Cyan).bold()
                    } else {
                        Style::default().fg(Color::White)
                    },
                ),
                Span::styled(active, Style::default().fg(Color::Green).bold()),
            ]));
        }
    }

    lines.push(Line::raw(""));
    lines.push(Line::styled(
        "[↑/↓] Selecteren | [Enter] Verbinden | [Esc] Sluiten",
        Style::default().fg(Color::DarkGray),
    ));

    let p = Paragraph::new(lines)
        .block(block)
        .alignment(Alignment::Center);
    frame.render_widget(p, modal_area);
}

fn centered_rect(percent_x: u16, percent_y: u16, r: Rect) -> Rect {
    let popup_layout = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Percentage((100 - percent_y) / 2),
            Constraint::Percentage(percent_y),
            Constraint::Percentage((100 - percent_y) / 2),
        ])
        .split(r);

    Layout::default()
        .direction(Direction::Horizontal)
        .constraints([
            Constraint::Percentage((100 - percent_x) / 2),
            Constraint::Percentage(percent_x),
            Constraint::Percentage((100 - percent_x) / 2),
        ])
        .split(popup_layout[1])[1]
}
