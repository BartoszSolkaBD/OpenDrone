//! Readable checks for the developer command that draws a Map from any
//! camera into PNG files (#291): how a view is read, where the camera is when
//! a view leaves it out, and what the files are called. The window part is
//! checked by hand: `docs/building-a-map.md` says how. Lengths in metres,
//! angles in degrees.

mod common;

use opendrone::camera::look::VideoLook;
use opendrone::map_views::{Kind, PLAIN_FOV, ViewRequest, file_name};
use opendrone::set_up::{FlightPlan, landed_height};
use opendrone::settings::Settings;
use opendrone_maths::Vec3;

fn plan(quad: &str, map: &str) -> FlightPlan {
    FlightPlan::from_packs(&common::packs(), Settings::defaults(quad).on_map(map))
        .unwrap_or_else(|_| panic!("{map} is in the built-in Pack"))
}

fn near(a: Vec3, b: Vec3) -> bool {
    (a - b).length() < 1e-9
}

#[test]
fn a_view_reads_its_name_position_aim_fov_and_look() {
    // Basis: Rule (#291): a view takes a position, a look direction or a
    // target point, a FOV, and a choice of plain or a Video Look.
    let view = ViewRequest::parse("name=front at=0,-5,1.5 toward=0,0,2 fov=100 look=digital")
        .expect("a full view reads");
    assert_eq!(view.name, "front");
    assert_eq!(view.at, Some(Vec3::new(0.0, -5.0, 1.5)));
    assert_eq!(view.toward, Some(Vec3::new(0.0, 0.0, 2.0)));
    assert_eq!(view.fov, Some(100.0));
    assert_eq!(view.kind, Kind::Video(VideoLook::Digital));

    let aimed = ViewRequest::parse("heading=90 pitch=-10 look=analog").expect("an aimed view");
    assert_eq!((aimed.heading, aimed.pitch), (Some(90.0), Some(-10.0)));
    assert_eq!(aimed.kind, Kind::Video(VideoLook::Analog));
    assert_eq!(aimed.name, "view");
}

#[test]
fn a_view_with_no_look_is_plain() {
    assert_eq!(ViewRequest::parse("at=1,2,3").unwrap().kind, Kind::Plain);
    assert_eq!(ViewRequest::parse("look=plain").unwrap().kind, Kind::Plain);
}

#[test]
fn a_view_that_cannot_be_read_says_why() {
    for (text, says) in [
        ("zoom=3", "isn't a view key"),
        ("at=1,2", "east,north,up"),
        ("at=1,2,x", "takes a number"),
        ("fov=wide", "takes a number"),
        ("look=night", "plain, analog or digital"),
        ("pitch=120", "-90"),
        ("name=a/b", "letters, digits"),
        ("fov=90 fov=100", "twice"),
        ("toward=0,0,0 heading=10", "toward goes alone"),
        ("front", "key=value"),
    ] {
        let problem = ViewRequest::parse(text).expect_err(text);
        assert!(problem.contains(says), "{text}: {problem}");
    }
}

#[test]
fn a_list_of_views_is_one_a_line_and_a_bad_line_is_named() {
    let views =
        ViewRequest::parse_list("# the front\nname=a at=0,0,1\n\n  name=b look=digital  \n")
            .expect("a list reads");
    let names: Vec<&str> = views.iter().map(|v| v.name.as_str()).collect();
    assert_eq!(names, ["a", "b"]);
    let problem = ViewRequest::parse_list("name=a\nname=b zoom=1\n").expect_err("a bad line");
    assert!(problem.starts_with("line 2:"), "{problem}");
}

#[test]
fn with_nothing_asked_the_camera_is_the_quads_camera_on_the_launch_spot() {
    // Basis: Rule (#291): the Quad's launch spot is the default position.
    // The Whoop 65's camera is 40 mm forward and 16 mm up of its centre of
    // mass (quad.toml), the Quad landed facing the spot's heading: north on
    // Bando, west on the Skate Park. It looks along that heading at the Quad's
    // 30° Camera Tilt.
    let tilt = 30f64.to_radians();
    for (map, forward) in [
        ("opendrone/bando", Vec3::new(0.0, 1.0, 0.0)),
        ("opendrone/skate-park", Vec3::new(-1.0, 0.0, 0.0)),
    ] {
        let plan = plan("whoop-65", map);
        let spot = plan
            .map
            .launch_spot
            .as_ref()
            .expect("a Launch Spot")
            .position;
        let landed = landed_height(&plan.quad.parameters.shape);
        let view = ViewRequest::launch().resolve(&plan).expect("resolves");
        let camera = spot + forward * 0.040 + Vec3::new(0.0, 0.0, landed + 0.016);
        assert!(near(view.position, camera), "{map}: {:?}", view.position);
        let looking = forward * tilt.cos() + Vec3::new(0.0, 0.0, tilt.sin());
        assert!(near(view.direction, looking), "{map}: {:?}", view.direction);
        assert_eq!(view.fov, PLAIN_FOV);
    }
}

