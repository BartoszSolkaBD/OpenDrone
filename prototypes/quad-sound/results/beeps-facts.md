# T34 beep facts: Bluejay ESC v0.21.0 + Betaflight 2026.6.2

Source-checked reference for the issue #34 sound prototype. Every number below comes from source code at a pinned tag. Lines marked **[derived]** are my own arithmetic on that source. Lines marked **[UNVERIFIED]** could not be confirmed from a primary source.

Versions used:

| Thing | Ref used | Notes |
|---|---|---|
| Bluejay | tag **`v0.21.0`** (tag object `cb44142f`) | Latest *stable* release (`gh api repos/bird-sanctuary/bluejay/releases/latest` → `v0.21.0`, 2024-07-19). A newer prerelease, `v0.21.1-RC1`, exists. I checked the diff `v0.21.0...v0.21.1-RC1`: `Fx.asm` (all beep code) is unchanged, the melody bytes are unchanged, and the defaults only moved to `src/Settings/BluejaySettings.asm` with the same values. Everything below holds for both. |
| Betaflight | tag **`2026.6.2`** (commit `e0b7bb01`) | The tag exists. |
| esc-configurator (cross-check) | `stylesuxx/esc-configurator@21b407a3` (master) | Uses npm `bluejay-rtttl-parse` for the melody format. |
| bluejay-rtttl-parse (cross-check) | `saidinesh5/bluejay-rtttl-parse@c7a2b7f0` (HEAD) | Converts melodies between RTTTL and Bluejay bytes. Contains the configurator's period formula. |

Abbreviations: `BJ` = `https://github.com/bird-sanctuary/bluejay/blob/v0.21.0/`, `BF` = `https://github.com/betaflight/betaflight/blob/2026.6.2/`.

---

# Part 1: Bluejay (v0.21.0)

## 1.1 How the ESC makes sound

The ESC has no speaker. The beep routine bit-bangs the motor FETs in software. Each "beep pulse" (one tone period) is:

1. Pre-charge the B gate driver: B com FET off, B pwm FET on, then off, then B com (low side) on. There is a `djnz ACC,$` with A=0 between each step, about 1023 cycles (≈41.8 µs) each.
2. **Current pulse 1**: A pwm FET on for `Beep_Strength` loops, then off, then 25 µs off.
3. Repeat step 1, then **current pulse 2** on the C pwm FET (Temp2 counts 2 → 1).
4. "Off" wait loop: `Temp3` × (inner loop of 200) → **this sets the pitch**.
5. Repeat `Temp4` times → **this sets the duration**.

So the motor windings get two short current pulses per tone period: A→B, then C→B, about 199 µs apart. The rest of the period is silent. The sound is a buzzy pulse train, not a sine.

Verbatim, `BJ src/Modules/Fx.asm#L131-L183`:

```asm
beep:
    mov  A, Beep_Strength
    jnz  beep_start                     ; Start if beep strength is not 0
    ret

beep_start:
    mov  Temp2, #2

beep_on_off:
    clr  A
    B_Com_Fet_Off                       ; B com FET off
    djnz ACC, $                         ; Allow some time after com FET is turned off
    B_Pwm_Fet_On                        ; B pwm FET on (in order to charge the driver of the B com FET)
    djnz ACC, $                         ; Let the pwm FET be turned on a while
    B_Pwm_Fet_Off                       ; B pwm FET off again
    djnz ACC, $                         ; Allow some time after pwm FET is turned off
    B_Com_Fet_On                        ; B com FET on
    djnz ACC, $                         ; Allow some time after com FET is turned on

    mov  A, Temp2                       ; Turn on pwm FET
    jb   ACC.0, beep_a_pwm_on
    A_Pwm_Fet_On
beep_a_pwm_on:
    jnb  ACC.0, beep_c_pwm_on
    C_Pwm_Fet_On
beep_c_pwm_on:

    mov  A, Beep_Strength               ; On time according to beep strength
    djnz ACC, $

    mov  A, Temp2                       ; Turn off pwm FET
    jb   ACC.0, beep_a_pwm_off
    A_Pwm_Fet_Off
beep_a_pwm_off:
    jnb  ACC.0, beep_c_pwm_off
    C_Pwm_Fet_Off
beep_c_pwm_off:

    mov  A, #150                        ; Off for 25 us
    djnz ACC, $

    djnz Temp2, beep_on_off             ; Toggle next pwm FET

    mov  A, Temp3
beep_off:                               ; Fets off loop
    mov  Temp1, #200
    djnz Temp1, $
    djnz ACC, beep_off                  ; Off time according to beep frequency

    djnz Temp4, beep_start              ; Number of beep pulses (duration)

    B_Com_Fet_Off
    ret
```

**Beep strength** is the on-time of each current pulse.

* Defaults (`BJ src/Bluejay.asm#L160-L162`): `DEFAULT_PGM_BEEP_STRENGTH EQU 40 ; 0..255 (BLHeli_S is 1..255)` and `DEFAULT_PGM_BEACON_STRENGTH EQU 80 ; 0..255`.
* `decode_settings` loads it (`BJ src/Modules/Settings.asm#L164-L165`). The beacon routine swaps in the beacon strength (`BJ src/Modules/Fx.asm#L221-L250`).
* Strength 0 means `beep` returns at once: no sound, and no time spent.
* **[derived]** Each `djnz ACC,$` iteration takes 4 cycles at 24.5 MHz, so the on-time is ≈ 4·BS/24.5 MHz. That gives **6.5 µs at BS=40** and 13.1 µs at the beacon's 80.

**Clock during beeps**:

* Bluejay states: "Master clock is internal 24MHz oscillator (or 48MHz…) … the exact clock frequencies are 24.5MHz or 49.0 MHz" (`BJ src/Bluejay.asm#L34-L35`).
* At power-up, `mov CLKSEL, #00h ; Set clock divider to 1 (Oscillator 0 at 24MHz)` runs (`BJ src/Bluejay.asm#L439`).
* `init_no_signal` forces `Set_MCU_Clk_24MHz` (`#L512-L516`). The comment there reads: "While not armed, all MCUs run at 24MHz … After arming those MCUs that support it (BB2 & BB51) are switched to 48MHz". `exit_run_mode` switches back to 24 MHz before any stall beep (`#L1045-L1046`).
* **So every beep (melody, arming beeps, stall, beacon) runs at 24.5 MHz.** The beep loop is software-timed and never references `PWM_FREQ`, so the **"96k" PWM build does not change any beep**.

## 1.2 Turning loop counts into Hz and ms

**Method A, from the configurator.** Verbatim from `bluejay-rtttl-parse` `src/index.js#L519-L532`. The ESC Configurator uses this library to convert and preview Bluejay melodies:

```js
static _calculateBluejayTemp3FromFrequency(freq) {
  return freq === 0 ? 0 : Math.round(1000000 / (freq * 24.72) - 399.3 / 24.72);
}
static _calculateFrequencyFromBluejayTemp3(temp3) {
  return temp3 === 0 ? 0 : 1000000 / (24.72 * temp3 + 399.3);
}
```

