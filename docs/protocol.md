# Nothing protocol facts

Sections 1 to 6 cover the CMF Headphone Pro (B175) and the transport every
model shares. Section 7 covers what ear-web does the same way for every
other model, and sections 8 to 29 cover each model ear-web supports, B175
included, with every frame this tool can send to it.

B175 was extracted 2026-09-30 from the live ear-web site (earweb.bttl.xyz) page
`MainControl_forretress` and the scripts it loads: `bluetooth_socket.js`,
`device_common.js`, `gestures_common.js`, `eq_common.js`, `eq_espeon.js`,
`control.js`, `forretress.js`, `dual_connect.js`, `audio_codec.js`,
`personal_sound.js`, `spatial_audio.js`, `transitions.js`, plus the model
identification step in `nothing_connected.js` / `index.html` and the model
capability file `/js/ear_config_file.json`. ear-web is AGPL-3.0; this file
records protocol facts only, in our own words, with no code from it.

All multi-byte ids below are written as 16-bit numbers (`0xC007`) and appear
on the wire little-endian (`07 C0`). Byte offsets count from 0 at the `0x55`
start byte.

## 1. Transport

Model identification (index page, before the B175 page loads):

- Web Serial `requestPort` with `filters: [{ bluetoothServiceClassId:
  df21fe2c-2515-4fdb-8886-f12c4d67927c }]` and `allowedBluetoothServiceClassIds`
  containing both that UUID and `aeac4a03-dff5-498f-843a-34487cf133eb`.
- Port opened with `baudRate: 9600`. ear-web writes nothing to this port. It
  reads until a chunk exactly 7 bytes long arrives, takes bytes 4..6 of that
  chunk as a 3-byte model id (hex, upper-cased), closes the port. Bytes 0..3
  are not checked.
- B175 model ids: `563DA5` (green), `73C9EB` (white), `1EFB39` (black). Any of
  them routes to `MainControl_forretress`.
- Before scanning, the index page calls Web Serial `port.forget()` on every
  previously granted port. That only revokes browser permission; it does not
  touch Bluetooth pairing.

Control channel (the B175 page):

- On load the page calls `navigator.serial.getPorts()` and picks the port whose
  `getInfo().bluetoothServiceClassId` equals
  `aeac4a03-dff5-498f-843a-34487cf133eb` (granted during the index-page
  `requestPort` above, which listed it as allowed). If no such port is granted,
  nothing connects. The page's connect routine only falls back to its own
  `requestPort` (that UUID as both filter and allowed id) when handed no port,
  which the B175 page never does.
- Port opened with `{ baudRate: 9600, bufferSize: 2048 }`. The baud rate is a
  Web Serial API requirement; the link is Bluetooth RFCOMM.
- **The RFCOMM service we must connect to is service class UUID
  `aeac4a03-dff5-498f-843a-34487cf133eb`.** ear-web never names an RFCOMM
  channel number: Chrome resolves the channel from that UUID over SDP. A
  native client should do the same SDP lookup rather than hardcode a channel.
  The Fast Pair UUID `df21fe2c-...` is only for the passive model id read and
  carries no control traffic.
- On disconnect the page navigates back to the index, except after a codec
  change (section 4), where it waits 10 s and reopens the same port, retrying
  every 5 s up to 6 times.

## 2. Framing

### Outgoing frame

| Offset | Size | Value |
|---|---|---|
| 0 | 1 | `0x55` start byte |
| 1 | 1 | `0x60` (constant in every frame ear-web sends) |
| 2 | 1 | `0x01` (constant in every frame ear-web sends) |
| 3..4 | 2 | command id, little-endian |
| 5 | 1 | payload length in bytes |
| 6 | 1 | `0x00` always (ear-web never sets it) |
| 7 | 1 | operation id |
| 8.. | n | payload |
| 8+n..9+n | 2 | CRC-16, little-endian (low byte first) |

- ear-web gives bytes 1 and 2 no meaning anywhere; it writes `60 01` and never
  reads them back. Whether byte 6 is the high byte of a 16-bit length is not
  stated in ear-web; it only ever writes 0 there and reads only byte 5.
- No escaping or byte stuffing of any kind, in either direction.

### Operation id

- A single global counter starting at 0, incremented **before** every send, so
  the first frame after the page loads carries `0x01`, the second `0x02`, and so
  on. Every frame increments it, reads and writes alike, in the order the
  frames are handed to the writer.
- After handling each received chunk, if the counter is 250 or more it is set
  back to 1 (so the next send uses 2). With no incoming traffic the counter
  keeps climbing and the byte wraps modulo 256 (256 goes out as `0x00`).
- ear-web keeps a map of op id to a label for some sends but never reads it.
  Responses are **not** matched by op id.

### CRC

CRC-16/MODBUS: polynomial `0x8005` reflected (`0xA001` in the shift-right
form), initial value `0xFFFF`, input and output reflected, no final XOR. Check
value over ASCII `123456789` is `0x4B37`. It covers bytes 0 through the end of
the payload (header plus payload, not the CRC itself). Appended low byte first.

Cross-check: no complete frame with its CRC appears literally anywhere in the
ear-web source or comments, so instead ear-web's own frame builder was run
unmodified in JavaScriptCore (`osascript -l JavaScript`, stubbing only the
browser globals and capturing its debug log line). All 298 frames it produced
(every B175 command in section 4, plus 260 battery reads to push the op id past
255) have CRCs identical to the Python implementation used for section 5, and
its first three frames (`55600107c0000001acdf`, `55600150c00000029912`,
`55600142c0000003e0d1`) are byte-identical to the golden frames below.

### Incoming frames

- ear-web processes each Web Serial `read()` chunk as exactly one frame starting
  at offset 0. A chunk is dropped unless byte 0 is `0x55` and it is at least 8
  bytes long.
- The command id is bytes 3..4, little-endian. Dispatch is **by command id
  only**. ear-web does not check the CRC, the op id, bytes 1, 2 or 6, or the
  frame length (the firmware reply is the one place it reads byte 5, as the
  string length).
- ear-web does not split multiple frames in one chunk (everything after the
  first frame is ignored) and does not reassemble a frame split across chunks
  (the tail chunk fails the `0x55` check and is dropped). Whether device frames
  also end in a 2-byte CRC is not visible from ear-web, which never reads past
  the payload. A native client should buffer and split on `8 + byte5` plus the
  trailing CRC, but that trailing length is an inference, not something
  ear-web confirms.

### Command id pattern (observed, not stated by ear-web)

Every read ear-web sends is `0xC0xx`; its reply comes back as `0x40xx` with the
same low byte (`0xC007` -> `0x4007`). Writes are `0xF0xx`. Unsolicited pushes
are `0xE0xx`. ear-web handles no reply or acknowledgement to any write.

## 3. Connect sequence

The sequence starts the moment the port opens. ear-web does not wait for any
reply between frames; it only sleeps 100 ms between steps. Steps that do not
apply to B175 send nothing and do not consume an op id.

| Step | Time | Frame | Op | Notes |
|---|---|---|---|---|
| battery read | 0 ms | `0xC007`, no payload | 01 | |
| EQ read | +100 | none | | skipped for B175 |
| listening mode read | +200 | `0xC050`, no payload | 02 | B175 EQ preset |
| firmware read | +300 | `0xC042`, no payload | 03 | reply triggers the config step below |
| UTC time write | +400 | `0xF00A`, 4 bytes | 04 | Unix seconds, big-endian |
| in-ear read | +500 | none | | skipped for B175 |
| latency read | +600 | `0xC041`, no payload | 05 | |
| personalized ANC read | +700 | none | | B155 only |
| gesture read | +800 | `0xC018`, no payload | 06 | |
| ANC read | +900 | `0xC01E`, no payload | 07 | |
| advanced EQ status read | +1000 | `0xC04C`, no payload | 08 | |
| enhanced bass read | +1100 | none | | B175 not in its model list |
| loading overlay hidden | +1200 | none | | |

Config-driven reads: when the firmware reply (`0x4042`) arrives, ear-web
fetches `/js/ear_config_file.json` over HTTP, finds the B175 entry (it has a
single config, so no firmware range matching), and immediately sends, in this
order:

1. `0xC027` dual connection enabled read
2. `0xC028` dual connection device list, payload `00`
3. `0xC029` audio codec read
4. `0xC04F` spatial audio read
5. `0xC05A` Audiodo personal sound read

These interleave with the timed steps above depending on when the reply and
the HTTP fetch complete, so their op ids are not fixed: each takes the next op
id in actual send order. If they all go out after step `0xC04C`, they carry op
ids 09 to 0D.

Follow-ups triggered by replies:

- Each `0x4028` list reply that adds a MAC ear-web has not seen triggers another
  `0xC028` with payload = the number of devices known so far (one byte). Stops
  when a page adds nothing new or after 20 list requests.
- A `0xE00E` dual device push for an unknown MAC restarts the list read from
  payload `00`.

Periodic polling: **none.** No timer on the B175 page sends anything. Battery
and ANC changes arrive as pushes (`0xE001`, `0xE003`). Opening the gesture
screen resends `0xC018`.

## 4. B175 commands

B175 capabilities from its config entry (`id: "B175"`, `single: 1`,
`deviceType: 6`): `ancLevel` 63, `earDetection` 0, `dualConnection` 1,
`dualConnectionReboot` 0, `highQualityAudio` 4, `spatialAudio` 28,
`audiodo` 1, `diracOpteoSupport` 2, `findDevice` 1, custom EQ bands
low shelf 140 Hz Q 0.8, peak 980 Hz Q 0.7, high shelf 3500 Hz Q 1.0.

Tags: **safe read**, **setting write**, **risky**.

| Feature | Send id | Payload | Reply / push id and decoding | Tag |
|---|---|---|---|---|
| Battery | `0xC007` | none | `0x4007` reply and `0xE001` push. Byte 8 = entry count N. Entry i at 9+2i: device id, level byte. Level bits 0..6 = percent, bit 7 = charging. Ids: 2 left, 3 right, 4 case, 6 single/stereo. | safe read |
| Firmware version | `0xC042` | none | `0x4042`. Byte 5 = length L, bytes 8..8+L-1 = ASCII version string. | safe read |
| ANC / noise control read | `0xC01E` | none | `0x401E` reply and `0xE003` push. Byte 9 = mode code (table below). Byte 8 not read. | safe read |
| ANC / noise control set | `0xF00F` | `01 <mode> 00` | none handled | setting write |
| EQ preset read | `0xC050` | none | `0x4050`. Byte 8 = preset (table below). | safe read |
| EQ preset set | `0xF01D` | `<preset> 00` | none handled | setting write |
| Custom EQ read | `0xC044` | none | `0x4044`. Band gains as float32 at bytes 14..17, 27..30, 40..43 (ear-web takes these as peak, high shelf, low shelf, shown as mid, treble, bass). | safe read |
| Custom EQ set | `0xF041` | 53 bytes, layout below | none handled | setting write |
| Advanced EQ status | `0xC04C` | none | `0x404C`. Byte 8 = 1 enabled. For B175 it only sets an internal flag; no UI. | safe read |
| Low latency read | `0xC041` | none | `0x4041`. Byte 8: 1 on, 2 off. | safe read |
| Low latency set | `0xF040` | `01 00` on, `02 00` off | none handled | setting write |
| Gestures read | `0xC018` | none | `0x4018`. Byte 8 = record count N. Record i at 9+4i: device, button, gesture, action. | safe read |
| Gesture set | `0xF003` | `01 06 <button> <gesture> <action>` | none handled | setting write |
| UTC time | `0xF00A` | 4 bytes, Unix seconds big-endian | none handled | setting write (sent automatically on connect) |
| Spatial audio read | `0xC04F` | none | `0x404F`. Byte 8 = mode, byte 9 = head tracking. | safe read |
| Spatial audio set | `0xF052` | `<mode> <head>` | none handled | setting write |
| Personal sound (Audiodo) read | `0xC05A` | none | `0x405A` reply and `0xE019` push. Byte 8 = 1 on. | safe read |
| Personal sound (Audiodo) set | `0xF05C` | `01` on, `00` off | none handled | setting write |
| Audio codec read | `0xC029` | none | `0x4029`. Byte 8 = 0 AAC, 1 LHDC, 2 LDAC (out of range read as AAC). | safe read |
| Audio codec set | `0xF01C` | `00` AAC, `02` LDAC | the device reboots; ear-web expects the link to drop and reconnects (section 1) | **risky** (reboots device) |
| Dual connection read | `0xC027` | none | `0x4027`. Byte 8 = 1 enabled. | safe read |
| Dual connection enable | `0xF01A` | `01` on, `00` off | none handled; ear-web rereads the list 300 ms after enabling. No reboot on B175. | **risky** (multipoint change) |
| Dual connection list | `0xC028` | 1 byte start index | `0x4028`. Bytes 8..9 not read. Byte 10 = entry count. From byte 11 each entry: flags (high nibble nonzero = this host, low nibble nonzero = connected), 6-byte MAC in display order, name length (bits 0..6), UTF-8 name. | safe read |
| Dual device connect / disconnect | `0xF01B` | `01` or `00`, then 6-byte MAC | `0xE00E` push: byte 8 = 1 connected, bytes 9..14 MAC. ear-web rereads the list 300 ms later. | **risky** (drops or connects a paired host) |
| Ring / find | `0xF002` | see section 6 | none | **risky, unverified**: never actually sent on the B175 page |

ANC mode codes (byte 9 of the reply, byte 1 of the set payload). B175's
`ancLevel` 63 has the transparency (0x10), strength (0x04), mid (0x02) and
adaptive (0x08) bits, so all six are offered:

| Code | Mode |
|---|---|
| `0x05` | Off |
| `0x07` | Transparency |
| `0x01` | Noise cancellation, High |
| `0x02` | Noise cancellation, Mid |
| `0x03` | Noise cancellation, Low |
| `0x04` | Noise cancellation, Adaptive |

EQ presets (listening mode byte):

| Value | Preset |
|---|---|
| 0 | Dirac Opteo (its button is hidden on the page; the value is still decoded on read) |
| 1 | Rock |
| 2 | Electronic |
| 3 | Pop |
| 4 | Enhance vocals |
| 5 | Classical |
| 6 | Custom |

Choosing Custom sends `0xF01D 06 00` and then `0xC044` twice (two code paths
each request it).

Custom EQ set payload (53 bytes). Floats are IEEE-754 float32
little-endian. Gains are integers from -6 to +6 as the UI allows:

| Offset in payload | Size | Content |
|---|---|---|
| 0 | 1 | band count, `03` |
| 1 | 4 | total gain = minus the largest band gain, where the largest is taken over the gains and 0 (all gains <= 0 gives -0.0, bytes `00 00 00 80`) |
| 5 | 13 | band 1: type `01` (peak), gain = mid, freq 980.0, Q 0.7 |
| 18 | 13 | band 2: type `02` (high shelf), gain = treble, freq 3500.0, Q 1.0 |
| 31 | 13 | band 3: type `00` (low shelf), gain = bass, freq 140.0, Q 0.8 |
| 44 | 9 | zero bytes (3 per band) |

Each 13-byte band is type (1), gain (4), frequency (4), Q (4). On reading,
ear-web byte-reverses each 4-byte float and decodes it big-endian (that is, a
little-endian float32), with one quirk inherited from the vendor app: if the
three high bytes are zero and bit 7 of the low byte is set, it clears that bit
and negates the value. No realistic gain hits that case.

Gesture fields on B175 (device code is always 6):

| Control | button | gesture | Actions ear-web offers |
|---|---|---|---|
| Button, single press | 10 | 1 | 31 Essential News, 11 voice assistant, noise control, 27 spatial audio, 29 mic on/off, 1 no action |
| Button, press and hold | 10 | 7 | 31, 11, noise control, 27, 33 Essential Space, 29, 1 |
| Energy slider | 5 | 1 | 35 bass tuning, 36 treble tuning |
| Roller, press and hold | 1 | 7 | noise control only |

Noise control is one of four action codes naming which modes the gesture cycles
through: 10 = transparency + noise cancellation + off, 20 = noise cancellation +
off, 21 = transparency + off, 22 = transparency + noise cancellation. The
config's factory default record set is
`04 060A0701 060A010B 06050123 06010716` (count 4, then records): hold = no
action, single = voice assistant, slider = bass, roller hold = 22. The
`nothingDefaultControl` variant differs only in hold = 11.

Spatial audio on B175: `spatialAudio` 28 = bits 0x04, 0x08, 0x10. ear-web maps
0x08 to Concert and 0x10 to Theatre and has no mode for 0x04. Head Tracked
(0x01), Fixed (0x02) and Game (0x20) are not offered.

| Mode | Payload | Read back as |
|---|---|---|
| Off | `00 00` | head 0, mode 0 |
| Concert | `02 00` | head 0, mode 2 |
| Theatre | `03 00` | head 0, mode 3 |

(For other models: Head Tracked `01 01`, Fixed `01 00`, Game `04 00`. Any
reply with byte 9 = 1 is shown as Head Tracked.)

Audio codec on B175: `highQualityAudio` 4 has the LDAC bit (0x04) and not LHDC
(0x02), so the choices are AAC (0) and LDAC (2). `diracOpteoSupport` is 2, so
the Dirac Opteo / LDAC exclusion (tier 1 only) does not apply.

Not in ear-web at all (no frame exists in the reviewed source for any model):
firmware update / OTA, factory reset, device rename, Bluetooth pairing or
unpairing. The config's `otaProtocol`, `supportCTKD` and `isUseGattOverBREDR`
fields are never read.

## 5. Golden frames

Built exactly as ear-web builds them. The op id byte is offset 7; every frame
below uses op `01` unless stated. For any other op id, replace byte 7 and
recompute the CRC over bytes 0 to the end of the payload.

### Connect sequence, actual op ids

| Op | Frame |
|---|---|
| 01 battery | `55600107c0000001acdf` |
| 02 listening mode | `55600150c00000029912` |
| 03 firmware | `55600142c0000003e0d1` |
| 04 UTC time, example epoch 1790726400 (2026-09-30 00:00:00 UTC, `6abc5100`) | `5560010af00400046abc51002342` |
| 05 latency | `55600141c000000524d3` |
| 06 gestures | `55600118c000000678df` |
| 07 ANC | `5560011ec0000007311f` |
| 08 advanced EQ status | `5560014cc0000008c8d7` |
| 09 dual enabled (if after 08) | `55600127c00000092cde` |
| 0A dual list from 0 (if after 08) | `55600128c001000a00de2e` |
| 0B codec (if after 08) | `55600129c000000bc4de` |
| 0C spatial (if after 08) | `5560014fc000000c8d14` |
| 0D Audiodo (if after 08) | `5560015ac000000d4117` |

### Reads, op 01

| Command | Frame |
|---|---|
| battery `0xC007` | `55600107c0000001acdf` |
| firmware `0xC042` | `55600142c00000016110` |
| ANC `0xC01E` | `5560011ec0000001b11d` |
| listening mode `0xC050` | `55600150c0000001d913` |
| custom EQ `0xC044` | `55600144c0000001e910` |
| advanced EQ status `0xC04C` | `5560014cc000000108d1` |
| latency `0xC041` | `55600141c00000012510` |
| gestures `0xC018` | `55600118c0000001391d` |
| spatial `0xC04F` | `5560014fc00000014cd1` |
| Audiodo `0xC05A` | `5560015ac00000014112` |
| codec `0xC029` | `55600129c000000144d9` |
| dual enabled `0xC027` | `55600127c00000012d18` |
| dual list, start 0 | `55600128c001000100d91e` |
| dual list, start 1 | `55600128c00100010118de` |

### ANC set `0xF00F`, op 01

| Mode | Frame |
|---|---|
| Off | `5560010ff0030001010500fb17` |
| Transparency | `5560010ff0030001010700fa77` |
| NC High | `5560010ff0030001010100f9d7` |
| NC Mid | `5560010ff0030001010200f927` |
| NC Low | `5560010ff0030001010300f8b7` |
| NC Adaptive | `5560010ff0030001010400fa87` |

### EQ preset set `0xF01D`, op 01

| Preset | Frame |
|---|---|
| 0 Dirac Opteo | `5560011df00200010000fb69` |
| 1 Rock | `5560011df00200010100faf9` |
| 2 Electronic | `5560011df00200010200fa09` |
| 3 Pop | `5560011df00200010300fb99` |
| 4 Enhance vocals | `5560011df00200010400f9a9` |
| 5 Classical | `5560011df00200010500f839` |
| 6 Custom | `5560011df00200010600f8c9` |

### Custom EQ set `0xF041`, op 01

Bass +3, mid 0, treble -3 (total gain -3.0):

`55600141f035000103000040c00100000000000075443333333f02000040c000c05a450000803f000000404000000c43cdcc4c3f0000000000000000003252`

Flat, all gains 0 (total gain -0.0):

`55600141f035000103000000800100000000000075443333333f020000000000c05a450000803f000000000000000c43cdcc4c3f0000000000000000000615`

### Low latency set `0xF040`, op 01

| State | Frame |
|---|---|
| On | `55600140f00200010100763c` |
| Off | `55600140f0020001020076cc` |

### Gesture set `0xF003`, op 01

| Control, action | Frame |
|---|---|
| single press, 31 Essential News | `55600103f005000101060a011f2335` |
| single press, 11 voice assistant | `55600103f005000101060a010b233a` |
| single press, 10 noise control (all three) | `55600103f005000101060a010ae2fa` |
| single press, 20 NC + off | `55600103f005000101060a011462f2` |
| single press, 21 transparency + off | `55600103f005000101060a0115a332` |
| single press, 22 transparency + NC | `55600103f005000101060a0116e333` |
| single press, 27 spatial audio | `55600103f005000101060a011b22f6` |
| single press, 29 mic on/off | `55600103f005000101060a011da2f4` |
| single press, 1 no action | `55600103f005000101060a0101a33d` |
| press and hold, 31 | `55600103f005000101060a071f2095` |
| press and hold, 11 | `55600103f005000101060a070b209a` |
| press and hold, 10 | `55600103f005000101060a070ae15a` |
| press and hold, 20 | `55600103f005000101060a07146152` |
| press and hold, 21 | `55600103f005000101060a0715a092` |
| press and hold, 22 | `55600103f005000101060a0716e093` |
| press and hold, 27 | `55600103f005000101060a071b2156` |
| press and hold, 33 Essential Space | `55600103f005000101060a0721a145` |
| press and hold, 29 | `55600103f005000101060a071da154` |
| press and hold, 1 | `55600103f005000101060a0701a09d` |
| slider, 35 bass | `55600103f005000101060501231327` |
| slider, 36 treble | `55600103f0050001010605012452e5` |
| roller hold, 10 | `55600103f0050001010601070a9098` |
| roller hold, 20 | `55600103f005000101060107141090` |
| roller hold, 21 | `55600103f00500010106010715d150` |
| roller hold, 22 | `55600103f005000101060107169151` |

### Spatial audio set `0xF052`, op 01

| Mode | Frame |
|---|---|
| Off | `55600152f0020001000045ad` |
| Concert | `55600152f0020001020044cd` |
| Theatre | `55600152f00200010300455d` |

### Personal sound (Audiodo) set `0xF05C`, op 01

| State | Frame |
|---|---|
| On | `5560015cf001000101526e` |
| Off | `5560015cf00100010093ae` |

### Risky writes, op 01 (listed for recognition, not for sending)

| Command | Frame |
|---|---|
| codec AAC `0xF01C 00` (reboots) | `5560011cf0010001009d6e` |
| codec LDAC `0xF01C 02` (reboots) | `5560011cf0010001021caf` |
| dual connection off `0xF01A 00` | `5560011af0010001009d08` |
| dual connection on `0xF01A 01` | `5560011af0010001015cc8` |
| connect host AA-BB-CC-DD-EE-FF `0xF01B` | `5560011bf007000101aabbccddeeffae0b` |
| disconnect host AA-BB-CC-DD-EE-FF `0xF01B` | `5560011bf007000100aabbccddeeffbecb` |
| ring, earbud-left form `0xF002 02 01` (unverified on B175) | `55600102f00200010201d508` |
| ring stop, earbud-left form `0xF002 02 00` | `55600102f0020001020014c8` |

## 6. How B175 differs from the earbud models

- **Battery**: the config marks B175 `single: 1`. ear-web's B175 page only draws
  the single battery (`stereo`, device id `0x06`); its battery handler ignores
  left, right and case ids. There is no case. If the headphone reported ids 2 or
  3 instead, the page would show nothing, so id 6 is what ear-web expects. That
  it is the id the device actually sends is inferred from the page working, not
  stated in the source.
- **No in-ear detection**: the `0xC00E` read is skipped for B175, `earDetection`
  is 0, and the headphone quick-settings panel has no in-ear toggle. Never send
  `0xF004` to it.
- **EQ uses the listening-mode commands** (`0xC050` / `0xF01D` / `0x4050`), not
  the earbud EQ pair (`0xC01F` / `0xF010`). Preset numbering is in section 4.
- **No bass enhance**: B175 is left out of both enhanced-bass lists
  (`0xC04E` / `0xF051`) and the page has no bass panel. Bass and treble tuning
  live on the energy slider gesture (actions 35 / 36) instead.
- **Gestures address device 6** (earbuds use 2 left, 3 right) and use the
  button field (10 magic button, 5 energy slider, 1 roller). The roller's own
  press, double, triple press and roll (button 8) are listed in the config as
  fixed and ear-web never writes them.
- **ANC** has all six states, including Mid and Adaptive.
- **Absent for B175**: ear fit test (`0xF014`), personalized ANC (`0xC020` /
  `0xF011`, B155 only), case LED colour (B181 only), super mic, call
  transparency, walkie-talkie, long battery mode, leakage protection (none are
  flagged in its config).
- **Ring** goes through the shared earbud form `0xF002 02 01` (left) on this
  page: the headphone form `06 01` is only used for B170, B164, B186 and B192,
  and B175 is not in that list even though its config says `deviceType: 6` and
  `findDevice: 1`. At `4bcaa8a` the header's Ring button threw before building
  a frame; at `d076e13` the left-ring handler no longer throws, so ear-web now
  sends `02 01` to a B175. Which form B175 accepts cannot be settled from
  ear-web.

## 7. Shared facts for every model

Extracted 2026-10-13 from the ear-web repository
(github.com/radiance-project/ear-web) at `d076e13` (2026-10-07). Every script
and the config file are byte-identical to what earweb.bttl.xyz served that
day; the HTML pages differ only by the Cloudflare challenge snippet. The B175
extraction matches `4bcaa8a` (2026-08-20); nothing B175 sends changed since.
ear-web's code was read, never run. The frames in sections 8 to 29 were built
with an independent Python encoder (the one that reproduced section 5) and the
tests rebuild every one of them from the model tables.

Routing: the index page maps the Fast Pair model id to a base model through
its own table and opens that base's page. `altBase` entries (B183, B187,
B198, B201) only pick product images; the config is always looked up by base.

Connect sequence: every page runs the timed steps of section 3, and each
step is sent or skipped by hardcoded model lists, not by the config:

| Step | Sent to |
|---|---|
| `0xC01F` EQ, +100 | earbud EQ models (all but the listening-mode ones) |
| `0xC050` listening mode, +200 | B168, B172, B175, B179, B184, B185, B189, B195, B197 |
| `0xF00A` UTC, +400 | every model but B181 |
| `0xC00E` in-ear, +500 | every model but B174, B175, B185, B186, B189 |
| `0xC020` personalized ANC, +700 | B155 |
| `0xC01E` ANC, +900 | every model; B157 only on firmware whose third part is `2`, checked when the step runs |
| `0xC04E` bass enhance, +1100 | B162, B164, B168, B170, B171, B172, B173, B179, B184, B185, B186, B189, B195, B197 |

After the firmware reply, the config-driven reads go out in this order, each
only when its config flag is set: dual connection (`0xC027`, `0xC028 00`),
codec (`0xC029`), spatial audio (`0xC04F`, skipped when `useAudiodoSpatial`
is set), personal sound (`0xC022` Mimi or `0xC05A` Audiodo), super mic
(`0xC05E`, then mic mode `0xC064`), call transparency (`0xC05F`),
walkie-talkie (`0xC060`), long battery mode (`0xC067`), anti-leakage
(`0xC071`), flat EQ (`0xC068`). A model with several configs picks the first
whose `minFirmware`/`maxFirmware` contain the firmware: parts compare
numerically, `*` skips a part, anything after `-` is dropped, both bounds are
inclusive. Only B170's configs differ in a flag that changes frames
(`audiodo` from `*.*.1.75`).

`0x6003` is handled like the `0xE003` ANC push, and `0x6019` like the
`0xE019` Audiodo push.

Commands the models use beyond section 4:

| Feature | Send id | Payload | Reply / push id and decoding | Tag |
|---|---|---|---|---|
| EQ preset read (earbud EQ) | `0xC01F` | none | `0x401F`. Byte 8 = preset. | safe read |
| EQ preset set (earbud EQ) | `0xF010` | `<preset> 00` | none handled | setting write |
| Advanced EQ switch | `0xF04F` | `01 00` on, `00 00` off | none handled | setting write |
| Advanced EQ bands read | `0xC04D` | `FF` (current profile) | `0x404D`, bands; not used here | safe read |
| Advanced EQ bands set | `0xF050` | profile, 8 bands | none handled; not sent by this tool | setting write |
| Precision driver EQ mode read | `0xC06C` | none | `0x406C`. Byte 8 = 1 enabled. If 0, ear-web writes `0xF06C 01` once. | safe read |
| Precision driver EQ bands read | `0xC06D` | `FF` | `0x406D`, bands; not used here | safe read |
| In-ear detection read | `0xC00E` | none | `0x400E`. Byte 10 = 1 on, anything else off. | safe read |
| In-ear detection set | `0xF004` | `01 01 01` on, `01 01 00` off | none handled | setting write |
| Personalized ANC read | `0xC020` | none | `0x4020`. Byte 8 nonzero = on. | safe read |
| Personalized ANC set | `0xF011` | `01` on, `00` off | none handled | setting write |
| Personal sound (Mimi) read | `0xC022` | none | `0x4022`. Byte 8 = 1 on. | safe read |
| Personal sound (Mimi) set | `0xF015` | `01` on, `00` off | none handled | setting write |
| Bass enhance read | `0xC04E` | none | `0x404E`. Byte 8 = 1 enabled, byte 9 = wire level. | safe read |
| Bass enhance set | `0xF051` | `<enabled> <wire level>` | none handled | setting write |
| Super mic read / set | `0xC05E` / `0xF05F` | `01` on, `00` off | `0x405E`. Byte 8 = 1 on. | setting write |
| Mic mode read / set | `0xC064` / `0xF065` | `00` natural, `01` voice focus | `0x4064`. Byte 8 = mode. | setting write |
| Call transparency read / set | `0xC05F` / `0xF060` | `01` on, `00` off | `0x405F`. Byte 8 = 1 on. | setting write |
| Walkie-talkie read / set | `0xC060` / `0xF061` | `01` on, `00` off | `0x4060`. Byte 8 = 1 on. | setting write |
| Long battery mode read | `0xC067` | none | `0x4067`. Byte 8 = 1 on. | safe read |
| Long battery mode set | `0xF068` | `01` on, `00` off | the device reboots (ear-web expects the drop, section 1) | **risky** (reboots device) |
| Anti-leakage read | `0xC071` | none | `0x4071`. Byte 8 = mode: 0 off, 1 calls only, 2 audio and calls. | safe read |
| Anti-leakage set | `0xF075` | `<mode>` | none handled | setting write |
| Flat EQ read | `0xC068` | none | `0x4068` reply, `0xE01C` and `0x601C` pushes. Byte 8 = 1 on. | safe read |
| Ear fit test | `0xF014` | `01` | `0xE00D` push: byte 8 left, 9 right; 0 good, 1 adjust, 2 not detected | **risky** (starts a test tone) |
| Case LED colour read / set (B181) | `0xC017` / `0xF00D` | 5 records `<id> R G B` | `0x4017` | not offered here |

ANC: the full widget offers the six modes of section 4 by `ancLevel` bits.
The basic widget (B168, B185) has noise cancellation, transparency and off;
its noise cancellation button always sends the High code `01`, and a read of
`01` to `04` shows that button. B181, B157 and B163 override the widget with
their own subsets, listed in their sections.

EQ, earbud pages: each preset button sends `0xF010 <preset> 00`; on pages
with an Advanced button it is preceded by `0xF04F 00 00`, and Advanced itself
sends `0xF04F 01 00` then `0xC04D FF` (plus `0xC06C`, `0xC06D FF` where
`supportMiddleEq` is set). A read of 6 shows Advanced; on B155, B157, B170,
B171, B174, B186 and B192 an advanced EQ status reply of 1 does too. Custom
sends `0xC044` twice after the preset write, as on B175.

Bass enhance comes in two schemes. Five levels: wire value = level × 2; a
level click keeps the current enabled bit and the on/off switch keeps the
current level, so `01 <2n>` is what the switch sends at level n. Three
stages (B186, B189, B195, B197): levels 0, 1, 2 are wire 0, 5, 10, and a
level click sends enabled = level is not 0. B185 reads bass but ear-web never
writes it: the base is missing from the write list.

Spatial audio modes come from the `spatialAudio` bits: 0x01 Head Tracked
(`01 01`), 0x02 Fixed (`01 00`), 0x08 Concert (`02 00`), 0x10 Theatre
(`03 00`), 0x20 Game (`04 00`); 0x04 has no mode. A reply with head byte 1
reads as Head Tracked.

Reads, op 01:

| Command | Frame |
|---|---|
| EQ preset `0xC01F` | `5560011fc00000018cdd` |
| advanced EQ bands `0xC04D FF` | `5560014dc0010001ff90ab` |
| precision driver EQ mode `0xC06C` | `5560016cc00000018916` |
| precision driver EQ bands `0xC06D FF` | `5560016dc0010001ff97cb` |
| in-ear `0xC00E` | `5560010ec000000170de` |
| bass enhance `0xC04E` | `5560014ec00000017111` |
| long battery mode `0xC067` | `55600167c00000012cd7` |
| anti-leakage `0xC071` | `55600171c00000016514` |
| flat EQ `0xC068` | `55600168c000000178d6` |

Risky, op 01 (listed for recognition, not for sending):

| Command | Frame |
|---|---|
| long battery mode off `0xF068 00` (reboots) | `55600168f00100010097da` |
| long battery mode on `0xF068 01` (reboots) | `55600168f001000101561a` |
| ring headphone form `0xF002 06 01` | `55600102f00200010601d7c8` |
| ring right bud `0xF002 03 01` | `55600102f00200010301d498` |
| ring both, B181 form `0xF002 01` | `55600102f0010001015f10` |
| ear fit test `0xF014 01` | `55600114f0010001015de6` |

## 8. Nothing ear (2) (B155)

Page `MainControl_two`. Fast Pair `ACC520`, `DEE8C0`. Earbuds with a case
(battery ids 2, 3, 4). Two configs (up to `*.*.1.94`, from `*.*.1.95`) differ
only in `advancedEq`, which ear-web never reads.

- ANC: all six modes. Codec AAC or LHDC.
- EQ: earbud EQ with Advanced; advanced EQ status 1 shows Advanced. Custom
  EQ high shelf 3400 Hz.
- Personalized ANC (B155 only): read at +700 ms, set `0xF011`.
- Personal sound through Mimi (`0xC022` / `0xF015`).
- In-ear detection and low latency. No bass enhance or spatial audio.
- Dual connection writes reboot this model (`dualConnectionReboot` 1).

Connect sequence, actual op ids:

| Op | Frame |
|---|---|
| 01 battery, 0 ms | `55600107c0000001acdf` |
| 02 EQ, +100 | `5560011fc0000002ccdc` |
| 03 firmware, +300 | `55600142c0000003e0d1` |
| 04 UTC time, +400 | `5560010af00400046abc51002342` |
| 05 in-ear, +500 | `5560010ec0000005711d` |
| 06 latency, +600 | `55600141c000000664d2` |
| 07 personalized ANC, +700 | `55600120c000000718da` |
| 08 gestures, +800 | `55600118c0000008f91b` |
| 09 ANC, +900 | `5560011ec0000009b0db` |
| 0A advanced EQ status, +1000 | `5560014cc000000a4916` |
| 0B dual enabled | `55600127c000000bad1f` |
| 0C dual list from 0 | `55600128c001000c00dd8e` |
| 0D codec | `55600129c000000d44dc` |
| 0E Mimi personal sound | `55600122c000000ea11c` |

Writes, op 01:

| Setting | Frames, in order |
|---|---|
| `anc off` | `5560010ff0030001010500fb17` |
| `anc transparency` | `5560010ff0030001010700fa77` |
| `anc high` | `5560010ff0030001010100f9d7` |
| `anc mid` | `5560010ff0030001010200f927` |
| `anc low` | `5560010ff0030001010300f8b7` |
| `anc adaptive` | `5560010ff0030001010400fa87` |
| `eq balanced` | `5560014ff0020001000088ac` `55600110f0020001000027a9` |
| `eq voice` | `5560014ff0020001000088ac` `55600110f002000101002639` |
| `eq treble` | `5560014ff0020001000088ac` `55600110f0020001020026c9` |
| `eq bass` | `5560014ff0020001000088ac` `55600110f002000103002759` |
| `eq custom` | `5560014ff0020001000088ac` `55600110f0020001050024f9` `55600144c0000001e910` `55600144c0000001e910` |
| `eq advanced` | `5560014ff00200010100893c` `5560014dc0010001ff90ab` |
| `eq-custom 3 0 -3` | `55600141f035000103000040c00100000000000075443333333f02000040c0008054450000803f000000404000000c43cdcc4c3f0000000000000000002aa2` |
| `eq-custom 0 0 0` | `55600141f035000103000000800100000000000075443333333f0200000000008054450000803f000000000000000c43cdcc4c3f0000000000000000001ee5` |
| `low-latency on` | `55600140f00200010100763c` |
| `low-latency off` | `55600140f0020001020076cc` |
| `in-ear on` | `55600104f003000101010179a4` |
| `in-ear off` | `55600104f0030001010100b864` |
| `personalized-anc on` | `55600111f0010001015db3` |
| `personalized-anc off` | `55600111f0010001009c73` |
| `personal-sound on` | `55600115f0010001015c37` |
| `personal-sound off` | `55600115f0010001009df7` |

## 9. Nothing ear (stick) (B157)

Page `MainControl_sticks`. Fast Pair `1016DD`. Earbuds with a case (ids
2, 3, 4). Three configs; the flags that differ (`ancLevel`, `advancedEq`)
change no frame.

- ANC exists only on firmware whose third part is the string `2` (`1.0.2.x`):
  the page shows it and the `0xC01E` read at +900 ms goes out only then, and
  only if the firmware reply arrived first. Two buttons: noise cancellation
  sends `01 01 00` (High) and off `01 05 00`; a read of `01` or `03` shows
  noise cancellation, `07` shows nothing.
- EQ: earbud EQ with Advanced on every firmware; advanced EQ status 1 shows
  Advanced. Custom EQ peak Q is **0.66**, high shelf 3500 Hz.
- In-ear detection and low latency. No codec, dual connection or bass.

Connect sequence, actual op ids, firmware `1.0.2.0`:

| Op | Frame |
|---|---|
| 01 battery, 0 ms | `55600107c0000001acdf` |
| 02 EQ, +100 | `5560011fc0000002ccdc` |
| 03 firmware, +300 | `55600142c0000003e0d1` |
| 04 UTC time, +400 | `5560010af00400046abc51002342` |
| 05 in-ear, +500 | `5560010ec0000005711d` |
| 06 latency, +600 | `55600141c000000664d2` |
| 07 gestures, +800 | `55600118c0000007b91f` |
| 08 ANC, +900 | `5560011ec0000008711b` |
| 09 advanced EQ status, +1000 | `5560014cc00000090917` |

Connect sequence, actual op ids, firmware `1.0.1.80`:

| Op | Frame |
|---|---|
| 01 battery, 0 ms | `55600107c0000001acdf` |
| 02 EQ, +100 | `5560011fc0000002ccdc` |
| 03 firmware, +300 | `55600142c0000003e0d1` |
| 04 UTC time, +400 | `5560010af00400046abc51002342` |
| 05 in-ear, +500 | `5560010ec0000005711d` |
| 06 latency, +600 | `55600141c000000664d2` |
| 07 gestures, +800 | `55600118c0000007b91f` |
| 08 advanced EQ status, +1000 | `5560014cc0000008c8d7` |

