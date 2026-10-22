//! Readable checks for the FPV Camera's calculations: the Lens, the picture's
//! shape, the Video Looks, Auto-exposure, Camera Tilt and FOV, and Digital's
//! late picture. None of it enters the Simulation, so none of it is a
//! Scenario. Angles in degrees, lengths in metres, times in seconds, light in
//! stops. How the picture looks is judged by eye, never here.

mod common;

use std::time::Duration;

use bevy::input::ButtonInput;
use bevy::math::{Quat, UVec2, Vec3};
use bevy::prelude::KeyCode;
use bevy::time::{Fixed, Time};
use opendrone::camera::controls::{
    CAMERA_KEYS, CameraSettings, QuadCamera, REPEAT_AFTER, REPEAT_EVERY, Repeater, camera_keys,
};
use opendrone::camera::exposure::{adaptation, compensation_points, curve_at, metering_mask};
use opendrone::camera::lens::{Aspect, GENTLE_CURVE, Lens, Picture, STOCK_CURVE};
use opendrone::camera::look::{
    ANALOG_AUTO_EXPOSURE, ANALOG_GRADE, DIGITAL_AUTO_EXPOSURE, DIGITAL_CAMERA, DIGITAL_GRADE,
    VideoLook, camera_limits,
};
use opendrone::camera::params::FrameInputs;
use opendrone::camera::pose::{DIGITAL_DELAY, MAX_EDGE_DELAY, Pose, PoseHistory, picture_delay};
use opendrone::camera::{NEAR, drawn_moment};
use opendrone::reference;
use opendrone::set_up::{FlightPlan, repo_folder};
use opendrone::settings::Settings;
use opendrone::stepping::Flight;
use opendrone_input::profile::Action;
use opendrone_maths::Vec3 as Vec3d;
use opendrone_sim::{Channel, Channels, FlightInput, InputDeviceFacts, SimulationTime};

fn close(a: f64, b: f64, within: f64) -> bool {
    (a - b).abs() <= within
}

// ---------------------------------------------------------------------------
// The Lens and the picture's shape

#[test]
fn fov_is_measured_to_the_corner_of_the_4_3_picture() {
    // Basis: Rule. FOV is corner to corner of the 4:3 picture, like the number
    // on a camera's box (#14 §2): the corner sees half the FOV off the axis,
    // whatever the FOV and the Lens curve.
    for fov in [90.0f32, 120.0, 155.0, 160.0, 170.0] {
        for curve in [STOCK_CURVE, GENTLE_CURVE] {
            let lens = Lens::new(fov, curve);
            let corner = lens.angle_at(1.0).to_degrees();
            assert!(
                (corner - fov / 2.0).abs() < 1e-3,
                "{fov}° at curve {curve}: the corner sees {corner}°"
            );
        }
    }
}

#[test]
fn a_160_degree_lens_sees_128_degrees_across_and_96_degrees_up_and_down() {
    // Basis: Source. #14 §2: "a 160° fisheye is about 128° across and 96° top
    // to bottom", with the stock curve, in a 4:3 picture.
    let picture = Picture::in_window(UVec2::new(1280, 720), Aspect::FourThree);
    let lens = Lens::new(160.0, STOCK_CURVE);
    let half_width = picture.width / 2.0 / picture.frame_radius;
    let half_height = picture.height / 2.0 / picture.frame_radius;
    assert!(close(
        f64::from(lens.angle_at(half_width).to_degrees()) * 2.0,
        128.0,
        1e-3
    ));
    assert!(close(
        f64::from(lens.angle_at(half_height).to_degrees()) * 2.0,
        96.0,
        1e-3
    ));
}

#[test]
fn a_16_9_picture_crops_the_top_and_bottom_and_never_stretches() {
    // Basis: Rule. 16:9 keeps the view across the same and crops the top and
    // bottom (#14 §3): the same angle to each side, less up and down, and the
    // picture a true 16:9, scaled the same both ways.
    let window = UVec2::new(1280, 720);
    let four_three = Picture::in_window(window, Aspect::FourThree);
    let sixteen_nine = Picture::in_window(window, Aspect::SixteenNine);
    let lens = Lens::new(160.0, STOCK_CURVE);
    let across = |p: &Picture| lens.angle_at(p.width / 2.0 / p.frame_radius).to_degrees();
    let up = |p: &Picture| lens.angle_at(p.height / 2.0 / p.frame_radius).to_degrees();
    assert!((across(&four_three) - across(&sixteen_nine)).abs() < 1e-3);
    assert!(up(&sixteen_nine) < up(&four_three) - 10.0);
    assert_eq!(sixteen_nine.size(), UVec2::new(1280, 720));
    assert_eq!(four_three.size(), UVec2::new(960, 720));
    // Pixels per degree, the same across and up and down, at the middle of
    // each picture: a circle on the Map stays a circle.
    let per_pixel = |p: &Picture| lens.angle_at(1.0 / p.frame_radius).to_degrees();
    let ratio = per_pixel(&sixteen_nine) / per_pixel(&four_three);
    assert!((ratio - 960.0 / 1280.0).abs() < 1e-3, "{ratio}");
}

#[test]
fn a_picture_is_centred_in_a_window_of_another_shape() {
    // Basis: Rule. A 4:3 picture in a 16:9 window has bars either side, a
    // 16:9 picture in a 4:3 window has bars above and below.
    let wide = Picture::in_window(UVec2::new(1920, 1080), Aspect::FourThree);
    assert_eq!(
        (wide.x, wide.y, wide.width, wide.height),
        (240.0, 0.0, 1440.0, 1080.0)
    );
    let tall = Picture::in_window(UVec2::new(1600, 1200), Aspect::SixteenNine);
    assert_eq!(
        (tall.x, tall.y, tall.width, tall.height),
        (0.0, 150.0, 1600.0, 900.0)
    );
}