So **tone period T(µs) = 24.72·Temp3 + 399.3**, and **duration = Temp4 · T**. The library defines `Temp4 = number of pulses (~ duration)` and `Temp3 = duration of each pulse (~ pitch)` (`#L57-L60`). It computes pulse counts as `Math.round(item.duration / (1000/item.frequency))` (`#L100-L101`).

**Method B, my cycle count [derived].** I counted cycles on the Fx.asm code above using the 8051-core (CIP-51) instruction timings below. These timings are from my knowledge of the EFM8 reference manual. silabs.com returned "Access Denied", so the table itself is **[UNVERIFIED]**:

| Instruction | Cycles |
|---|---|
| `djnz direct,rel` | 3 not taken / 4 taken |
| `djnz Rn,rel` | 2 / 3 |
| `jb`/`jnb` | 3 / 4 |
| `mov Rn,#d` | 2 |
| `mov A,direct` | 2 |
| `clr`/`setb bit` | 2 |
| `mov A,Rn` | 1 |

Bluejay's own comment confirms the `djnz ACC` figure: `djnz ACC, $ ; Inner loop (41.6us - 1020 cycles)` (`BJ src/Modules/Fx.asm#L80`).

* One tone period ≈ `9456 + 8·BeepStrength + 605·Temp3` cycles at 24.5 MHz.
* At BS=40 that is **399.0 µs + 24.69 µs·Temp3**.
* This matches the configurator's 399.3 + 24.72·Temp3 to within 0.15%, so the two methods agree.
* The fixed part grows with strength: at the beacon's BS=80 add 320 cycles (+13.1 µs per period). That lowers beacon pitch by about 1–1.5%.
* Within one period, current pulse 1 starts ≈168 µs in. Pulse 2 follows ≈199 µs later. Both last ≈6.5 µs at BS=40.
* Oscillator tolerance: the EFM8 internal oscillator is nominally ±2% **[UNVERIFIED: I could not fetch the datasheet]**.

**Wait routine**, verbatim from `BJ src/Modules/Fx.asm#L70-L86`. It is 24 middle loops of (2 + 1019 + 3) cycles plus 5 cycles of outer loop per ms. That is **[derived] 24 581 cycles = 1.0033 ms per "ms"**. So wait100ms ≈ 100.3 ms, wait250ms ≈ 250.8 ms, and a melody rest of N "ms" ≈ 1.0033·N ms.

## 1.3 The plain beeps

Verbatim, `BJ src/Modules/Fx.asm#L96-L129`:

```asm
beep_f1:
    mov  Temp3, #66                     ; Off wait loop length (Tone)
    mov  Temp4, #(3500 / 66)            ; Number of beep pulses (Duration)
    sjmp beep
beep_f2:
    mov  Temp3, #45
    mov  Temp4, #(3500 / 45)
beep_f3:
    mov  Temp3, #38
    mov  Temp4, #(3500 / 38)
beep_f4:
    mov  Temp3, #25
    mov  Temp4, #(3500 / 25)
beep_f5:
    mov  Temp3, #20
    mov  Temp4, #(3500 / 20)
beep_f1_short:
    mov  Temp3, #66
    mov  Temp4, #(2000 / 66)
beep_f2_short:
    mov  Temp3, #45
    mov  Temp4, #(2000 / 45)
```

The `sjmp beep` lines are omitted after the first. The assembler divides integers, so 3500/66 = 53.

Table **[derived]** at Beep_Strength 40. The "configurator" columns use T = 24.72·Temp3 + 399.3 µs. The "cycle model" columns are in brackets.

| Routine | Temp3 | Temp4 (pulses) | Period | **Freq (Hz)** | **Duration (ms)** |
|---|---|---|---|---|---|
| `beep_f1` | 66 | 53 | 2030.8 µs | **492.4** (492.9) | **107.6** (107.5) |
| `beep_f2` | 45 | 77 | 1511.7 µs | **661.5** (662.1) | **116.4** (116.3) |
| `beep_f3` | 38 | 92 | 1338.7 µs | **747.0** (747.7) | **123.2** (123.0) |
| `beep_f4` | 25 | 140 | 1017.3 µs | **983.0** (983.9) | **142.4** (142.3) |
| `beep_f5` | 20 | 175 | 893.7 µs | **1118.9** (1119.9) | **156.4** (156.3) |
| `beep_f1_short` | 66 | 30 | 2030.8 µs | **492.4** | **60.9** |
| `beep_f2_short` | 45 | 44 | 1511.7 µs | **661.5** | **66.5** |

The same tones at **beacon strength 80**, from the cycle model only (the configurator formula has no strength term):

| Tone | Freq (Hz) | Duration (ms) |
|---|---|---|
| f1 | 489.7 | 108.2 |
| f2 | 656.5 | 117.3 |
| f3 | 740.5 | 124.2 |
| f4 | 971.4 | 144.1 |
| f5 | 1103.8 | 158.5 |

Beep sequences, verbatim from `BJ src/Modules/Fx.asm#L185-L206`:

```asm
beep_signal_lost:      call beep_f1 / call beep_f2 / call beep_f3      ; rising
beep_enter_bootloader: call beep_f2_short / call beep_f1
beep_motor_stalled:    call beep_f3 / call beep_f2 / call beep_f1      ; falling
beep_safety_no_arm:    call beep_f2_short / call beep_f1_short
```

The real source puts each `call` on its own line, followed by `ret`. These calls are back-to-back with no gap: the next tone starts within microseconds.

## 1.4 Start-up melody

**Storage**: `CSEG AT CSEG_MELODY`, where `CSEG_MELODY EQU 1A70h` on BB1/BB2 and `3070h` on BB51 (`BJ src/Modules/Codespace.asm#L38,#L47`). The block is `MELODY_SIZE EQU 140` bytes (`BJ src/Modules/Eeprom.asm#L36`). The ESC Configurator maps it as `STARTUP_MELODY: { offset: 0x70, size: 128 }`.

**Default bytes**, verbatim from `BJ src/Bluejay.asm#L406-L407`:

```asm
CSEG AT CSEG_MELODY
Eep_Pgm_Beep_Melody: DB 2,58,4,32,52,66,13,0,69,45,13,0,52,66,13,0,78,39,211,0,69,45,208,25,52,25,0
```

**Player**: `play_beep_melody` (`BJ src/Modules/Fx.asm#L252-L307`). The routine's own header comment:

```
; A melody has 64 pairs of (item1, item2) - a total of 128 items.
; the first 4 values of the 128 items are metadata
; item2 - is the duration of each pulse of the musical note.
;         The lower the value, the higher the pitch.
; item1 - if item2 is zero, it is the number of milliseconds of wait time, else
;         it is the number of pulses of item2.
```

The code does this:

* If byte 0 is 0xFF, it skips the melody (`cpl A / jz`).
* Otherwise it starts at offset +4 and plays at most `Temp5 = 62` pairs.
* For each pair: `Temp4 = item1`. If item1 is 0, the melody ends.
* `Temp3 = item2`. If item2 ≠ 0 it calls the same `beep` routine as above, with Temp3 = pitch and Temp4 = pulse count. If item2 = 0 it calls `wait_ms` with Temp2 = item1, a rest of item1 ms.
* So a melody note is **exactly** a `beep_fX` with custom loop counts. Same pulse train, same Beep_Strength.

**Metadata** (first 4 bytes): bluejay-rtttl-parse `src/index.js#L57-L58` gives `[2 bytes of bpm],[1 byte of default octave],[1 byte of default duration]`. The player ignores them. Here they are `2,58` → BPM = 2·256 + 58 = **570**, default octave **4**, default duration **32**.

**The configurator ships the same melody.** `esc-configurator src/melodies.json` lines 3–5 read `"name": "Bluejay Default"` with the track `"bluejay:b=570,o=4,d=32:4b,p,4e5,p,4b,p,4f#5,2p,4e5,2b5,8b5"`. Re-encoding that RTTTL with the library formula reproduces every byte **[derived]**. For example, B4 (493.9 Hz) gives Temp3 = round(1e6/(493.9·24.72) − 16.15) = 66, and a quarter note at 570 bpm (105.26 ms) gives round(105.26 × 0.4939) = 52 pulses.

**Decoded default melody** **[derived]**. Hz and ms use the configurator formula; rests are scaled ×1.0033. t is measured from the start of the melody.

| # | bytes (item1, item2) | t start (ms) | **Freq (Hz)** | **Duration (ms)** | RTTTL note (nominal Hz) |
|---|---|---|---|---|---|
| 1 | 52, 66 | 0.0 | **492.4** | **105.6** | 4b (B4, 493.9) |
| 2 | 13, 0 | 105.6 | rest | **13.0** | p (1/32) |
| 3 | 69, 45 | 118.6 | **661.5** | **104.3** | 4e5 (E5, 659.3) |
| 4 | 13, 0 | 222.9 | rest | **13.0** | p |
| 5 | 52, 66 | 235.9 | **492.4** | **105.6** | 4b (B4) |
| 6 | 13, 0 | 341.5 | rest | **13.0** | p |
| 7 | 78, 39 | 354.5 | **733.5** | **106.3** | 4f#5 (F#5, 740.0) |
| 8 | 211, 0 | 460.9 | rest | **211.7** | 2p |
| 9 | 69, 45 | 672.6 | **661.5** | **104.3** | 4e5 (E5) |
| 10 | 208, 25 | 776.9 | **983.0** | **211.6** | 2b5 (B5, 987.8) |
| 11 | 52, 25 | 988.5 | **983.0** | **52.9** | 8b5 (B5) |
| end | 0 | 1041.4 | — | — | item1 = 0 ends the melody |

Notes 10 and 11 are the same pitch with no gap, so you hear **one B5 of ≈264.5 ms**. Total melody length is **≈1.041 s**. The cycle model gives the same notes to within 0.7 Hz.

`Pgm_Startup_Beep` / `DEFAULT_PGM_STARTUP_BEEP EQU 1 ; 0=Short beep,1=Melody` exists (`BJ src/Bluejay.asm#L171`), but nothing in v0.21.0 reads it. A grep of `src/` finds it only in `set_default_parameters`. **The melody plays whenever byte 0 ≠ 0xFF.** There is no separate "3 rising beeps" power-up path in v0.21.0. (The rising f1-f2-f3 is the signal-lost sound; see 1.6.)

## 1.5 Power-up sequence and timing

Code: `BJ src/Bluejay.asm#L492-L655`. Verbatim key lines:

```asm
    call set_default_parameters / read_all_eeprom_parameters / decode_settings
    clr  IE_EA
    call wait100ms                      ; Wait a bit to avoid audible resets if not properly powered
    call play_beep_melody               ; Play startup beep melody
    call led_control
    call wait100ms                      ; Wait for flight controller to get ready
init_no_signal:
    ...  ; If input signal is high for about ~150ms, enter bootloader mode   (exits at first low level)
    jnb  Flag_Had_Signal, setup_dshot
    call beep_signal_lost / wait250ms x3 / clr Flag_Had_Signal
setup_dshot:
    mov  TMR2CN0, #04h                  ; Timer2 enabled (system clock divided by 12)
    ...
    call wait1ms
    call detect_rcp_level
    [BB1 only: DShot150 test: wait100ms]
    ; DShot300 test: mov Rcp_Outside_Range_Cnt,#10 / call wait100ms / jz arming_begin
    ; DShot600 test (BB2/BB51): ... call wait100ms / jz arming_begin
    ljmp init_no_signal                 ; No valid signal detected, try again
arming_begin:
    setb Flag_Had_Signal
    mov  Startup_Stall_Cnt, #0
    call beep_f1_short                  ; Confirm RC pulse detection by beeping
; Make sure RC pulse has been zero for ~300ms
arming_wait:
    mov A, Rcp_Stop_Cnt / subb A, #10 / jc arming_wait
    call beep_f2_short                  ; Confirm arm state by beeping
wait_for_start: ...
```

The real source has one instruction per line; I joined some with `/`.

How the 0.32 s zero-throttle wait is counted:

* Timer2 interrupt (`BJ src/Modules/Isrs.asm#L508-L553`): "Happens every 32ms before arming and every 16 ms after arming (on 48MHz MCUs)".
* **[derived]** 65536 × 12 / 24.5 MHz = **32.10 ms**.
* While `Flag_Rcp_Stop` is set (zero throttle, or timeout), each interrupt does `inc Rcp_Stop_Cnt`.
* So "≥10" means **≈321 ms of zero throttle**. Any non-zero throttle frame resets the counter (`Isrs.asm#L337-L339`).
* The counter starts running at `setup_dshot`, so it overlaps the DShot detection waits.
* Interrupts are disabled during `beep_f1_short`. Of the two Timer2 overflows that fall inside it, only one is serviced, so the count reaches 10 at about the 11th overflow.

Power-up timeline **[derived]**. Assumptions: BB2/BB51 MCU (48 MHz, "H" in the hex name), default melody, and the FC already sending DShot zero-throttle frames by t ≈ 1.24 s.

