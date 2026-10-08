//! The collision stage: after a step's free move, the Quad meets the Map. The
//! crate's "The collision stage" says what it does, step by step.

use opendrone_maths::functions::max;
use opendrone_maths::{Attitude, Mat3, Vec3};
use parry3d_f64::bounding_volume::{Aabb, BoundingVolume};
use parry3d_f64::query::{
    self, ContactManifold, DefaultQueryDispatcher, PersistentQueryDispatcher, ShapeCastOptions,
    ShapeCastStatus,
};

use crate::QuadState;
use crate::geometry::{from_parry, pose, to_parry};
use crate::map::MapCollision;
use crate::shape::{Part, QuadPart};

/// How far beyond a step's own motion the physics looks for surfaces, in
/// metres: half a millimetre.
pub const SKIN: f64 = 0.0005;
/// How many times each step's pushes are worked through, point by point.
pub const PASSES: usize = 16;
/// How many times the bounce is worked through.
pub const BOUNCE_PASSES: usize = 4;
/// A touch slower than this, in m/s, doesn't bounce: 0.1 m/s is the speed of
/// a fall from about half a millimetre.
pub const BOUNCE_SPEED: f64 = 0.1;
/// Below this speed, in m/s, a Quad the Map holds stays still: 0.01 mm/s.
pub const REST_SPEED: f64 = 1e-5;
/// Below this rotation speed, in rad/s, a Quad the Map holds stays still:
/// about 0.006 °/s.
pub const REST_TURN: f64 = 1e-4;
/// How far a part may sit inside a surface before it is moved out, in
/// metres: a tenth of a millimetre.
pub const SINK_ALLOWANCE: f64 = 1e-4;
/// How many times the move out of the Map is worked through.
const OUT_PASSES: usize = 4;
/// The guard stops a move only when it would go at least this far into the
/// surface it meets, in metres: a micrometre. Sliding along a surface, or
/// closing a gap exactly as step 2 allows, never trips it.
const GUARD_DEPTH: f64 = 1e-6;

/// One place where the Map pushed the Quad during the last step.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Contact {
    /// Which part of the Quad it touched.
    pub part: QuadPart,
    /// Which Map shape, counting from 0 in the set-up's order.
    pub shape: usize,
    /// Where, in world axes: on the Quad's surface, where the step started.
    pub point: Vec3,
    /// The way the Map pushed the Quad, in world axes, with length one: out
    /// of the Map's surface.
    pub normal: Vec3,
    /// How hard the Map pushed, in newtons: the push over the step, divided
    /// by the step's length.
    pub push: f64,
    /// How hard, and which way, friction held or dragged the Quad along the
    /// surface, in newtons, in world axes, worked out the same way.
    pub friction: Vec3,
}

/// What the collision stage needs to know about the Quad.
pub(crate) struct Collider<'a> {
    pub mass: f64,
    pub inertia_inverse: Mat3,
    pub bounce: f64,
    pub friction: f64,
    pub parts: &'a [Part],
    /// How far the Quad's farthest point is from its centre of mass.
    pub reach: f64,
}

/// One direction at one contact point: how a push along it moves the Quad.
#[derive(Clone, Copy)]
struct Direction {
    /// In world axes.
    world: Vec3,
    /// The contact point's lever about the centre of mass, crossed with the
    /// direction, in body axes: a push along `world` turns the Quad about
    /// this.
    lever: Vec3,
    /// The mass a push along it meets: the Quad's mass and inertia, as felt
    /// at that point.
    mass: f64,
}

impl Direction {
    fn new(quad: &Collider<'_>, attitude: Attitude, at: Vec3, body: Vec3) -> Direction {
        let lever = at.cross(body);
        let give = 1.0 / quad.mass + lever.dot(quad.inertia_inverse * lever);
        Direction {
            world: attitude.body_to_world(body),
            lever,
            mass: 1.0 / give,
        }
    }

    /// How fast the contact point moves along this direction.
    fn speed(&self, velocity: Vec3, rotation: Vec3) -> f64 {
        velocity.dot(self.world) + rotation.dot(self.lever)
    }

    /// A push of `amount` (in N·s) along this direction.
    fn push(&self, quad: &Collider<'_>, amount: f64, velocity: &mut Vec3, rotation: &mut Vec3) {
        *velocity += self.world * (amount / quad.mass);
        *rotation += quad.inertia_inverse * self.lever * amount;
    }
}

/// One contact point.
struct Row {
    part: QuadPart,
    shape: usize,
    /// In world axes.
    point: Vec3,
    /// How far apart the Quad and the surface are at this point; negative
    /// when the part is inside.
    gap: f64,
    /// Out of the surface, then two directions along it.
    normal: Direction,
    along: [Direction; 2],
    /// How fast the point came toward the surface before any push (negative
    /// when it came in).
    came_in_at: f64,
    /// The pushes so far, in N·s.
    push: f64,
    rub: [f64; 2],
}