Writes, op 01:

| Setting | Frames, in order |
|---|---|
| `anc off` | `5560010ff0030001010500fb17` |
| `anc nc` | `5560010ff0030001010100f9d7` |
| `eq balanced` | `5560014ff0020001000088ac` `55600110f0020001000027a9` |
| `eq voice` | `5560014ff0020001000088ac` `55600110f002000101002639` |
| `eq treble` | `5560014ff0020001000088ac` `55600110f0020001020026c9` |
| `eq bass` | `5560014ff0020001000088ac` `55600110f002000103002759` |
| `eq custom` | `5560014ff0020001000088ac` `55600110f0020001050024f9` `55600144c0000001e910` `55600144c0000001e910` |
| `eq advanced` | `5560014ff00200010100893c` `5560014dc0010001ff90ab` |
| `eq-custom 3 0 -3` | `55600141f035000103000040c0010000000000007544c3f5283f02000040c000c05a450000803f000000404000000c43cdcc4c3f00000000000000000049fd` |
| `eq-custom 0 0 0` | `55600141f03500010300000080010000000000007544c3f5283f020000000000c05a450000803f000000000000000c43cdcc4c3f0000000000000000007dba` |
| `low-latency on` | `55600140f00200010100763c` |
| `low-latency off` | `55600140f0020001020076cc` |
| `in-ear on` | `55600104f003000101010179a4` |
| `in-ear off` | `55600104f0030001010100b864` |

## 10. Nothing Ear (a) (B162)

Page `MainControl_cleffa`. Fast Pair `5E3FBC`, `03464E`, `8B6380`, and
`C34F3B`, `404D6D`, `839E9A` with altBase B183. Earbuds with a case.

- ANC: all six modes. Codec AAC or LDAC. No spatial audio.
- EQ: earbud EQ without Advanced or a `0xF04F` switch: balanced, voice,
  treble, bass, custom. Custom EQ high shelf 3400 Hz.
- Bass enhance, five levels. In-ear detection and low latency.

Connect sequence, actual op ids:

| Op | Frame |
|---|---|
| 01 battery, 0 ms | `55600107c0000001acdf` |
| 02 EQ, +100 | `5560011fc0000002ccdc` |
| 03 firmware, +300 | `55600142c0000003e0d1` |
| 04 UTC time, +400 | `5560010af00400046abc51002342` |
| 05 in-ear, +500 | `5560010ec0000005711d` |
| 06 latency, +600 | `55600141c000000664d2` |
| 07 gestures, +800 | `55600118c0000007b91f` |
| 08 ANC, +900 | `5560011ec0000008711b` |
| 09 advanced EQ status, +1000 | `5560014cc00000090917` |
| 0A bass enhance, +1100 | `5560014ec000000a30d6` |
| 0B dual enabled | `55600127c000000bad1f` |
| 0C dual list from 0 | `55600128c001000c00dd8e` |
| 0D codec | `55600129c000000d44dc` |

Writes, op 01:

| Setting | Frames, in order |
|---|---|
| `anc off` | `5560010ff0030001010500fb17` |
| `anc transparency` | `5560010ff0030001010700fa77` |
| `anc high` | `5560010ff0030001010100f9d7` |
| `anc mid` | `5560010ff0030001010200f927` |
| `anc low` | `5560010ff0030001010300f8b7` |
| `anc adaptive` | `5560010ff0030001010400fa87` |
| `eq balanced` | `55600110f0020001000027a9` |
| `eq voice` | `55600110f002000101002639` |
| `eq treble` | `55600110f0020001020026c9` |
| `eq bass` | `55600110f002000103002759` |
| `eq custom` | `55600110f0020001050024f9` `55600144c0000001e910` `55600144c0000001e910` |
| `eq-custom 3 0 -3` | `55600141f035000103000040c00100000000000075443333333f02000040c0008054450000803f000000404000000c43cdcc4c3f0000000000000000002aa2` |
| `eq-custom 0 0 0` | `55600141f035000103000000800100000000000075443333333f0200000000008054450000803f000000000000000c43cdcc4c3f0000000000000000001ee5` |
| `bass 1` | `55600151f00200010102f6fc` |
| `bass 2` | `55600151f0020001010476fe` |
| `bass 3` | `55600151f00200010106f73f` |
| `bass 4` | `55600151f0020001010876fb` |
| `bass 5` | `55600151f0020001010af73a` |
| `bass on, at level 5` | `55600151f0020001010af73a` |
| `bass off, at level 5` | `55600151f0020001000af6aa` |
| `low-latency on` | `55600140f00200010100763c` |
| `low-latency off` | `55600140f0020001020076cc` |
| `in-ear on` | `55600104f003000101010179a4` |
| `in-ear off` | `55600104f0030001010100b864` |

## 11. CMF Buds Pro (B163)

Page `MainControl_corsola`. Fast Pair `5F8F82`, `2EB1CA`, `ADD2C4`.
Earbuds with a case. One config, `ancLevel` 55.

- ANC override: off, transparency, high, mid, low. A strength button only
  stores the strength and resends the current mode, so it resends
  transparency or off when one of those is active. A read of `04` shows noise
  cancellation without a strength.
- EQ: earbud EQ without Advanced or `0xF04F`. Custom EQ high shelf 3400 Hz.
- In-ear detection and low latency. No dual connection, codec or bass.

Connect sequence, actual op ids:

| Op | Frame |
|---|---|
| 01 battery, 0 ms | `55600107c0000001acdf` |
| 02 EQ, +100 | `5560011fc0000002ccdc` |
| 03 firmware, +300 | `55600142c0000003e0d1` |
| 04 UTC time, +400 | `5560010af00400046abc51002342` |
| 05 in-ear, +500 | `5560010ec0000005711d` |
| 06 latency, +600 | `55600141c000000664d2` |
| 07 gestures, +800 | `55600118c0000007b91f` |
| 08 ANC, +900 | `5560011ec0000008711b` |
| 09 advanced EQ status, +1000 | `5560014cc00000090917` |

Writes, op 01:

| Setting | Frames, in order |
|---|---|
| `anc off` | `5560010ff0030001010500fb17` |
| `anc transparency` | `5560010ff0030001010700fa77` |
| `anc high` | `5560010ff0030001010100f9d7` |
| `anc mid` | `5560010ff0030001010200f927` |
| `anc low` | `5560010ff0030001010300f8b7` |
| `eq balanced` | `55600110f0020001000027a9` |
| `eq voice` | `55600110f002000101002639` |
| `eq treble` | `55600110f0020001020026c9` |
| `eq bass` | `55600110f002000103002759` |
| `eq custom` | `55600110f0020001050024f9` `55600144c0000001e910` `55600144c0000001e910` |
| `eq-custom 3 0 -3` | `55600141f035000103000040c00100000000000075443333333f02000040c0008054450000803f000000404000000c43cdcc4c3f0000000000000000002aa2` |
| `eq-custom 0 0 0` | `55600141f035000103000000800100000000000075443333333f0200000000008054450000803f000000000000000c43cdcc4c3f0000000000000000001ee5` |
| `low-latency on` | `55600140f00200010100763c` |
| `low-latency off` | `55600140f0020001020076cc` |
| `in-ear on` | `55600104f003000101010179a4` |
| `in-ear off` | `55600104f0030001010100b864` |

## 12. CMF Neckband Pro (B164)

Page `MainControl_crobat`. Fast Pair `4DFC4A`, `26C190`, `AE35FD`. A
neckband: battery id 6 only.

- ANC: all six modes.
- EQ: the earbud EQ commands despite `deviceType` 6, no Advanced. Custom EQ
  high shelf 6900 Hz.
- Spatial audio: off and fixed. Bass enhance, five levels. Low latency.
- ear-web sends the `0xC00E` in-ear read but ignores the reply, and the page
  has no in-ear toggle.
- Dual connection writes reboot this model. Ring uses the headphone form.

Connect sequence, actual op ids:

| Op | Frame |
|---|---|
| 01 battery, 0 ms | `55600107c0000001acdf` |
| 02 EQ, +100 | `5560011fc0000002ccdc` |
| 03 firmware, +300 | `55600142c0000003e0d1` |
| 04 UTC time, +400 | `5560010af00400046abc51002342` |
| 05 in-ear, +500 | `5560010ec0000005711d` |
| 06 latency, +600 | `55600141c000000664d2` |
| 07 gestures, +800 | `55600118c0000007b91f` |
| 08 ANC, +900 | `5560011ec0000008711b` |
| 09 advanced EQ status, +1000 | `5560014cc00000090917` |
| 0A bass enhance, +1100 | `5560014ec000000a30d6` |
| 0B dual enabled | `55600127c000000bad1f` |
| 0C dual list from 0 | `55600128c001000c00dd8e` |
| 0D spatial | `5560014fc000000d4cd4` |

Writes, op 01:

| Setting | Frames, in order |
|---|---|
| `anc off` | `5560010ff0030001010500fb17` |
| `anc transparency` | `5560010ff0030001010700fa77` |
| `anc high` | `5560010ff0030001010100f9d7` |
| `anc mid` | `5560010ff0030001010200f927` |
| `anc low` | `5560010ff0030001010300f8b7` |
| `anc adaptive` | `5560010ff0030001010400fa87` |
| `eq balanced` | `55600110f0020001000027a9` |
| `eq voice` | `55600110f002000101002639` |
| `eq treble` | `55600110f0020001020026c9` |
| `eq bass` | `55600110f002000103002759` |
| `eq custom` | `55600110f0020001050024f9` `55600144c0000001e910` `55600144c0000001e910` |
| `eq-custom 3 0 -3` | `55600141f035000103000040c00100000000000075443333333f02000040c000a0d7450000803f000000404000000c43cdcc4c3f000000000000000000531b` |
| `eq-custom 0 0 0` | `55600141f035000103000000800100000000000075443333333f020000000000a0d7450000803f000000000000000c43cdcc4c3f000000000000000000675c` |
| `spatial off` | `55600152f0020001000045ad` |
| `spatial fixed` | `55600152f00200010100443d` |
| `bass 1` | `55600151f00200010102f6fc` |
| `bass 2` | `55600151f0020001010476fe` |
| `bass 3` | `55600151f00200010106f73f` |
| `bass 4` | `55600151f0020001010876fb` |
| `bass 5` | `55600151f0020001010af73a` |
| `bass on, at level 5` | `55600151f0020001010af73a` |
| `bass off, at level 5` | `55600151f0020001000af6aa` |
| `low-latency on` | `55600140f00200010100763c` |
| `low-latency off` | `55600140f0020001020076cc` |

## 13. CMF Buds (B168)

Page `MainControl_donphan`. Fast Pair `D35E18`, `ACCE54`, `150A27`.
Earbuds with a case.

- ANC: the basic widget (noise cancellation, transparency, off).
- EQ: listening-mode commands with a Dirac Opteo button that sends `00`; a
  read of 7 also shows Dirac. Custom EQ high shelf 6900 Hz.
- Bass enhance, five levels. In-ear detection and low latency.
- Dual connection writes reboot this model.

Connect sequence, actual op ids:

| Op | Frame |
|---|---|
| 01 battery, 0 ms | `55600107c0000001acdf` |
| 02 listening mode, +200 | `55600150c00000029912` |
| 03 firmware, +300 | `55600142c0000003e0d1` |
| 04 UTC time, +400 | `5560010af00400046abc51002342` |
| 05 in-ear, +500 | `5560010ec0000005711d` |
| 06 latency, +600 | `55600141c000000664d2` |
| 07 gestures, +800 | `55600118c0000007b91f` |
| 08 ANC, +900 | `5560011ec0000008711b` |
| 09 advanced EQ status, +1000 | `5560014cc00000090917` |
| 0A bass enhance, +1100 | `5560014ec000000a30d6` |
| 0B dual enabled | `55600127c000000bad1f` |
| 0C dual list from 0 | `55600128c001000c00dd8e` |

Writes, op 01:

| Setting | Frames, in order |
|---|---|
| `anc off` | `5560010ff0030001010500fb17` |
| `anc transparency` | `5560010ff0030001010700fa77` |
| `anc nc` | `5560010ff0030001010100f9d7` |
| `eq dirac` | `5560011df00200010000fb69` |
| `eq rock` | `5560011df00200010100faf9` |
| `eq electronic` | `5560011df00200010200fa09` |
| `eq pop` | `5560011df00200010300fb99` |
| `eq vocals` | `5560011df00200010400f9a9` |
| `eq classical` | `5560011df00200010500f839` |
| `eq custom` | `5560011df00200010600f8c9` `55600144c0000001e910` `55600144c0000001e910` |
| `eq-custom 3 0 -3` | `55600141f035000103000040c00100000000000075443333333f02000040c000a0d7450000803f000000404000000c43cdcc4c3f000000000000000000531b` |
| `eq-custom 0 0 0` | `55600141f035000103000000800100000000000075443333333f020000000000a0d7450000803f000000000000000c43cdcc4c3f000000000000000000675c` |
| `bass 1` | `55600151f00200010102f6fc` |
| `bass 2` | `55600151f0020001010476fe` |
| `bass 3` | `55600151f00200010106f73f` |
| `bass 4` | `55600151f0020001010876fb` |
| `bass 5` | `55600151f0020001010af73a` |
| `bass on, at level 5` | `55600151f0020001010af73a` |
| `bass off, at level 5` | `55600151f0020001000af6aa` |
| `low-latency on` | `55600140f00200010100763c` |
| `low-latency off` | `55600140f0020001020076cc` |
| `in-ear on` | `55600104f003000101010179a4` |
| `in-ear off` | `55600104f0030001010100b864` |