| t (ms) | Event | Sound |
|---|---|---|
| 0 | power-on, EEPROM read (sub-ms) | — |
| 0 → 100 | `wait100ms` | silence |
| 100 → 1142 | `play_beep_melody` | melody (table 1.4) |
| 1142 → 1242 | `wait100ms` | silence |
| ≈1242 | `init_no_signal` → `setup_dshot`; Timer2 starts | — |
| 1243 → 1343 | DShot300 test (100 ms) | — |
| 1343 → 1444 | DShot600 test (100 ms). **Skipped if the FC sends DShot300**: arming_begin is then at ≈1343. | — |
| ≈1444 (D600) / ≈1343 (D300) | `arming_begin`: "signal found" | **f1_short: 492 Hz, 60.9 ms** |
| → ≈1595 | `arming_wait` until ~11 Timer2 periods (≈353 ms) after setup_dshot | silence: ≈90 ms gap (D600) or ≈190 ms (D300) |
| ≈1595 → ≈1662 | "armed/ready" | **f2_short: 662 Hz, 66.5 ms** |
| ≈1662 | `wait_for_start`: ESC accepts throttle | — |

So **the ESC is ready ≈1.66 s after power-up.** Your 1.7 s figure is correct to within about 40 ms.

There is one more delay before the motor turns. On the first non-zero throttle the ESC runs `call wait100ms ; Wait to see if start pulse was glitch` (`BJ src/Bluejay.asm#L726-L730`), then `motor_start`. **The earliest a motor can spin is ≈1.76 s.** This assumes the signal is already present. If the FC starts DShot later, the ESC keeps looping `init_no_signal`. Each loop takes ≈1 + 100 + 100 ms on BB2, and the f1_short → f2_short chain runs from whenever the signal is detected. On a BB1 ("L") MCU a DShot150 test comes first, adding 100 ms.

The FC side has its own limit: Betaflight blocks arming until `pwr_on_arm_grace` = 5 s after FC boot (see 2.4). In practice the pilot cannot arm before ≈5 s anyway.

## 1.6 Signal lost

While waiting at zero throttle, `Rcp_Timeout_Cntd` counts down 10 Timer2 periods (≈321 ms before arming, 24 MHz) after the last valid frame. When it hits 0 the code takes `ljz init_no_signal` (`BJ src/Bluejay.asm#L719-L720`). Then:

```asm
    jnb  Flag_Had_Signal, setup_dshot
    call beep_signal_lost        ; f1, f2, f3 rising: 492 → 662 → 747 Hz, 107.6+116.4+123.2 ms
    call wait250ms ×3            ; ≈752 ms
    clr  Flag_Had_Signal
```

This plays **once only**. After that the flag is clear, the ESC loops silently looking for signal, and **no beacon plays while there is no signal**. When the signal returns you hear f1_short, then f2_short about 0.3 s later, as in 1.5.

Edge case: if the signal line stays **high** for about 150 ms (≈169 ms **[derived]**), the ESC plays `beep_enter_bootloader` (f2_short then f1) and jumps to the bootloader (`#L518-L530`).

## 1.7 Motor fails to start / stall

* `Startup_Stall_Cnt: DS 1 ; Counts start/run attempts that resulted in stall. Reset upon a proper stop` (`BJ src/Bluejay.asm#L245`).
* It is zeroed at `arming_begin` (`#L640`), at every pass of `normal_run_checks` (`#L957-L960`, meaning the motor really runs), and on a normal stop (`#L1122-L1124`).
* A comparator timeout in normal-run mode calls `exit_run_mode_on_timeout` (`BJ src/Modules/Timing.asm#L771-L788`). There it increments only if the motor never reached proper running (`BJ src/Bluejay.asm#L1033-L1035`): `jb Flag_Motor_Running, exit_run_mode / inc Startup_Stall_Cnt`.

`exit_run_mode`, verbatim (abridged), `BJ src/Bluejay.asm#L1066-L1107`:

```asm
    jb   Flag_Rcp_Stop, exit_run_mode_no_stall   ; throttle zero => normal stop, no beep
    setb Flag_Stall_Notify
    ; Check max consecutive stalls and exit if stall counter > 3
    clr  C
    mov  A, Startup_Stall_Cnt
    subb A, #3
    jnc  exit_run_mode_is_stall
    call wait100ms                      ; Wait for a bit between stall restarts
    ljmp motor_start                    ; Go back and try starting motors again
exit_run_mode_is_stall:
    ...
    call beep_motor_stalled             ; f3, f2, f1
    ljmp arming_begin                   ; Go back and wait for arming
```

* **Attempts**: the code checks `Startup_Stall_Cnt − 3 ≥ 0`, so the beep comes when the count reaches **3**. The comment says "> 3", but the code says ≥3. That is the initial start plus 2 retries, with **100 ms** pauses between them.
* **Sound after the 3rd failed start**:
  1. `beep_motor_stalled` = **747 → 662 → 492 Hz (123.2 + 116.4 + 107.6 ms)**, three falling tones back-to-back.
  2. Straight away, `arming_begin` plays **f1_short (492 Hz, 60.9 ms)**.
  3. The ESC then sits in `arming_wait` until throttle is zero for ≈321 ms. That happens when the FC disarms; while the FC is armed, idle is a non-zero DShot value. Then it plays **f2_short (662 Hz, 66.5 ms)** and is ready again.
* **Successful restart or retry**: no beep at all. A desync or stall while the motor was already running (`Flag_Motor_Running` set) does not increment the counter. The ESC just waits 100 ms and restarts without any sound, as many times as needed.

## 1.8 Beacon

Code: `wait_for_start_loop`, `BJ src/Bluejay.asm#L658-L708`:

```asm
    mov  A, Timer2_X / subb A, #94 / jc wait_for_start_no_beep   ; Counter wrapping (about 3 sec)
    mov  Timer2_X, #0
    inc  Beacon_Delay_Cnt
    ; Pgm_Beacon_Delay: 1 -> 20 (1 min), 2 -> 40 (2 min), 3 -> 100 (5 min), 4 -> 200 (10 min), else infinite
    ...
    dec  Beacon_Delay_Cnt               ; Decrement counter for continued beeping
    mov  Temp1, #4                      ; Beep tone 4
    call beacon_beep
```

* Default: `DEFAULT_PGM_BEACON_DELAY EQU 4 ; 1=1m 2=2m 3=5m 4=10m 5=Infinite`. Strength: `DEFAULT_PGM_BEACON_STRENGTH EQU 80` (`#L161-L162`).
* **[derived]** One step is 94 × 32.10 ms = 3.017 s. 200 steps = **603.5 s ≈ 10.06 min**.
* **When it starts**: the ESC must have a valid signal at zero throttle (in `wait_for_start`) for about 10 min. The counter is cleared every time `wait_for_start` is entered, so after every ready beep and every motor stop.
* **What it plays**: `beacon_beep` with tone 4, which is **`beep_f4` at strength 80: ≈971 Hz (983 Hz at strength 40), ≈144 ms**.
* **Repeat interval**: because of the `dec Beacon_Delay_Cnt`, the beep repeats every 94 counted Timer2 ticks. **[derived]** Overflows during the beep (interrupts disabled) are not counted, so the period is ≈3.02 s + ≈0.1 s ≈ **3.1 s**. It continues until throttle goes above zero or the signal is lost.

