//! The Map's solid parts, and the read-only line question.

use opendrone_maths::{Attitude, Fingerprinter, Vec3};
use parry3d_f64::bounding_volume::{Aabb, BoundingVolume};
use parry3d_f64::math::{Pose, Vector};
use parry3d_f64::query::{Ray, RayCast};
use parry3d_f64::shape::{SharedShape, TriMesh};

use crate::geometry::{from_parry, pose, to_parry};

/// One solid part of a Map, as plain data: what a Map's `.glb` collider tags
/// (`box`, `convex` and `trimesh`) become, and what the Test Maps are written
/// as. Positions are in world axes (east, north, up), in metres, from the
/// Map's origin.
#[derive(Clone, Debug, PartialEq)]
pub enum MapShape {
    /// A box.
    Box {
        centre: Vec3,
        /// Along the box's own axes.
        size: Vec3,
        /// Which way the box's own axes point in the world (it turns them
        /// into world directions, as a Quad's attitude turns its body axes).
        attitude: Attitude,
    },
    /// The smallest convex solid round these corners.
    Convex { corners: Vec<Vec3> },
    /// Triangles between corners, each listed by the index of its three
    /// corners, anticlockwise when seen from the side its surface faces. A
    /// closed mesh is a solid; an open one, such as a sheet of ground, is a
    /// surface.
    TriangleMesh {
        corners: Vec<Vec3>,
        triangles: Vec<[u32; 3]>,
    },
}

impl MapShape {
    /// Feeds the shape into a fingerprint. It starts with a number for its
    /// kind, and each of its lists with its length, so one shape after
    /// another reads back one way only.
    pub fn write_fingerprint(&self, f: &mut Fingerprinter) {
        let point = |f: &mut Fingerprinter, v: Vec3| f.write_f64s(&[v.x, v.y, v.z]);
        match self {
            MapShape::Box {
                centre,
                size,
                attitude,
            } => {
                f.write_u64(1);
                point(f, *centre);
                point(f, *size);
                f.write_f64s(&attitude.quaternion());
            }
            MapShape::Convex { corners } => {
                f.write_u64(2);
                f.write_u64(corners.len() as u64);
                for corner in corners {
                    point(f, *corner);
                }
            }
            MapShape::TriangleMesh { corners, triangles } => {
                f.write_u64(3);
                f.write_u64(corners.len() as u64);
                for corner in corners {
                    point(f, *corner);
                }
                f.write_u64(triangles.len() as u64);
                for triangle in triangles {
                    for corner in triangle {
                        f.write_u64(u64::from(*corner));
                    }
                }
            }
        }
    }
}

/// A Map shape that can't be a solid part.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum MapShapeProblem {
    /// A number in it isn't a real number.
    NotARealNumber,
    /// A box's size must be above zero along all three of its axes.
    BoxHasNoSize,
    /// A convex shape needs at least four corners that aren't all in one
    /// plane.
    ConvexHasNoVolume,
    /// A triangle mesh needs at least one triangle.
    MeshHasNoTriangles,
    /// A triangle names a corner the mesh doesn't have.
    MeshCornerMissing,
}

/// Which Map shape a problem is in, counting from 0 in the set-up's order.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct MapShapeError {
    pub shape: usize,
    pub problem: MapShapeProblem,
}

/// One Map surface the line passes through: where the line goes into it and
/// where it comes out. A line that ends inside a solid comes out where it
/// ends; one that starts inside goes in where it starts.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct LineCrossing {
    /// Which Map shape, counting from 0 in the set-up's order.
    pub shape: usize,
    /// In world axes.
    pub entry: Vec3,
    pub exit: Vec3,
    /// How far along the line, from its start, in metres.
    pub entry_distance: f64,
    pub exit_distance: f64,
}

/// The Map's solid parts, ready for the physics' geometry questions.
#[derive(Clone, Debug, Default)]
pub struct MapCollision {
    solids: Vec<Solid>,
}

