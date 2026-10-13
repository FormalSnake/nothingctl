# nothingctl

Battery, noise control and EQ for Nothing and CMF headphones and earbuds,
from the Linux command line.

nothingctl talks to the headset over the Bluetooth RFCOMM service the Nothing
X app and [ear-web](https://github.com/radiance-project/ear-web) use, through
BlueZ. `status` reads everything once, `set` changes one setting, and `watch`
holds the link open and prints a JSON line on every change, for status bars
and desktop shells.

```
 this machine                          headset
┌──────────────────────┐   RFCOMM    ┌──────────────────────────┐
│ nothingctl           │ ──────────▶ │ Nothing control service  │
│ status, set, watch   │ ◀────────── │ replies and pushes       │
└──────────────────────┘    BlueZ    └──────────────────────────┘
```

Every frame it sends is one ear-web sends, byte for byte, and a model only
connects once its table has been checked against ear-web.

## Devices

| Device | Model |
|---|---|
| CMF Headphone Pro | B175 |
| CMF Clip Pro | B189 |
| Nothing Ear (3a) | B190 |
| Nothing Headphone (1) Pro | B192 |

Anything else is refused after its model id is read, with nothing sent to it.
The other models ear-web supports are planned.

## What it controls

| | B175 | B189 | B190 | B192 |
|---|---|---|---|---|
| Battery, charging | live | live | live | live |
| Noise control: off, transparency, high, mid, low, adaptive | live, set | | live, set | live, set |
| EQ presets | set | set | set | set |
| Custom EQ: bass, mid, treble, -6 to +6 | set | set | set | set |
| Low latency | set | set | set | set |
| Spatial audio | set | set | set | |
| In-ear detection | | | set | set |
| Bass enhance, levels 0 to 2 | | set | | |
| Personal sound | live, set | | | live, set |
| Anti-leakage: off, calls, all | | set | | |
| Long battery mode | | read | | |
| Flat EQ (earcup slider) | | | | live |
| Firmware version | read | read | read | read |

Everything is read on connect; live means the device also pushes changes.

| Model | EQ presets | Spatial audio |
|---|---|---|
| B175 | rock, electronic, pop, vocals, classical, custom | off, concert, theatre |
| B189 | rock, electronic, pop, vocals, classical, custom | off, fixed |
| B190 | balanced, voice, treble, bass, custom, advanced | off, fixed |
| B192 | balanced, voice, treble, bass, custom, advanced, detail | |

`advanced` switches to the parametric EQ set up in the Nothing X app; its
bands are not edited here. B192's spatial audio runs through an SDK ear-web
cannot use, so it is not offered. Long battery mode is read only because
switching it reboots the device.

## Install

### Nix

```
nix run github:FormalSnake/nothingctl -- status
```

### NixOS

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

Needs Rust 1.88+, `pkg-config` and the dbus development headers.

```
cargo install --git https://github.com/FormalSnake/nothingctl
```

The headset has to be paired and connected through BlueZ first
(`bluetoothctl`, or your desktop's Bluetooth settings).

## Usage

```
nothingctl list                  # paired devices with the Nothing service
nothingctl status                # one snapshot, human readable
nothingctl status --json
nothingctl set anc high
nothingctl set eq custom
nothingctl set eq-custom 3 0 -2  # bass mid treble
nothingctl set low-latency on
nothingctl set spatial concert
nothingctl set in-ear off
nothingctl set bass 2            # 0 is off
nothingctl set personal-sound on
nothingctl set leakage calls
nothingctl watch                 # JSON lines until the link drops
```

| Flag | |
|---|---|
| `-d AA:BB:CC:DD:EE:FF` | the device; without it, the only connected one |
| `--json` | JSON instead of text, for `list` and `status` |
| `-v` | every frame sent and received, on stderr |
| `--dry-run` | print the frames a command would send, in hex, and connect to nothing |
| `--model B190` | the model to plan frames for with `--dry-run` (default B175) |

```
$ nothingctl --dry-run set anc high
5560010ff0030001010100f9d7  anc set
```

## watch

`watch` prints a full snapshot first and again on every change. It reads
`set` arguments on stdin, one per line (`anc high`), and answers
`{"type":"ack","cmd":"anc high"}` or `{"type":"error","message":"..."}`.

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

A feature the model lacks is `null`, and so is a value the device has not
reported yet. `kind` is `headphone` or `earbuds`; battery ids are `left`,
`right`, `case` or `single`. `eq.custom` is `[bass, mid, treble]`, read when
the custom preset is active. `eq.preset` can read `dirac` on B175 (Dirac
Opteo), which ear-web decodes but does not offer, so it is not in
`available`. `bass` is `{"enabled": true, "level": 1, "levels": 3}`; the
device keeps its level while disabled.

When the link drops, `watch` prints `{"type":"disconnected"}` and exits 1.
When stdin closes, it exits 0. A device refused over its model gets one
`{"type":"error","code":"unsupported-model","modelId":"...","message":"..."}`
line before `watch` exits 3; `modelId` is `null` when the device has no Fast
Pair service to read it from.

| Exit code | |
|---|---|
| 0 | done, or `watch` saw stdin close |
| 1 | BlueZ, link or connection failure, or `watch` saw the link drop |
| 2 | bad arguments |
| 3 | unsupported model, nothing was sent |

## Safety

The control link is opened only by service UUID
`aeac4a03-dff5-498f-843a-34487cf133eb`, with BlueZ resolving the RFCOMM
channel over SDP as Chrome does for ear-web. No channel number is hardcoded,
and a device without that record is refused. The model id is read from the
Fast Pair service (`df21fe2c-2515-4fdb-8886-f12c4d67927c`) the way ear-web
reads it, and nothing is written there.

All frames come from one encoder that only takes a known command. There are
no commands for firmware updates, factory reset, renaming, pairing, codec
switching, dual connection changes, long battery mode or ring, so those frames
cannot be built.

## Development

```
nix develop
cargo test
cargo run -- --dry-run --model B192 status   # the connect sequence, in hex
```

[docs/protocol.md](docs/protocol.md) has the protocol per model, with the
golden frames the tests rebuild byte for byte. Adding a model is one table in
`src/protocol/model.rs` once its frames are in that file.

## License

[MIT](LICENSE). Protocol facts come from ear-web; no code is copied from it.
Not affiliated with Nothing Technology.