#[test]
fn analog_uses_the_stock_curve_and_digital_a_gentler_one_of_0_35() {
    // Basis: Source. #28 §1: Analog keeps the stock fisheye curve, Digital's
    // is 0.35.
    assert_eq!(VideoLook::Analog.lens_curve(), 0.0);
    assert_eq!(VideoLook::Digital.lens_curve(), 0.35);
}

#[test]
fn digitals_gentler_curve_puts_more_of_the_view_in_the_middle_of_the_picture() {
    // Basis: Rule. Digital's sharper centre comes from the gentler curve
    // (#28 §1): in the same part of the picture it holds more of the view,
    // so more of the flat render's detail goes into each pixel, while the
    // corner still sees half the FOV.
    let analog = Lens::new(155.0, VideoLook::Analog.lens_curve());
    let digital = Lens::new(155.0, VideoLook::Digital.lens_curve());
    for distance in [0.1f32, 0.3, 0.5, 0.8] {
        assert!(
            digital.angle_at(distance) > analog.angle_at(distance),
            "at {distance}"
        );
    }
}

#[test]
fn an_angle_off_the_axis_and_its_place_in_the_picture_are_each_others_inverse() {
    // Basis: Rule. The Lens maps places to angles and back.
    for curve in [STOCK_CURVE, GENTLE_CURVE] {
        let lens = Lens::new(140.0, curve);
        for distance in [0.0f32, 0.2, 0.55, 0.9, 1.0] {
            let back = lens.distance_of(lens.angle_at(distance));
            assert!(
                (back - distance).abs() < 1e-4,
                "{distance} came back as {back}"
            );
        }
    }
}

#[test]
fn the_flat_render_reaches_wider_than_the_picture_to_cover_its_corners() {
    // Basis: Rule. ADR-0009: the Map is drawn once, wider than the picture, and
    // warped into the Lens. The flat render's far corner is the direction the
    // picture's corner sees: a tangent of half the FOV, and a vertical field
    // wider than the picture's 96°.
    let picture = Picture::in_window(UVec2::new(2560, 1440), Aspect::FourThree);
    let lens = Lens::new(160.0, STOCK_CURVE);
    let reach = lens.flat_reach(&picture);
    assert!((reach.length() - 80f32.to_radians().tan()).abs() < 1e-3);
    let flat_field = 2.0 * reach.y.atan().to_degrees();
    assert!(flat_field > 140.0 && flat_field < 150.0, "{flat_field}°");
}

#[test]
fn the_flat_render_stops_short_of_a_tangent_that_runs_out() {
    // Basis: Rule. A flat camera can't reach 90°: the render reaches at most
    // 87.7° off the axis, which the widest FOV (170°, 85° to a corner) stays
    // inside.
    let picture = Picture::in_window(UVec2::new(2560, 1440), Aspect::FourThree);
    let reach = Lens::new(170.0, STOCK_CURVE).flat_reach(&picture);
    assert!(reach.length().is_finite() && reach.length() < 25.0);
    assert!((reach.length() - 85f32.to_radians().tan()).abs() < 1e-2);
}

// ---------------------------------------------------------------------------
// The Video Looks and the cameras

#[test]
fn the_analog_look_is_soft_and_grainy_with_bleeding_colour_and_a_punchy_grade() {
    // Basis: Observed. #28 §3, tuned by eye with the maintainer: grain 0.1,
    // colour bleed 3 analog pixels, contrast 1.1, saturation 0.9.
    assert_eq!(ANALOG_GRADE.grain, 0.1);
    assert_eq!(ANALOG_GRADE.colour_bleed, 3.0);
    assert_eq!(ANALOG_GRADE.contrast, 1.1);
    assert_eq!(ANALOG_GRADE.saturation, 0.9);
}

#[test]
fn the_digital_look_is_clean_with_10_stops_and_1080_lines() {
    // Basis: Source. #28 §2 and §3: one generic camera for every Quad, 10
    // stops and 1080 lines, and no grain, bleed or grade of its own.
    assert_eq!(DIGITAL_CAMERA.dynamic_range, 10.0);
    assert_eq!(DIGITAL_CAMERA.lines, 1080.0);
    assert_eq!(
        (
            DIGITAL_GRADE.contrast,
            DIGITAL_GRADE.saturation,
            DIGITAL_GRADE.sharpen
        ),
        (1.0, 1.0, 0.0)
    );
    for quad in ["whoop-65", "freestyle-5"] {
        let plan = common::plan(quad);
        assert_eq!(
            camera_limits(VideoLook::Digital, &plan.quad.camera),
            DIGITAL_CAMERA
        );
    }
}

#[test]
fn each_quads_analog_camera_has_its_own_dynamic_range_lines_and_sharpness() {
    // Basis: Source. #28 §2: the Whoop 65's C03 camera is 7 stops, 480 lines
    // and 300 TV lines; the Freestyle 5″'s typical camera is 8.5 stops, 480
    // lines and 400 TV lines. They come from the Quad definitions.
    let whoop = common::plan("whoop-65");
    let limits = camera_limits(VideoLook::Analog, &whoop.quad.camera);
    assert_eq!(
        (limits.dynamic_range, limits.lines, limits.sharpness),
        (7.0, 480.0, 300.0)
    );
    let five = common::plan("freestyle-5");
    let limits = camera_limits(VideoLook::Analog, &five.quad.camera);
    assert_eq!(
        (limits.dynamic_range, limits.lines, limits.sharpness),
        (8.5, 480.0, 400.0)
    );
}