#[derive(Clone, Debug)]
pub(crate) struct Solid {
    pub shape: SharedShape,
    pub pose: Pose,
    /// Its bounding box in the world.
    pub aabb: Aabb,
    /// True for a triangle mesh, false for a convex solid.
    pub is_mesh: bool,
}

impl MapCollision {
    /// Builds the Map's solid parts, or says which shape can't be one.
    pub fn new(shapes: &[MapShape]) -> Result<MapCollision, MapShapeError> {
        let solids = shapes
            .iter()
            .enumerate()
            .map(|(index, shape)| {
                Solid::new(shape).map_err(|problem| MapShapeError {
                    shape: index,
                    problem,
                })
            })
            .collect::<Result<_, _>>()?;
        Ok(MapCollision { solids })
    }

    /// True when the Map has no solid parts at all, as in empty air.
    pub fn is_empty(&self) -> bool {
        self.solids.is_empty()
    }

    pub(crate) fn solids(&self) -> &[Solid] {
        &self.solids
    }

    /// The line question: every Map surface the straight line from `from` to
    /// `to` passes through, with where it goes in and comes out, in order
    /// along the line (by entry, then by shape). It changes nothing.
    ///
    /// A convex solid gives at most one crossing. A closed triangle mesh
    /// gives one per stretch the line spends inside it, told apart by which
    /// way each triangle faces; an open one (a sheet) counts the side its
    /// triangles face away from as inside. A line that only grazes a surface
    /// crosses nothing.
    pub fn line_question(&self, from: Vec3, to: Vec3) -> Vec<LineCrossing> {
        let length = (to - from).length();
        if length == 0.0 || !length.is_finite() {
            return Vec::new();
        }
        let direction = (to - from) / length;
        let ray = Ray::new(to_parry(from), to_parry(direction));
        let line_box = Aabb::from_points([to_parry(from), to_parry(to)]);
        let mut crossings = Vec::new();
        for (index, solid) in self.solids.iter().enumerate() {
            if !solid.aabb.intersects(&line_box) {
                continue;
            }
            let stretches = if solid.is_mesh {
                mesh_stretches(solid, &ray, length)
            } else {
                convex_stretch(solid, from, to, direction, length)
                    .into_iter()
                    .collect()
            };
            for (entry_distance, exit_distance) in stretches {
                crossings.push(LineCrossing {
                    shape: index,
                    entry: from + direction * entry_distance,
                    exit: from + direction * exit_distance,
                    entry_distance,
                    exit_distance,
                });
            }
        }
        crossings.sort_by(|a, b| {
            a.entry_distance
                .total_cmp(&b.entry_distance)
                .then(a.shape.cmp(&b.shape))
        });
        crossings
    }
}

impl Solid {
    fn new(shape: &MapShape) -> Result<Solid, MapShapeProblem> {
        let real = |points: &[Vec3]| points.iter().all(|p| p.is_finite());
        let (shape, pose, is_mesh) = match shape {
            MapShape::Box {
                centre,
                size,
                attitude,
            } => {
                if !real(&[*centre, *size]) || !attitude.is_finite() {
                    return Err(MapShapeProblem::NotARealNumber);
                }
                if !(size.x > 0.0 && size.y > 0.0 && size.z > 0.0) {
                    return Err(MapShapeProblem::BoxHasNoSize);
                }
                let half = *size / 2.0;
                (
                    SharedShape::cuboid(half.x, half.y, half.z),
                    pose(*centre, *attitude),
                    false,
                )
            }
            MapShape::Convex { corners } => {
                if !real(corners) {
                    return Err(MapShapeProblem::NotARealNumber);
                }
                let points: Vec<Vector> = corners.iter().map(|c| to_parry(*c)).collect();
                let hull =
                    SharedShape::convex_hull(&points).ok_or(MapShapeProblem::ConvexHasNoVolume)?;
                // A hull flat in one plane has no inside to collide with.
                let local = hull.compute_local_aabb();
                let extent = local.maxs - local.mins;
                if !(extent.x > 0.0 && extent.y > 0.0 && extent.z > 0.0) {
                    return Err(MapShapeProblem::ConvexHasNoVolume);
                }
                (hull, Pose::IDENTITY, false)
            }
            MapShape::TriangleMesh { corners, triangles } => {
                if !real(corners) {
                    return Err(MapShapeProblem::NotARealNumber);
                }
                if triangles.is_empty() {
                    return Err(MapShapeProblem::MeshHasNoTriangles);
                }
                let count = corners.len();
                if triangles
                    .iter()
                    .flatten()
                    .any(|corner| *corner as usize >= count)
                {
                    return Err(MapShapeProblem::MeshCornerMissing);
                }
                // No pre-processing flags: parry3d's "pseudo-normals" for
                // internal edges are worked out with std's `acos`, which can
                // differ from one operating system to another (ADR-0001).
                let mesh = TriMesh::new(
                    corners.iter().map(|c| to_parry(*c)).collect(),
                    triangles.clone(),
                )
                .map_err(|_| MapShapeProblem::MeshHasNoTriangles)?;
                (SharedShape::new(mesh), Pose::IDENTITY, true)
            }
        };
        let aabb = shape.compute_aabb(&pose);
        Ok(Solid {
            shape,
            pose,
            aabb,
            is_mesh,
        })
    }
}

