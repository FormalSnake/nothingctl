# nothingctl

Control Nothing and CMF audio devices from the Linux command line.

nothingctl talks to the headset over the same Bluetooth RFCOMM service the
Nothing X app and [ear-web](https://github.com/radiance-project/ear-web) use,
through BlueZ. It reads battery, noise control, EQ, low latency and spatial
audio, and changes them. `watch` keeps one connection open and prints a JSON
line on every change, for status bars and desktop shells.

## Supported devices

| Device | Model | Status |
|---|---|---|
| CMF Headphone Pro | B175 | yes |
| CMF Clip Pro | B189 | yes |
| Nothing Ear (3a) | B190 | yes |
| Nothing Headphone (1) Pro | B192 | yes |
| Other Nothing and CMF models ear-web supports | | planned |

A device connects only when its model has a verified table. Anything else is
refused after the model id is read, with nothing sent to it.

## Features

| | B175 | B189 | B190 | B192 |
|---|---|---|---|---|
| Battery, charging | read, live | read, live | read, live | read, live |
| Noise control: off, transparency, high, mid, low, adaptive | read, live, set | | read, live, set | read, live, set |
| EQ presets | read, set | read, set | read, set | read, set |
| Custom EQ: bass, mid, treble, -6 to +6 | read, set | read, set | read, set | read, set |
| Low latency | read, set | read, set | read, set | read, set |
| Spatial audio | read, set | read, set | read, set | |
| In-ear detection | | | read, set | read, set |
| Bass enhance, levels 0 to 2 | | read, set | | |
| Personal sound | read, live, set | | | read, live, set |
| Anti-leakage: off, calls, all | | read, set | | |
| Long battery mode | | read | | |
| Flat EQ (earcup slider) | | | | read, live |
| Firmware version | read | read | read | read |

EQ presets per model:

| Model | Presets |
|---|---|
| B175 | rock, electronic, pop, vocals, classical, custom |
| B189 | rock, electronic, pop, vocals, classical, custom |
| B190 | balanced, voice, treble, bass, custom, advanced |
| B192 | balanced, voice, treble, bass, custom, advanced, detail |

Spatial audio is off, concert or theatre on B175, and off or fixed on B189
and B190. B192's spatial audio runs through an SDK ear-web cannot use, so it
is not offered. `advanced` switches to the parametric EQ set up in the
Nothing X app; its bands are not edited here. Long battery mode is read only
because switching it reboots the device.

## Install

### Nix

```
nix run github:FormalSnake/nothingctl -- status
```

As a flake input:

```nix
{
  inputs.nothingctl = {
    url = "github:FormalSnake/nothingctl";
    inputs.nixpkgs.follows = "nixpkgs";
  };

  # in your configuration
  nixpkgs.overlays = [ inputs.nothingctl.overlays.default ];
  environment.systemPackages = [ pkgs.nothingctl ];
}
```

### Cargo

Needs Rust 1.85+, `pkg-config` and the dbus development headers.

```
cargo install --git https://github.com/FormalSnake/nothingctl
```

The headset has to be paired and connected through BlueZ first
(`bluetoothctl`, your desktop's Bluetooth settings).

## Usage

```
nothingctl list                       # paired devices with the Nothing service
nothingctl status                     # one snapshot, human readable
nothingctl status --json
nothingctl set anc high
nothingctl set eq rock
nothingctl set eq-custom 3 0 -2       # bass mid treble
nothingctl set low-latency on
nothingctl set spatial concert
nothingctl set in-ear off
nothingctl set bass 2                 # 0 is off
nothingctl set personal-sound on
nothingctl set leakage calls
nothingctl watch                      # JSON lines until the link drops
```

`-d AA:BB:CC:DD:EE:FF` picks a device; without it the only connected one is
used. `-v` prints every frame sent and received on stderr.

`watch` prints a full snapshot first and again on every change. It reads
`set` arguments on stdin, one per line (`anc high`), and answers
`{"type":"ack","cmd":"anc high"}` or `{"type":"error","message":"..."}`. It
prints `{"type":"disconnected"}` and exits 1 when the link drops, and exits 0
when stdin closes. A device refused over its model gets one
`{"type":"error","code":"unsupported-model","modelId":"...","message":"..."}`
line (`modelId` is `null` when the device offers no Fast Pair service to read
it from) before `watch` exits 3.

Snapshot:

```json
{
  "type": "state",
  "address": "AA:BB:CC:DD:EE:FF",
  "name": "CMF Headphone Pro",
  "model": { "base": "B175", "name": "CMF Headphone Pro", "kind": "headphone" },
  "firmware": "1.0.1.52",
  "battery": [{ "id": "single", "level": 80, "charging": false }],
  "anc": { "mode": "high", "available": ["off", "transparency", "high", "mid", "low", "adaptive"] },
  "eq": { "preset": "custom", "available": ["rock", "electronic", "pop", "vocals", "classical", "custom"], "custom": [3.0, 0.0, -2.0] },
  "lowLatency": false,
  "spatial": { "mode": "off", "available": ["off", "concert", "theatre"] },
  "inEar": null,
  "bass": null,
  "personalSound": true,
  "leakage": null,
  "longBatteryMode": null,
  "flatEq": null
}
```

`kind` is `headphone` or `earbuds`. Battery ids are `left`, `right`, `case` or
`single`. A feature the model lacks is `null`, and so is a value the device has
not reported yet. `eq.preset` can read `dirac` (Dirac Opteo), which ear-web
decodes but does not offer, so it is not in `available`. `eq.custom` is
`[bass, mid, treble]`, read when the custom preset is active. `bass` is
`{"enabled": true, "level": 1, "levels": 3}`: the device can report a level
while disabled.

## Exit codes

| Code | Meaning |
|---|---|
| 0 | Done, or `watch` saw stdin close |
| 1 | BlueZ, link or connection failure, or `watch` saw the link drop |
| 2 | Bad arguments |
| 3 | Unsupported model: the model id has no verified table, or cannot be read because the device has no Fast Pair service. Nothing was sent. `status` and `set` print the reason on stderr, `watch` also prints the `unsupported-model` line on stdout |

## Safety

- The control link is opened only by service UUID
  `aeac4a03-dff5-498f-843a-34487cf133eb`: BlueZ resolves the RFCOMM channel over
  SDP, as Chrome does for ear-web. No channel number is hardcoded, and a device
  without that record is refused.
- The model id is read the way ear-web reads it, from the Fast Pair service
  (`df21fe2c-2515-4fdb-8886-f12c4d67927c`), and nothing is written to that
  channel.
- Every frame goes through one encoder and matches the frame ear-web sends
  byte for byte. The tests rebuild every frame in
  [docs/protocol.md](docs/protocol.md) and compare.
- There are no commands for firmware updates, factory reset, renaming,
  pairing, codec switching, dual connection changes, long battery mode or
  ring. The encoder cannot build those frames.
- `--dry-run` prints the frames a command would send, in hex, without
  connecting:

  ```
  $ nothingctl --dry-run set anc high
  5560010ff0030001010100f9d7  anc set
  ```

## Credits

Protocol facts from ear-web (github.com/radiance-project/ear-web); no code is
copied from it.

## License

[MIT](LICENSE)