/// Meets the Map after a step's free move: `before` is the state the step
/// started from, `state` the free move's result, changed here only if the
/// Quad touches something. `contacts` is cleared and filled with every point
/// where the Map pushed.
pub(crate) fn collide(
    quad: &Collider<'_>,
    map: &MapCollision,
    before: &QuadState,
    state: &mut QuadState,
    dt: f64,
    contacts: &mut Vec<Contact>,
) {
    contacts.clear();
    if map.is_empty() {
        return;
    }
    let reach = state.velocity.length() * dt + state.rotation.length() * quad.reach * dt + SKIN;
    let within = quad.reach + reach;
    let around = Aabb::new(
        to_parry(before.position - Vec3::new(within, within, within)),
        to_parry(before.position + Vec3::new(within, within, within)),
    );
    let candidates: Vec<usize> = map
        .solids()
        .iter()
        .enumerate()
        .filter(|(_, solid)| solid.aabb.intersects(&around))
        .map(|(index, _)| index)
        .collect();
    if candidates.is_empty() {
        return;
    }

    let mut rows = contact_points(quad, map, &candidates, before, reach);
    if rows.is_empty() {
        return;
    }

    // 2. Push and friction.
    let mut velocity = state.velocity;
    let mut rotation = state.rotation;
    for row in &mut rows {
        row.came_in_at = row.normal.speed(velocity, rotation);
    }
    for _ in 0..PASSES {
        for row in &mut rows {
            let limit = quad.friction * row.push;
            let mut rub = [0.0; 2];
            for (i, along) in row.along.iter().enumerate() {
                rub[i] = row.rub[i] - along.speed(velocity, rotation) * along.mass;
            }
            let size = (rub[0] * rub[0] + rub[1] * rub[1]).sqrt();
            if size > limit {
                let scale = limit / size;
                rub = [rub[0] * scale, rub[1] * scale];
            }
            for (i, along) in row.along.iter().enumerate() {
                along.push(quad, rub[i] - row.rub[i], &mut velocity, &mut rotation);
            }
            row.rub = rub;

            // A gap may be closed this step, never crossed.
            let allowed = if row.gap > 0.0 { -row.gap / dt } else { 0.0 };
            let speed = row.normal.speed(velocity, rotation);
            let push = max(row.push + (allowed - speed) * row.normal.mass, 0.0);
            row.normal
                .push(quad, push - row.push, &mut velocity, &mut rotation);
            row.push = push;
        }
    }

    // 3. Bounce.
    let hit = |row: &Row| row.came_in_at < -BOUNCE_SPEED && row.push > 0.0;
    for _ in 0..BOUNCE_PASSES {
        for row in rows.iter_mut().filter(|row| hit(row)) {
            let out = -quad.bounce * row.came_in_at;
            let speed = row.normal.speed(velocity, rotation);
            let push = max(row.push + (out - speed) * row.normal.mass, 0.0);
            row.normal
                .push(quad, push - row.push, &mut velocity, &mut rotation);
            row.push = push;
        }
    }

    // 4. Rest.
    let held = rows.iter().any(|row| row.push > 0.0)
        && !rows.iter().any(hit)
        && velocity.length() < REST_SPEED
        && rotation.length() < REST_TURN;
    if held {
        velocity = Vec3::ZERO;
        rotation = Vec3::ZERO;
    }

    // 5. The move, redone with the new speeds.
    let pushed = rows
        .iter()
        .any(|row| row.push != 0.0 || row.rub != [0.0, 0.0]);
    if pushed || held {
        state.velocity = velocity;
        state.rotation = rotation;
        state.position = before.position + velocity * dt;
        state.attitude = before.attitude.turned_by(rotation * dt);
    }

    // 6. The guard.
    let motion = state.position - before.position;
    if motion != Vec3::ZERO
        && let Some((fraction, normal)) = first_hit(quad, map, &candidates, before, motion)
    {
        state.position = before.position + motion * fraction;
        let into = state.velocity.dot(normal);
        if into < 0.0 {
            state.velocity -= normal * into;
        }
    }

    // 7. Out of the Map.
    let moved = state.position - before.position;
    let turned = state.rotation * dt;
    let mut out = Vec3::ZERO;
    for _ in 0..OUT_PASSES {
        for row in &rows {
            let n = &row.normal;
            let gap = row.gap + (moved + out).dot(n.world) + turned.dot(n.lever);
            let sunk = -gap - SINK_ALLOWANCE;
            if sunk > 0.0 {
                out += n.world * sunk;
            }
        }
    }
    state.position += out;

    for row in rows.iter().filter(|row| row.push > 0.0) {
        contacts.push(Contact {
            part: row.part,
            shape: row.shape,
            point: row.point,
            normal: row.normal.world,
            push: row.push / dt,
            friction: (row.along[0].world * row.rub[0] + row.along[1].world * row.rub[1]) / dt,
        });
    }
}