**DShot beacon commands 1–5** (`BJ src/Modules/DShot.asm#L79-L96`): `subb A, #6 ; Beacon beeps for command 1-5` → `call beacon_beep` → `call wait200ms`. Unlike other commands, these need no 6× repetition. `beacon_beep` (`Fx.asm#L221-L250`) maps:

* `CMD_BEEP_1` → `beep_f1`
* `CMD_BEEP_2` → `beep_f2`
* `CMD_BEEP_3` → `beep_f3`
* `CMD_BEEP_4` → `beep_f4`
* anything else (5) → `beep_f5`

It plays at beacon strength (80), then restores the normal beep strength. So DSHOT_CMD_BEACONn plays tone fn at strength 80, followed by a 200 ms ESC-side pause.

## 1.9 Bluejay items I could not verify

* **Whether the Meteor65 Pro's factory ESC has the default melody.** BetaFPV may flash a custom melody, settings, or Bluejay version. A configurator flash that keeps settings preserves the old melody block. The ESC's MCU type (BB1, BB2 or BB51) and layout are also unconfirmed, which affects the DShot150 step. **[UNVERIFIED]**
* **The CIP-51 per-instruction cycle table** (silabs.com blocked). The resulting frequencies do match the independent configurator formula.
* **Acoustic loudness or frequency response of a 0802/1102 whoop motor.** Not in source.

---

# Part 2: Betaflight 2026.6.2

## 2.1 Sequence format and units

Verbatim, `BF src/main/io/beeper.c#L86-L87` and `#L111-L121`:

```c
#define BEEPER_COMMAND_REPEAT 0xFE
#define BEEPER_COMMAND_STOP   0xFF
/* Beeper Sound Sequences: (Square wave generation)
 * Sequence must end with 0xFF or 0xFE. 0xFE repeats the sequence from
 * start when 0xFF stops the sound when it's completed.
 *
 * "Sound" Sequences are made so that 1st, 3rd, 5th.. are the delays how
 * long the beeper is on and 2nd, 4th, 6th.. are the delays how long beeper
 * is off. Delays are in milliseconds/10 (i.e., 5 => 50ms).
 *
 * if first value is zero, sequence starts with pause
 */
```

The code confirms this:

* **Units are 10 ms.** `beeperNextToggleTime = currentTimeUs + 1000 * 10 * currentBeeperEntry->sequence[beeperPos];` (`#L409`).
* A **0** entry is skipped, but on/off is "strictly index-based" by array position parity (`#L405-L416`). So `{0, 245, 10, …}` means OFF 2450 ms, then ON 100 ms.
* `BEEPER_COMMAND_STOP` ends the sequence and calls `beeperSilence()`. `BEEPER_COMMAND_REPEAT` jumps back to position 0. **No array in 2026.6.2 uses REPEAT.** Repeating sounds such as RX_LOST and BAT_LOW repeat only because their caller calls `beeper(mode)` again every task tick.
* `beeper()` ignores a request with priority ≥ the current entry's (0 = highest) (`#L284-L288`). So a repeating caller restarts the sequence only after it has finished.
* The beeper task runs at **100 Hz**: `[TASK_BEEPER] = DEFINE_TASK("BEEPER", NULL, NULL, beeperUpdate, TASK_PERIOD_HZ(100), TASK_PRIORITY_LOW)` (`BF src/main/fc/tasks.c#L397`). Edges therefore land on a 10 ms grid, with about one tick of jitter.

## 2.2 Every pattern, verbatim, with decoded ms

Verbatim, `BF src/main/io/beeper.c#L122-L195`:

```c
static const uint8_t beep_shortBeep[] = { 10, 10, BEEPER_COMMAND_STOP };
static const uint8_t beep_armingBeep[] = { 30, 5, 5, 5, BEEPER_COMMAND_STOP };
static const uint8_t beep_armingGpsFix[] = { 5, 5, 15, 5, 5, 5, 15, 30, BEEPER_COMMAND_STOP };
static const uint8_t beep_armingGpsNoFix[] = { 30, 5, 30, 5, 30, 5, BEEPER_COMMAND_STOP };
static const uint8_t beep_armedBeep[] = { 0, 245, 10, 5, BEEPER_COMMAND_STOP };
static const uint8_t beep_disarmBeep[] = { 15, 5, 15, 5, BEEPER_COMMAND_STOP };
static const uint8_t beep_disarmRepeatBeep[] = { 0, 100, 10, BEEPER_COMMAND_STOP };
static const uint8_t beep_lowBatteryBeep[] = { 25, 50, BEEPER_COMMAND_STOP };
static const uint8_t beep_critBatteryBeep[] = { 50, 2, BEEPER_COMMAND_STOP };
static const uint8_t beep_txLostBeep[] = { 50, 50, BEEPER_COMMAND_STOP };
static const uint8_t beep_sos[] = {
    10, 10, 10, 10, 10, 40, 40, 10, 40, 10, 40, 40, 10, 10, 10, 10, 10, 70, BEEPER_COMMAND_STOP
};
static const uint8_t beep_readyBeep[] = { 4, 5, 4, 5, 8, 5, 15, 5, 8, 5, 4, 5, 4, 5, BEEPER_COMMAND_STOP };
static const uint8_t beep_2shortBeeps[] = { 5, 5, 5, 5, BEEPER_COMMAND_STOP };
static const uint8_t beep_2longerBeeps[] = { 20, 15, 35, 5, BEEPER_COMMAND_STOP };
static const uint8_t beep_gyroCalibrated[] = { 20, 10, 20, 10, 20, 10, BEEPER_COMMAND_STOP };
static const uint8_t beep_camOpenBeep[] = { 5, 15, 10, 15, 20, BEEPER_COMMAND_STOP };
static const uint8_t beep_camCloseBeep[] = { 10, 8, 5, BEEPER_COMMAND_STOP };
static uint8_t beep_multiBeeps[MAX_MULTI_BEEPS * 2 + 1];   // MAX_MULTI_BEEPS = 32
```

(The source spreads each array over several lines with comments; the values are exact.)

Decoded **[derived]** as on/off in ms. A trailing off is still part of the sequence, during which lower-priority beeps cannot start.

| Array | Pattern (ms) | Total |
|---|---|---|
| shortBeep | ON 100, off 100 | 200 |
| armingBeep | ON 300, off 50, ON 50, off 50 | 450 |
| armingGpsFix | ON 50, off 50, ON 150, off 50, ON 50, off 50, ON 150, off 300 | 850 |
| armingGpsNoFix | ON 300, off 50, ON 300, off 50, ON 300, off 50 | 1050 |
| armedBeep | off 2450, ON 100, off 50 | 2600 |
| disarmBeep | ON 150, off 50, ON 150, off 50 | 400 |
| disarmRepeatBeep | off 1000, ON 100 | 1100 |
| lowBatteryBeep | ON 250, off 500 | 750 |
| critBatteryBeep | ON 500, off 20 | 520 |
| txLostBeep | ON 500, off 500 | 1000 |
| sos | ON100 off100 ON100 off100 ON100 off400 · ON400 off100 ON400 off100 ON400 off400 · ON100 off100 ON100 off100 ON100 off700 | 3900 |
| readyBeep | ON40 off50 ON40 off50 ON80 off50 ON150 off50 ON80 off50 ON40 off50 ON40 off50 | 820 |
| 2shortBeeps | ON 50, off 50, ON 50, off 50 | 200 |
| 2longerBeeps | ON 200, off 150, ON 350, off 50 | 750 |
| gyroCalibrated | (ON 200, off 100) × 3 | 900 |
| camOpenBeep | ON 50, off 150, ON 100, off 150, ON 200 | 650 |
| camCloseBeep | ON 100, off 80, ON 50 | 230 |

