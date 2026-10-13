# nothingctl

Battery, noise control and EQ for Nothing and CMF headphones and earbuds,
from the Linux command line.

nothingctl talks to the headset over the Bluetooth RFCOMM service the Nothing
X app uses, through BlueZ. `status` reads everything once, `set` changes one
setting, and `watch` holds the link open and prints a JSON line on every
change, for status bars and desktop shells.

```
 this machine                          headset
┌──────────────────────┐   RFCOMM    ┌──────────────────────────┐
│ nothingctl           │ ──────────▶ │ Nothing control service  │
│ status, set, watch   │ ◀────────── │ replies and pushes       │
└──────────────────────┘    BlueZ    └──────────────────────────┘
```

The protocol and the device list come from
[ear-web](https://github.com/radiance-project/ear-web): nothingctl covers every
device it supports and sends the same frames byte for byte, with none of its
code copied.

## Devices

Every model reports battery and firmware and has low latency mode.

| Device | Model | Noise control | EQ | Also |
|---|---|---|---|---|
| Nothing ear (1) | B181 | off, transparency, high, low | Nothing, no custom | in-ear |
| Nothing ear (stick) | B157 | off, nc, on `x.x.2.x` firmware | Nothing, advanced | in-ear |
| Nothing ear (2) | B155 | all six | Nothing, advanced | in-ear, personalized ANC, personal sound |
| Nothing Ear | B171 | all six | Nothing, advanced | bass 1 to 5, in-ear, personal sound |
| Nothing Ear (a) | B162 | all six | Nothing | bass 1 to 5, in-ear |
| Nothing Ear (open) | B174 | | Nothing, advanced | |
| Nothing Ear (3) | B173 | all six | Nothing, advanced | spatial, bass 1 to 5, in-ear, personal sound, super mic, mic mode, call transparency, walkie-talkie |
| Nothing Ear (3a) | B190 | all six | Nothing, advanced | spatial, in-ear |
| Nothing Headphone (1) | B170 | all six | Nothing, advanced | spatial with head tracking, bass 1 to 5, in-ear, personal sound from firmware `x.x.1.75` |
| Nothing Headphone (a) | B186 | all six | Nothing, advanced | spatial stage, bass 0 to 2 |
| Nothing Headphone (1) Pro | B192 | all six | Nothing, advanced, detail | in-ear, personal sound, flat EQ (read) |
| CMF Buds | B168 | off, transparency, nc | CMF, Dirac | bass 1 to 5, in-ear |
| CMF Buds Pro | B163 | off, transparency, high, mid, low | Nothing | in-ear |
| CMF Buds Pro 2 | B172 | all six | CMF, Dirac | spatial, bass 1 to 5, in-ear |
| CMF Buds Pro 2 | B195 | all six | CMF, Dirac | spatial, bass 0 to 2, in-ear |
| CMF Buds 2 | B179 | all six | CMF, Dirac | spatial, bass 1 to 5, in-ear |
| CMF Buds 2 | B197 | all six | CMF, Dirac | spatial, bass 0 to 2, in-ear |
| CMF Buds 2 Plus | B184 | all six | CMF | spatial, bass 1 to 5, in-ear, personal sound |
| CMF Buds 2a | B185 | off, transparency, nc | CMF, Dirac | bass (read) |
| CMF Neckband Pro | B164 | all six | Nothing | spatial, bass 1 to 5 |
| CMF Headphone Pro | B175 | all six | CMF | spatial stage, personal sound |
| CMF Clip Pro | B189 | | CMF | spatial, bass 0 to 2, anti-leakage, long battery mode (read) |

A device with any other model id is refused once the id is read, with nothing
sent to it.

- **Noise control:** all six is off, transparency, high, mid, low, adaptive.
  `nc` is the single noise cancellation setting.
- **EQ:** Nothing presets are balanced, voice, treble, bass and custom; CMF
  presets are rock, electronic, pop, vocals, classical and custom. Custom is a
  three-band EQ (bass, mid, treble, -6 to +6) on every model but the ear (1).
  `advanced` switches to the parametric EQ set up in the Nothing X app, whose
  bands are not edited here. `dirac` is Dirac Opteo.
- **Spatial audio:** off and fixed. Spatial stage is off, concert and theatre;
  head tracking adds head-tracked.
- **Bass enhance:** a level, plus on and off that keep the level. On the 0 to 2
  models level 0 is off.
- **Read only:** long battery mode reboots the device when switched, and flat
  EQ follows a slider on the earcup. The Buds 2a's bass level can be read but
  not set.

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
nothingctl set bass 2
nothingctl set bass off
nothingctl set personal-sound on
nothingctl set leakage calls
nothingctl set mic-mode voice-focus
nothingctl watch                 # JSON lines until the link drops
```

| Flag | |
|---|---|
| `-d AA:BB:CC:DD:EE:FF` | the device; without it, the only connected one |
| `--json` | JSON instead of text, for `list` and `status` |
| `-v` | every frame sent and received, on stderr |
| `--dry-run` | print the frames a command would send, in hex, and connect to nothing |
| `--model B190` | the model to plan frames for with `--dry-run` (default B175) |
| `--firmware 1.0.2.3` | the firmware to plan for with `--dry-run`, for features some firmware lacks |

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
  "spatial": { "mode": "off", "available": ["off", "concert", "theatre"] },
  "bass": null,
  "callTransparency": null,
  "flatEq": null,
  "inEar": null,
  "leakage": null,
  "longBatteryMode": null,
  "lowLatency": false,
  "micMode": null,
  "personalSound": true,
  "personalizedAnc": null,
  "superMic": null,
  "walkieTalkie": null
}
```

A feature the model lacks is `null`, and so is a value the device has not
reported yet. `kind` is `headphone` or `earbuds`; battery ids are `left`,
`right`, `case` or `single`. `kind` can also be `neckband`. `eq.custom` is `[bass, mid, treble]`, read when
the custom preset is active. `eq.preset` can read `dirac` on B175 (Dirac
Opteo), which is decoded but not offered, so it is not in `available`.
`bass` is `{"enabled": true, "level": 1, "levels": [1, 5]}`, `levels` being
the lowest and highest level `set bass` takes; the device keeps its level
while disabled.

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
channel over SDP as Chrome does for a Web Serial page. No channel number is
hardcoded, and a device without that record is refused. The model id is read
from the Fast Pair service (`df21fe2c-2515-4fdb-8886-f12c4d67927c`), and
nothing is written there.

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

[MIT](LICENSE). Not affiliated with Nothing Technology.