/// 1. Every contact point within `reach` of the Quad where the step started:
///    each part against each Map shape that could be reached, in a fixed
///    order.
fn contact_points(
    quad: &Collider<'_>,
    map: &MapCollision,
    candidates: &[usize],
    before: &QuadState,
    reach: f64,
) -> Vec<Row> {
    let start = pose(before.position, before.attitude);
    let mut rows = Vec::new();
    let mut manifolds: Vec<ContactManifold<(), ()>> = Vec::new();
    for part in quad.parts {
        let part_pose = start * part.local;
        let part_box = part.shape.compute_aabb(&part_pose).loosened(reach);
        for &index in candidates {
            let solid = &map.solids()[index];
            if !part_box.intersects(&solid.aabb) {
                continue;
            }
            // A fresh question every step: parry3d may reuse what it found
            // last time, and the Simulation remembers nothing between steps.
            manifolds.clear();
            let mut workspace = None;
            let asked = DefaultQueryDispatcher.contact_manifolds(
                &part_pose.inv_mul(&solid.pose),
                &*part.shape,
                &*solid.shape,
                reach,
                &mut manifolds,
                &mut workspace,
            );
            // Every pair of shapes the physics builds is one parry3d answers.
            debug_assert!(asked.is_ok(), "parry3d can't pair {:?}", part.part);
            for manifold in &manifolds {
                // parry3d's normal points out of the part, toward the Map;
                // the Map pushes the other way.
                let normal = -from_parry(part.local.rotation * manifold.local_n1);
                for point in &manifold.points {
                    let at = from_parry(part.local.transform_point(point.local_p1));
                    rows.push(row(quad, part.part, index, before, at, normal, point.dist));
                }
            }
        }
    }
    rows
}

fn row(
    quad: &Collider<'_>,
    part: QuadPart,
    shape: usize,
    before: &QuadState,
    at: Vec3,
    normal: Vec3,
    gap: f64,
) -> Row {
    let attitude = before.attitude;
    let [first, second] = along(normal);
    Row {
        part,
        shape,
        point: before.position + attitude.body_to_world(at),
        gap,
        normal: Direction::new(quad, attitude, at, normal),
        along: [
            Direction::new(quad, attitude, at, first),
            Direction::new(quad, attitude, at, second),
        ],
        came_in_at: 0.0,
        push: 0.0,
        rub: [0.0; 2],
    }
}

/// Two directions at right angles to `normal` and to each other.
fn along(normal: Vec3) -> [Vec3; 2] {
    // Cross with the axis least in line with the normal, so the result is
    // never tiny.
    let (x, y, z) = (normal.x.abs(), normal.y.abs(), normal.z.abs());
    let axis = if x <= y && x <= z {
        Vec3::new(1.0, 0.0, 0.0)
    } else if y <= z {
        Vec3::new(0.0, 1.0, 0.0)
    } else {
        Vec3::new(0.0, 0.0, 1.0)
    };
    let first = normal.cross(axis);
    let first = first / first.length();
    [first, normal.cross(first)]
}

/// 6. The guard: sweeps every part along `motion` from where the step
///    started, and gives the first surface met (as a share of the move, and
///    the way it pushes out), if one is met before the move ends with the
///    move going into it.
fn first_hit(
    quad: &Collider<'_>,
    map: &MapCollision,
    candidates: &[usize],
    before: &QuadState,
    motion: Vec3,
) -> Option<(f64, Vec3)> {
    let start = pose(before.position, before.attitude);
    let sweep = motion.length();
    let options = ShapeCastOptions {
        max_time_of_impact: 1.0,
        target_distance: 0.0,
        stop_at_penetration: false,
        compute_impact_geometry_on_penetration: false,
    };
    let mut first: Option<(f64, Vec3)> = None;
    for part in quad.parts {
        let part_pose = start * part.local;
        let swept_box = part.shape.compute_aabb(&part_pose).loosened(sweep);
        for &index in candidates {
            let solid = &map.solids()[index];
            if !swept_box.intersects(&solid.aabb) {
                continue;
            }
            let Ok(Some(hit)) = query::cast_shapes(
                &part_pose,
                to_parry(motion),
                &*part.shape,
                &solid.pose,
                to_parry(Vec3::ZERO),
                &*solid.shape,
                options,
            ) else {
                continue;
            };
            // Already touching where the step started: step 2 dealt with it.
            if hit.status == ShapeCastStatus::PenetratingOrWithinTargetDist {
                continue;
            }
            let normal = -from_parry(part_pose.rotation * hit.normal1);
            let goes_in = motion.dot(normal) * (1.0 - hit.time_of_impact) < -GUARD_DEPTH;
            if goes_in && first.is_none_or(|(fraction, _)| hit.time_of_impact < fraction) {
                first = Some((hit.time_of_impact, normal));
            }
        }
    }
    first
}