fn frame(quad: &str, look: VideoLook, window: UVec2, aspect: Aspect) -> FrameInputs {
    let plan = common::plan(quad);
    let picture = Picture::in_window(window, aspect);
    let lens = Lens::new(plan.quad.camera.fov.to_degrees() as f32, look.lens_curve());
    FrameInputs {
        picture,
        lens,
        reach: lens.flat_reach(&picture),
        source: picture.size(),
        look,
        limits: camera_limits(look, &plan.quad.camera),
        time: 0.0,
        frame: 0,
    }
}

#[test]
fn the_whoops_analog_picture_is_softer_than_the_freestyles_and_both_have_480_lines() {
    // Basis: Rule. A camera's TV lines are counted over the picture's height:
    // at native 1440p a 4:3 picture is 1440 pixels tall, so 300 TV lines make
    // an analog pixel 4.8 window pixels across and 400 make it 3.6. Both
    // cameras have 480 lines, 3 window pixels each.
    let window = UVec2::new(2560, 1440);
    let whoop = frame("whoop-65", VideoLook::Analog, window, Aspect::FourThree).params();
    let five = frame("freestyle-5", VideoLook::Analog, window, Aspect::FourThree).params();
    assert!((whoop.analog.x - 4.8).abs() < 1e-4, "{}", whoop.analog.x);
    assert!((five.analog.x - 3.6).abs() < 1e-4, "{}", five.analog.x);
    assert!((whoop.camera.y - 3.0).abs() < 1e-4);
    assert!((five.camera.y - 3.0).abs() < 1e-4);
}

#[test]
fn a_camera_with_fewer_stops_squeezes_the_light_harder() {
    // Basis: Rule. The picture holds 8.6 stops with the punchy curve, so a
    // 7-stop camera squeezes the light around the middle grey by 8.6 / 7 and a
    // 10-stop camera by 8.6 / 10: more squeeze means more blown highlights and
    // sunk shadows.
    let window = UVec2::new(1280, 720);
    let analog = frame("whoop-65", VideoLook::Analog, window, Aspect::FourThree).params();
    let digital = frame("whoop-65", VideoLook::Digital, window, Aspect::FourThree).params();
    assert!((analog.camera.x - 8.6 / 7.0).abs() < 1e-4);
    assert!((digital.camera.x - 8.6 / 10.0).abs() < 1e-4);
    assert!(analog.camera.x > digital.camera.x);
}

#[test]
fn digital_draws_1080_lines_one_to_one_in_a_1080p_window_and_a_16_9_picture_shows_810_of_them() {
    // Basis: Rule. 1080 lines over a 4:3 picture 1080 pixels tall (a 1080p
    // window) is one window pixel a line. A 16:9 picture is that frame with its top and bottom
    // cropped: 1920 × 1080 pixels over a frame 1440 pixels tall shows 810 of the
    // 1080 lines.
    let four_three = frame(
        "whoop-65",
        VideoLook::Digital,
        UVec2::new(1920, 1080),
        Aspect::FourThree,
    );
    assert!((four_three.params().camera.z - 1.0).abs() < 1e-4);
    let sixteen_nine = frame(
        "whoop-65",
        VideoLook::Digital,
        UVec2::new(1920, 1080),
        Aspect::SixteenNine,
    );
    let line = sixteen_nine.params().camera.z;
    assert!(
        (sixteen_nine.picture.height / line - 810.0).abs() < 0.5,
        "{line}"
    );
}

// ---------------------------------------------------------------------------
// Auto-exposure

#[test]
fn analog_may_brighten_5_stops_and_darken_1_5_digital_6_and_2_at_their_own_speeds() {
    // Basis: Source. #28 §4: Analog brightens up to 5 stops (3 a second) and
    // darkens up to 1.5 (4 a second); Digital brightens up to 6 (4 a second)
    // and darkens up to 2 (6 a second); both aim 2 stops below Bevy's default.
    let a = VideoLook::Analog.auto_exposure();
    assert_eq!(
        (
            a.max_brighten,
            a.speed_brighten,
            a.max_darken,
            a.speed_darken
        ),
        (5.0, 3.0, 1.5, 4.0)
    );
    let d = VideoLook::Digital.auto_exposure();
    assert_eq!(
        (
            d.max_brighten,
            d.speed_brighten,
            d.max_darken,
            d.speed_darken
        ),
        (6.0, 4.0, 2.0, 6.0)
    );
    assert_eq!((a.target, d.target), (-2.0, -2.0));
}

#[test]
fn a_dark_room_is_brightened_only_as_far_as_the_camera_may_and_the_sun_darkened_only_as_far() {
    // Basis: Rule. Flying into a room 6 stops under the target, Analog brightens
    // 5 stops, so the room stays near black, and Digital 6; in the sun 3 stops
    // over the target, Analog darkens 1.5 stops, so the picture blows out,
    // and Digital 2. Within its limits the camera aims the view's average light at
    // the target.
    assert_eq!(adaptation(&ANALOG_AUTO_EXPOSURE, -8.0), 5.0);
    assert_eq!(adaptation(&DIGITAL_AUTO_EXPOSURE, -8.0), 6.0);
    assert_eq!(adaptation(&ANALOG_AUTO_EXPOSURE, 1.0), -1.5);
    assert_eq!(adaptation(&DIGITAL_AUTO_EXPOSURE, 1.0), -2.0);
    assert_eq!(adaptation(&ANALOG_AUTO_EXPOSURE, -3.0), 1.0);
    assert_eq!(adaptation(&ANALOG_AUTO_EXPOSURE, -2.0), 0.0);
}