#[test]
fn a_video_look_view_has_the_quads_own_fov_unless_it_says() {
    // Basis: Rule (quad.toml): the Whoop 65's camera has a 160° FOV.
    let plan = plan("whoop-65", "opendrone/bando");
    let own = ViewRequest::parse("look=analog")
        .unwrap()
        .resolve(&plan)
        .unwrap();
    assert!((own.fov - 160.0).abs() < 1e-9);
    let set = ViewRequest::parse("look=digital fov=120")
        .unwrap()
        .resolve(&plan)
        .unwrap();
    assert_eq!(set.fov, 120.0);
}

#[test]
fn a_target_point_aims_the_camera_at_it() {
    let plan = plan("whoop-65", "opendrone/bando");
    let view = ViewRequest::parse("at=0,-10,2 toward=0,0,2")
        .unwrap()
        .resolve(&plan)
        .unwrap();
    assert!(near(view.direction, Vec3::new(0.0, 1.0, 0.0)));
    assert_eq!(view.position, Vec3::new(0.0, -10.0, 2.0));
}

#[test]
fn a_heading_and_pitch_aim_the_camera_by_compass() {
    let plan = plan("whoop-65", "opendrone/bando");
    let east = ViewRequest::parse("heading=90 pitch=0")
        .unwrap()
        .resolve(&plan)
        .unwrap();
    assert!(near(east.direction, Vec3::new(1.0, 0.0, 0.0)));
    let down = ViewRequest::parse("heading=0 pitch=-90")
        .unwrap()
        .resolve(&plan)
        .unwrap();
    assert!(near(down.direction, Vec3::new(0.0, 0.0, -1.0)));
}

#[test]
fn a_view_that_looks_at_its_own_position_or_has_a_wild_fov_is_refused() {
    let plan = plan("whoop-65", "opendrone/bando");
    let same = ViewRequest::parse("at=1,2,3 toward=1,2,3")
        .unwrap()
        .resolve(&plan);
    assert!(same.expect_err("no direction").contains("same point"));
    // The game's FOV runs from 90° to 170°; a plain lens has more room.
    for (text, fine) in [
        ("look=analog fov=60", false),
        ("look=analog fov=170", true),
        ("fov=60", true),
        ("fov=10", false),
        ("fov=175", false),
    ] {
        let resolved = ViewRequest::parse(text).unwrap().resolve(&plan);
        assert_eq!(resolved.is_ok(), fine, "{text}");
    }
}

#[test]
fn each_picture_is_named_after_its_map_its_place_in_the_list_and_its_name() {
    assert_eq!(
        file_name("opendrone/bando", 1, "front"),
        "bando-01-front.png"
    );
    assert_eq!(
        file_name("opendrone/skate-park", 12, "view"),
        "skate-park-12-view.png"
    );
    assert_eq!(
        file_name("test/flat-floor", 2, "launch"),
        "flat-floor-02-launch.png"
    );
}

/// Runs the game's command line with these arguments, which must stop before
/// any window opens: the exit code and what it said on the error stream.
fn command_line(args: &[&str]) -> (Option<i32>, String) {
    let output = std::process::Command::new(env!("CARGO_BIN_EXE_opendrone"))
        .args(args)
        .output()
        .expect("the game starts");
    (
        output.status.code(),
        String::from_utf8_lossy(&output.stderr).into_owned(),
    )
}

#[test]
fn the_command_line_refuses_a_view_it_cannot_read_before_opening_a_window() {
    let (code, said) = command_line(&["--render-views", "target/never-made", "--view", "zoom=3"]);
    assert_eq!(code, Some(2));
    assert!(said.contains("--view: \"zoom\" isn't a view key"), "{said}");
    let (code, said) = command_line(&["--render-views"]);
    assert_eq!(code, Some(2));
    assert!(said.contains("takes the folder"), "{said}");
}

#[test]
fn the_command_line_refuses_views_without_the_command_that_draws_them() {
    let (code, said) = command_line(&["--view", "name=a"]);
    assert_eq!(code, Some(2));
    assert!(said.contains("go with --render-views"), "{said}");
}

#[test]
fn the_command_line_refuses_the_settings_each_view_sets_for_itself() {
    let (code, said) = command_line(&["--render-views", "target/never-made", "--look", "digital"]);
    assert_eq!(code, Some(2));
    assert!(said.contains("in each view"), "{said}");
}
