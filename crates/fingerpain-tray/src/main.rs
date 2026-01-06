//! FingerPain System Tray App
//!
//! Menu bar / system tray application for quick stats access.

use anyhow::Result;
use fingerpain_core::{
    db::Database,
    metrics::{Metrics, TimeRange},
};
use std::sync::{Arc, Mutex};
use tao::{
    event::{Event, WindowEvent},
    event_loop::{ControlFlow, EventLoop},
    window::WindowBuilder,
};
use tray_icon::{
    menu::{Menu, MenuEvent, MenuItem, PredefinedMenuItem},
    TrayIcon, TrayIconBuilder,
};

fn main() -> Result<()> {
    tracing_subscriber::fmt()
        .with_env_filter("fingerpain=info")
        .init();

    // Open database
    let db = Arc::new(Mutex::new(Database::open_default()?));

    // Build the event loop
    let event_loop = EventLoop::new();

    // Create a hidden window (required on some platforms)
    let _window = WindowBuilder::new()
        .with_visible(false)
        .build(&event_loop)?;

    // Create tray menu
    let tray_menu = Menu::new();

    // Stats items (will be updated dynamically)
    let stats_chars = MenuItem::new("Characters: -", false, None);
    let stats_words = MenuItem::new("Words: -", false, None);
    let stats_wpm = MenuItem::new("WPM: -", false, None);

    tray_menu.append(&stats_chars)?;
    tray_menu.append(&stats_words)?;
    tray_menu.append(&stats_wpm)?;
    tray_menu.append(&PredefinedMenuItem::separator())?;

    let open_dashboard = MenuItem::new("Open Dashboard", true, None);
    tray_menu.append(&open_dashboard)?;

    tray_menu.append(&PredefinedMenuItem::separator())?;

    let quit = MenuItem::new("Quit", true, None);
    tray_menu.append(&quit)?;

    // Create tray icon
    let icon = load_icon();
    let tray_icon = TrayIconBuilder::new()
        .with_menu(Box::new(tray_menu))
        .with_tooltip("FingerPain - Typing Analytics")
        .with_icon(icon)
        .build()?;

    // Store menu item IDs for event handling
    let open_dashboard_id = open_dashboard.id().clone();
    let quit_id = quit.id().clone();
    let stats_chars_id = stats_chars.id().clone();
    let stats_words_id = stats_words.id().clone();
    let stats_wpm_id = stats_wpm.id().clone();

    // Clone for the update closure
    let db_clone = db.clone();

    // Update stats periodically
    let update_stats = move || -> Result<()> {
        let db_guard = db_clone.lock().unwrap();
        let metrics = Metrics::new(&*db_guard);
        let stats = metrics.stats(TimeRange::Today)?;

        stats_chars.set_text(&format!(
            "Characters: {}",
            Metrics::format_chars(stats.total_chars)
        ));
        stats_words.set_text(&format!(
            "Words: {}",
            Metrics::format_words(stats.total_words)
        ));

        let wpm_text = stats
            .avg_wpm
            .map(|w| format!("{:.0}", w))
            .unwrap_or_else(|| "-".to_string());
        stats_wpm.set_text(&format!("Avg WPM: {}", wpm_text));

        Ok(())
    };

    // Initial update
    let _ = update_stats();

    // Set up menu event receiver
    let menu_channel = MenuEvent::receiver();

    // Run event loop
    event_loop.run(move |event, _, control_flow| {
        *control_flow = ControlFlow::Wait;

        // Check for menu events
        if let Ok(event) = menu_channel.try_recv() {
            if event.id == quit_id {
                *control_flow = ControlFlow::Exit;
            } else if event.id == open_dashboard_id {
                // Open web dashboard
                let _ = open::that("http://localhost:7890");
            }
        }

        match event {
            Event::WindowEvent {
                event: WindowEvent::CloseRequested,
                ..
            } => {
                *control_flow = ControlFlow::Exit;
            }
            _ => {}
        }
    });
}

fn load_icon() -> tray_icon::Icon {
    // Create a simple icon (16x16 RGBA)
    // In production, you'd load this from a file
    let size = 16;
    let mut rgba = Vec::with_capacity(size * size * 4);

    for y in 0..size {
        for x in 0..size {
            // Create a simple keyboard-like icon
            let in_key = (x >= 2 && x < 14) && (y >= 4 && y < 12);
            let is_border = in_key && (x == 2 || x == 13 || y == 4 || y == 11);

            if is_border {
                // Border color (dark gray)
                rgba.extend_from_slice(&[80, 80, 80, 255]);
            } else if in_key {
                // Key color (light blue)
                rgba.extend_from_slice(&[100, 150, 255, 255]);
            } else {
                // Transparent
                rgba.extend_from_slice(&[0, 0, 0, 0]);
            }
        }
    }

    tray_icon::Icon::from_rgba(rgba, size as u32, size as u32)
        .expect("Failed to create icon")
}
