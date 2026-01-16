use midir::{MidiOutput, MidiOutputConnection, SendError};
use std::{
    fmt,
    sync::mpsc::{self, RecvTimeoutError, Sender},
    thread,
    time::{Duration, Instant},
};

const PPQN: f64 = 24.0;
const MIN_BPM: f64 = 30.0;
const MAX_BPM: f64 = 300.0;
const THREAD_NAME: &str = "x1-tap-clock-midi";

#[derive(Debug)]
pub struct MidiClock {
    tx: Sender<Command>,
    thread: Option<thread::JoinHandle<()>>,
    port_name: String,
}

impl MidiClock {
    pub fn new(port_hint: &str, initial_bpm: f64) -> Result<Self, MidiClockError> {
        let midi_out = MidiOutput::new("x1-tap-clock")
            .map_err(|err| MidiClockError::MidiInit(err.to_string()))?;
        let ports = midi_out.ports();

        if ports.is_empty() {
            return Err(MidiClockError::PortNotFound(port_hint.to_string()));
        }

        let target_port = if port_hint.trim().is_empty() {
            ports[0].clone()
        } else {
            let hint = port_hint.to_lowercase();
            ports
                .iter()
                .find(|port| {
                    midi_out
                        .port_name(port)
                        .map(|name| name.to_lowercase().contains(&hint))
                        .unwrap_or(false)
                })
                .cloned()
                .ok_or_else(|| MidiClockError::PortNotFound(port_hint.to_string()))?
        };

        let port_name = midi_out
            .port_name(&target_port)
            .unwrap_or_else(|_| "<unknown>".into());

        let connection = midi_out
            .connect(&target_port, "x1-tap-clock-out")
            .map_err(|err| MidiClockError::Connection(err.to_string()))?;

        let (tx, rx) = mpsc::channel::<Command>();

        let initial_bpm = sanitize_bpm(initial_bpm);
        let port_label = port_name.clone();

        let thread = thread::Builder::new()
            .name(THREAD_NAME.into())
            .spawn(move || run_clock(connection, rx, initial_bpm, port_label))
            .map_err(|err| MidiClockError::Thread(err.to_string()))?;

        Ok(Self {
            tx,
            thread: Some(thread),
            port_name,
        })
    }

    pub fn port_name(&self) -> &str {
        &self.port_name
    }

    pub fn start(&self) -> Result<(), MidiClockError> {
        self.send_command(Command::Start)
    }

    pub fn stop(&self) -> Result<(), MidiClockError> {
        self.send_command(Command::Stop)
    }

    pub fn set_bpm(&self, bpm: f64) -> Result<(), MidiClockError> {
        self.send_command(Command::SetBpm(sanitize_bpm(bpm)))
    }

    fn send_command(&self, command: Command) -> Result<(), MidiClockError> {
        self.tx
            .send(command)
            .map_err(|_| MidiClockError::Thread("clock thread has stopped".into()))
    }
}

impl Drop for MidiClock {
    fn drop(&mut self) {
        if self.tx.send(Command::Shutdown).is_ok() {
            if let Some(handle) = self.thread.take() {
                let _ = handle.join();
            }
        }
    }
}

#[derive(Debug)]
pub enum MidiClockError {
    MidiInit(String),
    PortNotFound(String),
    Connection(String),
    Thread(String),
}

impl fmt::Display for MidiClockError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            MidiClockError::MidiInit(err) => write!(f, "failed to initialise MIDI output: {err}"),
            MidiClockError::PortNotFound(port) => {
                write!(f, "no MIDI output port matching \"{port}\" was found")
            }
            MidiClockError::Connection(err) => write!(f, "failed to open MIDI connection: {err}"),
            MidiClockError::Thread(err) => write!(f, "midi clock thread error: {err}"),
        }
    }
}

impl std::error::Error for MidiClockError {}

#[derive(Debug)]
enum Command {
    Start,
    Stop,
    SetBpm(f64),
    Shutdown,
}

fn run_clock(
    mut connection: MidiOutputConnection,
    rx: mpsc::Receiver<Command>,
    initial_bpm: f64,
    port_name: String,
) {
    let mut bpm = initial_bpm;
    let mut tick_duration = duration_from_bpm(bpm);
    let mut running = false;
    let mut next_tick = Instant::now();

    loop {
        if running {
            let now = Instant::now();
            if now >= next_tick {
                if let Err(err) = send_byte(&mut connection, 0xF8) {
                    eprintln!(
                        "midi clock ({}): failed to send CLOCK message: {}",
                        port_name, err
                    );
                    running = false;
                } else {
                    next_tick = next_tick
                        .checked_add(tick_duration)
                        .unwrap_or_else(Instant::now);
                }
                continue;
            }

            let timeout = next_tick - now;
            match rx.recv_timeout(timeout) {
                Ok(Command::Start) => {
                    if let Err(err) = send_byte(&mut connection, 0xFA) {
                        eprintln!(
                            "midi clock ({}): failed to send START message: {}",
                            port_name, err
                        );
                        running = false;
                    } else {
                        running = true;
                        next_tick = Instant::now();
                    }
                }
                Ok(Command::Stop) => {
                    if let Err(err) = send_byte(&mut connection, 0xFC) {
                        eprintln!(
                            "midi clock ({}): failed to send STOP message: {}",
                            port_name, err
                        );
                    }
                    running = false;
                }
                Ok(Command::SetBpm(new_bpm)) => {
                    bpm = new_bpm;
                    tick_duration = duration_from_bpm(bpm);
                    next_tick = Instant::now()
                        .checked_add(tick_duration)
                        .unwrap_or_else(Instant::now);
                }
                Ok(Command::Shutdown) => {
                    if running {
                        if let Err(err) = send_byte(&mut connection, 0xFC) {
                            eprintln!(
                                "midi clock ({}): failed to send STOP message: {}",
                                port_name, err
                            );
                        }
                    }
                    break;
                }
                Err(RecvTimeoutError::Timeout) => continue,
                Err(RecvTimeoutError::Disconnected) => break,
            }
        } else {
            match rx.recv() {
                Ok(Command::Start) => {
                    if let Err(err) = send_byte(&mut connection, 0xFA) {
                        eprintln!(
                            "midi clock ({}): failed to send START message: {}",
                            port_name, err
                        );
                        running = false;
                    } else {
                        running = true;
                        next_tick = Instant::now();
                    }
                }
                Ok(Command::Stop) => {
                    if let Err(err) = send_byte(&mut connection, 0xFC) {
                        eprintln!(
                            "midi clock ({}): failed to send STOP message: {}",
                            port_name, err
                        );
                    }
                }
                Ok(Command::SetBpm(new_bpm)) => {
                    bpm = new_bpm;
                    tick_duration = duration_from_bpm(bpm);
                }
                Ok(Command::Shutdown) => break,
                Err(_) => break,
            }
        }
    }

    let _ = connection.close();
}

fn send_byte(connection: &mut MidiOutputConnection, byte: u8) -> Result<(), SendError> {
    connection.send(&[byte])
}

fn duration_from_bpm(bpm: f64) -> Duration {
    let nanos = (60_000_000_000_f64 / (bpm * PPQN)).max(1.0);
    Duration::from_nanos(nanos as u64)
}

fn sanitize_bpm(raw: f64) -> f64 {
    raw.clamp(MIN_BPM, MAX_BPM)
}
