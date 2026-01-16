mod link_controller;
mod tap_tempo;
mod x1_controller;

use std::{
    sync::mpsc,
    thread,
    time::{Duration, Instant},
};

use link_controller::LinkController;
use tap_tempo::TapTempo;
use x1_controller::{
    ButtonEvent, ButtonEventKind, ButtonId, Timestamp, X1Controller, LED_BRIGHT, LED_DIM,
};

const START_BPM: f64 = 120.0;
const TAP_LED_INDEX: usize = 23;
const FLASH_DURATION_MS: u64 = 160;
const LED_MEDIUM: u8 = 0x30;
const QUANTUM_BEATS: f64 = 4.0;
const DOWNBEAT_WINDOW: f64 = 0.12;
const BEAT_WINDOW: f64 = 0.08;

#[derive(Debug, Clone, Copy)]
enum ControlMessage {
    Button {
        event: ButtonEvent,
        timestamp: Timestamp,
    },
}

fn main() -> rusb::Result<()> {
    let mut controller = X1Controller::connect()?;

    // Ensure the tap LED starts dimmed.
    controller.set_led_raw(TAP_LED_INDEX, LED_DIM);

    let (tx, rx) = mpsc::channel::<ControlMessage>();
    controller.set_button_callback(move |_, event, timestamp, _handle| {
        if matches!(event.kind, ButtonEventKind::Pressed) {
            let _ = tx.send(ControlMessage::Button { event, timestamp });
        }
    });

    let mut link = LinkController::new(START_BPM);
    let mut tapper = TapTempo::new(4, 2.0);
    let mut playing = false;
    let mut current_bpm: Option<f64> = Some(START_BPM);

    let mut flash_until: Option<Instant> = None;
    let mut current_led_value: u8 = LED_DIM;

    let app_start = Instant::now();

    loop {
        controller.poll_once()?;

        // Drain controller events.
        while let Ok(message) = rx.try_recv() {
            match message {
                ControlMessage::Button { event, timestamp } => {
                    handle_button_event(
                        event,
                        timestamp,
                        &app_start,
                        &mut tapper,
                        &mut link,
                        &mut controller,
                        &mut playing,
                        &mut current_bpm,
                        &mut flash_until,
                        &mut current_led_value,
                    );
                }
            }
        }

        update_led_feedback(
            &mut link,
            &mut controller,
            &mut flash_until,
            playing,
            &mut current_led_value,
        );

        thread::sleep(Duration::from_millis(2));
    }
}

#[allow(clippy::too_many_arguments)]
fn handle_button_event(
    event: ButtonEvent,
    timestamp: Timestamp,
    app_start: &Instant,
    tapper: &mut TapTempo,
    link: &mut LinkController,
    controller: &mut X1Controller,
    playing: &mut bool,
    current_bpm: &mut Option<f64>,
    flash_until: &mut Option<Instant>,
    current_led_value: &mut u8,
) {
    match event.id {
        ButtonId::Deck1Sync if event.modifiers.shift => {
            let tap_time = timestamp
                .checked_duration_since(*app_start)
                .map(|d| d.as_secs_f64())
                .unwrap_or(0.0);

            if let Some(bpm) = tapper.add_tap(tap_time) {
                link.set_tempo(bpm);
                if !*playing {
                    link.set_playing(true);
                    *playing = true;
                    println!("Clock START @ {:.2} BPM", bpm);
                } else {
                    println!("Tempo set to {:.2} BPM", bpm);
                }
                *current_bpm = Some(bpm);
            }

            *flash_until = Some(Instant::now() + Duration::from_millis(FLASH_DURATION_MS));
            controller.set_led_raw(TAP_LED_INDEX, LED_BRIGHT);
            *current_led_value = LED_BRIGHT;
        }
        ButtonId::Deck1Sync => {
            return;
        }
        ButtonId::Deck1Play => {
            *playing = !*playing;
            link.set_playing(*playing);
            if *playing {
                println!("Clock START @ {:.2} BPM", current_bpm.unwrap_or(START_BPM));
                *flash_until = Some(Instant::now() + Duration::from_millis(FLASH_DURATION_MS));
                controller.set_led_raw(TAP_LED_INDEX, LED_BRIGHT);
                *current_led_value = LED_BRIGHT;
            } else {
                println!("Clock STOP");
                *flash_until = None;
                controller.set_led_raw(TAP_LED_INDEX, LED_DIM);
                *current_led_value = LED_DIM;
            }
        }
        _ => return,
    }
}

fn update_led_feedback(
    link: &mut LinkController,
    controller: &mut X1Controller,
    flash_until: &mut Option<Instant>,
    playing: bool,
    current_led_value: &mut u8,
) {
    let now = Instant::now();
    let flash_active = if let Some(deadline) = flash_until {
        if now < *deadline {
            true
        } else {
            *flash_until = None;
            false
        }
    } else {
        false
    };

    let mut desired_led = if flash_active { LED_BRIGHT } else { LED_DIM };

    if !flash_active && playing {
        let now_micros = link.clock().micros();
        let mut phase_opt = None;
        link.with_session_state(|state| {
            phase_opt = Some(state.phase_at_time(now_micros, QUANTUM_BEATS));
        });

        if let Some(phase) = phase_opt {
            let beat_phase = phase.fract();
            desired_led = if phase < DOWNBEAT_WINDOW {
                LED_BRIGHT
            } else if beat_phase < BEAT_WINDOW {
                LED_MEDIUM
            } else {
                LED_DIM
            };
        }
    }

    if desired_led != *current_led_value {
        controller.set_led_raw(TAP_LED_INDEX, desired_led);
        *current_led_value = desired_led;
    }
}