Generated `beep_multiBeeps` contents (`#L197-L204`, `#L314-L359`, `BF src/main/drivers/system.h#L50-L54`):

* **`beeperConfirmationBeeps(n)`**: n × (ON `BEEPER_CONFIRMATION_BEEP_DURATION 2` = **20 ms**, off `BEEPER_CONFIRMATION_BEEP_GAP_DURATION 20` = **200 ms**). The 200 ms gap also follows the last beep.
* **`beeperWarningBeeps(code)`** (arming refused). `WARNING_FLASH_DURATION_MS 50`, `WARNING_FLASH_COUNT 5`, `WARNING_PAUSE_DURATION_MS 500`, `WARNING_CODE_DURATION_LONG_MS 250`, `WARNING_CODE_DURATION_SHORT_MS 50`. The pattern is:
  1. 4 × (ON 50, off 50), with the last off stretched to 500.
  2. (code/5) × (ON 250, off 250), with the last off 500.
  3. (code%5) × (ON 50, off 250), with no trailing off.

  `code = ffs(armingDisableFlags)` (`BF src/main/fc/core.c#L681-L685`). Examples: RX_FAILSAFE (1<<2) gives code 3, THROTTLE (1<<7) gives code 8, ANGLE (1<<8) gives code 9 (`BF src/main/fc/runtime_config.h`). It plays only when the reason changes.
* **GPS sat count**: numSat × (ON 50, off 100), last off 500 (`beeper.c#L362-L376`).

## 2.3 Mode → pattern → priority table (verbatim)

`BF src/main/io/beeper.c#L224-L250`:

```c
// IMPORTANT: these are in priority order, 0 = Highest
static const beeperTableEntry_t beeperTable[] = {
    { BEEPER_ENTRY(BEEPER_GYRO_CALIBRATED,       0, beep_gyroCalibrated,   "GYRO_CALIBRATED") },
    { BEEPER_ENTRY(BEEPER_RX_LOST,               1, beep_txLostBeep,       "RX_LOST") },
    { BEEPER_ENTRY(BEEPER_RX_LOST_LANDING,       2, beep_sos,              "RX_LOST_LANDING") },
    { BEEPER_ENTRY(BEEPER_DISARMING,             3, beep_disarmBeep,       "DISARMING") },
    { BEEPER_ENTRY(BEEPER_ARMING,                4, beep_armingBeep,       "ARMING")  },
    { BEEPER_ENTRY(BEEPER_ARMING_GPS_FIX,        5, beep_armingGpsFix,     "ARMING_GPS_FIX") },
    { BEEPER_ENTRY(BEEPER_ARMING_GPS_NO_FIX,     6, beep_armingGpsNoFix,   "ARMING_GPS_NO_FIX") },
    { BEEPER_ENTRY(BEEPER_BAT_CRIT_LOW,          7, beep_critBatteryBeep,  "BAT_CRIT_LOW") },
    { BEEPER_ENTRY(BEEPER_BAT_LOW,               8, beep_lowBatteryBeep,   "BAT_LOW") },
    { BEEPER_ENTRY(BEEPER_GPS_STATUS,            9, beep_multiBeeps,       "GPS_STATUS") },
    { BEEPER_ENTRY(BEEPER_RX_SET,                10, beep_shortBeep,       "RX_SET") },
    { BEEPER_ENTRY(BEEPER_ACC_CALIBRATION,       11, beep_2shortBeeps,     "ACC_CALIBRATION") },
    { BEEPER_ENTRY(BEEPER_ACC_CALIBRATION_FAIL,  12, beep_2longerBeeps,    "ACC_CALIBRATION_FAIL") },
    { BEEPER_ENTRY(BEEPER_READY_BEEP,            13, beep_readyBeep,       "READY_BEEP") },
    { BEEPER_ENTRY(BEEPER_MULTI_BEEPS,           14, beep_multiBeeps,      "MULTI_BEEPS") }, // FIXME This entry must not be called directly.
    { BEEPER_ENTRY(BEEPER_DISARM_REPEAT,         15, beep_disarmRepeatBeep,"DISARM_REPEAT") },
    { BEEPER_ENTRY(BEEPER_ARMED,                 16, beep_armedBeep,       "ARMED") },
    { BEEPER_ENTRY(BEEPER_SYSTEM_INIT,           17, NULL,                 "SYSTEM_INIT") },
    { BEEPER_ENTRY(BEEPER_USB,                   18, NULL,                 "ON_USB") },
    { BEEPER_ENTRY(BEEPER_BLACKBOX_ERASE,        19, beep_2shortBeeps,     "BLACKBOX_ERASE") },
    { BEEPER_ENTRY(BEEPER_CRASHFLIP_MODE,        20, beep_2longerBeeps,    "CRASHFLIP") },
    { BEEPER_ENTRY(BEEPER_CAM_CONNECTION_OPEN,   21, beep_camOpenBeep,     "CAM_CONNECTION_OPEN") },
    { BEEPER_ENTRY(BEEPER_CAM_CONNECTION_CLOSE,  22, beep_camCloseBeep,    "CAM_CONNECTION_CLOSE") },
    { BEEPER_ENTRY(BEEPER_ALL,                   23, NULL,                 "ALL") },
};
```

SYSTEM_INIT and USB have `NULL` sequences: `beeper()` ignores them (`#L280-L283`). SYSTEM_INIT is played directly in `init.c` (see 2.4). USB is only an off-flag, meaning "silence on USB power with no battery" (`#L97-L109`). The enum and its comments are in `BF src/main/io/beeper.h#L35-L64`.

## 2.4 What plays when

