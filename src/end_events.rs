/// This module defines the `EndEvent` enum and related functions for handling end events in the Pomodoro application.
///
/// The `EndEvent` enum represents different types of end events that can occur after a Pomodoro session, such as playing a sound or locking the screen.
///
/// # Examples
///
/// ```
/// use pomodoro::end_events::{EndEvent, lock_screen, play_sound};
/// use std::path::PathBuf;
///
/// // Use internal embedded sound (no filepath or empty filepath)
/// let sound_event_internal = EndEvent::Sound {
///     filepath_sound: None,
/// };
///
/// // Use external sound file
/// let sound_event_external = EndEvent::Sound {
///     filepath_sound: Some(PathBuf::from("sound.wav")),
/// };
///
/// let screensaver_event = EndEvent::LockScreen;
///
/// // Lock the screen
/// if let EndEvent::LockScreen = screensaver_event {
///     lock_screen();
/// }
/// ```
///
/// # Note
///
/// - The `Sound` variant of `EndEvent` uses an embedded Alarm01.wav by default (when filepath_sound is None or empty).
/// - If filepath_sound is provided but the file doesn't exist, a warning is printed and the internal sound is used.
/// - The internal sound is Alarm01.wav embedded in the binary at compile time.
/// - The `LockScreen` variant of `EndEvent` locks the screen across Windows, Linux, and macOS.
/// - The `play_sound` function plays a sound file using the `rodio` crate.
use rodio::{Decoder, OutputStream, Sink};
use serde;
use serde::{Deserialize, Serialize};
use std::path::PathBuf;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::thread;
use std::time::Duration;

/// Represents different types of end events that can occur after a Pomodoro session.
#[derive(Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", rename_all_fields = "camelCase")]
pub(crate) enum EndEvent {
    /// Play a sound. If filepath_sound is empty or the file doesn't exist, uses the internal embedded Alarm01.wav.
    Sound {
        /// Path to external sound file. If empty or file doesn't exist, uses internal sound.
        #[serde(default, skip_serializing_if = "Option::is_none")]
        filepath_sound: Option<PathBuf>,
    },
    /// Lock the screen.
    LockScreen,
}

/// Starts the specified end event.
pub(crate) fn start_end_event(end_event: &EndEvent) {
    match end_event {
        EndEvent::Sound { filepath_sound } => {
            play_sound(filepath_sound);
        }
        EndEvent::LockScreen => lock_screen(),
    }
}

/// Starts the specified end event with continuous monitoring for the given duration.
///
/// For LockScreen events, this will continuously lock the screen for the duration,
/// re-locking whenever the user tries to unlock.
/// For other events, it just calls the event at the end of the duration.
pub(crate) fn start_end_event_with_duration(end_event: &EndEvent, duration: Duration) {
    match end_event {
        EndEvent::Sound { filepath_sound } => {
            thread::sleep(duration);
            play_sound(filepath_sound);
        }
        EndEvent::LockScreen => {
            continuously_lock_screen(duration);
        }
    }
}

/// Locks the screen.
pub fn lock_screen() {
    if cfg!(windows) {
        lock_screen_on_windows();
    } else if cfg!(target_os = "linux") {
        lock_screen_on_linux();
    } else if cfg!(target_os = "macos") {
        lock_screen_on_macos();
    } else {
        eprintln!("Screen locking is not implemented for this platform.");
    }
}

/// Locks the screen on Windows.
pub fn lock_screen_on_windows() {
    // Turn on the screen saver for windows and lock the screen.
    std::process::Command::new("cmd")
        .args(&["/C", "rundll32", "user32.dll,LockWorkStation"])
        .output()
        .expect("Failed to start screen saver.");
}

