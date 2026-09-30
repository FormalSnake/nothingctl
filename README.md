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
| Other Nothing and CMF models ear-web supports | | planned |

A device connects only when its model has a verified table. Anything else is
refused after the model id is read, with nothing sent to it.

## Features

| | B175 |
|---|---|
| Battery, charging | read, live |
| Noise control: off, transparency, high, mid, low, adaptive | read, live, set |
| EQ presets: rock, electronic, pop, vocals, classical, custom | read, set |
| Custom EQ: bass, mid, treble, -6 to +6 | read, set |
| Low latency | read, set |
| Spatial audio: off, concert, theatre | read, set |
| Firmware version | read |

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
nothingctl watch                      # JSON lines until the link drops
```

`-d AA:BB:CC:DD:EE:FF` picks a device; without it the only connected one is
used. `-v` prints every frame sent and received on stderr.

`watch` prints a full snapshot first and again on every change. It reads
`set` arguments on stdin, one per line (`anc high`), and answers
`{"type":"ack","cmd":"anc high"}` or `{"type":"error","message":"..."}`. It
prints `{"type":"disconnected"}` and exits 1 when the link drops, and exits 0
when stdin closes.

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
  "spatial": { "mode": "off", "available": ["off", "concert", "theatre"] }
}
```

`kind` is `headphone` or `earbuds`. Battery ids are `left`, `right`, `case` or
`single`. A feature the model lacks is `null`, and so is a value the device has
not reported yet. `eq.preset` can read `dirac` (Dirac Opteo), which ear-web
decodes but does not offer, so it is not in `available`. `eq.custom` is
`[bass, mid, treble]`, read when the custom preset is active.

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
  pairing, codec switching, dual connection changes or ring. The encoder
  cannot build those frames.
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
