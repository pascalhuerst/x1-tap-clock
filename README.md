# x1-tap-clock

A small Rust project that turns a Native Instruments Kontrol X1 Mk1 into a tap-tempo remote for Ableton Link and a MIDI clock generator. The code talks directly to the controller over USB, interprets button/encoder/pot changes, and lets you map that state to downstream logic without touching evdev or Traktor drivers.

## Features

- **Direct USB polling** – uses `rusb` to read the 24-byte report buffer, parse button/pot/encoder states, and control LEDs.
- **Tap-tempo detection** – four taps (Deck A Sync while holding Shift) estimate BPM via a sliding-window average.
- **Ableton Link integration** – pushes the detected BPM and transport state to a Link session.
- **MIDI clock output** – optionally emits 24 PPQN clocks plus start/stop messages to a selected MIDI output port.
- **Custom LED handling** – callbacks receive an LED handle so you can tailor feedback (e.g., blink the Deck A Sync LED on beat).

## Hardware mapping

- **Tap button** – Deck A Sync (hold Shift while tapping).
- **Start/Stop** – Deck A Play toggles the Link transport.
- **Tap LED** – Deck A Sync LED (index 23) flashes on tap and blinks to the beat once playing.

## Building

```bash
cargo build
```

(You’ll need a Rust toolchain ≥ 1.70.)

The `ableton-link` crate depends on an older `nom`/`cexpr` combo that emits future-compatibility warnings. They’re harmless for now but noted in `cargo check`. Patching the dependencies or updating the bindings will silence them.

## Running

```bash
cargo run -- <midi-port-hint>
```

Make sure the Kontrol X1 Mk1 is connected before launching. Provide an optional substring to select the MIDI output port; leave it blank to use the first enumerated port. The binary:

1. Connects to the first device with vendor ID `0x17cc` / product ID `0x2305`.
2. Sets up callbacks for button/encoder/pot events (with LED handles and timestamps).
3. Taps into Ableton Link to sync tempo and transport.
4. Drives LED feedback from the event loop.

If no controller is found, the app prints “No X1 controller found.” and exits without error.

## File layout

- `src/main.rs` – glue logic: event loop, tap-tempo handling, LED feedback, comms with Link.
- `src/x1_controller/` – USB controller abstraction (state parsing, callbacks, LED helper).
- `src/tap_tempo.rs` – Tap tempo logic.
- `src/link_controller.rs` – thin wrapper around `ableton-link` for tempo/transport control.


## Next steps

- Add richer LED animations (hotcue layers, shift indicators, etc.).
- Clean up `ableton-link` dependency warnings via dependency patching or upstream updates.
- Provide configuration or UI affordances for selecting MIDI ports and clock options.

### Contributing

PRs and patches are welcome—especially if you’d like to help modernize the Link bindings or build out the MIDI clock layer.

### License

Licensed under the GPL-3.0-only license. See [LICENSE](./LICENSE) for details.