#[test]
fn the_compensation_curve_makes_bevys_auto_exposure_follow_those_limits() {
    // Basis: Rule. For every average luminance the curve gives the exposure the
    // average plus the camera's own adaptation.
    for limits in [ANALOG_AUTO_EXPOSURE, DIGITAL_AUTO_EXPOSURE] {
        let points = compensation_points(&limits);
        for average in [-13.0f32, -9.0, -7.0, -4.0, -2.0, -1.0, 0.0, 1.5, 4.0, 5.5] {
            let curve = curve_at(&points, average);
            let wanted = average + adaptation(&limits, average);
            assert!(
                (curve - wanted).abs() < 0.02,
                "at {average}: {curve}, not {wanted}"
            );
        }
    }
}

#[test]
fn metering_counts_the_middle_of_the_picture_more_than_its_edges() {
    // Basis: Rule. The metering mask weighs the flat render by how much of the
    // picture each part becomes, and the middle counts for more.
    let picture = Picture::in_window(UVec2::new(1280, 720), Aspect::FourThree);
    let lens = Lens::new(160.0, STOCK_CURVE);
    let reach = lens.flat_reach(&picture);
    let (size, weights) = metering_mask(&picture, &lens, reach);
    let at = |x: u32, y: u32| weights[(y * size.x + x) as usize];
    let middle = at(size.x / 2, size.y / 2);
    assert!(middle > 200, "{middle}");
    assert!(middle > at(size.x / 2, size.y * 9 / 20) / 2);
    // The flat render's left and right edges reach wider than the picture
    // shows across, so there the mask is zero.
    assert_eq!(at(0, size.y / 2), 0);
    assert_eq!(at(size.x - 1, size.y / 2), 0);
}

// ---------------------------------------------------------------------------
// Camera Tilt and FOV

#[test]
fn each_quads_camera_tilt_and_fov_start_from_its_definition() {
    // Basis: Source. #14 §2: the Whoop 65 starts at Camera Tilt 30° and FOV
    // 160°, the Freestyle 5″ at 30° and 155°.
    let settings = CameraSettings::default();
    let whoop = common::plan("whoop-65");
    let five = common::plan("freestyle-5");
    let quad = settings.quad(&whoop.quad.id, &whoop.quad.camera);
    assert!(close(quad.tilt, 30.0, 1e-9) && close(quad.fov, 160.0, 1e-9));
    let quad = settings.quad(&five.quad.id, &five.quad.camera);
    assert!(close(quad.tilt, 30.0, 1e-9) && close(quad.fov, 155.0, 1e-9));
}

#[test]
fn the_camera_starts_4_3_and_analog() {
    // Basis: Rule. Aspect ratio is 4:3 by default (#14 §3).
    let settings = CameraSettings::default();
    assert_eq!(settings.aspect, Aspect::FourThree);
    assert_eq!(settings.look, VideoLook::Analog);
}

#[test]
fn up_and_down_change_camera_tilt_by_1_degree_a_press_within_0_to_80_degrees() {
    // Basis: Source. #14 §2 and the spec: ↑/↓ change Camera Tilt by 1° a press,
    // in a range of 0 to 80°.
    let plan = common::plan("whoop-65");
    let mut settings = CameraSettings::default();
    let tilt = |s: &CameraSettings| s.quad(&plan.quad.id, &plan.quad.camera).tilt;
    settings.press(&plan.quad.id, &plan.quad.camera, Action::CameraTiltUp);
    assert!(close(tilt(&settings), 31.0, 1e-9));
    settings.press(&plan.quad.id, &plan.quad.camera, Action::CameraTiltDown);
    settings.press(&plan.quad.id, &plan.quad.camera, Action::CameraTiltDown);
    assert!(close(tilt(&settings), 29.0, 1e-9));
    for _ in 0..100 {
        settings.press(&plan.quad.id, &plan.quad.camera, Action::CameraTiltUp);
    }
    assert!(close(tilt(&settings), 80.0, 1e-9));
    for _ in 0..100 {
        settings.press(&plan.quad.id, &plan.quad.camera, Action::CameraTiltDown);
    }
    assert!(close(tilt(&settings), 0.0, 1e-9));
}

#[test]
fn right_and_left_change_fov_by_1_degree_a_press_within_90_to_170_degrees() {
    // Basis: Source. #14 §2 and the spec: →/← change FOV by 1° a press, in a
    // range of 90 to 170°; wider is →.
    let plan = common::plan("freestyle-5");
    let mut settings = CameraSettings::default();
    let fov = |s: &CameraSettings| s.quad(&plan.quad.id, &plan.quad.camera).fov;
    settings.press(&plan.quad.id, &plan.quad.camera, Action::FovWider);
    assert!(close(fov(&settings), 156.0, 1e-9));
    settings.press(&plan.quad.id, &plan.quad.camera, Action::FovNarrower);
    settings.press(&plan.quad.id, &plan.quad.camera, Action::FovNarrower);
    assert!(close(fov(&settings), 154.0, 1e-9));
    for _ in 0..100 {
        settings.press(&plan.quad.id, &plan.quad.camera, Action::FovWider);
    }
    assert!(close(fov(&settings), 170.0, 1e-9));
    for _ in 0..200 {
        settings.press(&plan.quad.id, &plan.quad.camera, Action::FovNarrower);
    }
    assert!(close(fov(&settings), 90.0, 1e-9));
}

#[test]
fn a_quad_keeps_the_camera_tilt_and_fov_the_pilot_gave_it_and_the_other_quad_is_untouched() {
    // Basis: Rule. The values are kept for each Quad (#14 §2).
    let whoop = common::plan("whoop-65");
    let five = common::plan("freestyle-5");
    let mut settings = CameraSettings::default();
    for _ in 0..10 {
        settings.press(&whoop.quad.id, &whoop.quad.camera, Action::CameraTiltUp);
    }
    settings.press(&whoop.quad.id, &whoop.quad.camera, Action::FovNarrower);
    let kept = settings.quad(&whoop.quad.id, &whoop.quad.camera);
    assert!(close(kept.tilt, 40.0, 1e-9) && close(kept.fov, 159.0, 1e-9));
    let other = settings.quad(&five.quad.id, &five.quad.camera);
    assert!(close(other.tilt, 30.0, 1e-9) && close(other.fov, 155.0, 1e-9));
}

