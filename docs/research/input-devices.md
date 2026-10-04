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
- **Prove it first with a short prototype.** The SDL recommendation rests on a few unverified points, listed in [what the prototype must prove](#what-the-prototype-must-prove).

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

- **EdgeTX version.** The latest stable release is **v2.12.4** (2026-09-02). The Pocket has been supported since 2.10.0.
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
| **Linux** | **High.** Event device from `hid-playstation`, with kernel timestamps. No root needed. 250 Hz over USB, about 800–1000 Hz over Bluetooth. | **High.** Event device from `hid-input`, with kernel timestamps, 11-bit, 1 kHz with RF off. |
| **macOS** | **High.** IOKit HID callbacks on our own thread. Each value carries an OS timestamp: `IOHIDValueGetTimeStamp`, "OS AbsoluteTime" (`IOKit/hid/IOHIDValue.h`, macOS 26.2 SDK). Non-exclusive, works alongside Apple's GameController framework. | **High.** Same path. Apple's GameController framework ignores the Pocket, but IOKit sees it. |
| **Windows** | **Medium.** SDL's HIDAPI reads every report, with no focus requirement. Timestamps are read times, not device times. Steam Input can inject a virtual Xbox pad, so tell pilots to disable it (Liftoff and Uncrashed both do). | **Medium.** SDL's DirectInput path, background-capable, all 8 axes. Read-time timestamps. gilrs's Windows.Gaming.Input path would cap at 125 Hz and need focus unless patched. |

## How other simulators handle Radio calibration, endpoints and channel mapping

**Liftoff** ([official support page](https://www.liftoff-game.com/support?topic=3&category=1&post=101)):
- It ships defaults for well-known radios ("Taranis will work out of the box") and has an in-game calibration on PC.
- It tells players to disable Steam Input.
- On PS4 there is no calibration menu, so the Radio must output a fixed channel order.

**DRL, TRYP FPV, Uncrashed and VelociDrone.** What we know comes only from developer posts on the Steam forums and from manuals we couldn't extract. Treat it as **unverified** detail:
- DRL auto-calibrates: move both sticks in circles, then each axis. It also has a manual per-axis min/centre/max dropdown.
- TRYP FPV has a "throttle 0 at middle" toggle that separates Gamepads from Radios.
- Uncrashed has an axis assignment screen.

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
| **All OSes** | 8-bit sticks, a hardware limit. USB runs at 250 Hz, slower than Bluetooth. | Classic mode only (EdgeTX 2.11+): fixed CH1–8 axes and CH9–32 on/off buttons. Only 1 kHz with RF modules off. Output shaped by the model's mixer and Limits. Shares its USB ID with every EdgeTX radio. |
| **Linux** | Raw HID access needs a udev rule. The event-device path works without one. | None known. Slider and Dial arrive as `ABS_THROTTLE` and `ABS_RUDDER`. |
| **macOS** | Whether the Input Monitoring permission prompt stays away for gamepads is **unverified**: SDL's comments imply only keyboards trigger it. A Bluetooth pad switched to the full report by another app may confuse descriptor-based readers like gilrs (inferred). | Same Input Monitoring question. Apple's GameController framework doesn't support it, so IOKit is required. |
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