/// Locks the screen on Linux.
pub fn lock_screen_on_linux() {
    // Try loginctl first (works on most modern Linux distributions with systemd)
    let result = std::process::Command::new("loginctl")
        .arg("lock-session")
        .output();

    if let Ok(output) = result {
        if output.status.success() {
            return;
        }
    }

    // Fallback: Try GNOME screen lock
    let result = std::process::Command::new("gnome-screensaver-command")
        .arg("-l")
        .output();

    if let Ok(output) = result {
        if output.status.success() {
            return;
        }
    }

    // Fallback: Try D-Bus method (works for GNOME/KDE)
    let result = std::process::Command::new("dbus-send")
        .args(&[
            "--type=method_call",
            "--dest=org.gnome.ScreenSaver",
            "/org/gnome/ScreenSaver",
            "org.gnome.ScreenSaver.Lock",
        ])
        .output();

    if let Ok(output) = result {
        if output.status.success() {
            return;
        }
    }

    eprintln!("Warning: Failed to lock screen. Please ensure 'loginctl' or 'gnome-screensaver-command' is available.");
}

/// Locks the screen on macOS.
pub fn lock_screen_on_macos() {
    std::process::Command::new("pmset")
        .args(&["displaysleepnow"])
        .output()
        .expect("Failed to lock screen on macOS.");
}

/// Checks if the screen is currently locked on Linux.
fn is_screen_locked_linux() -> bool {
    // Try freedesktop.org standard ScreenSaver interface (works with KDE, GNOME, etc.)
    if let Ok(active_output) = std::process::Command::new("gdbus")
        .args(&[
            "call",
            "--session",
            "--dest",
            "org.freedesktop.ScreenSaver",
            "--object-path",
            "/ScreenSaver",
            "--method",
            "org.freedesktop.ScreenSaver.GetActive",
        ])
        .output()
    {
        if active_output.status.success() {
            if let Ok(result) = String::from_utf8(active_output.stdout) {
                // Result will be "(true,)" if locked, "(false,)" if unlocked
                return result.contains("true");
            }
        }
    }

    // Fallback: Try GNOME-specific interface
    if let Ok(active_output) = std::process::Command::new("gdbus")
        .args(&[
            "call",
            "--session",
            "--dest",
            "org.gnome.ScreenSaver",
            "--object-path",
            "/org/gnome/ScreenSaver",
            "--method",
            "org.gnome.ScreenSaver.GetActive",
        ])
        .output()
    {
        if active_output.status.success() {
            if let Ok(result) = String::from_utf8(active_output.stdout) {
                return result.contains("true");
            }
        }
    }

    // Fallback: assume unlocked if we can't determine
    false
}

/// Checks if the screen is currently locked on Windows.
fn is_screen_locked_windows() -> bool {
    // On Windows, we'll use a simple heuristic: if we just locked it, assume it's locked
    // A more robust solution would require Win32 API calls
    false
}

/// Checks if the screen is currently locked on macOS.
fn is_screen_locked_macos() -> bool {
    // Check if the screen saver is running
    if let Ok(output) = std::process::Command::new("pgrep")
        .arg("ScreenSaverEngine")
        .output()
    {
        return output.status.success();
    }
    false
}

/// Checks if the screen is currently locked.
fn is_screen_locked() -> bool {
    if cfg!(target_os = "linux") {
        is_screen_locked_linux()
    } else if cfg!(windows) {
        is_screen_locked_windows()
    } else if cfg!(target_os = "macos") {
        is_screen_locked_macos()
    } else {
        false
    }
}