| Event | Trigger (source) | Sound |
|---|---|---|
| **Power-up "chirp"** | `BF src/main/fc/init.c#L773-L790`: `for (int i = 0; i < 10; i++) { … delay(25); … BEEP_ON; delay(25); BEEP_OFF; }`. It bypasses `beeper()` and honours the SYSTEM_INIT / USB off-flags. | **10 × (off 25 ms, ON 25 ms) = 500 ms**: a 20 Hz buzz at the buzzer's own pitch. It plays during init. **[UNVERIFIED]** the exact ms after power-on depends on hardware init. |
| **Gyro calibration done** | Calibration starts at `init.c#L868` `gyroStartCalibration(false)`, after the chirp. Duration `gyroCalibrationDuration = 125; // 1.25 seconds` (`BF src/main/sensors/gyro.c#L123`). It restarts if the craft moves (stddev > threshold). When done, `beeper(BEEPER_GYRO_CALIBRATED)` (`gyro.c#L263-L267`). | **3 × (ON 200, off 100)** at priority 0 (highest). |
| **Arming** | `tryArm()` (`BF src/main/fc/core.c#L652-L667`): `beeper(BEEPER_ARMING)`. With the GPS feature on: `ARMING_GPS_FIX` / `ARMING_GPS_NO_FIX`. | **ON 300, off 50, ON 50, off 50**. |
| Arming refused | `core.c#L678-L688` → `beeperWarningBeeps(ffs(flags))` | warning code (2.2) |
| **Disarming** | `disarm()` (`core.c#L555-L558`): `beeper(BEEPER_DISARMING)`, unless RUNAWAY_TAKEOFF / CRASH_DETECTED | **ON 150, off 50, ON 150, off 50** |
| Disarm stick held while disarmed | `BF src/main/fc/rc_controls.c#L202-L212`: `beeper(BEEPER_DISARM_REPEAT)` + `repeatAfter(STICK_AUTOREPEAT_MS)` (250) | off 1000, ON 100 |
| Armed, idle, throttle low | `core.c#L971-L1013`: `beeper(BEEPER_ARMED)`, only if `FEATURE_MOTOR_STOP` is on, airmode is off, not 3D, not fixed-wing. Called every RX-task run. | ON 100 every ≈2.6 s |
| **Flight-mode change** | `BF src/main/fc/runtime_config.c#L105-L131`: `enableFlightMode()` / `disableFlightMode()` call `beeperConfirmationBeeps(1)` whenever `flightModeFlags` actually changes. Covers ANGLE, HORIZON, MAG, ALT_HOLD, POS_HOLD, HEADFREE, CHIRP, PASSTHRU, FAILSAFE, GPS_RESCUE, AUTOPILOT (`runtime_config.h` `flightModeFlags_e`). E.g. `core.c#L1055-L1059` (ANGLE), `#L1207-L1213` (HORIZON). | **One ON 20 ms** (then 200 ms quiet), priority 14. Modes that are boxes but **not** flight-mode flags (AIRMODE, BEEPER, CRASHFLIP/turtle, etc.) give **no** chirp. |
| PID profile change | `BF src/main/config/config.c#L847` `beeperConfirmationBeeps(pidProfileIndex + 1)` | n × (ON 20, off 200) |
| Save settings / stick trims | `config.c#L795`, `rc_controls.c#L356`: `beeperConfirmationBeeps(1)` | ON 20 |
| Adjustment (inflight) | `BF src/main/fc/rc_adjustments.c#L291` `beeperConfirmationBeeps(delta > 0 ? 2 : 1)`, `#L677` | 1 or 2 × ON 20 |
| **LOW BATTERY** (`BAT_LOW`) | Battery-alerts task at 5 Hz (`tasks.c#L372` `TASK_PERIOD_HZ(5)`) → `updateBatteryBeeperAlert()` → `beeper(BEEPER_BAT_LOW)` while state = WARNING (`BF src/main/sensors/battery.c#L186-L200`). WARNING when `displayFiltered <= cells*vbatwarningcellvoltage - vbathysteresis` (defaults **350** = 3.50 V/cell, hysteresis 1 = 0.01 V; `battery.c#L117,#L142,#L217-L220,#L288`). | **ON 250, off 500**, repeated. **[derived]** Re-triggered on the 200 ms task grid, so steady state is ≈ON 250 / off ≈550 (period ≈0.8 s). |
| **LAND NOW** (`BAT_CRIT_LOW`) | Same path; CRITICAL when `<= cells*vbatmincellvoltage - hysteresis`, with `VBAT_CELL_VOLTAGE_DEFAULT_MIN 330` = 3.30 V/cell (`BF src/main/sensors/battery.h#L35`) | **ON 500, off 20**, repeated. **[derived]** ≈ON 500 / off ≈100 (period ≈0.6 s), nearly continuous. |
| **RX lost** | `BF src/main/flight/failsafe.c#L301-L304`: `if (!receivingRxData && !mspSerialIsConfiguratorActive()) beeperMode = BEEPER_RX_LOST;` → `beeper()` at `#L534-L535`, every failsafe tick (10 ms, `#L278-L279`). | **ON 500, off 500**, continuously while the link is down. |
| — when it starts | `receivingRxData` = `rxLinkState == FAILSAFE_RXLINK_UP` (`#L162-L168`). The link goes DOWN when `millis() - validRxDataReceivedAt > rxDataFailurePeriod` (`#L224-L233`). That period is `failsafe_delay * 100 ms` with default `.failsafe_delay = 15, // 1.5 sec stage 1 period` (`#L84`, `#L105`). | **≈1.5 s after the last valid RX frame**, i.e. when **failsafe stage 2** begins, not at stage 1. Monitoring only starts after `FAILSAFE_POWER_ON_DELAY_US (1000*1000*5)` (`failsafe.h#L25`, `core.c#L838-L840`). With the TX off at power-up, RX_LOST starts ≈5 s after FC boot. It is silent while a configurator is connected over MSP. |
| RX lost while landing / rescue | `failsafe.c#L402-L409` (and GPS-rescue/autopilot branches): `BEEPER_RX_LOST_LANDING` while armed in FAILSAFE_LANDING. The default procedure is **`FAILSAFE_PROCEDURE_DROP_IT`** (`#L87`), so by default it disarms and you hear RX_LOST, not SOS. | SOS (3.9 s) |
| **Crash flip (turtle)** | On arming with BOXCRASHFLIP active, `crashFlipModeActive` is set (`core.c#L608`). Then each RX-task run does `if (crashFlipModeActive) beeper(BEEPER_CRASHFLIP_MODE);` (`core.c#L1032-L1036`). | **ON 200, off 150, ON 350, off 50**, repeating while armed in crash-flip. Priority 20, so the ARMING beep (450 ms) plays first. |
| Beeper switch (BOXBEEPERON) | `beeper.c#L428-L430` → `BEEPER_RX_SET` every 10 ms | ON 100 / off 100, continuous |
| GPS ready | `BF src/main/io/gps.c#L1603` `beeper(BEEPER_READY_BEEP)` | readyBeep |
| Blackbox erase | `BF src/main/blackbox/blackbox.c#L2195,#L2201`; `cli.c#L3243` | 2 × 50 ms |
| Inflight ACC cal | `core.c#L705-L710`, `acceleration_init.c#L499` | 2short / 2longer |
| Cam (RunCam 5-key) | `BF src/main/io/rcdevice_cam.c#L154-L161` | camOpen / camClose |