#[test]
fn a_press_shows_a_brief_readout_of_the_new_value_and_stopping_at_the_end_shows_none() {
    // Basis: Rule. The readout names the value, in whole degrees, and a press
    // that changes nothing, at the end of the range, has nothing to show.
    let plan = common::plan("whoop-65");
    let mut settings = CameraSettings::default();
    let up = settings
        .press(&plan.quad.id, &plan.quad.camera, Action::CameraTiltUp)
        .expect("tilt moved");
    assert_eq!(up.text, "Camera Tilt 31°");
    let wider = settings
        .press(&plan.quad.id, &plan.quad.camera, Action::FovNarrower)
        .expect("FOV moved");
    assert_eq!(wider.text, "FOV 159°");
    settings.set_quad(
        &plan.quad.id,
        QuadCamera {
            tilt: 80.0,
            fov: 170.0,
        },
    );
    assert!(
        settings
            .press(&plan.quad.id, &plan.quad.camera, Action::CameraTiltUp)
            .is_none()
    );
    assert!(
        settings
            .press(&plan.quad.id, &plan.quad.camera, Action::FovWider)
            .is_none()
    );
}

fn acts(
    repeater: &mut Repeater,
    held: &[Action],
    from_ms: u64,
    to_ms: u64,
    step_ms: u64,
) -> Vec<u64> {
    let mut at = Vec::new();
    let mut now = from_ms;
    while now <= to_ms {
        for _ in repeater.update(Duration::from_millis(now), held) {
            at.push(now);
        }
        now += step_ms;
    }
    at
}

#[test]
fn a_held_key_acts_at_once_again_after_1_s_and_then_every_200_ms() {
    // Basis: Source. #14 §2: "Holding a key repeats it after 1 s, every
    // 200 ms." Frames 10 ms apart.
    assert_eq!(REPEAT_AFTER, Duration::from_secs(1));
    assert_eq!(REPEAT_EVERY, Duration::from_millis(200));
    let mut repeater = Repeater::new();
    let held = [Action::FovWider];
    let at = acts(&mut repeater, &held, 0, 2000, 10);
    assert_eq!(at, vec![0, 1000, 1200, 1400, 1600, 1800, 2000]);
}

#[test]
fn a_repeat_is_not_lost_when_a_frame_runs_long_and_a_released_key_starts_afresh() {
    // Basis: Rule. A frame that spans several repeats acts for each, and a key
    // let go and pressed again acts at once.
    let mut repeater = Repeater::new();
    let held = [Action::CameraTiltUp];
    assert_eq!(repeater.update(Duration::ZERO, &held).len(), 1);
    // One long frame from 0.9 s to 1.45 s: the repeats due at 1.0, 1.2, 1.4 s.
    assert_eq!(repeater.update(Duration::from_millis(900), &held).len(), 0);
    assert_eq!(repeater.update(Duration::from_millis(1450), &held).len(), 3);
    // Let go, then press again: it acts at once and waits a second again.
    assert_eq!(repeater.update(Duration::from_millis(1500), &[]).len(), 0);
    assert_eq!(repeater.update(Duration::from_millis(1600), &held).len(), 1);
    assert_eq!(repeater.update(Duration::from_millis(2500), &held).len(), 0);
    assert_eq!(repeater.update(Duration::from_millis(2600), &held).len(), 1);
}

#[test]
fn two_keys_held_together_repeat_on_their_own() {
    // Basis: Rule. Camera Tilt and FOV keys each follow their own clock.
    let mut repeater = Repeater::new();
    let tilt = [Action::CameraTiltUp];
    let both = [Action::CameraTiltUp, Action::FovWider];
    assert_eq!(repeater.update(Duration::ZERO, &tilt).len(), 1);
    // FOV is pressed at 0.5 s: it acts at once, tilt does not.
    assert_eq!(
        repeater.update(Duration::from_millis(500), &both),
        vec![Action::FovWider]
    );
    // At 1.0 s tilt repeats; FOV waits until 1.5 s.
    assert_eq!(
        repeater.update(Duration::from_millis(1000), &both),
        vec![Action::CameraTiltUp]
    );
    assert_eq!(repeater.update(Duration::from_millis(1500), &both).len(), 3);
}

/// One frame of the game's camera keys, as `read_the_camera_keys` runs it:
/// what the keyboard did since the last frame, then what the Actions do.
/// `now_ms` is the frame's moment.
fn camera_frame(
    keys: &mut ButtonInput<KeyCode>,
    repeater: &mut Repeater,
    now_ms: u64,
    keyboard: impl FnOnce(&mut ButtonInput<KeyCode>),
) -> Vec<Action> {
    // Bevy clears last frame's presses before it reads this frame's events.
    keys.clear();
    keyboard(keys);
    let (held, tapped) = camera_keys(keys);
    repeater.update_with_taps(Duration::from_millis(now_ms), &held, &tapped)
}

#[test]
fn a_key_pressed_and_let_go_within_one_frame_acts_once() {
    // Basis: Rule. docs/context/input.md: an Action fires once when its key
    // moves into the bound position. A tap shorter than a frame is down and
    // up again by the time the frame looks (#164 item 6), and still counts.
    let mut keys = ButtonInput::<KeyCode>::default();
    let mut repeater = Repeater::new();
    let tap = |keys: &mut ButtonInput<KeyCode>| {
        keys.press(KeyCode::ArrowUp);
        keys.release(KeyCode::ArrowUp);
    };
    assert_eq!(
        camera_frame(&mut keys, &mut repeater, 0, tap),
        vec![Action::CameraTiltUp]
    );
    // The frames after it have nothing to act on.
    for now in [10, 20, 1500] {
        assert!(camera_frame(&mut keys, &mut repeater, now, |_| {}).is_empty());
    }
    // Each of the four keys does it, and a second tap acts again.
    for (key, action) in CAMERA_KEYS {
        let tap = |keys: &mut ButtonInput<KeyCode>| {
            keys.press(key);
            keys.release(key);
        };
        assert_eq!(
            camera_frame(&mut keys, &mut repeater, 2000, tap),
            vec![action]
        );
        assert_eq!(
            camera_frame(&mut keys, &mut repeater, 2010, tap),
            vec![action]
        );
    }
}

