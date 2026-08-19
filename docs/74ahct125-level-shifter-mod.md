# 74AHCT125 Level-Shifter Mod (WS2815B LED Outputs)

Adds four 5 V-level WS281x data outputs to the pico2-artnet-node board by
fitting a 74AHCT125 quad buffer in the prototype area and bypassing the
on-board MC14504B (U7).

Target hardware: `pico2-artnet-node` rev with `U7 = MC14504BDR2G`
(see [klnspdr/pico-node](https://codeberg.org/klnspdr/pico-node)).

---

## Why bypass U7?

The board already has a hex level shifter (U7, MC14504BDR2G) wired from six
GPIOs to the 16-pin GPIO connector J6, complete with series-resistor
footprints and a V_HIGH voltage selector (JP1). The *translation* design is
sound — pin 13 tied to GND selects TTL input mode, dropping V_IH to 2 V so a
3.3 V GPIO is a legal input.

The problem is the dynamics at the one output voltage WS2815B allows:

| CD4504B spec at V_DD = 5 V | Value | WS2815B requirement |
|---|---|---|
| Recommended V_DD range | 5–18 V | we sit at the very bottom |
| TTL-mode prop delay @ V_DD=5 | not characterised | — |
| t_PHL / t_PLH (nearest V_DD=5 rows) | 275/550 ns and 200/400 ns | **75 ns typical edge skew, 150 ns worst case** |
| Transition time | 100 ns typ / 200 ns max (into 50 pF) | T0H pulse is ~300 ns |
| Output drive I_OL / I_OH | ±0.51 mA min, ±1 mA typ | 74AHCT125 does ±8 mA |

The WS2815B **T0H window is 220–380 ns — only 160 ns wide**. Typical edge skew
eats half of it; worst case eats all of it. And at 1 mA, slewing 5 V into
~150 pF of pigtail plus pixel input takes ≈750 ns, longer than an entire bit.

You cannot buy margin by raising V_HIGH either: the WS2815B absolute-maximum
logic input is **3.7–5.3 V**, so 5 V is a hard ceiling — precisely where the
4504 is slowest and weakest.

U7 does **not** need to be desoldered. Its outputs only reach J6 through
R6–R11, which ship unpopulated, so leaving them empty isolates it completely.

---

## Bill of materials

| Qty | Part | Note |
|---|---|---|
| 1 | 74AHCT125N (DIP-14) | Quad buffer, 3-state, TTL-level inputs. **AHCT / HCT only** — plain AHC/HC has CMOS thresholds and will not accept 3.3 V reliably. |
| 1 | 14-pin DIP socket (optional) | Recommended; lets you swap the chip without reworking the grid. |
| 1 | 100 nF ceramic, 2.54 mm lead pitch | Decoupling. Still required — see below; the board's existing caps do not cover this chip. |
| 4 | 33 Ω resistor, **0603 SMD** | Series damping. Goes into the empty R6–R9 footprints, not on the grid. 22–100 Ω all work. |
| — | 0.5 mm solid-core hookup wire | For the grid links. |

Optional but recommended: a 10 µF cap across the +5V rail, since the supply
reaches the prototype area over a fair length of flying wire.

This assumes **U7 and R6–R11 are unpopulated**, which is the case on boards
built from the current production files. If your board *does* have U7 fitted,
everything below still works — just leave R6–R9 empty and inject on their J6
side (pad 2) instead, so you are not driving into a powered MC14504B output.

---

## The prototype area

J7 is a `Breadboard_13x27` grid: 1.0 mm holes on 2.54 mm pitch, footprint
origin at board coordinate **(125.207, 49.878)**, rotated −90°.

**The holes are bussed into strips by board-level copper — this is a real
breadboard, not isolated perfboard.** Getting this wrong shorts pins together,
so the layout matters:

- **21 vertical strips** at X = 66.787 … 117.587, one every 2.54 mm. Each strip
  is split into **two electrically independent banks of 5 holes**:
  - **Bank A**: Y = 52.418, 54.958, 57.498, 60.038, 62.578
  - **Bank B**: Y = 67.658, 70.198, 72.738, 75.278, 77.818
- **The channel between the banks sits at Y = 65.118 — the GPIO27 row.** There
  are no strip holes on that row at all; it is the breadboard's central gap.
- **Four full-width power rails**, each bussed along X from 66.787 to 125.207:
  **Y = 47.338** and **Y = 49.878** (top), **Y = 80.358** and **Y = 82.898**
  (bottom).
- The **GPIO stubs** are separate: 3-hole horizontal groups at
  X = 120.127 / 122.667 / 125.207, one row per net. There is a 2.54 mm gap
  between the end of each vertical strip (X = 117.587) and the stubs, with no
  copper across it — **the stubs are not connected to the strips.**

```
   Y (mm)      X=102.347 ... 112.507  115.047  117.587 | 120.127 122.667 125.207
   ---------------------------------------------------------------------------
   47.338     ══════════════ RAIL (bussed along X) ════════════════════════════
   49.878     ══════════════ RAIL (bussed along X) ════════════════════════════
   52.418        │        │      │        │        │   |  [O       O       O]  GPIO0
   54.958        │        │      │        │        │   |  [O       O       O]  GPIO1
   57.498      BANK A   BANK A  ...     BANK A   BANK A|  [O       O       O]  GPIO2
   60.038        │        │      │        │        │   |  [O       O       O]  GPIO3
   62.578        │        │      │        │        │   |  [O       O       O]  GPIO28
   65.118     ------------- CHANNEL, no holes -------- |  [O       O       O]  GPIO27
   67.658        │        │      │        │        │   |  [O       O       O]  GPIO26
   70.198        │        │      │        │        │   |  [O       O       O]  GPIO22
   72.738      BANK B   BANK B  ...     BANK B   BANK B|   o       o       o
   75.278        │        │      │        │        │   |   o       o       o
   77.818        │        │      │        │        │   |   o       o       o
   80.358     ══════════════ RAIL (bussed along X) ════════════════════════════
   82.898     ══════════════ RAIL (bussed along X) ════════════════════════════

   Each │ is one independent 5-hole strip. Same X, different bank = NOT joined.
```

---

## Placing the 74AHCT125

The DIP must lie with its **long axis along X**, one pin per strip, straddling
the Y = 65.118 channel exactly as it would on a solderless breadboard.

A 0.3" DIP needs its two pin rows 7.62 mm apart. The channel is only 5.08 mm
wide, so the rows do not land on the two innermost bank rows — they land on
**Y = 70.198** (bank B, second row) and **Y = 62.578** (bank A, last row).

Orientation matters, and only two are physically possible. Viewed from above,
a DIP numbers counter-clockwise from the notch, so when the package lies
horizontally:

- **notch on the left** → pin 1 is **bottom-left**, pins 1→7 run left-to-right
  along the bottom, pin 8 is top-right, pins 8→14 run right-to-left along the top
- **notch on the right** → pin 1 is **top-right**, pins 1→7 run right-to-left
  along the top, pin 8 is bottom-left, pins 8→14 run left-to-right along the bottom

Use the second: **notch faces right (+X), toward the GPIO stubs, with pin 1 at
(117.587, 62.578)**. This puts V_CC on the bank-B end nearest J2, which keeps
the supply wire short.

Remember board Y increases *downward*, so Y = 62.578 is the upper row on screen
and in the KiCad top view.

| Pin | Function | Board coordinate (mm) | Bank | Row |
|---|---|---|---|---|
| 1 | 1OE | (117.587, 62.578) | A | top |
| 2 | **1A** | (115.047, 62.578) | A | top |
| 3 | **1Y** | (112.507, 62.578) | A | top |
| 4 | 2OE | (109.967, 62.578) | A | top |
| 5 | **2A** | (107.427, 62.578) | A | top |
| 6 | **2Y** | (104.887, 62.578) | A | top |
| 7 | GND | (102.347, 62.578) | A | top |
| 8 | **3Y** | (102.347, 70.198) | B | bottom |
| 9 | **3A** | (104.887, 70.198) | B | bottom |
| 10 | 3OE | (107.427, 70.198) | B | bottom |
| 11 | **4Y** | (109.967, 70.198) | B | bottom |
| 12 | **4A** | (112.507, 70.198) | B | bottom |
| 13 | 4OE | (115.047, 70.198) | B | bottom |
| 14 | VCC | (117.587, 70.198) | B | bottom |

Each pin sits on its own strip, and pins 1 and 14 — same X = 117.587, opposite
banks — are correctly isolated from one another by the channel.

The chip body floats over the channel row and over bank B's Y = 67.658 holes.
That costs you nothing: every strip still has free holes for wiring (bank A
keeps Y = 52.418–60.038, bank B keeps Y = 72.738–77.818).

Two input pins land on a GPIO row and need only a straight horizontal run:
**pin 2 on the GPIO28 row** and **pin 12 on the GPIO22 row**.

---

## Wiring

Assign the rails first. GND is needed in both banks (the OE pins are split
across them), +5V only in bank B:

| Rail | Y (mm) | Use |
|---|---|---|
| top, outer | 47.338 | spare |
| top, inner | 49.878 | **GND** — serves bank A |
| bottom, inner | 80.358 | **GND** — serves bank B |
| bottom, outer | 82.898 | **+5V** |

Both bottom rails run out to X = 125.207, the edge nearest J2.

### Power

| From | To | Note |
|---|---|---|
| J2 pin 4 (`+5V`) | (125.207, 82.898) | J2 is the 4-pin power breakout at (146.5, 76.74) |
| J2 pin 1 or 2 (`GND`) | (125.207, 80.358) | bottom GND rail |
| J2 pin 1 or 2 (`GND`) | (125.207, 49.878) | top GND rail — second wire, rather than bridging rails |
| (117.587, 77.818) | (117.587, 82.898) | pin 14's strip → +5V rail. **Skips over** the GND rail hole at 80.358 — route the wire above the board |
| (102.347, 52.418) | (102.347, 49.878) | 2.54 mm jumper: pin 7's strip → top GND rail |

Only the single V_CC wire has to cross a rail; everything else is a 2.54 mm
hop.

Note the rail: the net called `+5V` is the Pico's **VSYS**, not a regulated
5 V. Expect **4.6–4.8 V** (USB VBUS minus a Schottky drop through D2). That is
fine for WS2815B — comfortably inside its 3.7–5.3 V logic input window, with
headroom below the 5.3 V ceiling — but it is near the bottom of the AHCT
V_CC range (4.5–5.5 V), so keep the supply wiring short.

### Decoupling — still required

The board's existing capacitors do **not** cover this chip, even though they
are populated:

- C27 (0.1 µF) and C28 (1 µF) sit on `+3V3`, a different rail entirely.
- C29 (1 µF) and C30 (0.1 µF) sit on `V_HIGH`, which with U7 unpopulated is an
  orphaned net going nowhere.
- All four are clustered around U7 at (157.56, 61) — **40–55 mm away** from the
  prototype area.

Decoupling is about the loop inductance between the chip's own supply pins and
the nearest charge reservoir. Forty millimetres of track plus a flying supply
wire is tens of nanohenries, and a 74AHCT125 switching four outputs with ~4 ns
edges into cable capacitance pulls exactly the kind of fast current spike that
inductance turns into supply droop and ringing. **Fit the local 100 nF.** It is
the cheapest part of this entire mod.

Convenient placement: pin 13 (4OE) is tied to GND and sits on the strip
immediately adjacent to pin 14 (VCC), so the cap spans two neighbouring bank-A
strips with straight legs:

| Component | From | To |
|---|---|---|
| 100 nF ceramic | (117.587, 75.278) — pin 14 strip | (115.047, 75.278) — pin 13 strip (GND) |

Make pin 13's strip a solid GND with a short direct wire to the bottom GND
rail, so the cap's return path is real. A 10 µF bulk cap anywhere on the +5V
rail is worthwhile too, given the length of the supply wire from J2.

### Output enables

All four `OE` pins are active-low and **must not float**. Pins 1 and 4 are in
bank A, pins 10 and 13 in bank B, so each goes to its own side's GND rail —
all four are single 2.54 mm hops:

| Pin | Strip free hole | To GND rail |
|---|---|---|
| 1 (1OE) | (117.587, 52.418) | (117.587, 49.878) |
| 4 (2OE) | (109.967, 52.418) | (109.967, 49.878) |
| 10 (3OE) | (107.427, 77.818) | (107.427, 80.358) |
| 13 (4OE) | (115.047, 77.818) | (115.047, 80.358) |

### Signal wiring

The buffer channel number is arbitrary — what matters is that each GPIO's
signal comes back out on the right J6 pin, so the firmware's output numbering
holds. With this orientation the two straight-run pairings fall out as GP28 on
channel 1 and GP22 on channel 4, which fixes the rest:

| LED output (firmware) | GPIO | Stub row Y | AHCT input pin | AHCT output pin | Series R | J6 signal | J6 GND |
|---|---|---|---|---|---|---|---|
| 0 | **GP22** | 70.198 | 12 (4A) @ (112.507, 70.198) — straight | 11 (4Y) @ (109.967, 70.198) | R6 = 33 Ω | **5** | 6 |
| 1 | **GP26** | 67.658 | 9 (3A) @ (104.887, 70.198) — one row over | 8 (3Y) @ (102.347, 70.198) | R7 = 33 Ω | **7** | 8 |
| 2 | **GP27** | 65.118 | 5 (2A) @ (107.427, 62.578) — one row over | 6 (2Y) @ (104.887, 62.578) | R8 = 33 Ω | **9** | 10 |
| 3 | **GP28** | 62.578 | 2 (1A) @ (115.047, 62.578) — straight | 3 (1Y) @ (112.507, 62.578) | R9 = 33 Ω | **11** | 12 |

Each GPIO stub has three holes; use whichever is convenient and leave the
others for probing.

Because R6–R11 are unpopulated *and* U7 is absent, the `LVL SHIFT B*` nets are
dead stubs — which makes the output side cleaner than it would otherwise be.
**Fit the 33 Ω resistors into the R6–R9 footprints** and land the output wire
on their U7 side, so the series resistor is a proper SMD part in its designed
place rather than a flying component on the grid.

| Series R | Inject at | Footprint at (mm) |
|---|---|---|
| R6 | R6 pad 1 / U7 pad 15 | (151.7, 53.5) |
| R7 | R7 pad 1 / U7 pad 12 | (153.3, 53.5) |
| R8 | R8 pad 1 / U7 pad 10 | (154.9, 53.5) |
| R9 | R9 pad 1 / U7 pad 6 | (164.4, 53.7) |

**Pad 1 is the U7-facing pad** (larger Y) on each 0603 footprint. With U7 not
fitted there is nothing else on that net, so no contention. U7's own SOIC-16
pads carry the same nets and are longer targets (0.6 × 1.97 mm) if you find
them easier to solder to than 0603.

Leave R10 and R11 empty — those are the two unused level-shifter channels.

### JP1

JP1 is out of the signal path and, with U7 unpopulated, out of the circuit
entirely. Fitting it across pins 1–2 (`+5V`) simply parks C29 + C30's 1.1 µF
on the 5 V rail as extra bulk — harmless, mildly useful. Leaving it off is
equally fine. **Never** move it to the external-supply position: anything above
5.3 V on a WS2815B data pin destroys the first pixel.

---

## Connecting the LED strips

J6 conveniently alternates signal and ground: pins 5/7/9/11 are the four data
lines, and pins 6/8/10/12 are GND — one ground per signal.

```
   12 V PSU  ──── +12V ────────────────► strip V+
             │
             └── GND ──┬───────────────► strip GND
                       │
                       └── J6 pin 6 ────► (board GND, common reference)

   J6 pin 5  ─── 33 Ω (on grid) ────────► strip DIN
```

Rules that actually matter:

1. **The board does not power the strips.** Nothing here is remotely close —
   D2 alone is a 0.5 A part, and the `+5V` rail already carries U2, the 2 W
   isolated DC/DC feeding all four RS-485 transceivers. Strip power comes from
   an external 12 V PSU.
2. **Grounds must be common.** Tie the PSU's 0 V to a J6 GND pin. Without a
   shared reference the data line has no return path and the strip will behave
   erratically or not at all.
3. **Data cable short.** The AHCT125's ±8 mA drives a real cable, but keep the
   run from J6 to the first pixel under ~1 m if you can, and use a
   twisted/adjacent ground return. Published testing puts ~10 m at AWG26 as the
   outer limit with a level shifter — treat that as a ceiling, not a target.
4. **Power injection.** 1056 RGBW pixels is a substantial load; budget it from
   your reel's spec sheet and inject every 100–150 px. Do not attempt to feed a
   long run from one end.
5. **Backup data line (BI).** The WS2815's BI pin gives redundancy against a
   *dead pixel*, not against a corrupted input. At the head of the strip you can
   tie BI to the same data as DIN, or leave it per the strip's own pigtail — it
   changes nothing about this mod.

---

## Bring-up checklist

Before applying power:

- [ ] Continuity: pin 14 → J2 pin 4, pin 7 → J2 GND.
- [ ] Continuity: pins 1, 4, 10, 13 all → pin 7.
- [ ] **No** continuity between pin 14 and pin 7 (the classic decoupling-cap
      solder bridge).
- [ ] **No** continuity between pin 1 and pin 14 — they share X = 117.587 and
      must stay isolated by the channel. If they beep, the DIP is in the wrong
      rows or a stray wire bridges the banks.
- [ ] Each `A` pin reads through to its GPIO stub and nowhere else.
- [ ] Each `Y` pin reads through its 33 Ω to the correct J6 odd pin.
- [ ] R6–R9 fitted with 33 Ω; R10 and R11 still empty.

Powered, no strip attached:

- [ ] pin 14 measures 4.6–4.8 V against pin 7.
- [ ] With firmware running, each `Y` output toggles.

On a scope at the J6 end of the cable, with a real strip attached — this is the
measurement that actually validates the mod:

- [ ] **T0H lands inside 220–380 ns.** This is the tight one.
- [ ] T1L inside 220–420 ns.
- [ ] Rise/fall times are a small fraction of the pulse, not comparable to it.
- [ ] High level ≥ 3.5 V at the far end of the cable, and never above 5.3 V.
- [ ] Inter-frame gap > 280 µs (the WS2815B reset/latch time).

---

## Signal path summary

```
RP2350 GPIO (3.3 V)       74AHCT125 (VCC ≈ 4.7 V)              J6         strip
  GP22 ───────────────►  4A(12) ▷  4Y(11) ──► R6 pad1 ─[33Ω]─► pin 5  ───► DIN
  GP26 ───────────────►  3A(9)  ▷  3Y(8)  ──► R7 pad1 ─[33Ω]─► pin 7  ───► DIN
  GP27 ───────────────►  2A(5)  ▷  2Y(6)  ──► R8 pad1 ─[33Ω]─► pin 9  ───► DIN
  GP28 ───────────────►  1A(2)  ▷  1Y(3)  ──► R9 pad1 ─[33Ω]─► pin 11 ───► DIN
                         OE (1,4,10,13) tied low           6/8/10/12 ────► GND
                         VCC(14) / GND(7) + local 100 nF

  Buffer channel order is reversed relative to LED output order — that is a
  consequence of the DIP orientation, not a mistake. Follow the wiring table.

  (U7 not fitted. Its footprint's output pads carry the same LVL SHIFT nets
   as R6–R9 pad 1, and are an easier soldering target if you prefer.)
```