## 14. Nothing Headphone (1) (B170)

Page `MainControl_elekid`. Fast Pair `C19ECD`, `2D6FDA`. Headphone,
battery id 6 only. Two configs: from `*.*.1.75` the config adds `audiodo`,
which adds the `0xC05A` read and the personal sound toggle.

- ANC: all six modes. Codec AAC or LDAC.
- EQ: earbud EQ with Advanced; advanced EQ status 1 shows Advanced. Custom
  EQ high shelf 3500 Hz.
- Spatial audio: off, head tracked, fixed (native `0xF052`).
- Bass enhance, five levels. In-ear (over-ear) detection and low latency.
- UI only: bass enhance and Advanced are blocked while spatial audio is on,
  and spatial audio while either is on (`mutuallyExclusive`).

Connect sequence, actual op ids, firmware `1.0.1.75`:

| Op | Frame |
|---|---|
| 01 battery, 0 ms | `55600107c0000001acdf` |
| 02 EQ, +100 | `5560011fc0000002ccdc` |
| 03 firmware, +300 | `55600142c0000003e0d1` |
| 04 UTC time, +400 | `5560010af00400046abc51002342` |
| 05 in-ear, +500 | `5560010ec0000005711d` |
| 06 latency, +600 | `55600141c000000664d2` |
| 07 gestures, +800 | `55600118c0000007b91f` |
| 08 ANC, +900 | `5560011ec0000008711b` |
| 09 advanced EQ status, +1000 | `5560014cc00000090917` |
| 0A bass enhance, +1100 | `5560014ec000000a30d6` |
| 0B dual enabled | `55600127c000000bad1f` |
| 0C dual list from 0 | `55600128c001000c00dd8e` |
| 0D codec | `55600129c000000d44dc` |
| 0E spatial | `5560014fc000000e0cd5` |
| 0F Audiodo personal sound | `5560015ac000000fc0d6` |

Connect sequence, actual op ids, firmware `1.0.1.74`:

| Op | Frame |
|---|---|
| 01 battery, 0 ms | `55600107c0000001acdf` |
| 02 EQ, +100 | `5560011fc0000002ccdc` |
| 03 firmware, +300 | `55600142c0000003e0d1` |
| 04 UTC time, +400 | `5560010af00400046abc51002342` |
| 05 in-ear, +500 | `5560010ec0000005711d` |
| 06 latency, +600 | `55600141c000000664d2` |
| 07 gestures, +800 | `55600118c0000007b91f` |
| 08 ANC, +900 | `5560011ec0000008711b` |
| 09 advanced EQ status, +1000 | `5560014cc00000090917` |
| 0A bass enhance, +1100 | `5560014ec000000a30d6` |
| 0B dual enabled | `55600127c000000bad1f` |
| 0C dual list from 0 | `55600128c001000c00dd8e` |
| 0D codec | `55600129c000000d44dc` |
| 0E spatial | `5560014fc000000e0cd5` |

Writes, op 01:

| Setting | Frames, in order |
|---|---|
| `anc off` | `5560010ff0030001010500fb17` |
| `anc transparency` | `5560010ff0030001010700fa77` |
| `anc high` | `5560010ff0030001010100f9d7` |
| `anc mid` | `5560010ff0030001010200f927` |
| `anc low` | `5560010ff0030001010300f8b7` |
| `anc adaptive` | `5560010ff0030001010400fa87` |
| `eq balanced` | `5560014ff0020001000088ac` `55600110f0020001000027a9` |
| `eq voice` | `5560014ff0020001000088ac` `55600110f002000101002639` |
| `eq treble` | `5560014ff0020001000088ac` `55600110f0020001020026c9` |
| `eq bass` | `5560014ff0020001000088ac` `55600110f002000103002759` |
| `eq custom` | `5560014ff0020001000088ac` `55600110f0020001050024f9` `55600144c0000001e910` `55600144c0000001e910` |
| `eq advanced` | `5560014ff00200010100893c` `5560014dc0010001ff90ab` |
| `eq-custom 3 0 -3` | `55600141f035000103000040c00100000000000075443333333f02000040c000c05a450000803f000000404000000c43cdcc4c3f0000000000000000003252` |
| `eq-custom 0 0 0` | `55600141f035000103000000800100000000000075443333333f020000000000c05a450000803f000000000000000c43cdcc4c3f0000000000000000000615` |
| `spatial off` | `55600152f0020001000045ad` |
| `spatial head-tracked` | `55600152f0020001010185fd` |
| `spatial fixed` | `55600152f00200010100443d` |
| `bass 1` | `55600151f00200010102f6fc` |
| `bass 2` | `55600151f0020001010476fe` |
| `bass 3` | `55600151f00200010106f73f` |
| `bass 4` | `55600151f0020001010876fb` |
| `bass 5` | `55600151f0020001010af73a` |
| `bass on, at level 5` | `55600151f0020001010af73a` |
| `bass off, at level 5` | `55600151f0020001000af6aa` |
| `low-latency on` | `55600140f00200010100763c` |
| `low-latency off` | `55600140f0020001020076cc` |
| `in-ear on` | `55600104f003000101010179a4` |
| `in-ear off` | `55600104f0030001010100b864` |
| `personal-sound on` | `5560015cf001000101526e` |
| `personal-sound off` | `5560015cf00100010093ae` |

## 15. Nothing Ear (B171)

Page `MainControl_twos`. Fast Pair `FEB1C7`, `A20444`. Earbuds with a
case.

- ANC: all six modes. Codec AAC, LHDC or LDAC.
- EQ: earbud EQ with Advanced; advanced EQ status 1 shows Advanced. Custom
  EQ high shelf 3400 Hz.
- Personal sound through Mimi; ear-web blocks turning it on while LDAC is
  the codec (UI only).
- Bass enhance, five levels. In-ear detection and low latency.

Connect sequence, actual op ids:

| Op | Frame |
|---|---|
| 01 battery, 0 ms | `55600107c0000001acdf` |
| 02 EQ, +100 | `5560011fc0000002ccdc` |
| 03 firmware, +300 | `55600142c0000003e0d1` |
| 04 UTC time, +400 | `5560010af00400046abc51002342` |
| 05 in-ear, +500 | `5560010ec0000005711d` |
| 06 latency, +600 | `55600141c000000664d2` |
| 07 gestures, +800 | `55600118c0000007b91f` |
| 08 ANC, +900 | `5560011ec0000008711b` |
| 09 advanced EQ status, +1000 | `5560014cc00000090917` |
| 0A bass enhance, +1100 | `5560014ec000000a30d6` |
| 0B dual enabled | `55600127c000000bad1f` |
| 0C dual list from 0 | `55600128c001000c00dd8e` |
| 0D codec | `55600129c000000d44dc` |
| 0E Mimi personal sound | `55600122c000000ea11c` |

Writes, op 01:

| Setting | Frames, in order |
|---|---|
| `anc off` | `5560010ff0030001010500fb17` |
| `anc transparency` | `5560010ff0030001010700fa77` |
| `anc high` | `5560010ff0030001010100f9d7` |
| `anc mid` | `5560010ff0030001010200f927` |
| `anc low` | `5560010ff0030001010300f8b7` |
| `anc adaptive` | `5560010ff0030001010400fa87` |
| `eq balanced` | `5560014ff0020001000088ac` `55600110f0020001000027a9` |
| `eq voice` | `5560014ff0020001000088ac` `55600110f002000101002639` |
| `eq treble` | `5560014ff0020001000088ac` `55600110f0020001020026c9` |
| `eq bass` | `5560014ff0020001000088ac` `55600110f002000103002759` |
| `eq custom` | `5560014ff0020001000088ac` `55600110f0020001050024f9` `55600144c0000001e910` `55600144c0000001e910` |
| `eq advanced` | `5560014ff00200010100893c` `5560014dc0010001ff90ab` |
| `eq-custom 3 0 -3` | `55600141f035000103000040c00100000000000075443333333f02000040c0008054450000803f000000404000000c43cdcc4c3f0000000000000000002aa2` |
| `eq-custom 0 0 0` | `55600141f035000103000000800100000000000075443333333f0200000000008054450000803f000000000000000c43cdcc4c3f0000000000000000001ee5` |
| `bass 1` | `55600151f00200010102f6fc` |
| `bass 2` | `55600151f0020001010476fe` |
| `bass 3` | `55600151f00200010106f73f` |
| `bass 4` | `55600151f0020001010876fb` |
| `bass 5` | `55600151f0020001010af73a` |
| `bass on, at level 5` | `55600151f0020001010af73a` |
| `bass off, at level 5` | `55600151f0020001000af6aa` |
| `low-latency on` | `55600140f00200010100763c` |
| `low-latency off` | `55600140f0020001020076cc` |
| `in-ear on` | `55600104f003000101010179a4` |
| `in-ear off` | `55600104f0030001010100b864` |
| `personal-sound on` | `55600115f0010001015c37` |
| `personal-sound off` | `55600115f0010001009df7` |

## 16. CMF Buds Pro 2 (B172)

Page `MainControl_espeon`. Fast Pair `F29566`, `CA36A6`, `A7B220`,
`2B353E`, and `2F45F5`, `E1BE45`, `1253C0`, `0F1A4F` with altBase B187.
Earbuds with a case and a smart knob (its gestures are out of scope).

- ANC: all six modes. Codec AAC or LDAC.
- EQ: listening-mode commands with a Dirac Opteo button (`00`). ear-web
  blocks Dirac while LDAC is active (`diracOpteoSupport` 1, UI only). Custom
  EQ high shelf 6900 Hz.
- Spatial audio: off and fixed. Bass enhance, five levels. In-ear detection
  and low latency.
- Dual connection writes reboot this model.

Connect sequence, actual op ids:

| Op | Frame |
|---|---|
| 01 battery, 0 ms | `55600107c0000001acdf` |
| 02 listening mode, +200 | `55600150c00000029912` |
| 03 firmware, +300 | `55600142c0000003e0d1` |
| 04 UTC time, +400 | `5560010af00400046abc51002342` |
| 05 in-ear, +500 | `5560010ec0000005711d` |
| 06 latency, +600 | `55600141c000000664d2` |
| 07 gestures, +800 | `55600118c0000007b91f` |
| 08 ANC, +900 | `5560011ec0000008711b` |
| 09 advanced EQ status, +1000 | `5560014cc00000090917` |
| 0A bass enhance, +1100 | `5560014ec000000a30d6` |
| 0B dual enabled | `55600127c000000bad1f` |
| 0C dual list from 0 | `55600128c001000c00dd8e` |
| 0D codec | `55600129c000000d44dc` |
| 0E spatial | `5560014fc000000e0cd5` |

Writes, op 01:

| Setting | Frames, in order |
|---|---|
| `anc off` | `5560010ff0030001010500fb17` |
| `anc transparency` | `5560010ff0030001010700fa77` |
| `anc high` | `5560010ff0030001010100f9d7` |
| `anc mid` | `5560010ff0030001010200f927` |
| `anc low` | `5560010ff0030001010300f8b7` |
| `anc adaptive` | `5560010ff0030001010400fa87` |
| `eq dirac` | `5560011df00200010000fb69` |
| `eq rock` | `5560011df00200010100faf9` |
| `eq electronic` | `5560011df00200010200fa09` |
| `eq pop` | `5560011df00200010300fb99` |
| `eq vocals` | `5560011df00200010400f9a9` |
| `eq classical` | `5560011df00200010500f839` |
| `eq custom` | `5560011df00200010600f8c9` `55600144c0000001e910` `55600144c0000001e910` |
| `eq-custom 3 0 -3` | `55600141f035000103000040c00100000000000075443333333f02000040c000a0d7450000803f000000404000000c43cdcc4c3f000000000000000000531b` |
| `eq-custom 0 0 0` | `55600141f035000103000000800100000000000075443333333f020000000000a0d7450000803f000000000000000c43cdcc4c3f000000000000000000675c` |
| `spatial off` | `55600152f0020001000045ad` |
| `spatial fixed` | `55600152f00200010100443d` |
| `bass 1` | `55600151f00200010102f6fc` |
| `bass 2` | `55600151f0020001010476fe` |
| `bass 3` | `55600151f00200010106f73f` |
| `bass 4` | `55600151f0020001010876fb` |
| `bass 5` | `55600151f0020001010af73a` |
| `bass on, at level 5` | `55600151f0020001010af73a` |
| `bass off, at level 5` | `55600151f0020001000af6aa` |
| `low-latency on` | `55600140f00200010100763c` |
| `low-latency off` | `55600140f0020001020076cc` |
| `in-ear on` | `55600104f003000101010179a4` |
| `in-ear off` | `55600104f0030001010100b864` |

## 17. Nothing Ear (3) (B173)

Page `MainControl_feraligator`. Fast Pair `7D46E5`, `C1EBFD` (altBase
B201). Earbuds with a case and a case button (its gestures are out of scope).

- ANC: all six modes. Codec AAC or LDAC.
- EQ: earbud EQ with Advanced; the advanced EQ status reply is not mapped.
  Custom EQ high shelf 3400 Hz.
- Spatial audio: off and fixed. Bass enhance, five levels.
- Personal sound (Audiodo), super mic, mic mode (natural, voice focus; the
  page only offers it while super mic is on), call transparency and
  walkie-talkie, each read after the firmware reply.
- In-ear detection and low latency.

Connect sequence, actual op ids:

| Op | Frame |
|---|---|
| 01 battery, 0 ms | `55600107c0000001acdf` |
| 02 EQ, +100 | `5560011fc0000002ccdc` |
| 03 firmware, +300 | `55600142c0000003e0d1` |
| 04 UTC time, +400 | `5560010af00400046abc51002342` |
| 05 in-ear, +500 | `5560010ec0000005711d` |
| 06 latency, +600 | `55600141c000000664d2` |
| 07 gestures, +800 | `55600118c0000007b91f` |
| 08 ANC, +900 | `5560011ec0000008711b` |
| 09 advanced EQ status, +1000 | `5560014cc00000090917` |
| 0A bass enhance, +1100 | `5560014ec000000a30d6` |
| 0B dual enabled | `55600127c000000bad1f` |
| 0C dual list from 0 | `55600128c001000c00dd8e` |
| 0D codec | `55600129c000000d44dc` |
| 0E spatial | `5560014fc000000e0cd5` |
| 0F Audiodo personal sound | `5560015ac000000fc0d6` |
| 10 super mic | `5560015ec000001070de` |
| 11 mic mode | `55600164c0000011691b` |
| 12 call transparency | `5560015fc0000012ccdf` |
| 13 walkie-talkie | `55600160c0000013191a` |

Writes, op 01:

| Setting | Frames, in order |
|---|---|
| `anc off` | `5560010ff0030001010500fb17` |
| `anc transparency` | `5560010ff0030001010700fa77` |
| `anc high` | `5560010ff0030001010100f9d7` |
| `anc mid` | `5560010ff0030001010200f927` |
| `anc low` | `5560010ff0030001010300f8b7` |
| `anc adaptive` | `5560010ff0030001010400fa87` |
| `eq balanced` | `5560014ff0020001000088ac` `55600110f0020001000027a9` |
| `eq voice` | `5560014ff0020001000088ac` `55600110f002000101002639` |
| `eq treble` | `5560014ff0020001000088ac` `55600110f0020001020026c9` |
| `eq bass` | `5560014ff0020001000088ac` `55600110f002000103002759` |
| `eq custom` | `5560014ff0020001000088ac` `55600110f0020001050024f9` `55600144c0000001e910` `55600144c0000001e910` |
| `eq advanced` | `5560014ff00200010100893c` `5560014dc0010001ff90ab` |
| `eq-custom 3 0 -3` | `55600141f035000103000040c00100000000000075443333333f02000040c0008054450000803f000000404000000c43cdcc4c3f0000000000000000002aa2` |
| `eq-custom 0 0 0` | `55600141f035000103000000800100000000000075443333333f0200000000008054450000803f000000000000000c43cdcc4c3f0000000000000000001ee5` |
| `spatial off` | `55600152f0020001000045ad` |
| `spatial fixed` | `55600152f00200010100443d` |
| `bass 1` | `55600151f00200010102f6fc` |
| `bass 2` | `55600151f0020001010476fe` |
| `bass 3` | `55600151f00200010106f73f` |
| `bass 4` | `55600151f0020001010876fb` |
| `bass 5` | `55600151f0020001010af73a` |
| `bass on, at level 5` | `55600151f0020001010af73a` |
| `bass off, at level 5` | `55600151f0020001000af6aa` |
| `low-latency on` | `55600140f00200010100763c` |
| `low-latency off` | `55600140f0020001020076cc` |
| `in-ear on` | `55600104f003000101010179a4` |
| `in-ear off` | `55600104f0030001010100b864` |
| `personal-sound on` | `5560015cf001000101526e` |
| `personal-sound off` | `5560015cf00100010093ae` |
| `super-mic on` | `5560015ff001000101525d` |
| `super-mic off` | `5560015ff001000100939d` |
| `call-transparency on` | `55600160f0010001015752` |
| `call-transparency off` | `55600160f0010001009692` |
| `walkie-talkie on` | `55600161f0010001015683` |
| `walkie-talkie off` | `55600161f0010001009743` |
| `mic-mode natural` | `55600165f00100010096c7` |
| `mic-mode voice-focus` | `55600165f0010001015707` |

## 18. Nothing Ear (open) (B174)

Page `MainControl_flaaffy`. Fast Pair `FC3AAF`, `CC3444`. Open earbuds
with a case. Three configs that differ in no flag ear-web reads.

- No ANC: the page has no widget, but ear-web still sends the `0xC01E` read.
- EQ: earbud EQ with Advanced; advanced EQ status 1 shows Advanced. Custom
  EQ high shelf 3400 Hz.
- Low latency only: no in-ear read, codec, bass or spatial audio.

Connect sequence, actual op ids:

| Op | Frame |
|---|---|
| 01 battery, 0 ms | `55600107c0000001acdf` |
| 02 EQ, +100 | `5560011fc0000002ccdc` |
| 03 firmware, +300 | `55600142c0000003e0d1` |
| 04 UTC time, +400 | `5560010af00400046abc51002342` |
| 05 latency, +600 | `55600141c000000524d3` |
| 06 gestures, +800 | `55600118c000000678df` |
| 07 ANC, +900 | `5560011ec0000007311f` |
| 08 advanced EQ status, +1000 | `5560014cc0000008c8d7` |
| 09 dual enabled | `55600127c00000092cde` |
| 0A dual list from 0 | `55600128c001000a00de2e` |

Writes, op 01:

| Setting | Frames, in order |
|---|---|
| `eq balanced` | `5560014ff0020001000088ac` `55600110f0020001000027a9` |
| `eq voice` | `5560014ff0020001000088ac` `55600110f002000101002639` |
| `eq treble` | `5560014ff0020001000088ac` `55600110f0020001020026c9` |
| `eq bass` | `5560014ff0020001000088ac` `55600110f002000103002759` |
| `eq custom` | `5560014ff0020001000088ac` `55600110f0020001050024f9` `55600144c0000001e910` `55600144c0000001e910` |
| `eq advanced` | `5560014ff00200010100893c` `5560014dc0010001ff90ab` |
| `eq-custom 3 0 -3` | `55600141f035000103000040c00100000000000075443333333f02000040c0008054450000803f000000404000000c43cdcc4c3f0000000000000000002aa2` |
| `eq-custom 0 0 0` | `55600141f035000103000000800100000000000075443333333f0200000000008054450000803f000000000000000c43cdcc4c3f0000000000000000001ee5` |
| `low-latency on` | `55600140f00200010100763c` |
| `low-latency off` | `55600140f0020001020076cc` |

## 19. CMF Headphone Pro (B175)

Sections 1 to 6 hold the facts. This section lists its frames in the
same form as the other models.

Connect sequence, actual op ids:

| Op | Frame |
|---|---|
| 01 battery, 0 ms | `55600107c0000001acdf` |
| 02 listening mode, +200 | `55600150c00000029912` |
| 03 firmware, +300 | `55600142c0000003e0d1` |
| 04 UTC time, +400 | `5560010af00400046abc51002342` |
| 05 latency, +600 | `55600141c000000524d3` |
| 06 gestures, +800 | `55600118c000000678df` |
| 07 ANC, +900 | `5560011ec0000007311f` |
| 08 advanced EQ status, +1000 | `5560014cc0000008c8d7` |
| 09 dual enabled | `55600127c00000092cde` |
| 0A dual list from 0 | `55600128c001000a00de2e` |
| 0B codec | `55600129c000000bc4de` |
| 0C spatial | `5560014fc000000c8d14` |
| 0D Audiodo personal sound | `5560015ac000000d4117` |

Writes, op 01:

| Setting | Frames, in order |
|---|---|
| `anc off` | `5560010ff0030001010500fb17` |
| `anc transparency` | `5560010ff0030001010700fa77` |
| `anc high` | `5560010ff0030001010100f9d7` |
| `anc mid` | `5560010ff0030001010200f927` |
| `anc low` | `5560010ff0030001010300f8b7` |
| `anc adaptive` | `5560010ff0030001010400fa87` |
| `eq rock` | `5560011df00200010100faf9` |
| `eq electronic` | `5560011df00200010200fa09` |
| `eq pop` | `5560011df00200010300fb99` |
| `eq vocals` | `5560011df00200010400f9a9` |
| `eq classical` | `5560011df00200010500f839` |
| `eq custom` | `5560011df00200010600f8c9` `55600144c0000001e910` `55600144c0000001e910` |
| `eq-custom 3 0 -3` | `55600141f035000103000040c00100000000000075443333333f02000040c000c05a450000803f000000404000000c43cdcc4c3f0000000000000000003252` |
| `eq-custom 0 0 0` | `55600141f035000103000000800100000000000075443333333f020000000000c05a450000803f000000000000000c43cdcc4c3f0000000000000000000615` |
| `spatial off` | `55600152f0020001000045ad` |
| `spatial concert` | `55600152f0020001020044cd` |
| `spatial theatre` | `55600152f00200010300455d` |
| `low-latency on` | `55600140f00200010100763c` |
| `low-latency off` | `55600140f0020001020076cc` |
| `personal-sound on` | `5560015cf001000101526e` |
| `personal-sound off` | `5560015cf00100010093ae` |

## 20. CMF Buds 2 (B179)

Page `MainControl_girafarig` (it loads `espeon.js`). Fast Pair `19EF24`,
`FF2AB0`, `D9AB5D`. Earbuds with a case.

- ANC: all six modes. No codec.
- EQ: listening-mode commands with a Dirac Opteo button (`00`). Custom EQ
  high shelf 6900 Hz.
- Spatial audio: off and fixed. Bass enhance, five levels. In-ear detection
  and low latency.

Connect sequence, actual op ids:

| Op | Frame |
|---|---|
| 01 battery, 0 ms | `55600107c0000001acdf` |
| 02 listening mode, +200 | `55600150c00000029912` |
| 03 firmware, +300 | `55600142c0000003e0d1` |
| 04 UTC time, +400 | `5560010af00400046abc51002342` |
| 05 in-ear, +500 | `5560010ec0000005711d` |
| 06 latency, +600 | `55600141c000000664d2` |
| 07 gestures, +800 | `55600118c0000007b91f` |
| 08 ANC, +900 | `5560011ec0000008711b` |
| 09 advanced EQ status, +1000 | `5560014cc00000090917` |
| 0A bass enhance, +1100 | `5560014ec000000a30d6` |
| 0B dual enabled | `55600127c000000bad1f` |
| 0C dual list from 0 | `55600128c001000c00dd8e` |
| 0D spatial | `5560014fc000000d4cd4` |

Writes, op 01:

| Setting | Frames, in order |
|---|---|
| `anc off` | `5560010ff0030001010500fb17` |
| `anc transparency` | `5560010ff0030001010700fa77` |
| `anc high` | `5560010ff0030001010100f9d7` |
| `anc mid` | `5560010ff0030001010200f927` |
| `anc low` | `5560010ff0030001010300f8b7` |
| `anc adaptive` | `5560010ff0030001010400fa87` |
| `eq dirac` | `5560011df00200010000fb69` |
| `eq rock` | `5560011df00200010100faf9` |
| `eq electronic` | `5560011df00200010200fa09` |
| `eq pop` | `5560011df00200010300fb99` |
| `eq vocals` | `5560011df00200010400f9a9` |
| `eq classical` | `5560011df00200010500f839` |
| `eq custom` | `5560011df00200010600f8c9` `55600144c0000001e910` `55600144c0000001e910` |
| `eq-custom 3 0 -3` | `55600141f035000103000040c00100000000000075443333333f02000040c000a0d7450000803f000000404000000c43cdcc4c3f000000000000000000531b` |
| `eq-custom 0 0 0` | `55600141f035000103000000800100000000000075443333333f020000000000a0d7450000803f000000000000000c43cdcc4c3f000000000000000000675c` |
| `spatial off` | `55600152f0020001000045ad` |
| `spatial fixed` | `55600152f00200010100443d` |
| `bass 1` | `55600151f00200010102f6fc` |
| `bass 2` | `55600151f0020001010476fe` |
| `bass 3` | `55600151f00200010106f73f` |
| `bass 4` | `55600151f0020001010876fb` |
| `bass 5` | `55600151f0020001010af73a` |
| `bass on, at level 5` | `55600151f0020001010af73a` |
| `bass off, at level 5` | `55600151f0020001000af6aa` |
| `low-latency on` | `55600140f00200010100763c` |
| `low-latency off` | `55600140f0020001020076cc` |
| `in-ear on` | `55600104f003000101010179a4` |
| `in-ear off` | `55600104f0030001010100b864` |

## 21. Nothing ear (1) (B181)

Page `MainControl_one`. Fast Pair `31D53D`, `624011`. Earbuds with a
case. Three configs with the same frame-relevant flags.

- No UTC write: B181 is the one model ear-web skips it for.
- ANC override: off, transparency, high (`01`), low (`03`). Reads of `02` and
  `04` show noise cancellation without a strength.
- EQ: four presets (balanced, voice, treble, bass), `0xF010` only, no custom
  EQ and no advanced switch.
- In-ear detection and low latency. Case LED colours (`0xC017` / `0xF00D`)
  are not offered here. Ring is one byte, `01` on and `00` off.

Connect sequence, actual op ids:

| Op | Frame |
|---|---|
| 01 battery, 0 ms | `55600107c0000001acdf` |
| 02 EQ, +100 | `5560011fc0000002ccdc` |
| 03 firmware, +300 | `55600142c0000003e0d1` |
| 04 in-ear, +500 | `5560010ec0000004b0dd` |
| 05 latency, +600 | `55600141c000000524d3` |
| 06 gestures, +800 | `55600118c000000678df` |
| 07 ANC, +900 | `5560011ec0000007311f` |
| 08 advanced EQ status, +1000 | `5560014cc0000008c8d7` |

Writes, op 01:

| Setting | Frames, in order |
|---|---|
| `anc off` | `5560010ff0030001010500fb17` |
| `anc transparency` | `5560010ff0030001010700fa77` |
| `anc high` | `5560010ff0030001010100f9d7` |
| `anc low` | `5560010ff0030001010300f8b7` |
| `eq balanced` | `55600110f0020001000027a9` |
| `eq voice` | `55600110f002000101002639` |
| `eq treble` | `55600110f0020001020026c9` |
| `eq bass` | `55600110f002000103002759` |
| `low-latency on` | `55600140f00200010100763c` |
| `low-latency off` | `55600140f0020001020076cc` |
| `in-ear on` | `55600104f003000101010179a4` |
| `in-ear off` | `55600104f0030001010100b864` |

## 22. CMF Buds 2 Plus (B184)

Page `MainControl_gligar`. Fast Pair `5C587F`, `4AEB6E`. Earbuds with a
case.

- ANC: all six modes. Codec AAC or LDAC.
- EQ: listening-mode commands; the Dirac Opteo button is hidden but a read of
  0 still shows it. Custom EQ high shelf 6900 Hz.
- Spatial audio: off and fixed. Bass enhance, five levels.
- Personal sound (Audiodo). In-ear detection and low latency.

Connect sequence, actual op ids:

| Op | Frame |
|---|---|
| 01 battery, 0 ms | `55600107c0000001acdf` |
| 02 listening mode, +200 | `55600150c00000029912` |
| 03 firmware, +300 | `55600142c0000003e0d1` |
| 04 UTC time, +400 | `5560010af00400046abc51002342` |
| 05 in-ear, +500 | `5560010ec0000005711d` |
| 06 latency, +600 | `55600141c000000664d2` |
| 07 gestures, +800 | `55600118c0000007b91f` |
| 08 ANC, +900 | `5560011ec0000008711b` |
| 09 advanced EQ status, +1000 | `5560014cc00000090917` |
| 0A bass enhance, +1100 | `5560014ec000000a30d6` |
| 0B dual enabled | `55600127c000000bad1f` |
| 0C dual list from 0 | `55600128c001000c00dd8e` |
| 0D codec | `55600129c000000d44dc` |
| 0E spatial | `5560014fc000000e0cd5` |
| 0F Audiodo personal sound | `5560015ac000000fc0d6` |

Writes, op 01:

| Setting | Frames, in order |
|---|---|
| `anc off` | `5560010ff0030001010500fb17` |
| `anc transparency` | `5560010ff0030001010700fa77` |
| `anc high` | `5560010ff0030001010100f9d7` |
| `anc mid` | `5560010ff0030001010200f927` |
| `anc low` | `5560010ff0030001010300f8b7` |
| `anc adaptive` | `5560010ff0030001010400fa87` |
| `eq rock` | `5560011df00200010100faf9` |
| `eq electronic` | `5560011df00200010200fa09` |
| `eq pop` | `5560011df00200010300fb99` |
| `eq vocals` | `5560011df00200010400f9a9` |
| `eq classical` | `5560011df00200010500f839` |
| `eq custom` | `5560011df00200010600f8c9` `55600144c0000001e910` `55600144c0000001e910` |
| `eq-custom 3 0 -3` | `55600141f035000103000040c00100000000000075443333333f02000040c000a0d7450000803f000000404000000c43cdcc4c3f000000000000000000531b` |
| `eq-custom 0 0 0` | `55600141f035000103000000800100000000000075443333333f020000000000a0d7450000803f000000000000000c43cdcc4c3f000000000000000000675c` |
| `spatial off` | `55600152f0020001000045ad` |
| `spatial fixed` | `55600152f00200010100443d` |
| `bass 1` | `55600151f00200010102f6fc` |
| `bass 2` | `55600151f0020001010476fe` |
| `bass 3` | `55600151f00200010106f73f` |
| `bass 4` | `55600151f0020001010876fb` |
| `bass 5` | `55600151f0020001010af73a` |
| `bass on, at level 5` | `55600151f0020001010af73a` |
| `bass off, at level 5` | `55600151f0020001000af6aa` |
| `low-latency on` | `55600140f00200010100763c` |
| `low-latency off` | `55600140f0020001020076cc` |
| `in-ear on` | `55600104f003000101010179a4` |
| `in-ear off` | `55600104f0030001010100b864` |
| `personal-sound on` | `5560015cf001000101526e` |
| `personal-sound off` | `5560015cf00100010093ae` |

## 23. CMF Buds 2a (B185)

Page `MainControl_hoothoot` (it loads `donphan.js`). Fast Pair `70F8E3`,
`ED5412`, `509CAE`. Earbuds with a case.

- ANC: the basic widget.
- EQ: listening-mode commands; the Dirac button sends **`07`**, and a read of
  0 shows it too. Custom EQ high shelf 6900 Hz.
- Bass enhance is read but never written: ear-web's write list leaves B185
  out, so its bass panel only changes the page.
- Low latency. No in-ear read.

Connect sequence, actual op ids:

| Op | Frame |
|---|---|
| 01 battery, 0 ms | `55600107c0000001acdf` |
| 02 listening mode, +200 | `55600150c00000029912` |
| 03 firmware, +300 | `55600142c0000003e0d1` |
| 04 UTC time, +400 | `5560010af00400046abc51002342` |
| 05 latency, +600 | `55600141c000000524d3` |
| 06 gestures, +800 | `55600118c000000678df` |
| 07 ANC, +900 | `5560011ec0000007311f` |
| 08 advanced EQ status, +1000 | `5560014cc0000008c8d7` |
| 09 bass enhance, +1100 | `5560014ec000000970d7` |
| 0A dual enabled | `55600127c000000a6cdf` |
| 0B dual list from 0 | `55600128c001000b00dfbe` |

Writes, op 01:

| Setting | Frames, in order |
|---|---|
| `anc off` | `5560010ff0030001010500fb17` |
| `anc transparency` | `5560010ff0030001010700fa77` |
| `anc nc` | `5560010ff0030001010100f9d7` |
| `eq dirac` | `5560011df00200010700f959` |
| `eq rock` | `5560011df00200010100faf9` |
| `eq electronic` | `5560011df00200010200fa09` |
| `eq pop` | `5560011df00200010300fb99` |
| `eq vocals` | `5560011df00200010400f9a9` |
| `eq classical` | `5560011df00200010500f839` |
| `eq custom` | `5560011df00200010600f8c9` `55600144c0000001e910` `55600144c0000001e910` |
| `eq-custom 3 0 -3` | `55600141f035000103000040c00100000000000075443333333f02000040c000a0d7450000803f000000404000000c43cdcc4c3f000000000000000000531b` |
| `eq-custom 0 0 0` | `55600141f035000103000000800100000000000075443333333f020000000000a0d7450000803f000000000000000c43cdcc4c3f000000000000000000675c` |
| `low-latency on` | `55600140f00200010100763c` |
| `low-latency off` | `55600140f0020001020076cc` |

## 24. Nothing Headphone (a) (B186)

Page `MainControl_hoppip`. Fast Pair `BFD53B`, `DE8953`, `98D02B`, and
`97EF75`, `6F6C71`, `810478`, `79B3A9`, `A292C6` with altBase B198. Headphone,
battery id 6 only.

- ANC: all six modes. Codec AAC or LDAC.
- EQ: earbud EQ with Advanced; advanced EQ status 1 shows Advanced. Custom
  EQ high shelf 3500 Hz.
- Spatial audio: off, concert, theatre. Bass enhance, three stages.
- Low latency. No in-ear read. Ring uses the headphone form.

Connect sequence, actual op ids:

| Op | Frame |
|---|---|
| 01 battery, 0 ms | `55600107c0000001acdf` |
| 02 EQ, +100 | `5560011fc0000002ccdc` |
| 03 firmware, +300 | `55600142c0000003e0d1` |
| 04 UTC time, +400 | `5560010af00400046abc51002342` |
| 05 latency, +600 | `55600141c000000524d3` |
| 06 gestures, +800 | `55600118c000000678df` |
| 07 ANC, +900 | `5560011ec0000007311f` |
| 08 advanced EQ status, +1000 | `5560014cc0000008c8d7` |
| 09 bass enhance, +1100 | `5560014ec000000970d7` |
| 0A dual enabled | `55600127c000000a6cdf` |
| 0B dual list from 0 | `55600128c001000b00dfbe` |
| 0C codec | `55600129c000000c851c` |
| 0D spatial | `5560014fc000000d4cd4` |

Writes, op 01:

| Setting | Frames, in order |
|---|---|
| `anc off` | `5560010ff0030001010500fb17` |
| `anc transparency` | `5560010ff0030001010700fa77` |
| `anc high` | `5560010ff0030001010100f9d7` |
| `anc mid` | `5560010ff0030001010200f927` |
| `anc low` | `5560010ff0030001010300f8b7` |
| `anc adaptive` | `5560010ff0030001010400fa87` |
| `eq balanced` | `5560014ff0020001000088ac` `55600110f0020001000027a9` |
| `eq voice` | `5560014ff0020001000088ac` `55600110f002000101002639` |
| `eq treble` | `5560014ff0020001000088ac` `55600110f0020001020026c9` |
| `eq bass` | `5560014ff0020001000088ac` `55600110f002000103002759` |
| `eq custom` | `5560014ff0020001000088ac` `55600110f0020001050024f9` `55600144c0000001e910` `55600144c0000001e910` |
| `eq advanced` | `5560014ff00200010100893c` `5560014dc0010001ff90ab` |
| `eq-custom 3 0 -3` | `55600141f035000103000040c00100000000000075443333333f02000040c000c05a450000803f000000404000000c43cdcc4c3f0000000000000000003252` |
| `eq-custom 0 0 0` | `55600141f035000103000000800100000000000075443333333f020000000000c05a450000803f000000000000000c43cdcc4c3f0000000000000000000615` |
| `spatial off` | `55600152f0020001000045ad` |
| `spatial concert` | `55600152f0020001020044cd` |
| `spatial theatre` | `55600152f00200010300455d` |
| `bass 0` | `55600151f0020001000076ad` |
| `bass 1` | `55600151f00200010105b73e` |
| `bass 2` | `55600151f0020001010af73a` |
| `bass on, at level 2` | `55600151f0020001010af73a` |
| `bass off, at level 2` | `55600151f0020001000af6aa` |
| `low-latency on` | `55600140f00200010100763c` |
| `low-latency off` | `55600140f0020001020076cc` |

## 25. CMF Clip Pro (B189)

Page `MainControl_igglybuff`. Fast Pair `8F28ED`, `E6673A`, `DCD2CB`,
`DA1280`. Open earbuds with a case.

- No ANC page, but the `0xC01E` read still goes out.
- EQ: listening-mode commands, B175's values without Dirac. ear-web refuses
  EQ changes while long battery mode is on or anti-leakage is on audio and
  calls (UI only). Custom EQ high shelf 3400 Hz.
- Bass enhance, three stages. A read can report enabled 0 with a nonzero
  level; the device keeps its last level.
- Spatial audio: off and fixed, decoded from the mode byte alone (mode 1 is
  fixed, anything else off), because the head byte carries noise here.
- Anti-leakage: off, calls only, audio and calls. Long battery mode is read
  only: switching it reboots the device.
- Low latency. No in-ear detection or personal sound.

Connect sequence, actual op ids:

| Op | Frame |
|---|---|
| 01 battery, 0 ms | `55600107c0000001acdf` |
| 02 listening mode, +200 | `55600150c00000029912` |
| 03 firmware, +300 | `55600142c0000003e0d1` |
| 04 UTC time, +400 | `5560010af00400046abc51002342` |
| 05 latency, +600 | `55600141c000000524d3` |
| 06 gestures, +800 | `55600118c000000678df` |
| 07 ANC, +900 | `5560011ec0000007311f` |
| 08 advanced EQ status, +1000 | `5560014cc0000008c8d7` |
| 09 bass enhance, +1100 | `5560014ec000000970d7` |
| 0A dual enabled | `55600127c000000a6cdf` |
| 0B dual list from 0 | `55600128c001000b00dfbe` |
| 0C codec | `55600129c000000c851c` |
| 0D spatial | `5560014fc000000d4cd4` |
| 0E long battery mode | `55600167c000000e6cd3` |
| 0F anti-leakage | `55600171c000000fe4d0` |

Writes, op 01:

| Setting | Frames, in order |
|---|---|
| `eq rock` | `5560011df00200010100faf9` |
| `eq electronic` | `5560011df00200010200fa09` |
| `eq pop` | `5560011df00200010300fb99` |
| `eq vocals` | `5560011df00200010400f9a9` |
| `eq classical` | `5560011df00200010500f839` |
| `eq custom` | `5560011df00200010600f8c9` `55600144c0000001e910` `55600144c0000001e910` |
| `eq-custom 3 0 -3` | `55600141f035000103000040c00100000000000075443333333f02000040c0008054450000803f000000404000000c43cdcc4c3f0000000000000000002aa2` |
| `eq-custom 0 0 0` | `55600141f035000103000000800100000000000075443333333f0200000000008054450000803f000000000000000c43cdcc4c3f0000000000000000001ee5` |
| `spatial off` | `55600152f0020001000045ad` |
| `spatial fixed` | `55600152f00200010100443d` |
| `bass 0` | `55600151f0020001000076ad` |
| `bass 1` | `55600151f00200010105b73e` |
| `bass 2` | `55600151f0020001010af73a` |
| `bass on, at level 2` | `55600151f0020001010af73a` |
| `bass off, at level 2` | `55600151f0020001000af6aa` |
| `low-latency on` | `55600140f00200010100763c` |
| `low-latency off` | `55600140f0020001020076cc` |
| `leakage off` | `55600175f0010001009457` |
| `leakage calls` | `55600175f0010001015597` |
| `leakage all` | `55600175f0010001021596` |