#[test]
fn a_held_key_acts_once_and_not_every_frame() {
    // Basis: Rule. A key held down acts when it goes down, and then waits the
    // repeat time (#14 §2): the frames in between don't act.
    let mut keys = ButtonInput::<KeyCode>::default();
    let mut repeater = Repeater::new();
    let down = camera_frame(&mut keys, &mut repeater, 0, |keys| {
        keys.press(KeyCode::ArrowRight)
    });
    assert_eq!(down, vec![Action::FovWider]);
    for now in (10..1000).step_by(10) {
        assert!(
            camera_frame(&mut keys, &mut repeater, now, |_| {}).is_empty(),
            "acted again at {now} ms"
        );
    }
    let repeat = camera_frame(&mut keys, &mut repeater, 1000, |_| {});
    assert_eq!(repeat, vec![Action::FovWider]);
    // Let go within the frame after: nothing more.
    let up = camera_frame(&mut keys, &mut repeater, 1010, |keys| {
        keys.release(KeyCode::ArrowRight)
    });
    assert!(up.is_empty());
}

#[test]
fn a_held_key_let_go_and_pressed_again_within_one_frame_acts_again_at_once() {
    // Basis: Rule. The second press is a press: it acts at once and waits a
    // second again.
    let mut keys = ButtonInput::<KeyCode>::default();
    let mut repeater = Repeater::new();
    camera_frame(&mut keys, &mut repeater, 0, |keys| {
        keys.press(KeyCode::ArrowLeft)
    });
    let again = camera_frame(&mut keys, &mut repeater, 500, |keys| {
        keys.release(KeyCode::ArrowLeft);
        keys.press(KeyCode::ArrowLeft);
    });
    assert_eq!(again, vec![Action::FovNarrower]);
    assert!(camera_frame(&mut keys, &mut repeater, 1000, |_| {}).is_empty());
    assert_eq!(
        camera_frame(&mut keys, &mut repeater, 1500, |_| {}),
        vec![Action::FovNarrower]
    );
}

// ---------------------------------------------------------------------------
// Digital's late picture

/// The Quad flying east at 10 m/s, a pose every millisecond, as the game
/// records them.
fn flying_east() -> PoseHistory {
    let mut history = PoseHistory::new();
    for tick in 0..=1000u32 {
        let t = f64::from(tick) / 1000.0;
        history.push(
            t,
            Pose {
                position: Vec3::new((10.0 * t) as f32, 2.0, 0.0),
                rotation: Quat::IDENTITY,
            },
        );
    }
    history
}

#[test]
fn digital_shows_the_picture_15_ms_later_than_analog() {
    // Basis: Source. #14 §4: Digital is always 15 ms later than Analog. At
    // 10 m/s that is 0.15 m behind, and Digital shows exactly the place
    // Analog showed 15 ms before.
    let history = flying_east();
    let now = 1.0;
    let analog = history.shown(now, VideoLook::Analog, 0.0).expect("a pose");
    let digital = history.shown(now, VideoLook::Digital, 0.0).expect("a pose");
    assert!((analog.position.x - 10.0).abs() < 1e-4);
    assert!(((analog.position.x - digital.position.x) - 0.15).abs() < 1e-4);
    let analog_then = history
        .shown(now - 0.015, VideoLook::Analog, 0.0)
        .expect("a pose");
    assert!((digital.position.x - analog_then.position.x).abs() < 1e-5);
    assert_eq!(DIGITAL_DELAY, 0.015);
}

#[test]
fn the_picture_is_drawn_one_tick_behind_the_last_tick_by_how_far_the_clock_is_into_the_next() {
    // Basis: Rule. The Quad has always been drawn between the last two ticks'
    // states, by how far the game's clock is into the next tick: with 10 ticks
    // done and half a tick over, at 1 kHz, that is 9.5 ms.
    let mut fixed = Time::<Fixed>::from_hz(1000.0);
    fixed.advance_by(Duration::from_millis(10));
    fixed.accumulate_overstep(Duration::from_micros(500));
    assert!(
        (drawn_moment(&fixed) - 0.0095).abs() < 1e-9,
        "{}",
        drawn_moment(&fixed)
    );
    // Paused: the clock doesn't move, so the moment doesn't.
    let moment = drawn_moment(&fixed);
    assert_eq!(drawn_moment(&fixed), moment);
}

#[test]
fn near_the_edge_of_range_digital_may_be_up_to_20_ms_later_still_and_never_more() {
    // Basis: Source. #28 §3: Digital's delay rises by up to 20 ms near the edge
    // of range. Analog never has any.
    assert_eq!(picture_delay(VideoLook::Analog, 0.0), 0.0);
    assert_eq!(picture_delay(VideoLook::Analog, 0.02), 0.0);
    assert_eq!(picture_delay(VideoLook::Digital, 0.0), 0.015);
    assert!((picture_delay(VideoLook::Digital, 0.01) - 0.025).abs() < 1e-12);
    assert!((picture_delay(VideoLook::Digital, 0.02) - 0.035).abs() < 1e-12);
    assert!((picture_delay(VideoLook::Digital, 0.5) - 0.035).abs() < 1e-12);
    assert_eq!(picture_delay(VideoLook::Digital, -1.0), 0.015);
    assert_eq!(MAX_EDGE_DELAY, 0.020);
    let history = flying_east();
    let late = history
        .shown(1.0, VideoLook::Digital, 0.02)
        .expect("a pose");
    assert!((late.position.x - (10.0 - 0.35)).abs() < 1e-4);
}

