//! The sound device: the block the game asks for, and starting with or
//! without a device. CI's machines have none, so there this proves the
//! silent path.

mod common;

use std::time::{Duration, Instant};

use opendrone_sound::{BLOCK_FRAMES_ASKED, Device, Sound};

use common::*;

#[test]
fn the_game_asks_the_sound_device_for_a_256_frame_block() {
    // Rule: a 256-frame block is asked for, about 5 ms at 48 kHz (sound.md,
    // "Settings and screens"); Firewheel's own default is 1024.
    assert_eq!(BLOCK_FRAMES_ASKED, 256);
}

#[test]
fn starting_the_sound_never_fails_and_soon_says_where_it_plays_or_why_it_is_silent() {
    // Rule: with no sound device the game runs silent and says so
    // (sound.md). The Quad here sits still with its motors stopped, so a
    // machine with a device plays silence.
    let started = Instant::now();
    let sound = Sound::start(whoop());
    assert!(
        started.elapsed() < Duration::from_millis(500),
        "starting took {:?}",
        started.elapsed()
    );
    sound.hand_over(all_four(0.0, 0.0, 0.0));
    let mut device = sound.device();
    while device == Device::Opening && started.elapsed() < Duration::from_secs(10) {
        std::thread::sleep(Duration::from_millis(20));
        device = sound.device();
    }
    println!("the sound: {device:?}");
    match device {
        Device::Playing {
            block_frames,
            sample_rate,
            ..
        } => {
            assert!(block_frames > 0 && sample_rate > 0, "{device:?}");
        }
        Device::None(why) => assert!(!why.is_empty()),
        Device::Opening => panic!("the sound device was still opening after 10 s"),
    }
    sound.hand_over(all_four(0.0, 0.0, 0.0));
}