FC-side "can I arm yet" gate: `.powerOnArmingGraceTime = 5` (`BF src/main/config/config.c#L120`, CLI `pwr_on_arm_grace`). `core.c#L318-L330` clears `ARMING_DISABLED_BOOT_GRACE_TIME` only once `millis() >= 5000`, and with DShot also once `dshotStreamingCommandsAreEnabled()`.

Mute: `BOXBEEPERMUTE` silences everything (`beeper.c#L272-L277`).

## 2.5 The buzzer itself

Defaults, `BF src/main/pg/beeper_dev.c#L33-L54`:

```c
#ifdef BEEPER_INVERTED
#define IS_OPEN_DRAIN   false
#define IS_INVERTED     true
#else
#define IS_OPEN_DRAIN   true
#define IS_INVERTED     false
#endif
#ifndef BEEPER_PWM_HZ
#define BEEPER_PWM_HZ   0
#endif
PG_RESET_TEMPLATE(beeperDevConfig_t, beeperDevConfig,
    .isOpenDrain = IS_OPEN_DRAIN,
    .isInverted = IS_INVERTED,
    .ioTag = IO_TAG(BEEPER_PIN),
    .frequency = BEEPER_PWM_HZ
);
```

* CLI: `beeper_inversion`, `beeper_od`, and `beeper_frequency` (range 0–16000) (`BF src/main/cli/settings.c#L1039-L1041`).
* **The default `beeper_frequency` is 0.** The beeper pin is a plain GPIO switched on and off: `IOWrite(beeperIO, beeperInverted ? onoff : !onoff)` (`BF src/main/drivers/sound_beeper.c#L39-L44`). That only works with an **active (self-oscillating) buzzer**, so **the pitch is the buzzer's own resonance and the FC only gates it**.
* A non-zero `beeper_frequency` drives a passive piezo with a 50%-duty square wave at that frequency: `ccr = (PWM_TIMER_1MHZ / freqBeep) / 2` (`BF src/platform/common/stm32/pwm_output_beeper.c#L33-L46,#L63`).
* `beeper_inversion` / `beeper_od` defaults depend on whether the board target defines `BEEPER_INVERTED`. Most FCs drive the buzzer through a transistor and define it, which gives inversion ON, OD OFF. **[UNVERIFIED]** for the Meteor65 Pro's FC. **[UNVERIFIED]** whether the Meteor65 Pro FC has a buzzer at all; tiny whoops often have none.

Typical active-buzzer pitch, from manufacturer datasheets and product pages:

* **12 mm active (the "1209x"/"12085" size class)**: Same Sky (ex-CUI) **CMI-1295-0585T**, "magnetic buzzer indicator… internally driven". Rated voltage 5 Vdc (4–7 V), 30 mA, **rated frequency 2,000 / 2,300 / 2,600 Hz (min/typ/max)**, 85 dB at 10 cm, 12 × 9.5 mm. Datasheet: https://www.sameskydevices.com/product/resource/cmi-1295-0585t.pdf
* **9.6 mm active**: Same Sky **CMI-9605IC-0580T**, "internally driven… narrow frequency range". 5 Vdc (3–7 V), 30 mA, **rated frequency 2,600 / 2,700 / 2,800 Hz**, 80 dB at 10 cm, 9.6 × 5.0 mm. Datasheet: https://www.sameskydevices.com/product/resource/cmi-9605ic-0580t.pdf
* Seeed Grove Buzzer (active, 3.3/5 V): "Resonant Frequency 2300±300Hz", "≥85dB". https://wiki.seeedstudio.com/Grove-Buzzer/
* Adafruit #1536 "Buzzer 5V – Breadboard friendly" (active, 12 mm × 9.7 mm): "driver circuitry that makes it oscillate at 2KHz". https://www.adafruit.com/product/1536

**For the synth**: an FC active buzzer is roughly a **fixed ≈2.3 kHz (12 mm) or ≈2.7 kHz (9 mm) tone**, gated by the patterns above. **[UNVERIFIED]** the exact part on any particular FPV FC; "12085"/"9025" sellers rarely publish datasheets.

## 2.6 DShot beacon (FC → ESC)

* `dshotBeaconTone = 1` and `dshotBeaconOffFlags = DEFAULT_DSHOT_BEACON_OFF_FLAGS` (`BF src/main/pg/beeper.c#L34-L38`). CLI `beeper_dshot_beacon_tone` takes 1..`DSHOT_CMD_BEACON5` (`settings.c#L1045`).
* Commands: `DSHOT_CMD_MOTOR_STOP = 0, DSHOT_CMD_BEACON1, … DSHOT_CMD_BEACON5` = 1..5 (`BF src/main/drivers/dshot_command.h#L35-L40`).
* **Default is OFF.** `#define DEFAULT_DSHOT_BEACON_OFF_FLAGS DSHOT_BEACON_ALLOWED_MODES` (= RX_SET | RX_LOST) for non-`USE_RACE_PRO` builds (`BF src/main/io/beeper.h#L94-L109`). Both beacon triggers are therefore disabled until the user enables them, e.g. via the configurator's DShot-beacon checkboxes or the CLI `beacon` command. RACE_PRO builds turn off only RX_LOST.
* When enabled (`beeper.c#L438-L481`), it fires only when `!areMotorsRunning()` and one of these holds:
  * **RX_LOST**: `failsafeIsMonitoring() && !failsafeIsReceivingRxData() && !mspSerialIsConfiguratorActive()`, i.e. the same stage-2 condition as the RX_LOST piezo beep.
  * **RX_SET**: the BOXBEEPERON switch is on and the RX link is up.
* It is also blocked for `DSHOT_BEACON_GUARD_DELAY_US 1200000` (1.2 s) after disarm, and while trying to arm. It sends `dshotCommandWrite(ALL_MOTORS, …, dshotBeaconTone, DSHOT_CMD_TYPE_INLINE)` at most every `DSHOT_BEACON_MODE_INTERVAL_US 450000` (0.45 s) (`beeper.h#L29-L32`). The comment reads "the DShot Beacon tone duration is determined by the ESC, and should not exceed 250ms".
* **Mapping to Bluejay**: DSHOT_CMD_BEACONn plays Bluejay `beep_fn` at beacon strength 80 (1.8). With the default tone 1 that is **`beep_f1`: ≈490 Hz for ≈108 ms**, then the ESC pauses 200 ms. **[derived]** The result is one ≈108 ms, ≈490 Hz blip every ≈450 ms from every motor at once.

## 2.7 Betaflight items I could not verify

* Exact time from FC power-on to the init chirp and to motor output (`motorEnable()` at `init.c#L1053`). It depends on board init, so the source gives no figure.
* The Meteor65 Pro FC's target defines (buzzer pin, `BEEPER_INVERTED`, `DEFAULT_MOTOR_DSHOT_SPEED`). The generic default is DShot600: `motorConfig->dev.motorProtocol = MOTOR_PROTOCOL_DSHOT600` unless the target overrides it (`BF src/main/pg/motor.c#L60-L67`).