## 26. Nothing Ear (3a) (B190)

Page `MainControl_jumpluff`. Fast Pair `7B2328`, `E9C3BE`, `148887`,
`DB45D3`. Earbuds with a case. Three configs by firmware with the same
values.

- ANC: all six modes. Codec AAC or LDAC.
- EQ: earbud EQ with Advanced; the advanced EQ status reply is not mapped.
  Custom EQ high shelf 3400 Hz.
- Spatial audio: off and fixed. In-ear detection and low latency.
- The page has an ear tip test button, but `launchEarFitTest` sends nothing
  for B190.

Connect sequence, actual op ids:

| Op | Frame |
|---|---|
| 01 battery, 0 ms | `55600107c0000001acdf` |
| 02 EQ, +100 | `5560011fc0000002ccdc` |
| 03 firmware, +300 | `55600142c0000003e0d1` |
| 04 UTC time, +400 | `5560010af00400046abc51002342` |
| 05 in-ear, +500 | `5560010ec0000005711d` |
| 06 latency, +600 | `55600141c000000664d2` |
| 07 gestures, +800 | `55600118c0000007b91f` |
| 08 ANC, +900 | `5560011ec0000008711b` |
| 09 advanced EQ status, +1000 | `5560014cc00000090917` |
| 0A dual enabled | `55600127c000000a6cdf` |
| 0B dual list from 0 | `55600128c001000b00dfbe` |
| 0C codec | `55600129c000000c851c` |
| 0D spatial | `5560014fc000000d4cd4` |

Writes, op 01:

| Setting | Frames, in order |
|---|---|
| `anc off` | `5560010ff0030001010500fb17` |
| `anc transparency` | `5560010ff0030001010700fa77` |
| `anc high` | `5560010ff0030001010100f9d7` |
| `anc mid` | `5560010ff0030001010200f927` |
| `anc low` | `5560010ff0030001010300f8b7` |
| `anc adaptive` | `5560010ff0030001010400fa87` |
| `eq balanced` | `5560014ff0020001000088ac` `55600110f0020001000027a9` |
| `eq voice` | `5560014ff0020001000088ac` `55600110f002000101002639` |
| `eq treble` | `5560014ff0020001000088ac` `55600110f0020001020026c9` |
| `eq bass` | `5560014ff0020001000088ac` `55600110f002000103002759` |
| `eq custom` | `5560014ff0020001000088ac` `55600110f0020001050024f9` `55600144c0000001e910` `55600144c0000001e910` |
| `eq advanced` | `5560014ff00200010100893c` `5560014dc0010001ff90ab` |
| `eq-custom 3 0 -3` | `55600141f035000103000040c00100000000000075443333333f02000040c0008054450000803f000000404000000c43cdcc4c3f0000000000000000002aa2` |
| `eq-custom 0 0 0` | `55600141f035000103000000800100000000000075443333333f0200000000008054450000803f000000000000000c43cdcc4c3f0000000000000000001ee5` |
| `spatial off` | `55600152f0020001000045ad` |
| `spatial fixed` | `55600152f00200010100443d` |
| `low-latency on` | `55600140f00200010100763c` |
| `low-latency off` | `55600140f0020001020076cc` |
| `in-ear on` | `55600104f003000101010179a4` |
| `in-ear off` | `55600104f0030001010100b864` |

## 27. Nothing Headphone (1) Pro (B192)

Page `MainControl_lanturn`. Fast Pair `CDFDEC`, `086485`. Headphone,
battery id 6 only.

- ANC: all six modes. Codec AAC or LDAC.
- EQ: earbud EQ with Advanced and More detail (`07`). Advanced also requests
  the precision driver EQ (`0xC06C`, `0xC06D FF`). Advanced EQ status 1 shows
  Advanced. Custom EQ high shelf 3500 Hz.
- Spatial audio runs through the Audiodo SDK, so ear-web skips `0xC04F` and
  offers none; it only logs the `0xE029` / `0x6029` push.
- Personal sound (Audiodo). Flat EQ is read on connect and pushed when the
  earcup slider moves; while it is on ear-web blocks transparency, spatial
  audio and the EQ page (UI only).
- In-ear (over-ear) detection and low latency. Ring uses the headphone form.
- Reply-triggered writes ear-web sends and this tool does not: a gesture
  reply holding device 6, button 10, gesture 7, action 31 makes it send
  `0xF003 01 06 0a 07 01`; a precision driver EQ mode reply of 0 makes it send
  `0xF06C 01`.

Connect sequence, actual op ids:

| Op | Frame |
|---|---|
| 01 battery, 0 ms | `55600107c0000001acdf` |
| 02 EQ, +100 | `5560011fc0000002ccdc` |
| 03 firmware, +300 | `55600142c0000003e0d1` |
| 04 UTC time, +400 | `5560010af00400046abc51002342` |
| 05 in-ear, +500 | `5560010ec0000005711d` |
| 06 latency, +600 | `55600141c000000664d2` |
| 07 gestures, +800 | `55600118c0000007b91f` |
| 08 ANC, +900 | `5560011ec0000008711b` |
| 09 advanced EQ status, +1000 | `5560014cc00000090917` |
| 0A dual enabled | `55600127c000000a6cdf` |
| 0B dual list from 0 | `55600128c001000b00dfbe` |
| 0C codec | `55600129c000000c851c` |
| 0D Audiodo personal sound | `5560015ac000000d4117` |
| 0E flat EQ | `55600168c000000e38d2` |

Writes, op 01:

| Setting | Frames, in order |
|---|---|
| `anc off` | `5560010ff0030001010500fb17` |
| `anc transparency` | `5560010ff0030001010700fa77` |
| `anc high` | `5560010ff0030001010100f9d7` |
| `anc mid` | `5560010ff0030001010200f927` |
| `anc low` | `5560010ff0030001010300f8b7` |
| `anc adaptive` | `5560010ff0030001010400fa87` |
| `eq balanced` | `5560014ff0020001000088ac` `55600110f0020001000027a9` |
| `eq voice` | `5560014ff0020001000088ac` `55600110f002000101002639` |
| `eq treble` | `5560014ff0020001000088ac` `55600110f0020001020026c9` |
| `eq bass` | `5560014ff0020001000088ac` `55600110f002000103002759` |
| `eq custom` | `5560014ff0020001000088ac` `55600110f0020001050024f9` `55600144c0000001e910` `55600144c0000001e910` |
| `eq advanced` | `5560014ff00200010100893c` `5560014dc0010001ff90ab` `5560016cc00000018916` `5560016dc0010001ff97cb` |
| `eq detail` | `5560014ff0020001000088ac` `55600110f002000107002599` |
| `eq-custom 3 0 -3` | `55600141f035000103000040c00100000000000075443333333f02000040c000c05a450000803f000000404000000c43cdcc4c3f0000000000000000003252` |
| `eq-custom 0 0 0` | `55600141f035000103000000800100000000000075443333333f020000000000c05a450000803f000000000000000c43cdcc4c3f0000000000000000000615` |
| `low-latency on` | `55600140f00200010100763c` |
| `low-latency off` | `55600140f0020001020076cc` |
| `in-ear on` | `55600104f003000101010179a4` |
| `in-ear off` | `55600104f0030001010100b864` |
| `personal-sound on` | `5560015cf001000101526e` |
| `personal-sound off` | `5560015cf00100010093ae` |

## 28. CMF Buds Pro 2 (B195)

Page `MainControl_espeon`. Fast Pair `F51D3D`, `A544CE`, `9EAC04`,
`1D07D4`, `53BFE6`, `072714`. A later hardware revision of B172 with the same
page.

- As B172, except: bass enhance has three stages, dual connection writes do
  not reboot it, and the Dirac/LDAC block does not apply
  (`diracOpteoSupport` 2).

Connect sequence, actual op ids:

| Op | Frame |
|---|---|
| 01 battery, 0 ms | `55600107c0000001acdf` |
| 02 listening mode, +200 | `55600150c00000029912` |
| 03 firmware, +300 | `55600142c0000003e0d1` |
| 04 UTC time, +400 | `5560010af00400046abc51002342` |
| 05 in-ear, +500 | `5560010ec0000005711d` |
| 06 latency, +600 | `55600141c000000664d2` |
| 07 gestures, +800 | `55600118c0000007b91f` |
| 08 ANC, +900 | `5560011ec0000008711b` |
| 09 advanced EQ status, +1000 | `5560014cc00000090917` |
| 0A bass enhance, +1100 | `5560014ec000000a30d6` |
| 0B dual enabled | `55600127c000000bad1f` |
| 0C dual list from 0 | `55600128c001000c00dd8e` |
| 0D codec | `55600129c000000d44dc` |
| 0E spatial | `5560014fc000000e0cd5` |

Writes, op 01:

| Setting | Frames, in order |
|---|---|
| `anc off` | `5560010ff0030001010500fb17` |
| `anc transparency` | `5560010ff0030001010700fa77` |
| `anc high` | `5560010ff0030001010100f9d7` |
| `anc mid` | `5560010ff0030001010200f927` |
| `anc low` | `5560010ff0030001010300f8b7` |
| `anc adaptive` | `5560010ff0030001010400fa87` |
| `eq dirac` | `5560011df00200010000fb69` |
| `eq rock` | `5560011df00200010100faf9` |
| `eq electronic` | `5560011df00200010200fa09` |
| `eq pop` | `5560011df00200010300fb99` |
| `eq vocals` | `5560011df00200010400f9a9` |
| `eq classical` | `5560011df00200010500f839` |
| `eq custom` | `5560011df00200010600f8c9` `55600144c0000001e910` `55600144c0000001e910` |
| `eq-custom 3 0 -3` | `55600141f035000103000040c00100000000000075443333333f02000040c000a0d7450000803f000000404000000c43cdcc4c3f000000000000000000531b` |
| `eq-custom 0 0 0` | `55600141f035000103000000800100000000000075443333333f020000000000a0d7450000803f000000000000000c43cdcc4c3f000000000000000000675c` |
| `spatial off` | `55600152f0020001000045ad` |
| `spatial fixed` | `55600152f00200010100443d` |
| `bass 0` | `55600151f0020001000076ad` |
| `bass 1` | `55600151f00200010105b73e` |
| `bass 2` | `55600151f0020001010af73a` |
| `bass on, at level 2` | `55600151f0020001010af73a` |
| `bass off, at level 2` | `55600151f0020001000af6aa` |
| `low-latency on` | `55600140f00200010100763c` |
| `low-latency off` | `55600140f0020001020076cc` |
| `in-ear on` | `55600104f003000101010179a4` |
| `in-ear off` | `55600104f0030001010100b864` |

## 29. CMF Buds 2 (B197)

Page `MainControl_girafarig`. Fast Pair `3B1320`, `359A7D`, `00FDED`. A
later hardware revision of B179 with the same page.

- As B179, except bass enhance has three stages.

Connect sequence, actual op ids:

| Op | Frame |
|---|---|
| 01 battery, 0 ms | `55600107c0000001acdf` |
| 02 listening mode, +200 | `55600150c00000029912` |
| 03 firmware, +300 | `55600142c0000003e0d1` |
| 04 UTC time, +400 | `5560010af00400046abc51002342` |
| 05 in-ear, +500 | `5560010ec0000005711d` |
| 06 latency, +600 | `55600141c000000664d2` |
| 07 gestures, +800 | `55600118c0000007b91f` |
| 08 ANC, +900 | `5560011ec0000008711b` |
| 09 advanced EQ status, +1000 | `5560014cc00000090917` |
| 0A bass enhance, +1100 | `5560014ec000000a30d6` |
| 0B dual enabled | `55600127c000000bad1f` |
| 0C dual list from 0 | `55600128c001000c00dd8e` |
| 0D spatial | `5560014fc000000d4cd4` |

Writes, op 01:

| Setting | Frames, in order |
|---|---|
| `anc off` | `5560010ff0030001010500fb17` |
| `anc transparency` | `5560010ff0030001010700fa77` |
| `anc high` | `5560010ff0030001010100f9d7` |
| `anc mid` | `5560010ff0030001010200f927` |
| `anc low` | `5560010ff0030001010300f8b7` |
| `anc adaptive` | `5560010ff0030001010400fa87` |
| `eq dirac` | `5560011df00200010000fb69` |
| `eq rock` | `5560011df00200010100faf9` |
| `eq electronic` | `5560011df00200010200fa09` |
| `eq pop` | `5560011df00200010300fb99` |
| `eq vocals` | `5560011df00200010400f9a9` |
| `eq classical` | `5560011df00200010500f839` |
| `eq custom` | `5560011df00200010600f8c9` `55600144c0000001e910` `55600144c0000001e910` |
| `eq-custom 3 0 -3` | `55600141f035000103000040c00100000000000075443333333f02000040c000a0d7450000803f000000404000000c43cdcc4c3f000000000000000000531b` |
| `eq-custom 0 0 0` | `55600141f035000103000000800100000000000075443333333f020000000000a0d7450000803f000000000000000c43cdcc4c3f000000000000000000675c` |
| `spatial off` | `55600152f0020001000045ad` |
| `spatial fixed` | `55600152f00200010100443d` |
| `bass 0` | `55600151f0020001000076ad` |
| `bass 1` | `55600151f00200010105b73e` |
| `bass 2` | `55600151f0020001010af73a` |
| `bass on, at level 2` | `55600151f0020001010af73a` |
| `bass off, at level 2` | `55600151f0020001000af6aa` |
| `low-latency on` | `55600140f00200010100763c` |
| `low-latency off` | `55600140f0020001020076cc` |
| `in-ear on` | `55600104f003000101010179a4` |
| `in-ear off` | `55600104f0030001010100b864` |
