# Settings for SDL's source build on x86_64 Linux, read by CMake through
# CMAKE_TOOLCHAIN_FILE (set in .cargo/config.toml for the
# x86_64-unknown-linux-gnu target only, which is what CI and the alpha's Linux
# build use).
#
# SDL turns on support for a system library whenever its headers happen to be
# installed. Two of them OpenDrone doesn't need, and their licences would need
# naming under ADR-0014, so they stay off:
#
# - libusb (LGPL-2.1-or-later): a second way to reach HID devices. SDL reads
#   the DualSense through hidraw and the Pocket as a joystick without it.
# - D-Bus: desktop services (screensaver, input methods, thread priority)
#   that a no-window input thread doesn't use.
#
# libudev stays on: SDL loads the system's copy to notice devices being plugged
# in and out. deny.toml names it with its reason, and CI checks all three.
#
# Not for cross builds: with a toolchain file set, the cmake crate skips its
# own cross-compiling settings, so building for x86_64 Linux from another
# system needs its own toolchain file. On other Linux machines (aarch64, say)
# this file isn't used, and SDL turns libusb and D-Bus on if their headers are
# installed.
set(SDL_HIDAPI_LIBUSB OFF CACHE BOOL "OpenDrone: no libusb (crates/input/sdl-linux.cmake)" FORCE)
set(SDL_DBUS OFF CACHE BOOL "OpenDrone: no D-Bus (crates/input/sdl-linux.cmake)" FORCE)