#[test]
fn right_after_a_reset_the_picture_shows_no_motion_across_it_and_nothing_of_the_old_place() {
    // Basis: Rule. Reset moves the Quad at once: the picture never draws it
    // moving across the jump, and Digital doesn't show the old place late.
    let mut history = flying_east();
    history.restart(
        1.0,
        Pose {
            position: Vec3::new(0.0, 0.01, 0.0),
            rotation: Quat::IDENTITY,
        },
    );
    history.push(
        1.001,
        Pose {
            position: Vec3::new(0.0, 0.01, 0.0),
            rotation: Quat::IDENTITY,
        },
    );
    for look in [VideoLook::Analog, VideoLook::Digital] {
        for moment in [0.99, 1.0, 1.0005, 1.001, 1.02] {
            let pose = history.shown(moment, look, 0.0).expect("a pose");
            assert_eq!(
                pose.position,
                Vec3::new(0.0, 0.01, 0.0),
                "{look:?} at {moment}"
            );
        }
    }
}

#[test]
fn the_history_forgets_what_is_too_old_to_draw_and_before_the_first_pose_shows_the_first() {
    // Basis: Rule. It keeps about 0.1 s, more than the longest delay of 35 ms,
    // and a moment before its oldest pose shows that pose.
    let history = flying_east();
    assert!(history.len() < 130, "{} poses", history.len());
    let oldest = history.at(0.0).expect("a pose");
    assert!(oldest.position.x > 8.0, "{}", oldest.position.x);
    assert!(PoseHistory::new().at(1.0).is_none());
}

#[test]
fn the_late_picture_never_enters_the_simulation() {
    // Basis: Rule. The camera and the video are outside the Simulation (the
    // Camera and video deep dive): recording poses and drawing them late leaves
    // a flight's fingerprint exactly as it was. The history is checked on the
    // way: Analog shows the Quad's newest pose, and Digital with an edge
    // delay of 20 ms shows where the Quad was 35 ms (35 ticks at 1 kHz)
    // earlier, so a history that kept, shifted or smeared its poses shows in
    // the position drawn.
    // The Quad climbs and rolls (with Auto-arm on), so it is moving while the
    // history is read: a Quad at rest would pass whatever the history did.
    let mut settings = Settings::defaults("whoop-65");
    settings.auto_arm = true;
    let plan = FlightPlan::from_packs(&common::packs(), settings).unwrap();
    let device = InputDeviceFacts {
        report_rate: None,
        reports_at_rest: false,
    };
    let mut plain = Flight::new(plan.simulation(Some(device)).unwrap()).unwrap();
    let mut watched = Flight::new(plan.simulation(Some(device)).unwrap()).unwrap();
    let mut sticks = Channels::RESTING;
    plain.input(SimulationTime::START, FlightInput::Channels(sticks));
    watched.input(SimulationTime::START, FlightInput::Channels(sticks));
    let mut history = PoseHistory::new();
    let mut flown: Vec<Vec3> = Vec::new();
    for tick in 0..4000u32 {
        if tick == 2200 || tick == 3000 {
            // The first raise after the ESCs' ready beep, which Auto-arm arms
            // on, and then a roll.
            if tick == 2200 {
                sticks.throttle = Channel::from_micros(1500.0);
            } else {
                sticks.roll = Channel::from_micros(1731.4);
            }
            let at = SimulationTime::from_ticks(u64::from(tick + 1) * 8);
            plain.input(at, FlightInput::Channels(sticks));
            watched.input(at, FlightInput::Channels(sticks));
        }
        plain.tick();
        watched.tick();
        let (_, after) = watched.drawn_between();
        let at = f64::from(tick + 1) / 1000.0;
        let position = Vec3::new(
            after.position.x as f32,
            after.position.y as f32,
            after.position.z as f32,
        );
        flown.push(position);
        history.push(
            at,
            Pose {
                position,
                rotation: Quat::IDENTITY,
            },
        );
        let newest = history.shown(at, VideoLook::Analog, 0.0).expect("a pose");
        assert_eq!(newest.position, position, "Analog at tick {tick}");
        if tick >= 35 {
            let late = history.shown(at, VideoLook::Digital, 0.02).expect("a pose");
            let before = flown[(tick - 35) as usize];
            assert!(
                (late.position - before).length() < 1e-4,
                "Digital at tick {tick}: {} and not {before}",
                late.position
            );
        }
    }
    assert!(
        (flown[3999] - flown[2999]).length() > 0.05,
        "the Quad has to move for the history to be tested"
    );
    assert_eq!(
        plain.simulation().fingerprint(),
        watched.simulation().fingerprint()
    );
}

// ---------------------------------------------------------------------------
// Where the camera sits on the Whoop 65

