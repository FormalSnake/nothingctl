# Nothing protocol facts: CMF Headphone Pro (B175)

Extracted 2026-09-30 from the live ear-web site (earweb.bttl.xyz) page
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
- **Ring is broken on this page.** The headphone header's Ring button calls the
  left-ring handler with no argument, which throws before any frame is built,
  so ear-web never sends a find frame to a B175. Had it run, the shared ring
  function would send `0xF002 02 01` (the earbud-left form): the headphone form
  `06 01` is only used for B170, B164 and B186, and B175 is not in that list even
  though its config says `deviceType: 6` and `findDevice: 1`. Which form B175
  accepts cannot be settled from ear-web.
