# Input Devices: how the DualSense and Radiomaster Pocket appear on each OS, and which Rust input stack reads them

Research for [issue #3](https://github.com/BartoszSolkaBD/OpenDrone/issues/3), part of the [alpha spec map](https://github.com/BartoszSolkaBD/OpenDrone/issues/1). Researched on 2026-10-03 against primary sources: firmware, kernel, SDL and crate source code at pinned tags, plus vendor documentation. Secondary sources are labelled as such. Anything not confirmed from a primary source is marked **unverified**.

Vocabulary follows [CONTEXT.md](../../CONTEXT.md) and [docs/context/input.md](../context/input.md). An **Input Device** is a **Radio** (the Radiomaster Pocket) or a **Gamepad** (the DualSense).

## The answer in brief

**Recommendation: build our own small input layer and back it with SDL 3.4.**

- **Our own input layer.** This is an engine-free crate. It reads both Input Devices on a dedicated thread and gives every sample a timestamp. It applies no filtering. Each physics sub-step reads the latest sample, so the physics rate no longer depends on the frame rate. Bevy's built-in gamepad support (`bevy_gilrs`) is not used for flying. It can stay for menu navigation, or be turned off.
- **SDL 3.4 underneath.** We use it through the `sdl3-sys` crate, built from source, with only the joystick and HIDAPI parts. SDL is the only candidate that meets all four needs:
  - It reads the DualSense over USB and Bluetooth on all three desktop OSes.
  - It reads the Pocket as a generic joystick on all three desktop OSes.
  - It has no 125 Hz cap and no window-focus requirement on Windows.
  - It also runs on iOS and Android, which keeps the mobile door open.
- **Fallback: gilrs on our own thread.** gilrs is already very good on Linux, where it uses kernel timestamps, and on macOS, where it reacts the moment a value changes. On Windows it is capped at 125 Hz, needs window focus, and has an open DualSense Bluetooth bug. It has no mobile support.
- **Proven on the dev Mac by a prototype ([#18](https://github.com/BartoszSolkaBD/OpenDrone/issues/18)).** Through SDL on its own thread, beside a Bevy window, the Pocket delivered 1 kHz at 11 bits and the DualSense 250 Hz over USB at 8 bits. macOS showed no permission prompt. The prototype also corrected how samples get their timestamps. See [prototype results](#prototype-results-issue-18).

**Key facts**

| | Radiomaster Pocket (EdgeTX USB joystick) | DualSense |
|---|---|---|
| Stick resolution | About 11 bits (0–2048, centre 1024) | **8 bits** (0–255): a hardware limit |
| Axes | 8 (CH1–CH8) | 4 stick axes, plus 2 triggers (8-bit) |
| Buttons | 24 (CH9–CH32, pressed when above 0) | 15 or more, plus a hat, touchpad and motion sensors |
| Report rate | **1000 Hz, but only with its RF modules off.** Otherwise it runs at the RF module's rate. | **USB 250 Hz**; Bluetooth about 800–1000 Hz |
| Seen by Apple's GameController framework | No | Yes (macOS 11.3+) |
| Seen as an Xbox-style Gamepad on Windows | No (generic HID joystick) | No (generic HID game controller) |
| Linux kernel driver | `hid-generic` / `hid-input` | `hid-playstation` (kernel 5.12+) |

## Radiomaster Pocket (Radio, EdgeTX USB joystick)

### Firmware and mode

- **EdgeTX version.** The latest stable release is **v2.12.4** (2026-09-02). The Pocket has been supported since 2.10.0, and Radiomaster ships it with **2.10.5** (its factory SD packages dated 2025-03-04). 2.10 works too: see [Binding and calibration facts](#binding-and-calibration-facts-issue-19).
- **The Pocket only has the "Classic" joystick mode on EdgeTX 2.11.0 and later.**
  - The Pocket's MCU is an STM32F407xE with 512 KB of flash ([CMakeLists.txt#L347](https://github.com/EdgeTX/edgetx/blob/def35ad324896b45d6607d4778536b1bc5360d20/radio/src/targets/taranis/CMakeLists.txt#L347)).
  - For that chip, the build turns "Advanced" joystick mode off with `set(USBJ_EX OFF)` ([#L544-L549](https://github.com/EdgeTX/edgetx/blob/def35ad324896b45d6607d4778536b1bc5360d20/radio/src/targets/taranis/CMakeLists.txt#L544-L549)).
  - This was deliberate, done in [PR #5443](https://github.com/EdgeTX/edgetx/pull/5443) and confirmed by a maintainer in [issue #6434](https://github.com/EdgeTX/edgetx/issues/6434).
- **What Advanced mode would offer.** It arrived in 2.9.0 and works on radios with 1 MB of flash. It allows remapping channels to axes, setting a Joystick or Gamepad device type, per-channel invert, and "sim" axes ([manual](https://manual.edgetx.org/edgetx-how-to/configure-advanced-joystick-with-edgetx)). **We cannot rely on it for the Pocket.**

### What the computer receives

**HID report descriptor** ([usbd_hid_joystick.c#L105-L135](https://github.com/EdgeTX/edgetx/blob/def35ad324896b45d6607d4778536b1bc5360d20/radio/src/targets/common/arm/stm32/usbd_hid_joystick.c#L105-L135)):
- Usage Page Generic Desktop, Usage **Game Pad (0x05)**.
- 24 one-bit buttons.
- 8 axes in this order: **X, Y, Z, Rx, Ry, Rz, Slider, Dial**. Each is 16 bits wide with a logical range of **0–2048**.
- There is no report ID. Each report is 19 bytes.

**How values are encoded** ([usb_driver.cpp#L267-L294](https://github.com/EdgeTX/edgetx/blob/def35ad324896b45d6607d4778536b1bc5360d20/radio/src/targets/common/arm/stm32/usb_driver.cpp#L267-L294)):
- **Axes:** `limit(0, channelOutputs[i] + 1024, 2048)`. Channel output ±100% maps to 0 and 2048, which is about 11 bits of resolution.
- **Buttons:** button *n* is pressed when channel output CH(8+*n*) is above 0.
- So the joystick carries **CH1–CH8 as axes and CH9–CH32 as on/off buttons**.

**What the sim sees is the model's mixer output, not the raw sticks.** `channelOutputs[i] = applyLimits(i, q)` ([mixer.cpp#L1251-L1253](https://github.com/EdgeTX/edgetx/blob/def35ad324896b45d6607d4778536b1bc5360d20/radio/src/mixer.cpp#L1251-L1253)). So everything the active model applies reaches the sim:
- the model's curves, subtrim, endpoints (Limits) and reverse;
- trainer input and channel overrides.

Anything beyond ±100% is clamped.

**Default channel order is AETR** on Pocket builds ([input_mapping.cpp#L47-L72](https://github.com/EdgeTX/edgetx/blob/def35ad324896b45d6607d4778536b1bc5360d20/radio/src/input_mapping.cpp#L47-L72), [CMakeLists.txt#L423-L428](https://github.com/EdgeTX/edgetx/blob/def35ad324896b45d6607d4778536b1bc5360d20/radio/src/CMakeLists.txt#L423-L428)). On a fresh model this gives:

| Channel | Function | HID axis |
|---|---|---|
| CH1 | Aileron (roll) | X |
| CH2 | Elevator (pitch) | Y |
| CH3 | Throttle | Z |
| CH4 | Rudder (yaw) | Rx |

Switches only reach the sim if the pilot mixes them into a channel, and how they arrive depends on the channel:
- **CH5–CH8 arrive as axes.** A 3-position switch becomes three axis values.
- **CH9 and above arrive as on/off buttons.**

### Report rate

- **The USB polling interval is 1 ms** ([usbd_hid_joystick.c#L224](https://github.com/EdgeTX/edgetx/blob/def35ad324896b45d6607d4778536b1bc5360d20/radio/src/targets/common/arm/stm32/usbd_hid_joystick.c#L224)).
- **One report is sent per mixer cycle** ([mixer_task.cpp#L194-L198](https://github.com/EdgeTX/edgetx/blob/def35ad324896b45d6607d4778536b1bc5360d20/radio/src/tasks/mixer_task.cpp#L194-L198)). If the previous report hasn't gone out yet, the new one is dropped.
- **The mixer runs at 1 kHz in joystick mode, but only when no RF module has set its own period** ([mixer_scheduler.h#L27](https://github.com/EdgeTX/edgetx/blob/def35ad324896b45d6607d4778536b1bc5360d20/radio/src/mixer_scheduler.h#L27), [mixer_scheduler.cpp#L69-L86](https://github.com/EdgeTX/edgetx/blob/def35ad324896b45d6607d4778536b1bc5360d20/radio/src/mixer_scheduler.cpp#L69-L86)).
- The Pocket's internal module is ELRS/CRSF. **If it is switched on, the joystick updates at the ELRS packet rate instead.**
- The EdgeTX manual says: "Both internal and external RF modules should be turned off … The mixer runs at 1000Hz" ([manual](https://manual.edgetx.org/color-radios/model-settings/model-setup/usb-joystick)).

### Hardware

- **MCU and ADC:** STM32F407xE with a 12-bit ADC ([stm32_adc.cpp#L254-L258](https://github.com/EdgeTX/edgetx/blob/def35ad324896b45d6607d4778536b1bc5360d20/radio/src/targets/common/arm/stm32/stm32_adc.cpp#L254-L258)).
- **Gimbals:** the current (M2) Pocket has Hall-effect gimbals ([product page](https://radiomasterrc.com/products/pocket-radio-controller-m2)).
- **Controls:** 4 stick channels, one pot and switches SA–SE (from the board's `hal.h`).

### Identity quirks

- **Every EdgeTX and OpenTX radio shares one USB ID:** VID `0x1209`, PID `0x4F54` ([usbd_desc.c#L67-L91](https://github.com/EdgeTX/edgetx/blob/def35ad324896b45d6607d4778536b1bc5360d20/radio/src/targets/common/arm/stm32/usbd_desc.c#L67-L91)).
- **The manufacturer string is "OpenTX"** on purpose, for game compatibility ([PR #4653](https://github.com/EdgeTX/edgetx/pull/4653)).
- **The product string is "Radiomaster Pocket Joystick".** That is the best way to tell this radio apart from other EdgeTX radios.
- **The USB `bcdDevice` field changes with each EdgeTX release.** Device-specific defaults should match on name and VID/PID, never on that version field.
- **Changes to the report layout break strict parsers.** In 2.11–2.12.2, radios with Advanced mode (not the Pocket) sent an extra byte, which broke PS4/PS5 Liftoff ([issue #6320](https://github.com/EdgeTX/edgetx/issues/6320)). 2.12.3 restored the old layout ([PR #7532](https://github.com/EdgeTX/edgetx/pull/7532)). **Our layer should read axes through the device's HID descriptor, never from fixed byte offsets.**

### How it appears on each OS

**Windows**
- It is a generic HID joystick, visible through DirectInput, Raw Input, the HID APIs and `Windows.Gaming.Input.RawGameController`.
- **All 8 axes fit DirectInput.** `DIJOYSTATE2` has exactly X, Y, Z, Rx, Ry, Rz and two sliders, and Slider and Dial land in the two slider slots ([Microsoft Learn](https://learn.microsoft.com/en-us/previous-versions/windows/desktop/ee416628(v=vs.85))).
- It is not an Xbox-style `Gamepad` (see [the Windows section under DualSense](#how-it-appears-on-each-os-1)).

**macOS**
- It is visible to IOKit HID. The Game Pad usage is matched by both gilrs and SDL.
- **Apple's GameController framework is not expected to expose it.** Apple lists only known controller families ([overview](https://developer.apple.com/documentation/gamecontroller)). That Apple rejects generic HID joysticks is **unverified**; it is consistent with SDL's use of `supportsHIDDevice`.

**Linux**
- The kernel's `hid-input` maps the axes as follows ([hid-input.c#L795-L811](https://github.com/torvalds/linux/blob/e767a4ea70a3992c37ed604157d32f0dfbf9b1e3/drivers/hid/hid-input.c#L795-L811)):
  - X..Rz → `ABS_X`..`ABS_RZ`
  - Slider → `ABS_THROTTLE`
  - Dial → `ABS_RUDDER`
- Buttons map to `BTN_GAMEPAD`+*n*, then to `BTN_TRIGGER_HAPPY` codes.
- **No root access is needed.** systemd grants the logged-in user access to joystick event nodes ([70-uaccess.rules.in](https://github.com/systemd/systemd/blob/main/rules.d/70-uaccess.rules.in)).

## DualSense (Gamepad)

### Device basics

- **USB IDs:** VID `0x054C`. The DualSense is PID `0x0CE6`; the DualSense Edge is `0x0DF2` ([hid-ids.h](https://github.com/torvalds/linux/blob/e767a4ea70a3992c37ed604157d32f0dfbf9b1e3/drivers/hid/hid-ids.h#L1341-L1350)).
- **Report formats** ([hid-playstation.c#L76-L95](https://github.com/torvalds/linux/blob/e767a4ea70a3992c37ed604157d32f0dfbf9b1e3/drivers/hid/hid-playstation.c#L76-L95), [#L1449-L1468](https://github.com/torvalds/linux/blob/e767a4ea70a3992c37ed604157d32f0dfbf9b1e3/drivers/hid/hid-playstation.c#L1449-L1468)):
  - Over USB it sends its full report as report 0x01 (64 bytes).
  - Over Bluetooth it starts with a minimal report 0x01. It switches to the full report 0x31 (78 bytes, with a CRC32) once software reads a calibration or firmware feature report. **It stays in the full mode until the controller is power-cycled** ([SDL_hints.h](https://github.com/libsdl-org/SDL/blob/release-3.4.18/include/SDL3/SDL_hints.h)).
- **Sticks and triggers are 8-bit** (`u8 x, y, rx, ry, z, rz`, [hid-playstation.c#L245-L267](https://github.com/torvalds/linux/blob/e767a4ea70a3992c37ed604157d32f0dfbf9b1e3/drivers/hid/hid-playstation.c#L245-L267)). Linux reports them as 0–255 ([#L766-L778](https://github.com/torvalds/linux/blob/e767a4ea70a3992c37ed604157d32f0dfbf9b1e3/drivers/hid/hid-playstation.c#L766-L778)).
  - SDL scales them by ×257 ([SDL_hidapi_ps5.c#L1254](https://github.com/libsdl-org/SDL/blob/release-3.4.18/src/joystick/hidapi/SDL_hidapi_ps5.c#L1254)), which changes the range but not the number of steps.
  - **That gives 256 steps over full travel, about 128 per half.** This is a hardware limit that no software stack can raise.
- **Motion sensors:** the gyro and accelerometer are 16-bit and carry their own timestamps in 1/3 µs units.

### Report rate

**From SDL's source:**
- "Standard DualSense sensor update rate is 250 Hz over USB".
- "Bluetooth sensor update rate appears to be 1000 Hz".
- "DualSense Edge … 1000 Hz over USB".

All three quotes are from [SDL_hidapi_ps5.c#L851-L859](https://github.com/libsdl-org/SDL/blob/release-3.4.18/src/joystick/hidapi/SDL_hidapi_ps5.c#L851-L859). Sony publishes no figures.

**Secondary measurements** from [gamepadla.com](https://gamepadla.com/sony-dualsense.html) on Windows 11:
- USB: 250 Hz, about 7.4 ms average stick latency.
- Bluetooth: about 800 Hz, about 5.0 ms average, with spikes to about 16 ms.

**So the standard DualSense is slower over USB than over Bluetooth.**

### How it appears on each OS

**Linux**
- The `hid-playstation` driver has handled it over USB and Bluetooth since kernel 5.12.
- The driver creates three event devices: the gamepad, "Motion Sensors" and "Touchpad" ([#L1822-L1846](https://github.com/torvalds/linux/blob/e767a4ea70a3992c37ed604157d32f0dfbf9b1e3/drivers/hid/hid-playstation.c#L1822-L1846)).
- The gamepad device is readable without root through systemd's `uaccess` rule.
- Raw access through hidraw (SDL's preferred path) needs an extra udev rule, such as Valve's [`60-steam-input.rules`](https://github.com/ValveSoftware/steam-devices/blob/22ec85e5ff5ea2e15c56d71a41bcbef46356cd60/60-steam-input.rules#L40-L50). If hidraw isn't readable, SDL falls back to the event device, which still works.

**macOS**
- Apple supports it natively over USB and Bluetooth from macOS 11.3 ([GCDualSenseGamepad](https://developer.apple.com/documentation/gamecontroller/gcdualsensegamepad), [Apple support](https://support.apple.com/en-us/111100)).
- **The GameController framework sends no input while the app is in the background** unless `shouldMonitorBackgroundEvents` is set ([Apple docs](https://developer.apple.com/documentation/gamecontroller/gccontroller/shouldmonitorbackgroundevents)).
- IOKit HID still sees the device, and both SDL and gilrs read it alongside the framework. SDL prefers its own HIDAPI driver ([SDL_mfijoystick.m#L371-L394](https://github.com/libsdl-org/SDL/blob/4a17e772ea9a4ff77dd10fa6ea6f45fc6731eb01/src/joystick/apple/SDL_mfijoystick.m#L371-L394)).
- **Trap: the `hidapi` crate opens devices exclusively on macOS by default** ([hidapi-rs lib.rs#L59-L61](https://github.com/ruabmbua/hidapi-rs/blob/8729463f8a96b6d4671b7ef114b5ead65ccd7469/src/lib.rs#L59-L61)). SDL turns this off for itself.

**Windows**
- **It is not a `Windows.Gaming.Input.Gamepad`.** Microsoft says that class "supports any GIP … or XUSB compatible gamepad" ([docs](https://learn.microsoft.com/en-us/uwp/api/windows.gaming.input.gamepad)).
- It appears as a generic HID game controller instead: through DirectInput, the HID APIs and `RawGameController` (that last one is inferred).
- **SDL reads it directly through its HIDAPI PS5 driver,** over USB and Bluetooth.
- **SDL deliberately leaves a Bluetooth pad in its basic mode.** Switching it to the full mode "break[s] Windows DirectInput for other applications that don't use SDL" ([SDL_hints.h](https://github.com/libsdl-org/SDL/blob/release-3.4.18/include/SDL3/SDL_hints.h)).
- **Steam Input can interfere.** It hooks XInput, DirectInput, Raw Input and Windows.Gaming.Input and injects an emulated Xbox controller ([Steamworks](https://partner.steamgames.com/doc/features/steam_controller/steam_input_gamepad_emulation_bestpractices)).

## Rust input crates

### gilrs 0.11.2 (2026-05-30)

This is Bevy's default gamepad backend. It is maintained, but by effectively one maintainer ([changelog](https://gitlab.com/gilrs-project/gilrs/-/blob/v0.11.2/gilrs/CHANGELOG.md)).

**How it reads devices on each platform**

| | Linux | macOS | Windows |
|---|---|---|---|
| How it reads | Reads the event devices on the caller's thread | IOKit HID callbacks on a background thread, delivered immediately ([gamepad.rs#L51-93](https://gitlab.com/gilrs-project/gilrs/-/blob/v0.11.2/gilrs-core/src/platform/macos/gamepad.rs#L51-93)) | **Polls Windows.Gaming.Input every 8 ms (about 125 Hz)** ([gamepad.rs#L31-35](https://gitlab.com/gilrs-project/gilrs/-/blob/v0.11.2/gilrs-core/src/platform/windows_wgi/gamepad.rs#L31-35)) |
| Timestamp | **The kernel's own timestamp** ([gamepad.rs#L829-831](https://gitlab.com/gilrs-project/gilrs/-/blob/v0.11.2/gilrs-core/src/platform/linux/gamepad.rs#L829-831)) | Arrival time | Poll time |
| Needs window focus? | No | No | **Yes**: "a limitation of Windows Gaming Input" ([changelog](https://gitlab.com/gilrs-project/gilrs/-/blob/v0.11.2/gilrs/CHANGELOG.md), v0.10.0) |
| Sees the Pocket | Yes | Yes (Joystick, GamePad and MultiAxisController usages, [io_kit.rs#L30-40](https://gitlab.com/gilrs-project/gilrs/-/blob/v0.11.2/gilrs-core/src/platform/macos/io_kit.rs#L30-40)) | Yes (`RawGameController`) |

**Resolution is preserved.** gilrs converts raw values to an f32 between -1 and 1 without real loss. The Windows path quantises to 16 bits, which is still more than the Pocket's 11.

**Two default filters must be switched off,** with `with_default_filters(false)` ([gamepad.rs#L179-186](https://gitlab.com/gilrs-project/gilrs/-/blob/v0.11.2/gilrs/src/gamepad.rs#L179-186), [filter.rs#L84-92](https://gitlab.com/gilrs-project/gilrs/-/blob/v0.11.2/gilrs/src/ev/filter.rs#L84-92)):
- A "jitter" filter drops changes smaller than 0.01.
- A 10% stick deadzone.

**Pitfall: the same Radio axis gets a different name on each OS.** Map Radio axes by their raw code, never by gilrs's name.
- On macOS, Z (throttle, CH3) becomes "RightStickX" and Rx (yaw, CH4) is "Unknown".
- On Windows, axes 3 and 4 become trigger *buttons* ([gamepad.rs#L802-841](https://gitlab.com/gilrs-project/gilrs/-/blob/v0.11.2/gilrs-core/src/platform/windows_wgi/gamepad.rs#L802-841)).

**No mobile support.** Android and iOS fall through to a "not implemented" backend ([platform/mod.rs](https://gitlab.com/gilrs-project/gilrs/-/blob/v0.11.2/gilrs-core/src/platform/mod.rs)).

**Open issues that touch the DualSense:**
- [#184](https://gitlab.com/gilrs-project/gilrs/-/issues/184): the PS5 controller does not work over Bluetooth on Windows.
- [#162](https://gitlab.com/gilrs-project/gilrs/-/issues/162): a crash on macOS caused by a filter.
- [#193](https://gitlab.com/gilrs-project/gilrs/-/issues/193): the left stick Y axis is inverted on macOS.

### Bevy: `bevy_gilrs` and `bevy_input` (0.19.1 stable, 0.20.0-rc.2)

**Bevy reads gamepads once per frame**, in `PreUpdate`, and discards gilrs's timestamps ([lib.rs#L91-L111](https://github.com/bevyengine/bevy/blob/v0.19.1/crates/bevy_gilrs/src/lib.rs#L91-L111), [gilrs_system.rs#L48-L113](https://github.com/bevyengine/bevy/blob/v0.19.1/crates/bevy_gilrs/src/gilrs_system.rs#L48-L113)).

**Bevy 0.19.1 throws away every axis and button gilrs can't name** ([converter.rs#L24-L39](https://github.com/bevyengine/bevy/blob/v0.19.1/crates/bevy_gilrs/src/converter.rs#L24-L39)).
- With the naming above, the **Pocket's yaw axis (CH4) and CH5–CH8 never reach Bevy on macOS**, and yaw arrives as a trigger button on Windows.
- 0.20.0-rc.2 keeps unknown inputs as `GamepadAxis::Other(code)` ([PR #8560](https://github.com/bevyengine/bevy/pull/8560)). That fixes the dropping, but not the timing.

**Default `AxisSettings` filter the input** ([gamepad.rs#L1000-L1010](https://github.com/bevyengine/bevy/blob/v0.19.1/crates/bevy_input/src/gamepad.rs#L1000-L1010), [#L1279-L1284](https://github.com/bevyengine/bevy/blob/v0.19.1/crates/bevy_input/src/gamepad.rs#L1279-L1284)):
- a ±5% deadzone;
- a 0.01 change threshold. Changes below it are ignored and the old value is kept. On the Pocket that is roughly 10 of its 2,048 steps.

These can be set to zero. `RawGamepadAxisChangedEvent` is unfiltered.

**Bevy has nothing that reads input faster than the frame rate.**
- Its fixed-timestep example gathers input once per frame ([physics_in_fixed_timestep.rs](https://github.com/bevyengine/bevy/blob/v0.19.1/examples/movement/physics_in_fixed_timestep.rs)).
- Every fixed step within a frame sees the same values.
- Related open issues: [#6183](https://github.com/bevyengine/bevy/issues/6183) and [#9087](https://github.com/bevyengine/bevy/issues/9087) ("rewrite the gilrs integration so that gamepad events are polled continuously").

**`leafwing-input-manager` 0.21.0 inherits all of these limits,** because it reads Bevy's filtered values once per frame.

### SDL3 (SDL 3.4.18, 2026-10-02) through `sdl3-sys` 0.7.2 or `sdl3` 0.20.0

**Version.** The latest SDL release is [release-3.4.18](https://github.com/libsdl-org/SDL/releases/tag/release-3.4.18).

**How it reads each device:**
- **DualSense:** through SDL's HIDAPI PS5 driver, over USB and Bluetooth, on macOS, Windows, Linux and Android. HIDAPI is the highest-priority driver ([SDL_joystick.c#L51-L72](https://github.com/libsdl-org/SDL/blob/release-3.4.18/src/joystick/SDL_joystick.c#L51-L72)).
- **Pocket and other generic joysticks:**
  - macOS: IOKit.
  - Windows: DirectInput, opened in background mode ([SDL_dinputjoystick.c#L770-L772](https://github.com/libsdl-org/SDL/blob/release-3.4.18/src/joystick/windows/SDL_dinputjoystick.c#L770-L772)).
  - Linux: the event devices.

**Resolution.**
- Axes are signed 16-bit values. Axes of 16 bits or fewer scale over without loss, so the Pocket keeps its 11 bits ([SDL_iokitjoystick.c#L203-L217](https://github.com/libsdl-org/SDL/blob/release-3.4.18/src/joystick/darwin/SDL_iokitjoystick.c#L203-L217), [SDL_dinputjoystick.c#L660-L665](https://github.com/libsdl-org/SDL/blob/release-3.4.18/src/joystick/windows/SDL_dinputjoystick.c#L660-L665)).
- **One small quirk:** SDL holds each axis at its first reading until it moves more than about 1.2% of full travel ([SDL_joystick.c#L2528-L2539](https://github.com/libsdl-org/SDL/blob/release-3.4.18/src/joystick/SDL_joystick.c#L2528-L2539)). After that, every change passes through.

**Threading and focus.**
- "The SDL joystick functions are thread-safe" ([SDL_joystick.h#L178-L182](https://github.com/libsdl-org/SDL/blob/release-3.4.18/include/SDL3/SDL_joystick.h#L178-L182)).
- `SDL_UpdateJoysticks` and `SDL_GetJoystickAxis` are documented as safe from any thread in 3.4. **A dedicated input thread can therefore update and read at 1 kHz.**
- **The background-input gate only applies when SDL owns a window** ([SDL_joystick.c#L2307-L2318](https://github.com/libsdl-org/SDL/blob/release-3.4.18/src/joystick/SDL_joystick.c#L2307-L2318)). Here Bevy (winit) owns the window, so the gate does not apply.

**Mobile.**
- **iOS:** SDL uses only Apple's GameController framework ([SDL_build_config_ios.h#L147](https://github.com/libsdl-org/SDL/blob/release-3.4.18/include/build_config/SDL_build_config_ios.h#L147)). The DualSense works there; **a Radio over USB probably cannot work on iOS with any stack** (unverified).
- **Android:** SDL reads joystick-class input devices.

**Rust bindings.**
- [`sdl3-sys`](https://codeberg.org/maia/sdl3-sys-rs) 0.7.2+SDL-3.4.18:
  - It can build SDL from source with CMake.
  - It can build a slimmed SDL containing only joystick and HIDAPI.
- [`sdl3`](https://github.com/vhspace/sdl3-rs) 0.20.0 is a young safe wrapper ("Expect some bugs and missing features"). Its subsystem handles cannot move between threads, so SDL must be initialised on the input thread itself.
- SDL2 is in maintenance only, and SDL's own maintainers say new code should target SDL3 ([sdl2-compat README](https://github.com/libsdl-org/sdl2-compat/blob/main/README.md)).

**Bevy integration.** No maintained Bevy-plus-SDL input crate exists. We would write it ourselves, which we need to do anyway for the input layer.

### Raw HID and platform APIs (for reference)

- **[`hidapi`](https://github.com/ruabmbua/hidapi-rs) 2.6.7 and `async-hid` 0.5.3:** both give raw reports. We would have to parse each device's HID descriptor ourselves. Neither supports iOS. `hidapi` opens devices exclusively on macOS by default.
- **Windows `RawGameController`:** polled, with axes from 0.0 to 1.0 and a timestamp per reading ([guide](https://learn.microsoft.com/en-us/windows/uwp/gaming/raw-game-controller)). SDL's source says Windows.Gaming.Input gives no input while the window is unfocused ([SDL_windows_gaming_input.c#L862](https://github.com/libsdl-org/SDL/blob/release-3.4.18/src/joystick/windows/SDL_windows_gaming_input.c#L862)).
- **Microsoft GameInput** (NuGet 3.5.283, 2026-09-28):
  - It is a "functional superset" of XInput, DirectInput, Raw Input, HID and Windows.Gaming.Input.
  - It is lock-free and has a configurable focus policy ([overview](https://learn.microsoft.com/en-us/gaming/gdk/docs/features/common/input/overviews/input-overview)).
  - **It has no Rust bindings.** This is a possible later Windows backend.
- **Apple GameController and [`objc2-game-controller`](https://crates.io/crates/objc2-game-controller) 0.3.2:** cover Gamepads only, not Radios.

## Latency, polling, and sampling at the physics rate

**Where latency comes from.** The OpenDrone physics runs at about 1 kHz in sub-steps, and between the device and the physics there are three delays:
1. **The device's own report interval.** This is the floor:
   - Pocket: 1 ms with RF off.
   - DualSense: 4 ms over USB, about 1–1.25 ms over Bluetooth.
2. **The OS and stack delivery delay.**
3. **How often the app reads the input.**

Bevy adds a whole frame to step 3: about 22 ms at 45 fps. A dedicated input thread brings it under 1 ms.

**Sampling at the physics rate is feasible, as sample-and-hold.**
- Each physics sub-step reads the newest sample whose timestamp is not later than the sub-step's time.
- New samples arrive at the device's rate, so a 1 kHz physics loop sees a 250 Hz DualSense over USB as steps.
- **This is exactly what a real quad faces.** Its RC link (for example 250–500 Hz) is slower than its PID loop. Betaflight measures the link rate and sets its "RC smoothing" filter cutoff from it ([rc.c#L346-L360](https://github.com/betaflight/betaflight/blob/2026.6.2/src/main/fc/rc.c#L346-L360)).
- **Our Flight Controller should consume timestamped samples and apply the same RC smoothing.** That keeps the feel faithful, and no Assist is needed for this.

**Side benefit.** A stream of timestamped samples is easy to record and replay. That suits Scenarios now and input-based multiplayer later.

### Feasibility of our own input layer, per OS and Input Device

| OS | DualSense | Radiomaster Pocket |
|---|---|---|
| **Linux** | **High.** Event device from `hid-playstation`. No root needed. 250 Hz over USB, about 800–1000 Hz over Bluetooth. The kernel stamps each event, but SDL stamps sticks when it reads them ([#18](#prototype-results-issue-18)). | **High.** Event device from `hid-input`, 11-bit, 1 kHz with RF off. Same timestamp note. |
| **macOS** | **High, measured in [#18](#prototype-results-issue-18): 250 Hz over USB.** SDL's own DualSense driver reads every report, non-exclusively, alongside Apple's GameController framework. SDL stamps each report when it processes it; it doesn't use IOKit's per-value timestamp (`IOHIDValueGetTimeStamp`). | **High, measured in #18: 1 kHz.** SDL reads the Pocket's current values through IOKit each time our thread polls, and stamps them then. Apple's GameController framework ignores the Pocket, but IOKit sees it. |
| **Windows** | **Medium.** SDL's HIDAPI reads every report, with no focus requirement. Timestamps are read times, not device times. Steam Input can inject a virtual Xbox pad. Liftoff and DRL tell pilots to turn it off; Uncrashed's developer tells wireless PS5 users to turn it on ([#19](#binding-and-calibration-facts-issue-19)). | **Medium.** SDL's DirectInput path, background-capable, all 8 axes. Read-time timestamps. gilrs's Windows.Gaming.Input path would cap at 125 Hz and need focus unless patched. |

## How other simulators handle Radio calibration, endpoints and channel mapping

**Liftoff** ([official support page](https://www.liftoff-game.com/support?topic=3&category=1&post=101)):
- It ships defaults for well-known radios ("Taranis will work out of the box") and has an in-game calibration on PC.
- It tells players to disable Steam Input.
- On PS4 there is no calibration menu, so the Radio must output a fixed channel order.

**DRL, TRYP FPV, Uncrashed, VelociDrone and FPV SkyDive:** checked against first-party sources in [#19](#binding-and-calibration-facts-issue-19). Two earlier claims here were wrong:
- DRL's automatic calibration moves each stick through its full range for at least 3 s, then recentres. Rotating both sticks in circles is FPV SkyDive's method.
- Uncrashed doesn't tell pilots to disable Steam Input. Its developer tells wireless PS5 users to turn it on.

**Betaflight as the reference model** (firmware 2026.6.2):
- Channel map `AETR1234` by default ([rx.c#L119-L124](https://github.com/betaflight/betaflight/blob/2026.6.2/src/main/pg/rx.c#L119-L124)).
- Per-channel min/max range calibration, rescaled to 1000–2000.
- `midrc` 1500, and a deadband that defaults to 0 ([rc_controls.c#L80-L98](https://github.com/betaflight/betaflight/blob/2026.6.2/src/main/fc/rc_controls.c#L80-L98), [Receiver tab docs](https://betaflight.com/docs/wiki/app/receiver-tab)).

**Common pattern, as input to the "Input Device calibration and mapping" spec:**
1. **Defaults** for known devices.
2. **Calibration:** capture min, centre and max per axis by moving the sticks to their extremes, then centring them.
3. **Mapping:** assign each function (roll, pitch, yaw, throttle, arm, mode) to an axis or button, with per-axis invert.
4. **Throttle centred or at the bottom:** Gamepads centre it, Radios put it at the bottom.
5. **Optional deadband.** Expo and rates belong to the Flight Controller (Betaflight-style), not to calibration.

**Specific to the Pocket:**
- Advise pilots to fly a dedicated "sim" model on the Radio: plain 100% mixes, no expo, switches on CH5–CH8, and RF off.
- Even then, calibrate inside the sim, because the model's endpoints shape the output.

## Known gaps per OS and Input Device

| | DualSense | Radiomaster Pocket |
|---|---|---|
| **All OSes** | 8-bit sticks, a hardware limit. USB runs at 250 Hz, slower than Bluetooth. | Classic mode only on EdgeTX 2.11+ (2.10 also has Advanced mode, Classic by default): fixed CH1–8 axes and CH9–32 on/off buttons. Only 1 kHz with RF modules off. Output shaped by the model's mixer and Limits. Shares its USB ID with every EdgeTX radio. |
| **Linux** | Raw HID access needs a udev rule. The event-device path works without one. | None known. Slider and Dial arrive as `ABS_THROTTLE` and `ABS_RUDDER`. |
| **macOS** | No Input Monitoring prompt over USB (verified in [#18](#prototype-results-issue-18)). A Bluetooth pad switched to the full report by another app may confuse descriptor-based readers like gilrs (inferred). | No Input Monitoring prompt (verified in #18). Apple's GameController framework doesn't support it, so IOKit is required. |
| **Windows** | Steam Input interference. gilrs over Bluetooth is broken (#184). SDL is fine. | No native device timestamps. gilrs/Windows.Gaming.Input means 125 Hz and focus required. |
| **iOS** (roadmap) | Works through Apple's GameController framework (SDL). | Probably impossible with any stack (**unverified**). |
| **Android** (roadmap) | SDL HIDAPI/InputDevice. gilrs has no support. | SDL InputDevice over USB OTG, probably (**unverified**). |

## What the prototype must prove

Before the recommendation is locked, a small prototype should confirm these points on the dev Mac and on a Windows machine:

1. SDL's joystick subsystem can initialise and run on a non-main input thread on macOS while winit owns the main thread and the window. SDL only documents that video must be on the main thread.
2. Opening the DualSense and the Pocket through SDL on macOS triggers no Input Monitoring prompt.
3. Measured sample rates match expectations: the Pocket at about 1 kHz with RF off, the DualSense at about 250 Hz over USB and about 800 Hz or more over Bluetooth. The prototype should also confirm that all 8 Pocket axes and its buttons arrive on each OS.
4. Building SDL from source in CI works on all three OSes, and the added build time is acceptable.

If step 1 or step 4 fails, use the fallback: gilrs on our own thread, with its default filters off and axes mapped by raw code. On Windows that means either patching its 8 ms poll interval upstream or adding a small Windows backend of our own.

## Prototype results (issue #18)

Measured on the dev Mac (M4, macOS 26.6.2) on 2026-10-04 with a throwaway probe, in [#18](https://github.com/BartoszSolkaBD/OpenDrone/issues/18):
- SDL 3.4.18 through `sdl3-sys` 0.7.2, built from source and statically linked, with only the joystick and HIDAPI parts.
- SDL ran on its own thread while a Bevy 0.19.1 window owned the main thread.

The probe, its raw data and the summaries are on the branch [`prototype/sdl3-input-probe`](https://github.com/BartoszSolkaBD/OpenDrone/tree/prototype/sdl3-input-probe/prototypes/sdl3-input-probe).

| | Radiomaster Pocket (USB, RF off) | DualSense (USB) |
|---|---|---|
| Report rate | **1 kHz**: new values about 1 ms apart, up to 980 a second | **250 Hz**: gaps of exactly 4, 8 or 12 ms, on a fixed 4 ms beat |
| Resolution | **11 bits** | **8 bits**, sticks and triggers |
| What SDL exposes | 8 axes (yaw on axis 3, CH5–CH8 on axes 4–7), 24 buttons | 6 axes (4 stick, 2 trigger), 13 buttons, D-pad, gyro, accelerometer |
| While the sticks rest | no new values at all | the right stick rests about 10% right of centre and flickers by one step |
| macOS permission prompt | none | none |

**Point by point:**

1. **Own thread beside Bevy: works.**
   - SDL started on a non-main thread in 5–40 ms.
   - It checked the devices 3,000–3,400 times a second, with steady gaps of 0.36 ms at most, while Bevy drew at 165 fps.
   - Plugging a device in or out stalls the thread briefly: 9 ms when the DualSense connected, 4.6 ms around the Pocket's unplug.
   - Bevy's own gamepad library (gilrs) read the same devices at the same time with no conflict.
2. **Input Monitoring: no prompt.** The probe ran as its own app, and macOS's status for it stayed "never asked" for the whole run, with both devices.
3. **Rates and resolution: as expected over USB.** See the table. The DualSense over Bluetooth wasn't measured; it gets checked later, if Bluetooth support needs changes.
4. **Building SDL from source:**
   - It adds about 30 s to a clean build on the M4 and needs CMake.
   - On Linux, SDL must be told it is a build without a window system (`SDL_UNIX_CONSOLE_BUILD`, the `sdl-unix-console-build` feature). Otherwise its CMake stops because X11 and Wayland headers are missing.
   - **CI built and started it on all three OSes** (GitHub Actions: macos-latest, windows-latest, ubuntu-latest; Rust 1.99.0). On each, SDL started its gamepad support on a spawned thread.
   - A clean, uncached SDL-only build took 74 s on macOS, 55 s on Windows and 52 s on Linux. The whole probe, Bevy included, took 7–11 minutes uncached.
   - **Watch on Windows:** the test loop, sleeping 250 µs between polls, managed only about 1,240 polls a second on the Windows runner. That's barely above the Pocket's 1 kHz. Runners are virtual machines (the macOS runner managed about 730, against 3,000+ on the M4), so this needs checking on real Windows hardware. The Windows input thread will probably need a finer wait than a plain sleep.
   - Real devices on Windows and Linux weren't tested; only builds.

**Corrections to this document:**

- **Timestamps.** SDL stamps every stick and button sample with its own clock when our thread reads it, on every OS:
  - On macOS, the Pocket's path reads the current value at each poll.
  - The DualSense's path stamps each report when it processes it.
  - On Linux, SDL ignores the kernel's event time for sticks.

  So a sample's time is accurate to about one poll, roughly 0.3 ms at 3,000 polls a second. The input thread must poll well above the fastest device's report rate: on the Pocket's macOS path, only the latest value survives between polls. Only the DualSense's motion-sensor readings carry the controller's own clock. (gilrs on Linux does keep the kernel's timestamps.)
- **SDL reports changes only.** A resting Pocket produced no samples at all for 5 s, so silence can't mean a lost device; unplugging shows up as the operating system removing it. At unplug, SDL also sent false centre values on two switch channels in the same instant.
- **Window focus.** SDL only ignores background input when it owns a window. Our build has no SDL video at all, so it never does. This is checked in the source; the probe's not-focused step wasn't reached in the measured runs.
- **Gamepad centre.** On the test DualSense, the sticks rest 3–10% off centre and flicker by one step. Gamepad calibration needs a centre and a small deadband. The Pocket's centre is off by only +0.05%.

## Follow-up runs (issue #27)

[#27](https://github.com/BartoszSolkaBD/OpenDrone/issues/27) re-analysed #18's runs and added one DualSense run, `dualsense-usb-sensors-20261004-165521`. It was made with the same probe on the dev Mac and lasted 101.5 s. It includes fast circles, 76 s with motion sensors on, a 6 s hands-off with the controller on the desk, and slow edge tracing. Its raw data sits next to #18's runs on the `prototype/sdl3-input-probe` branch.

**A resting DualSense keeps reporting** when its motion sensors are on:
- **At rest:** 250.0 gyro readings a second, with the longest gap 4.4 ms.
- **Over the 76 s with sensors on:** 19,011 readings, and none went missing (by the controller's own clock). 19,000 of the 19,010 gaps were 3.5–4.5 ms. The longest, 9.4 ms, came from one report that arrived 5.8 ms late while buttons were being pressed.
- **Why it works:** SDL 3.4.18 sends a sensor reading for every report, changed or not (`SDL_SendJoystickSensor`, `SDL_hidapi_ps5.c`).
- **Why we need our own check:** over USB, SDL never treats silence as a disconnect. Only wireless dongles are dropped, after 500 ms of silence.
- **Permission:** macOS's Input Monitoring status stayed "never asked" again.

**The report beat against the Mac's clock:**
- **DualSense:** 250.0011 Hz, 4.5 ppm slow, measured on every report through the gyro readings. #18's run gave 6 ppm.
  - Reports reach the input thread a median 0.23 ms after they're due; 99.9% arrive within 0.48 ms.
  - During moves, the sticks change on nearly every report, so each 4 ms report carries a fresh sample.
- **Pocket:** 1000.004 Hz, 4 ppm slow, with arrivals 0–0.66 ms after they're due.
  - But during fast moves its values don't move every 1 ms. They jump unevenly every 1–4 ms (about 2 ms on average): jumps of 15–20 steps with single-step changes in between, on one axis at a time. The cause is unknown and has its own follow-up.

**What it means for the Radio Link** (a model of Betaflight 2026.6's feedforward, fed these traces):
- A free-running 250 Hz Radio Link whose tick lands 0.1–0.5 ms after the DualSense's beat makes feedforward ripple about 7–10× larger. Locking the Radio Link to the device's beat avoids it ([ADR-0020](../adr/0020-radio-link-locks-to-the-device-report-beat.md)).
- With the lock, 8-bit steps add 0.3% average and 2% peak of motor range on real fast circles on the 5" (1.1% and 6% on the whoop).
- The Pocket's uneven jumps give about 1.1% average and 12% peak on fast flicks (3.7% and 45% on the whoop), and locking doesn't change that.

## Binding and calibration facts (issue #19)

Checked on 2026-10-04 for [#19](https://github.com/BartoszSolkaBD/OpenDrone/issues/19). Sources:
- EdgeTX source at v2.10.0, v2.10.7, v2.11.0 and v2.12.4, plus `main`
- Radiomaster's Pocket manual A1.8 and its factory SD packages
- SDL 3.4.18 source
- Betaflight 2026.6.2 source
- the ExpressLRS docs
- first-party notes from other simulators

**What #19 decided** is in the [Input deep dive](../context/input.md) and [ADR-0017](../adr/0017-switches-reach-the-flight-controller-with-fixed-meanings.md).

### EdgeTX on the Pocket

- **Versions.**
  - **2.10.x** has two USB joystick modes on the Pocket, Classic and Advanced. Classic is every model's default and sends the same 19-byte report as 2.11, with axes 0–2047 instead of 0–2048. Advanced left at its defaults sends no axes at all, only button bits ([v2.10.0 `radio/src/CMakeLists.txt#L60-L64`](https://github.com/EdgeTX/edgetx/blob/v2.10.0/radio/src/CMakeLists.txt#L60-L64), [`usb_joystick.cpp#L80-L110`](https://github.com/EdgeTX/edgetx/blob/v2.10.0/radio/src/usb_joystick.cpp#L80-L110)).
  - **2.11.0** removed Advanced mode on 512 KB radios such as the Pocket ([PR #5443](https://github.com/EdgeTX/edgetx/pull/5443)), and widened the axes to 0–2048 ([PR #4883](https://github.com/EdgeTX/edgetx/pull/4883)).
  - **The extra-byte bug** ([#6320](https://github.com/EdgeTX/edgetx/issues/6320), fixed in 2.12.3 by [PR #7532](https://github.com/EdgeTX/edgetx/pull/7532)) only hit the Advanced-mode path, never the Pocket.
  - **Factory Pockets ship 2.10.5.** So the floor is **EdgeTX 2.10, in Classic mode**.
- **USB strings** ([v2.12.4 `usbd_desc.c#L71-L91`](https://github.com/EdgeTX/edgetx/blob/v2.12.4/radio/src/targets/common/arm/stm32/usbd_desc.c#L71-L91)):
  - Manufacturer: "OpenTX", in every version checked.
  - Product: "Radiomaster Pocket Joystick".
  - Serial: always "00000000001B".
  - bcdDevice: 0x0200 on 2.10.x, then major.minor in BCD (0x0211, 0x0212).
  - After 2.12.4, `main` renames the product "RadioMaster Pocket Joystick" ([PR #7786](https://github.com/EdgeTX/edgetx/pull/7786)).
  - **The name SDL showed in #18, "EdgeTX Radiomaster Pocket Joystick", isn't explained by any of these strings.** Recheck it on hardware.
- **USB mode.** Radio Setup › "USB mode" offers Ask, Joyst or SDCard, and the factory default is Ask. With Ask, plugging in shows "Select mode": pick "USB Joystick (HID)".
- **RF off.** MDL › SETUP › "Internal RF" › Mode OFF, and the same for "External RF".
  - A new model starts with both off, but Radiomaster's factory models have the internal module on.
  - EdgeTX never turns RF off by itself.
  - The mixer runs at 1 kHz only when neither module sets its own period ([`mixer_scheduler.cpp#L69-L87`](https://github.com/EdgeTX/edgetx/blob/v2.12.4/radio/src/mixer_scheduler.cpp#L69-L87)).
- **Controls.**
  - SA: 2-position. SB and SC: 3-position. SD: 2-position.
  - **SE is momentary:** 2-position hardware set up as a Toggle, and called "Momentary Switch" in Radiomaster's manual.
  - S1 is a pot. There's no SF or higher.
- **What a model sends on CH5 and up.**
  - **A new EdgeTX model:** only the four sticks, in "Def chan order". CH5–CH8 then sit at centre, which is 1500 µs and the middle of a 3-position range.
  - **EdgeTX's model wizard** (Multirotor) adds CH5 Arm, CH6 Beeper and CH7 Mode.
  - **Radiomaster's factory models:**
    - "POCKET", selected out of the box: CH5 SA, CH6 SD, CH7 SB, CH8 SC, CH9 SE, CH10 S1.
    - "FPV DRONE": CH5 SA, CH6 SB, CH7 SC, CH8 SD, CH9 SE, CH10 S1.
  - The maintainer's Pocket in #18 matched "FPV DRONE": 2 positions on CH5 and CH8, 3 on CH6 and CH7.
- **Stick mode and channel order.**
  - Mode 1–4 only changes which physical stick feeds each input. The channel order comes from the model's mixes.
  - "Def chan order" is AETR on Radiomaster's builds. Official EdgeTX builds on fresh settings default to Mode 1 and RETA.
- **Switch values.**
  - On an axis (CH5–CH8): a 2-position switch gives 0 or 2048, a 3-position one 0, 1024 or 2048 (2047 on 2.10).
  - Outputs beyond ±100% saturate in the joystick report.
  - A button (CH9 and up) counts as pressed only when its channel is above 0, so a 3-position switch in the middle isn't pressed.
- **Remapping.** 2.11+ has no joystick-specific invert or remap. Only the model's mixes and its output Direction change the layout.

### ExpressLRS and Betaflight

- **ExpressLRS wants Arm on AUX1 (CH5)**, low for disarmed and high for armed. AUX1 is the one switch sent in every packet ([switch modes](https://www.expresslrs.org/software/switch-config/)).
- **±100% in EdgeTX** is CRSF 172 and 1811, which Betaflight turns into 988 and 2012 µs. Centre is 992, or 1500 µs: µs = 0.62477 × value + 881 ([`rx/crsf.h`](https://github.com/betaflight/betaflight/blob/2026.6.2/src/main/rx/crsf.h)).
- **Betaflight's `aux` lines** read `aux <slot> <mode> <aux index> <start> <end> <logic> <linked>` ([`cli/cli.c#L1268-L1380`](https://github.com/betaflight/betaflight/blob/2026.6.2/src/main/cli/cli.c#L1268-L1380)).
  - Mode ids: ARM 0, ANGLE 1, HORIZON 2, FLIP OVER AFTER CRASH 35.
  - Ranges run from 900 to 2100 µs in 25 µs steps. A mode is active when start ≤ value < end ([`fc/rc_modes.c#L80-L89`](https://github.com/betaflight/betaflight/blob/2026.6.2/src/main/fc/rc_modes.c#L80-L89)).
  - Aux index 0 reads CH5.
  - Angle wins over Horizon ([`fc/core.c#L1042-L1060`](https://github.com/betaflight/betaflight/blob/2026.6.2/src/main/fc/core.c#L1042-L1060)), and Acro is simply no mode.
- **Deadband.** `deadband` and `yaw_deadband` default to 0.
  - They act on roll, pitch and yaw only, never throttle.
  - The deadband is subtracted and the rest rescaled, so full stick stays full ([`fc/rc.c#L666-L735`](https://github.com/betaflight/betaflight/blob/2026.6.2/src/main/fc/rc.c#L666-L735)).
- **Defaults:** `min_check` 1050, `max_check` 1900, `rxrange` 1000–2000 (a pass-through), `map` AETR1234.
- **Arming blocks tied to the switch:** NOT_DISARMED (the arm switch was on when the link appeared or came back, boot included), ARM_SWITCH (toggle it after any other block) and FLIP_SWITCH (Crash Flip turned off while armed).

### SDL 3.4.18

- **Radio or Gamepad.**
  - SDL's built-in database has no mapping for 1209:4F54, and macOS and Windows open the Pocket as a plain joystick.
  - **Linux builds a gamepad mapping for it automatically,** as for any event device with gamepad buttons ([`linux/SDL_sysjoystick.c#L2350`](https://github.com/libsdl-org/SDL/blob/release-3.4.18/src/joystick/linux/SDL_sysjoystick.c#L2350)).
  - So SDL's "is it a gamepad" can't tell a Radio from a Gamepad on every OS.
- **Names and ids.**
  - A device's name is "manufacturer product" on every backend ([`SDL_utils.c#L483-L620`](https://github.com/libsdl-org/SDL/blob/release-3.4.18/src/SDL_utils.c#L483-L620)).
  - The USB vendor and product ids are available everywhere.
  - bcdDevice is 0 on DirectInput.
  - A serial number only comes through SDL's HIDAPI driver or udev.
- **The DualSense through SDL's gamepad API.**
  - Buttons are named by position:
    - south ✕, east ○, west □, north △
    - back = Create, start = Options
    - shoulders, stick clicks and the D-pad
    - touchpad, and misc1 = microphone
  - Triggers run 0–32767, and the gamepad type is PS5.
  - `SDL_GetGamepadButtonLabel` gives Cross, Circle, Square and Triangle. Nintendo-style layouts are converted to positions.
- **No deadzone anywhere** in SDL's gamepad layer.
- **The jitter gate** ([`SDL_joystick.c#L2505-L2567`](https://github.com/libsdl-org/SDL/blob/release-3.4.18/src/joystick/SDL_joystick.c#L2505-L2567)):
  - Until an axis first moves more than 1.25% (409 SDL units), SDL drops its changes and keeps reporting its first value.
  - The DualSense's sticks start at 0, so a hands-off reading straight after plugging in can say 0 instead of the real rest position.
  - On unplug, SDL sets each axis back to its first value. That's the false centre seen in #18.
- **Hints worth setting** (from [#28](https://github.com/BartoszSolkaBD/OpenDrone/issues/28)):
  - `SDL_HINT_NO_SIGNAL_HANDLERS` = "1", so SDL doesn't take Ctrl+C and the kill signal away from Bevy.
  - `SDL_HINT_JOYSTICK_ALLOW_BACKGROUND_EVENTS` = "1" keeps sticks working when the window isn't focused. SDL only filters when it owns a window, so this is a safeguard.

### The #18 traces at rest

- **DualSense, as a share of half travel:**
  - Right stick sideways: rests at +9.0 to +10.6% after every release.
  - Left stick sideways: 0.4–3.5%, depending on which way it was let go.
  - Right stick up and down: −3.5 to −5.1%.
  - Left stick up and down: +0.4%.

  One 8-bit step is 0.8%.
- **Pocket:**
  - Stick values sit on SDL's 32-unit grid, offset by +15, so centre is exactly the radio's centre step. Nothing flickers at rest.
  - The throttle rests at the bottom.

### How other simulators bind and calibrate

Sources: Steam announcements, manuals, knowledge bases and developer forum posts, read 2026-10-04.

| Sim | Calibration | Deadzone | Gamepad throttle | Arming |
|---|---|---|---|---|
| **Liftoff** | Automatic, with a 3 s "centre stick" timer; manual calibration with live values; presets for known controllers; saved per controller | Adjustable after calibrating | A "throttle zero point" setting | "Throttle down" by default; arm, disarm and turtle can be bound |
| **VelociDrone** | Follow the on-screen sticks, centre, then move each stick quickly | Per axis, set by the wizard | "Use Gamepad mode": throttle from mid-stick up; the trigger is suggested | Auto-arm in races by default. Manual arming needs Arm and Flip After Crash switches, and turtle takes 7 steps |
| **Uncrashed** | Assign the axes, then move both sticks in all four directions | A setting since 2021 | Throttle expo and mid-point | Lower the throttle to arm after a reset; arm and disarm can be bound |
| **DRL** | Automatic (each stick's full range for at least 3 s, then recentre), manual, and trims | Adjustable for gamepads | A zero throttle point; triggers can be the throttle | Not found |
| **TRYP FPV** | A guided first flight, Mode 2 by default | A setting | "Use throttle at 0" | Betaflight-style arm and mode switches since June 2026 |
| **FPV SkyDive** | Rotate both sticks for 7 s, then pick the throttle mode: Manual, or Auto (sprung) | Not found | Auto mode | Auto-arm at 0% throttle, which can be switched off |

- **Steam Input.** Liftoff and DRL tell pilots to turn it off. Uncrashed's developer tells wireless PS5 users to turn it on.
- **Main links:**
  - [Liftoff support](https://www.liftoff-game.com/support)
  - [VelociDrone desktop manual](https://www.velocidrone.com/desktop_manual)
  - [Uncrashed developer FAQ](https://steamcommunity.com/app/1682970/discussions/0/6643422659556525796/)
  - [DRL 2.6 notes](https://steamstore-a.akamaihd.net/news/externalpost/steam_community_announcements/2403127338434427667)
  - [TRYP switch management](https://steamstore-a.akamaihd.net/news/externalpost/steam_community_announcements/1833968530900390)
  - [FPV SkyDive 2.1.1](https://steamstore-a.akamaihd.net/news/externalpost/steam_community_announcements/5138087875098098925)