/// Where a line crosses a convex solid: in from the start, and out from the
/// end, each a ray cast that counts a start inside the solid as a hit at 0.
fn convex_stretch(
    solid: &Solid,
    from: Vec3,
    to: Vec3,
    direction: Vec3,
    length: f64,
) -> Option<(f64, f64)> {
    let forward = Ray::new(to_parry(from), to_parry(direction));
    let backward = Ray::new(to_parry(to), to_parry(-direction));
    let entry = solid.shape.cast_ray(&solid.pose, &forward, length, true)?;
    let from_the_end = solid.shape.cast_ray(&solid.pose, &backward, length, true)?;
    let exit = length - from_the_end;
    (exit > entry).then_some((entry, exit))
}

/// Every stretch a line spends inside a triangle mesh: every triangle it
/// crosses, in order along it, each going in (the triangle faces the line's
/// start) or coming out (it faces away).
fn mesh_stretches(solid: &Solid, ray: &Ray, length: f64) -> Vec<(f64, f64)> {
    let Some(mesh) = solid.shape.as_trimesh() else {
        return Vec::new();
    };
    let direction = from_parry(ray.dir);
    let mut hits: Vec<(f64, bool)> = mesh
        .bvh()
        .leaves(|node| node.cast_ray(ray, length) <= length)
        .filter_map(|index| {
            let triangle = mesh.triangle(index);
            let faces = from_parry((triangle.b - triangle.a).cross(triangle.c - triangle.a));
            let facing = faces.dot(direction);
            if facing == 0.0 {
                // Edge-on: the line runs along the triangle's plane.
                return None;
            }
            let distance = triangle.cast_local_ray(ray, length, false)?;
            Some((distance, facing < 0.0))
        })
        .collect();
    hits.sort_by(|a, b| a.0.total_cmp(&b.0).then(b.1.cmp(&a.1)));
    // A line through an edge or a corner shared by several triangles meets
    // each of them at the same spot: that is one crossing.
    hits.dedup_by(|later, earlier| later.1 == earlier.1 && later.0 - earlier.0 <= 1e-9);

    let mut stretches = Vec::new();
    let mut depth = 0u32;
    let mut entered = 0.0;
    for (distance, going_in) in hits {
        if going_in {
            if depth == 0 {
                entered = distance;
            }
            depth += 1;
        } else if depth == 0 {
            // Coming out first: the line started inside.
            stretches.push((0.0, distance));
        } else {
            depth -= 1;
            if depth == 0 {
                stretches.push((entered, distance));
            }
        }
    }
    if depth > 0 {
        stretches.push((entered, length));
    }
    stretches.retain(|(entry, exit)| exit > entry);
    stretches
}