/// How far the Whoop 65's near plane gets past the Wall Test Map's wall, at
/// worst, in the hit a Scenario flies, in metres (negative while it stays
/// short). The wall's face is 5 m east. The near plane is a flat rectangle,
/// NEAR ahead of the camera along its axis, as wide and tall as the flat
/// render reaches (`Lens::flat_reach`): on a 160° FOV its bottom edge reaches
/// much nearer a wall than straight ahead does. The check takes its four
/// corners every tick, at the default Camera Tilt and FOV, under both Video
/// Looks' Lens curves.
fn deepest_near_plane_past_the_wall(scenario: &str) -> f64 {
    const WALL_FACE: f64 = 5.0;
    let repo = repo_folder();
    let loaded = reference::load(&repo, &repo.join(scenario)).unwrap();
    let (simulation, inputs) = opendrone_scenario::simulation(&loaded.scenario, &loaded.packs)
        .unwrap_or_else(|problems| panic!("{problems}"));
    let mut flight = Flight::scripted(simulation, inputs).unwrap();

    let camera = common::plan("whoop-65").quad.camera;
    let [forward, left, up] = camera.position;
    let at = Vec3d::new(forward, left, up);
    let tilt = camera.camera_tilt;
    let axis = Vec3d::new(tilt.cos(), 0.0, tilt.sin());
    let side = Vec3d::new(0.0, 1.0, 0.0);
    let above = Vec3d::new(-tilt.sin(), 0.0, tilt.cos());
    let picture = Picture::in_window(UVec2::new(1280, 720), Aspect::FourThree);
    let reaches: Vec<_> = [STOCK_CURVE, GENTLE_CURVE]
        .iter()
        .map(|curve| Lens::new(camera.fov.to_degrees() as f32, *curve).flat_reach(&picture))
        .collect();
    let near = f64::from(NEAR);

    let mut deepest = f64::NEG_INFINITY;
    let mut bounced = false;
    while !flight.finished() {
        flight.tick();
        let state = flight.simulation().quad_state(0);
        for reach in &reaches {
            for (across, high) in [(-1.0, -1.0), (-1.0, 1.0), (1.0, -1.0), (1.0, 1.0)] {
                let corner = at
                    + (axis
                        + side * (across * f64::from(reach.x))
                        + above * (high * f64::from(reach.y)))
                        * near;
                let corner = state.position + state.attitude.body_to_world(corner);
                deepest = deepest.max(corner.x - WALL_FACE);
            }
        }
        bounced |= state.velocity.x < 0.0;
    }
    assert!(bounced, "the whoop never hit the wall and bounced back");
    deepest
}

#[test]
fn after_a_level_nose_first_hit_into_a_wall_the_whoops_near_plane_stays_short_of_the_wall() {
    // Basis: Rule. The picture draws nothing nearer than the near plane, so
    // the picture shows through a wall once any part of the near plane is
    // past the wall's face (#208). The Scenario flies the Whoop 65 level,
    // nose first, at 10 m/s east into the wall. Metres.
    let deepest = deepest_near_plane_past_the_wall(
        "scenarios/physics/whoop-hits-a-wall-nose-first-with-its-front-duct-rings.toml",
    );
    assert!(
        deepest < 0.0,
        "the near plane's corner got {:.2} mm past the wall's face",
        deepest * 1000.0
    );
}

#[test]
fn after_a_45_degree_nose_down_hit_into_a_wall_the_whoops_near_plane_stays_short_of_the_wall() {
    // Basis: Rule. As for the level hit (#268): the picture shows through a
    // wall once any part of the near plane is past the wall's face. The
    // Scenario flies the Whoop 65 nose down 45° at 10 m/s east into the
    // wall: a nose-dive, which whoop flying meets often. Metres.
    let deepest = deepest_near_plane_past_the_wall(
        "scenarios/physics/whoop-hits-a-wall-nose-down-at-45-degrees.toml",
    );
    assert!(
        deepest < 0.0,
        "the near plane's corner got {:.2} mm past the wall's face",
        deepest * 1000.0
    );
}

#[test]
fn the_whoops_camera_sits_above_the_top_of_its_duct_rings() {
    // Basis: Rule. A camera below the ring tops sees their front tips in the
    // top corners of the picture once the Camera Tilt reaches about 43° (#208).
    // Metres, up from the centre of mass.
    let plan = common::plan("whoop-65");
    let shape = &plan.quad.parameters.shape;
    let rings = shape.duct_rings.expect("the Whoop 65 has duct rings");
    let top = shape.rotor_height + rings.height / 2.0;
    let up = plan.quad.camera.position[2];
    assert!(up >= top, "the camera is {up} m up, the ring tops {top} m");
}

#[test]
fn the_whoops_picture_at_its_default_tilt_and_fov_shows_none_of_its_front_duct_rings() {
    // Basis: Rule. The picture reaches as far off the axis as half the FOV, at
    // its corners (#14 §2). A ring point is out of the picture when it is
    // further than that from the camera's axis. Checked on the front rings'
    // outer walls, top and bottom edges, every degree round. Degrees.
    let plan = common::plan("whoop-65");
    let shape = &plan.quad.parameters.shape;
    let rings = shape.duct_rings.expect("the Whoop 65 has duct rings");
    let camera = &plan.quad.camera;
    let lens = Vec3d::new(camera.position[0], camera.position[1], camera.position[2]);
    let tilt = camera.camera_tilt;
    let axis = Vec3d::new(tilt.cos(), 0.0, tilt.sin());
    let reach = camera.fov.to_degrees() / 2.0;
    let centre = shape.diagonal / 2.0 * core::f64::consts::FRAC_1_SQRT_2;
    let outside = rings.inside_diameter / 2.0 + rings.wall;
    let mut nearest = f64::INFINITY;
    for side in [-1.0, 1.0] {
        for degrees in 0..360 {
            let round = f64::from(degrees).to_radians();
            for edge in [-1.0, 1.0] {
                let point = Vec3d::new(
                    centre + outside * round.cos(),
                    side * centre + outside * round.sin(),
                    shape.rotor_height + edge * rings.height / 2.0,
                );
                let to = point - lens;
                let angle = (to.dot(axis) / to.length()).acos().to_degrees();
                nearest = nearest.min(angle);
            }
        }
    }
    assert!(
        nearest > reach,
        "a ring point is {nearest}° off the camera's axis; the picture reaches {reach}°"
    );
}