/// Continuously locks the screen for the specified duration.
///
/// This function locks the screen and monitors it, re-locking whenever
/// the user tries to unlock it before the duration expires.
///
/// # Arguments
/// * `duration` - How long to keep the screen locked
pub fn continuously_lock_screen(duration: Duration) {
    let should_stop = Arc::new(AtomicBool::new(false));
    let should_stop_clone = should_stop.clone();

    // Lock the screen immediately
    println!("Initial screen lock...");
    lock_screen();

    // Spawn a monitoring thread
    let monitor_thread = thread::spawn(move || {
        // Wait a bit for the initial lock to take effect
        thread::sleep(Duration::from_secs(3));
        println!("Monitoring thread started. Checking lock status every second...");

        let mut check_count = 0;
        while !should_stop_clone.load(Ordering::Relaxed) {
            check_count += 1;
            let is_locked = is_screen_locked();

            // Debug output every 10 checks (every ~5 seconds)
            if check_count % 10 == 0 {
                println!("Lock status check #{}: Screen is {}", check_count, if is_locked { "LOCKED" } else { "UNLOCKED" });
            }

            // Check if screen is unlocked
            if !is_locked {
                println!("⚠️  Screen unlocked detected! Re-locking in 1 second...");
                thread::sleep(Duration::from_secs(1));
                lock_screen();
                println!("Screen re-locked.");
                // Wait a bit after locking
                thread::sleep(Duration::from_secs(2));
            }

            // Check every half second
            thread::sleep(Duration::from_millis(500));
        }
        println!("Monitoring thread stopped.");
    });

    // Wait for the duration
    thread::sleep(duration);

    // Signal the monitoring thread to stop
    println!("Break duration completed. Stopping lock monitoring...");
    should_stop.store(true, Ordering::Relaxed);

    // Wait for the monitoring thread to finish
    let _ = monitor_thread.join();
}

/// Plays a sound. If filepath_sound is None or the file doesn't exist, plays the internal embedded sound.
/// If the filepath is provided but the file doesn't exist, prints a warning.
pub fn play_sound(filepath_sound: &Option<PathBuf>) {
    // Embed the sound file at compile time
    const ALARM_SOUND: &[u8] = include_bytes!("../assets/Alarm01.wav");

    let (_stream, stream_handle) =
        OutputStream::try_default().expect("Failed to create output stream.");
    let sink = Sink::try_new(&stream_handle).expect("Failed to create sink.");

    // Check if we should use external or internal sound
    let use_internal = if let Some(path) = filepath_sound {
        if path.as_os_str().is_empty() {
            // Empty path - use internal sound
            true
        } else if !path.is_file() {
            // Path provided but file doesn't exist - warn and use internal sound
            eprintln!("Warning: Sound file not found: {:?}", path);
            eprintln!("Using internal default sound instead.");
            true
        } else {
            // Valid file path - use external sound
            false
        }
    } else {
        // No path provided - use internal sound
        true
    };

    if use_internal {
        // Play internal embedded sound
        let sound_cursor = std::io::Cursor::new(ALARM_SOUND);
        let source = Decoder::new(sound_cursor).expect("Failed to decode internal sound file.");
        sink.append(source);
    } else {
        // Play external sound file
        let path = filepath_sound.as_ref().unwrap();
        let sound_file = std::fs::File::open(path).expect("Failed to open sound file.");
        let source = Decoder::new(sound_file).expect("Failed to decode sound file.");
        sink.append(source);
    }

    sink.sleep_until_end();
}

#[test]
fn test_serialize_end_event_to_json() {
    // Test external sound
    let sound_event_external = EndEvent::Sound {
        filepath_sound: Some(PathBuf::from("sound.wav")),
    };

    // Test internal sound (no filepath)
    let sound_event_internal = EndEvent::Sound {
        filepath_sound: None,
    };

    let screensaver_event = EndEvent::LockScreen;

    let sound_event_external_json = serde_json::to_string(&sound_event_external).unwrap();
    let sound_event_internal_json = serde_json::to_string(&sound_event_internal).unwrap();
    let screensaver_event_json = serde_json::to_string(&screensaver_event).unwrap();

    assert_eq!(
        sound_event_external_json,
        r#"{"sound":{"filepathSound":"sound.wav"}}"#
    );
    assert_eq!(
        sound_event_internal_json,
        r#"{"sound":{}}"#
    );
    assert_eq!(screensaver_event_json, r#""lockScreen""#);
}
