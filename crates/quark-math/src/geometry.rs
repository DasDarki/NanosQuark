use std::f64::consts::TAU;

use mlua::prelude::*;
use quark_core::{Options, create_userdata};
use quark_random::{Rng, with_rng};

use crate::quaternion::{Quaternion, read_quaternion};
use quark_core::{Vec3, make_vector, make_vector2, read_vector};

const MAX_POINTS: usize = 1_000_000;

#[derive(Clone)]
struct Polygon {
    points: Vec<(f64, f64)>,
    triangles: Vec<[usize; 3]>,
    cumulative: Vec<f64>,
    area: f64,
}

#[derive(Clone)]
enum Shape {
    Circle {
        center: (f64, f64),
        radius: f64,
    },
    Rectangle {
        center: (f64, f64),
        extent: (f64, f64),
        angle: f64,
    },
    Polygon(Polygon),
    Sphere {
        center: Vec3,
        radius: f64,
    },
    Box {
        center: Vec3,
        extent: Vec3,
        rotation: Quaternion,
    },
    Cylinder {
        center: Vec3,
        radius: f64,
        half_height: f64,
    },
    Capsule {
        start: Vec3,
        end: Vec3,
        radius: f64,
    },
    Prism {
        polygon: Polygon,
        minimum: f64,
        maximum: f64,
    },
    Union {
        shapes: Vec<Shape>,
        cumulative: Vec<f64>,
        three_dimensional: bool,
    },
}

fn signed_area(points: &[(f64, f64)]) -> f64 {
    let mut area = 0.0;
    for index in 0..points.len() {
        let (x1, y1) = points[index];
        let (x2, y2) = points[(index + 1) % points.len()];
        area += x1 * y2 - x2 * y1;
    }
    area / 2.0
}

fn point_in_polygon(points: &[(f64, f64)], x: f64, y: f64) -> bool {
    let mut inside = false;
    let mut previous = points.len() - 1;
    for index in 0..points.len() {
        let (xi, yi) = points[index];
        let (xj, yj) = points[previous];
        if (yi > y) != (yj > y) && x < (xj - xi) * (y - yi) / (yj - yi) + xi {
            inside = !inside;
        }
        previous = index;
    }
    inside
}

fn closest_on_segment_2d(point: (f64, f64), start: (f64, f64), end: (f64, f64)) -> (f64, f64) {
    let (dx, dy) = (end.0 - start.0, end.1 - start.1);
    let length = dx * dx + dy * dy;
    if length < 1e-18 {
        return start;
    }
    let t = (((point.0 - start.0) * dx + (point.1 - start.1) * dy) / length).clamp(0.0, 1.0);
    (start.0 + dx * t, start.1 + dy * t)
}

pub fn closest_on_segment(point: Vec3, start: Vec3, end: Vec3) -> Vec3 {
    let direction = end.subtract(start);
    let length = direction.dot(direction);
    if length < 1e-18 {
        return start;
    }
    let t = (point.subtract(start).dot(direction) / length).clamp(0.0, 1.0);
    start.plus(direction.scale(t))
}

fn distance_2d(first: (f64, f64), second: (f64, f64)) -> f64 {
    ((first.0 - second.0).powi(2) + (first.1 - second.1).powi(2)).sqrt()
}

impl Polygon {
    fn new(points: Vec<(f64, f64)>) -> LuaResult<Self> {
        if points.len() < 3 {
            return Err(LuaError::runtime("a polygon needs at least 3 points"));
        }
        let flat: Vec<f64> = points.iter().flat_map(|(x, y)| [*x, *y]).collect();
        let indices = earcutr::earcut(&flat, &[], 2).map_err(|error| {
            LuaError::runtime(format!("the polygon cannot be triangulated: {error:?}"))
        })?;
        let triangles: Vec<[usize; 3]> = indices.as_chunks::<3>().0.to_vec();
        if triangles.is_empty() {
            return Err(LuaError::runtime("the polygon has no area"));
        }
        let mut cumulative = Vec::with_capacity(triangles.len());
        let mut total = 0.0;
        for [a, b, c] in &triangles {
            total += signed_area(&[points[*a], points[*b], points[*c]]).abs();
            cumulative.push(total);
        }
        Ok(Polygon {
            points,
            triangles,
            cumulative,
            area: total,
        })
    }

    fn contains(&self, x: f64, y: f64) -> bool {
        point_in_polygon(&self.points, x, y)
    }

    fn random(&self, rng: &mut Rng) -> (f64, f64) {
        let target = rng.float() * self.area;
        let index = self
            .cumulative
            .partition_point(|value| *value < target)
            .min(self.triangles.len() - 1);
        let [a, b, c] = self.triangles[index];
        let (a, b, c) = (self.points[a], self.points[b], self.points[c]);
        let (mut u, mut v) = (rng.float(), rng.float());
        if u + v > 1.0 {
            u = 1.0 - u;
            v = 1.0 - v;
        }
        (
            a.0 + u * (b.0 - a.0) + v * (c.0 - a.0),
            a.1 + u * (b.1 - a.1) + v * (c.1 - a.1),
        )
    }

    fn closest(&self, x: f64, y: f64) -> (f64, f64) {
        if self.contains(x, y) {
            return (x, y);
        }
        let mut best = self.points[0];
        let mut best_distance = f64::INFINITY;
        for index in 0..self.points.len() {
            let candidate = closest_on_segment_2d(
                (x, y),
                self.points[index],
                self.points[(index + 1) % self.points.len()],
            );
            let distance = distance_2d(candidate, (x, y));
            if distance < best_distance {
                best = candidate;
                best_distance = distance;
            }
        }
        best
    }

    fn bounds(&self) -> ((f64, f64), (f64, f64)) {
        let mut minimum = (f64::INFINITY, f64::INFINITY);
        let mut maximum = (f64::NEG_INFINITY, f64::NEG_INFINITY);
        for (x, y) in &self.points {
            minimum = (minimum.0.min(*x), minimum.1.min(*y));
            maximum = (maximum.0.max(*x), maximum.1.max(*y));
        }
        (minimum, maximum)
    }

    fn centroid(&self) -> (f64, f64) {
        let area = signed_area(&self.points);
        if area.abs() < 1e-12 {
            let count = self.points.len() as f64;
            return (
                self.points.iter().map(|point| point.0).sum::<f64>() / count,
                self.points.iter().map(|point| point.1).sum::<f64>() / count,
            );
        }
        let (mut x, mut y) = (0.0, 0.0);
        for index in 0..self.points.len() {
            let (x1, y1) = self.points[index];
            let (x2, y2) = self.points[(index + 1) % self.points.len()];
            let cross = x1 * y2 - x2 * y1;
            x += (x1 + x2) * cross;
            y += (y1 + y2) * cross;
        }
        (x / (6.0 * area), y / (6.0 * area))
    }
}

fn random_in_disc(rng: &mut Rng, radius: f64) -> (f64, f64) {
    let distance = radius * rng.float().sqrt();
    let angle = TAU * rng.float();
    (distance * angle.cos(), distance * angle.sin())
}

fn rotate_2d((x, y): (f64, f64), degrees: f64) -> (f64, f64) {
    let (sin, cos) = degrees.to_radians().sin_cos();
    (x * cos - y * sin, x * sin + y * cos)
}

impl Shape {
    fn is_3d(&self) -> bool {
        match self {
            Shape::Circle { .. } | Shape::Rectangle { .. } | Shape::Polygon(_) => false,
            Shape::Union {
                three_dimensional, ..
            } => *three_dimensional,
            _ => true,
        }
    }

    fn type_name(&self) -> &'static str {
        match self {
            Shape::Circle { .. } => "Circle",
            Shape::Rectangle { .. } => "Rectangle",
            Shape::Polygon(_) => "Polygon",
            Shape::Sphere { .. } => "Sphere",
            Shape::Box { .. } => "Box",
            Shape::Cylinder { .. } => "Cylinder",
            Shape::Capsule { .. } => "Capsule",
            Shape::Prism { .. } => "Prism",
            Shape::Union { .. } => "Union",
        }
    }

    fn measure(&self) -> f64 {
        match self {
            Shape::Circle { radius, .. } => std::f64::consts::PI * radius * radius,
            Shape::Rectangle { extent, .. } => 4.0 * extent.0 * extent.1,
            Shape::Polygon(polygon) => polygon.area,
            Shape::Sphere { radius, .. } => 4.0 / 3.0 * std::f64::consts::PI * radius.powi(3),
            Shape::Box { extent, .. } => 8.0 * extent.x * extent.y * extent.z,
            Shape::Cylinder {
                radius,
                half_height,
                ..
            } => std::f64::consts::PI * radius * radius * 2.0 * half_height,
            Shape::Capsule { start, end, radius } => {
                std::f64::consts::PI
                    * radius
                    * radius
                    * (end.subtract(*start).length() + 4.0 / 3.0 * radius)
            }
            Shape::Prism {
                polygon,
                minimum,
                maximum,
            } => polygon.area * (maximum - minimum),
            Shape::Union { cumulative, .. } => cumulative.last().copied().unwrap_or(0.0),
        }
    }

    fn contains(&self, point: Vec3) -> bool {
        match self {
            Shape::Circle { center, radius } => distance_2d((point.x, point.y), *center) <= *radius,
            Shape::Rectangle {
                center,
                extent,
                angle,
            } => {
                let (x, y) = rotate_2d((point.x - center.0, point.y - center.1), -angle);
                x.abs() <= extent.0 && y.abs() <= extent.1
            }
            Shape::Polygon(polygon) => polygon.contains(point.x, point.y),
            Shape::Sphere { center, radius } => point.subtract(*center).length() <= *radius,
            Shape::Box {
                center,
                extent,
                rotation,
            } => {
                let local = rotation.inverse().rotate(point.subtract(*center));
                local.x.abs() <= extent.x && local.y.abs() <= extent.y && local.z.abs() <= extent.z
            }
            Shape::Cylinder {
                center,
                radius,
                half_height,
            } => {
                (point.z - center.z).abs() <= *half_height
                    && distance_2d((point.x, point.y), (center.x, center.y)) <= *radius
            }
            Shape::Capsule { start, end, radius } => {
                point
                    .subtract(closest_on_segment(point, *start, *end))
                    .length()
                    <= *radius
            }
            Shape::Prism {
                polygon,
                minimum,
                maximum,
            } => point.z >= *minimum && point.z <= *maximum && polygon.contains(point.x, point.y),
            Shape::Union { shapes, .. } => shapes.iter().any(|shape| shape.contains(point)),
        }
    }

    fn bounds(&self) -> (Vec3, Vec3) {
        match self {
            Shape::Circle { center, radius } => (
                Vec3::new(center.0 - radius, center.1 - radius, 0.0),
                Vec3::new(center.0 + radius, center.1 + radius, 0.0),
            ),
            Shape::Rectangle {
                center,
                extent,
                angle,
            } => {
                let corners = [(-1.0, -1.0), (1.0, -1.0), (1.0, 1.0), (-1.0, 1.0)]
                    .map(|(sx, sy)| rotate_2d((sx * extent.0, sy * extent.1), *angle));
                let half_x = corners
                    .iter()
                    .map(|corner| corner.0.abs())
                    .fold(0.0, f64::max);
                let half_y = corners
                    .iter()
                    .map(|corner| corner.1.abs())
                    .fold(0.0, f64::max);
                (
                    Vec3::new(center.0 - half_x, center.1 - half_y, 0.0),
                    Vec3::new(center.0 + half_x, center.1 + half_y, 0.0),
                )
            }
            Shape::Polygon(polygon) => {
                let (minimum, maximum) = polygon.bounds();
                (
                    Vec3::new(minimum.0, minimum.1, 0.0),
                    Vec3::new(maximum.0, maximum.1, 0.0),
                )
            }
            Shape::Sphere { center, radius } => (
                center.subtract(Vec3::new(*radius, *radius, *radius)),
                center.plus(Vec3::new(*radius, *radius, *radius)),
            ),
            Shape::Box {
                center,
                extent,
                rotation,
            } => {
                let mut half = Vec3::default();
                for sx in [-1.0, 1.0] {
                    for sy in [-1.0, 1.0] {
                        for sz in [-1.0, 1.0] {
                            let corner = rotation.rotate(Vec3::new(
                                sx * extent.x,
                                sy * extent.y,
                                sz * extent.z,
                            ));
                            half = Vec3::new(
                                half.x.max(corner.x.abs()),
                                half.y.max(corner.y.abs()),
                                half.z.max(corner.z.abs()),
                            );
                        }
                    }
                }
                (center.subtract(half), center.plus(half))
            }
            Shape::Cylinder {
                center,
                radius,
                half_height,
            } => (
                center.subtract(Vec3::new(*radius, *radius, *half_height)),
                center.plus(Vec3::new(*radius, *radius, *half_height)),
            ),
            Shape::Capsule { start, end, radius } => (
                Vec3::new(
                    start.x.min(end.x) - radius,
                    start.y.min(end.y) - radius,
                    start.z.min(end.z) - radius,
                ),
                Vec3::new(
                    start.x.max(end.x) + radius,
                    start.y.max(end.y) + radius,
                    start.z.max(end.z) + radius,
                ),
            ),
            Shape::Prism {
                polygon,
                minimum,
                maximum,
            } => {
                let (low, high) = polygon.bounds();
                (
                    Vec3::new(low.0, low.1, *minimum),
                    Vec3::new(high.0, high.1, *maximum),
                )
            }
            Shape::Union { shapes, .. } => {
                let mut minimum = Vec3::new(f64::INFINITY, f64::INFINITY, f64::INFINITY);
                let mut maximum =
                    Vec3::new(f64::NEG_INFINITY, f64::NEG_INFINITY, f64::NEG_INFINITY);
                for shape in shapes {
                    let (low, high) = shape.bounds();
                    minimum = Vec3::new(
                        minimum.x.min(low.x),
                        minimum.y.min(low.y),
                        minimum.z.min(low.z),
                    );
                    maximum = Vec3::new(
                        maximum.x.max(high.x),
                        maximum.y.max(high.y),
                        maximum.z.max(high.z),
                    );
                }
                (minimum, maximum)
            }
        }
    }

    fn center(&self) -> Vec3 {
        match self {
            Shape::Circle { center, .. } | Shape::Rectangle { center, .. } => {
                Vec3::new(center.0, center.1, 0.0)
            }
            Shape::Polygon(polygon) => {
                let (x, y) = polygon.centroid();
                Vec3::new(x, y, 0.0)
            }
            Shape::Sphere { center, .. }
            | Shape::Box { center, .. }
            | Shape::Cylinder { center, .. } => *center,
            Shape::Capsule { start, end, .. } => start.lerp(*end, 0.5),
            Shape::Prism {
                polygon,
                minimum,
                maximum,
            } => {
                let (x, y) = polygon.centroid();
                Vec3::new(x, y, (minimum + maximum) / 2.0)
            }
            Shape::Union { .. } => {
                let (minimum, maximum) = self.bounds();
                minimum.lerp(maximum, 0.5)
            }
        }
    }

    fn random(&self, rng: &mut Rng) -> Vec3 {
        match self {
            Shape::Circle { center, radius } => {
                let (x, y) = random_in_disc(rng, *radius);
                Vec3::new(center.0 + x, center.1 + y, 0.0)
            }
            Shape::Rectangle {
                center,
                extent,
                angle,
            } => {
                let local = (
                    (rng.float() * 2.0 - 1.0) * extent.0,
                    (rng.float() * 2.0 - 1.0) * extent.1,
                );
                let (x, y) = rotate_2d(local, *angle);
                Vec3::new(center.0 + x, center.1 + y, 0.0)
            }
            Shape::Polygon(polygon) => {
                let (x, y) = polygon.random(rng);
                Vec3::new(x, y, 0.0)
            }
            Shape::Sphere { center, radius } => {
                let z = rng.float() * 2.0 - 1.0;
                let angle = TAU * rng.float();
                let ring = (1.0 - z * z).sqrt();
                let distance = radius * rng.float().cbrt();
                center.plus(Vec3::new(ring * angle.cos(), ring * angle.sin(), z).scale(distance))
            }
            Shape::Box {
                center,
                extent,
                rotation,
            } => {
                let local = Vec3::new(
                    (rng.float() * 2.0 - 1.0) * extent.x,
                    (rng.float() * 2.0 - 1.0) * extent.y,
                    (rng.float() * 2.0 - 1.0) * extent.z,
                );
                center.plus(rotation.rotate(local))
            }
            Shape::Cylinder {
                center,
                radius,
                half_height,
            } => {
                let (x, y) = random_in_disc(rng, *radius);
                center.plus(Vec3::new(x, y, (rng.float() * 2.0 - 1.0) * half_height))
            }
            Shape::Capsule { .. } => {
                let (minimum, maximum) = self.bounds();
                loop {
                    let candidate = Vec3::new(
                        minimum.x + rng.float() * (maximum.x - minimum.x),
                        minimum.y + rng.float() * (maximum.y - minimum.y),
                        minimum.z + rng.float() * (maximum.z - minimum.z),
                    );
                    if self.contains(candidate) {
                        return candidate;
                    }
                }
            }
            Shape::Prism {
                polygon,
                minimum,
                maximum,
            } => {
                let (x, y) = polygon.random(rng);
                Vec3::new(x, y, minimum + rng.float() * (maximum - minimum))
            }
            Shape::Union {
                shapes, cumulative, ..
            } => {
                let target = rng.float() * cumulative.last().copied().unwrap_or(0.0);
                let index = cumulative
                    .partition_point(|value| *value < target)
                    .min(shapes.len() - 1);
                shapes[index].random(rng)
            }
        }
    }

    fn closest(&self, point: Vec3) -> Vec3 {
        if self.contains(point) {
            return if self.is_3d() {
                point
            } else {
                Vec3::new(point.x, point.y, 0.0)
            };
        }
        match self {
            Shape::Circle { center, radius } => {
                let (dx, dy) = (point.x - center.0, point.y - center.1);
                let length = (dx * dx + dy * dy).sqrt().max(1e-12);
                Vec3::new(
                    center.0 + dx / length * radius,
                    center.1 + dy / length * radius,
                    0.0,
                )
            }
            Shape::Rectangle {
                center,
                extent,
                angle,
            } => {
                let (x, y) = rotate_2d((point.x - center.0, point.y - center.1), -angle);
                let (x, y) = rotate_2d(
                    (x.clamp(-extent.0, extent.0), y.clamp(-extent.1, extent.1)),
                    *angle,
                );
                Vec3::new(center.0 + x, center.1 + y, 0.0)
            }
            Shape::Polygon(polygon) => {
                let (x, y) = polygon.closest(point.x, point.y);
                Vec3::new(x, y, 0.0)
            }
            Shape::Sphere { center, radius } => {
                center.plus(point.subtract(*center).normalized().scale(*radius))
            }
            Shape::Box {
                center,
                extent,
                rotation,
            } => {
                let local = rotation.inverse().rotate(point.subtract(*center));
                let clamped = Vec3::new(
                    local.x.clamp(-extent.x, extent.x),
                    local.y.clamp(-extent.y, extent.y),
                    local.z.clamp(-extent.z, extent.z),
                );
                center.plus(rotation.rotate(clamped))
            }
            Shape::Cylinder {
                center,
                radius,
                half_height,
            } => {
                let (dx, dy) = (point.x - center.x, point.y - center.y);
                let length = (dx * dx + dy * dy).sqrt();
                let scale = if length > *radius {
                    radius / length
                } else {
                    1.0
                };
                Vec3::new(
                    center.x + dx * scale,
                    center.y + dy * scale,
                    point
                        .z
                        .clamp(center.z - half_height, center.z + half_height),
                )
            }
            Shape::Capsule { start, end, radius } => {
                let axis = closest_on_segment(point, *start, *end);
                axis.plus(point.subtract(axis).normalized().scale(*radius))
            }
            Shape::Prism {
                polygon,
                minimum,
                maximum,
            } => {
                let (x, y) = polygon.closest(point.x, point.y);
                Vec3::new(x, y, point.z.clamp(*minimum, *maximum))
            }
            Shape::Union { shapes, .. } => shapes
                .iter()
                .map(|shape| shape.closest(point))
                .min_by(|left, right| {
                    left.subtract(point)
                        .length()
                        .total_cmp(&right.subtract(point).length())
                })
                .unwrap_or(point),
        }
    }
}

pub struct GeometryShape(Shape);

fn output(lua: &Lua, shape: &Shape, point: Vec3) -> LuaResult<LuaValue> {
    if shape.is_3d() {
        make_vector(lua, point)
    } else {
        make_vector2(lua, point.x, point.y)
    }
}

fn generator(value: Option<LuaValue>) -> LuaResult<Option<LuaAnyUserData>> {
    match value {
        None | Some(LuaNil) => Ok(None),
        Some(LuaValue::UserData(userdata)) => Ok(Some(userdata)),
        Some(other) => Err(LuaError::runtime(format!(
            "expected a generator from Random.New, got {}",
            other.type_name()
        ))),
    }
}

impl LuaUserData for GeometryShape {
    fn add_methods<M: LuaUserDataMethods<Self>>(methods: &mut M) {
        methods.add_method("Contains", |_, shape, point: LuaValue| {
            Ok(shape.0.contains(read_vector(&point)?))
        });
        methods.add_method("RandomPoint", |lua, shape, value: Option<LuaValue>| {
            let point = with_rng(lua, generator(value)?, |rng| shape.0.random(rng))?;
            output(lua, &shape.0, point)
        });
        methods.add_method(
            "RandomPoints",
            |lua, shape, (count, value): (usize, Option<LuaValue>)| {
                if count > MAX_POINTS {
                    return Err(LuaError::runtime(format!(
                        "at most {MAX_POINTS} points can be generated at once"
                    )));
                }
                let points = with_rng(lua, generator(value)?, |rng| {
                    (0..count).map(|_| shape.0.random(rng)).collect::<Vec<_>>()
                })?;
                let list = lua.create_table_with_capacity(count, 0)?;
                for point in points {
                    list.raw_push(output(lua, &shape.0, point)?)?;
                }
                Ok(list)
            },
        );
        methods.add_method("ClosestPoint", |lua, shape, point: LuaValue| {
            output(lua, &shape.0, shape.0.closest(read_vector(&point)?))
        });
        methods.add_method("Distance", |_, shape, point: LuaValue| {
            let mut point = read_vector(&point)?;
            if !shape.0.is_3d() {
                point.z = 0.0;
            }
            Ok(shape.0.closest(point).subtract(point).length())
        });
        methods.add_method("GetBounds", |lua, shape, ()| {
            let (minimum, maximum) = shape.0.bounds();
            (
                output(lua, &shape.0, minimum)?,
                output(lua, &shape.0, maximum)?,
            )
                .into_lua_multi(lua)
        });
        methods.add_method("GetCenter", |lua, shape, ()| {
            output(lua, &shape.0, shape.0.center())
        });
        methods.add_method("GetArea", |_, shape, ()| {
            Ok((!shape.0.is_3d()).then(|| shape.0.measure()))
        });
        methods.add_method("GetVolume", |_, shape, ()| {
            Ok(shape.0.is_3d().then(|| shape.0.measure()))
        });
        methods.add_method("GetType", |_, shape, ()| Ok(shape.0.type_name()));
        methods.add_method("Is3D", |_, shape, ()| Ok(shape.0.is_3d()));
        methods.add_meta_method(LuaMetaMethod::ToString, |_, shape, ()| {
            Ok(format!("Geometry.{}", shape.0.type_name()))
        });
    }
}

fn read_points_2d(points: &LuaTable) -> LuaResult<Vec<(f64, f64)>> {
    points
        .sequence_values::<LuaValue>()
        .map(|value| read_vector(&value?).map(|vector| (vector.x, vector.y)))
        .collect()
}

fn read_points_3d(points: &LuaTable) -> LuaResult<Vec<Vec3>> {
    points
        .sequence_values::<LuaValue>()
        .map(|value| read_vector(&value?))
        .collect()
}

fn positive(value: f64, name: &str) -> LuaResult<f64> {
    if value.is_finite() && value > 0.0 {
        Ok(value)
    } else {
        Err(LuaError::runtime(format!(
            "the {name} has to be greater than 0"
        )))
    }
}

fn shape(lua: &Lua, shape: Shape) -> LuaResult<LuaAnyUserData> {
    create_userdata(lua, GeometryShape(shape))
}

fn convex_hull(mut points: Vec<(f64, f64)>) -> Vec<(f64, f64)> {
    points.sort_by(|left, right| left.0.total_cmp(&right.0).then(left.1.total_cmp(&right.1)));
    points.dedup();
    if points.len() < 3 {
        return points;
    }
    let cross = |o: (f64, f64), a: (f64, f64), b: (f64, f64)| {
        (a.0 - o.0) * (b.1 - o.1) - (a.1 - o.1) * (b.0 - o.0)
    };
    let mut lower: Vec<(f64, f64)> = Vec::new();
    for point in &points {
        while lower.len() >= 2
            && cross(lower[lower.len() - 2], lower[lower.len() - 1], *point) <= 0.0
        {
            lower.pop();
        }
        lower.push(*point);
    }
    let mut upper: Vec<(f64, f64)> = Vec::new();
    for point in points.iter().rev() {
        while upper.len() >= 2
            && cross(upper[upper.len() - 2], upper[upper.len() - 1], *point) <= 0.0
        {
            upper.pop();
        }
        upper.push(*point);
    }
    lower.pop();
    upper.pop();
    lower.extend(upper);
    lower
}

fn simplify(points: &[Vec3], tolerance: f64, keep: &mut [bool], start: usize, end: usize) {
    if end <= start + 1 {
        return;
    }
    let mut farthest = start;
    let mut distance = 0.0;
    for index in start + 1..end {
        let candidate = points[index]
            .subtract(closest_on_segment(
                points[index],
                points[start],
                points[end],
            ))
            .length();
        if candidate > distance {
            distance = candidate;
            farthest = index;
        }
    }
    if distance > tolerance {
        keep[farthest] = true;
        simplify(points, tolerance, keep, start, farthest);
        simplify(points, tolerance, keep, farthest, end);
    }
}

fn segment_intersection(
    a1: (f64, f64),
    a2: (f64, f64),
    b1: (f64, f64),
    b2: (f64, f64),
) -> Option<(f64, f64)> {
    let r = (a2.0 - a1.0, a2.1 - a1.1);
    let s = (b2.0 - b1.0, b2.1 - b1.1);
    let denominator = r.0 * s.1 - r.1 * s.0;
    if denominator.abs() < 1e-12 {
        return None;
    }
    let qp = (b1.0 - a1.0, b1.1 - a1.1);
    let t = (qp.0 * s.1 - qp.1 * s.0) / denominator;
    let u = (qp.0 * r.1 - qp.1 * r.0) / denominator;
    ((0.0..=1.0).contains(&t) && (0.0..=1.0).contains(&u))
        .then_some((a1.0 + t * r.0, a1.1 + t * r.1))
}

pub fn build(lua: &Lua) -> LuaResult<LuaTable> {
    let module = lua.create_table()?;

    module.set(
        "Circle",
        lua.create_function(|lua, (center, radius): (LuaValue, f64)| {
            let center = read_vector(&center)?;
            shape(
                lua,
                Shape::Circle {
                    center: (center.x, center.y),
                    radius: positive(radius, "radius")?,
                },
            )
        })?,
    )?;
    module.set(
        "Rectangle",
        lua.create_function(
            |lua, (center, extent, angle): (LuaValue, LuaValue, Option<f64>)| {
                let center = read_vector(&center)?;
                let extent = read_vector(&extent)?;
                shape(
                    lua,
                    Shape::Rectangle {
                        center: (center.x, center.y),
                        extent: (positive(extent.x, "extent")?, positive(extent.y, "extent")?),
                        angle: angle.unwrap_or(0.0),
                    },
                )
            },
        )?,
    )?;
    module.set(
        "Polygon",
        lua.create_function(|lua, points: LuaTable| {
            shape(lua, Shape::Polygon(Polygon::new(read_points_2d(&points)?)?))
        })?,
    )?;
    module.set(
        "Sphere",
        lua.create_function(|lua, (center, radius): (LuaValue, f64)| {
            shape(
                lua,
                Shape::Sphere {
                    center: read_vector(&center)?,
                    radius: positive(radius, "radius")?,
                },
            )
        })?,
    )?;
    module.set(
        "Box",
        lua.create_function(
            |lua, (center, extent, rotation): (LuaValue, LuaValue, Option<LuaValue>)| {
                let extent = read_vector(&extent)?;
                let rotation = match rotation {
                    None | Some(LuaNil) => Quaternion::IDENTITY,
                    Some(value) => read_quaternion(&value)?.normalized(),
                };
                shape(
                    lua,
                    Shape::Box {
                        center: read_vector(&center)?,
                        extent: Vec3::new(
                            positive(extent.x, "extent")?,
                            positive(extent.y, "extent")?,
                            positive(extent.z, "extent")?,
                        ),
                        rotation,
                    },
                )
            },
        )?,
    )?;
    module.set(
        "Cylinder",
        lua.create_function(|lua, (center, radius, height): (LuaValue, f64, f64)| {
            shape(
                lua,
                Shape::Cylinder {
                    center: read_vector(&center)?,
                    radius: positive(radius, "radius")?,
                    half_height: positive(height, "height")? / 2.0,
                },
            )
        })?,
    )?;
    module.set(
        "Capsule",
        lua.create_function(|lua, (start, end, radius): (LuaValue, LuaValue, f64)| {
            shape(
                lua,
                Shape::Capsule {
                    start: read_vector(&start)?,
                    end: read_vector(&end)?,
                    radius: positive(radius, "radius")?,
                },
            )
        })?,
    )?;
    module.set(
        "Prism",
        lua.create_function(|lua, (points, minimum, maximum): (LuaTable, f64, f64)| {
            if maximum <= minimum {
                return Err(LuaError::runtime(
                    "the maximum height has to be greater than the minimum",
                ));
            }
            shape(
                lua,
                Shape::Prism {
                    polygon: Polygon::new(read_points_2d(&points)?)?,
                    minimum,
                    maximum,
                },
            )
        })?,
    )?;
    module.set(
        "Union",
        lua.create_function(|lua, shapes: LuaTable| {
            let mut parts = Vec::new();
            for value in shapes.sequence_values::<LuaAnyUserData>() {
                let value =
                    value.map_err(|_| LuaError::runtime("a union needs a list of shapes"))?;
                parts.push(value.borrow::<GeometryShape>()?.0.clone());
            }
            let Some(first) = parts.first() else {
                return Err(LuaError::runtime("a union needs at least one shape"));
            };
            let three_dimensional = first.is_3d();
            if parts.iter().any(|part| part.is_3d() != three_dimensional) {
                return Err(LuaError::runtime("a union cannot mix 2D and 3D shapes"));
            }
            let mut cumulative = Vec::with_capacity(parts.len());
            let mut total = 0.0;
            for part in &parts {
                total += part.measure();
                cumulative.push(total);
            }
            shape(
                lua,
                Shape::Union {
                    shapes: parts,
                    cumulative,
                    three_dimensional,
                },
            )
        })?,
    )?;
    module.set(
        "IsShape",
        lua.create_function(|_, value: LuaValue| {
            Ok(matches!(value, LuaValue::UserData(userdata) if userdata.is::<GeometryShape>()))
        })?,
    )?;

    module.set(
        "PointInPolygon",
        lua.create_function(|_, (point, polygon): (LuaValue, LuaTable)| {
            let point = read_vector(&point)?;
            let polygon = read_points_2d(&polygon)?;
            Ok(polygon.len() >= 3 && point_in_polygon(&polygon, point.x, point.y))
        })?,
    )?;
    module.set(
        "PolygonArea",
        lua.create_function(|_, (polygon, signed): (LuaTable, Option<bool>)| {
            let area = signed_area(&read_points_2d(&polygon)?);
            Ok(if signed.unwrap_or(false) {
                area
            } else {
                area.abs()
            })
        })?,
    )?;
    module.set(
        "PolygonCentroid",
        lua.create_function(|lua, polygon: LuaTable| {
            let (x, y) = Polygon::new(read_points_2d(&polygon)?)?.centroid();
            make_vector2(lua, x, y)
        })?,
    )?;
    module.set(
        "ConvexHull",
        lua.create_function(|lua, points: LuaTable| {
            let hull = convex_hull(read_points_2d(&points)?);
            let list = lua.create_table_with_capacity(hull.len(), 0)?;
            for (x, y) in hull {
                list.raw_push(make_vector2(lua, x, y)?)?;
            }
            Ok(list)
        })?,
    )?;
    module.set(
        "SegmentIntersection",
        lua.create_function(
            |lua, (a1, a2, b1, b2): (LuaValue, LuaValue, LuaValue, LuaValue)| {
                let point =
                    |value: &LuaValue| read_vector(value).map(|vector| (vector.x, vector.y));
                match segment_intersection(point(&a1)?, point(&a2)?, point(&b1)?, point(&b2)?) {
                    Some((x, y)) => make_vector2(lua, x, y),
                    None => Ok(LuaNil),
                }
            },
        )?,
    )?;
    module.set(
        "ClosestPointOnSegment",
        lua.create_function(|lua, (point, start, end): (LuaValue, LuaValue, LuaValue)| {
            make_vector(
                lua,
                closest_on_segment(
                    read_vector(&point)?,
                    read_vector(&start)?,
                    read_vector(&end)?,
                ),
            )
        })?,
    )?;
    module.set(
        "DistanceToSegment",
        lua.create_function(|_, (point, start, end): (LuaValue, LuaValue, LuaValue)| {
            let point = read_vector(&point)?;
            Ok(point
                .subtract(closest_on_segment(
                    point,
                    read_vector(&start)?,
                    read_vector(&end)?,
                ))
                .length())
        })?,
    )?;
    module.set(
        "Triangulate",
        lua.create_function(|lua, polygon: LuaTable| {
            let polygon = Polygon::new(read_points_2d(&polygon)?)?;
            let list = lua.create_table_with_capacity(polygon.triangles.len(), 0)?;
            for [a, b, c] in polygon.triangles {
                list.raw_push(lua.create_sequence_from([a + 1, b + 1, c + 1])?)?;
            }
            Ok(list)
        })?,
    )?;
    module.set(
        "Delaunay",
        lua.create_function(|lua, points: LuaTable| {
            let points: Vec<delaunator::Point> = read_points_2d(&points)?
                .into_iter()
                .map(|(x, y)| delaunator::Point { x, y })
                .collect();
            let triangulation = delaunator::triangulate(&points);
            let list = lua.create_table_with_capacity(triangulation.triangles.len() / 3, 0)?;
            for chunk in triangulation.triangles.as_chunks::<3>().0 {
                list.raw_push(lua.create_sequence_from([
                    chunk[0] + 1,
                    chunk[1] + 1,
                    chunk[2] + 1,
                ])?)?;
            }
            Ok(list)
        })?,
    )?;
    module.set(
        "SimplifyPath",
        lua.create_function(|lua, (points, tolerance): (LuaTable, f64)| {
            let points = read_points_3d(&points)?;
            if points.len() <= 2 {
                return lua.create_sequence_from(
                    points
                        .into_iter()
                        .map(|point| make_vector(lua, point))
                        .collect::<LuaResult<Vec<_>>>()?,
                );
            }
            let mut keep = vec![false; points.len()];
            keep[0] = true;
            keep[points.len() - 1] = true;
            simplify(&points, tolerance.max(0.0), &mut keep, 0, points.len() - 1);
            let list = lua.create_table()?;
            for (point, kept) in points.into_iter().zip(keep) {
                if kept {
                    list.raw_push(make_vector(lua, point)?)?;
                }
            }
            Ok(list)
        })?,
    )?;
    module.set(
        "BoundingBox",
        lua.create_function(|lua, (points, options): (LuaTable, Options)| {
            let points = read_points_3d(&points)?;
            let Some(first) = points.first() else {
                return LuaNil.into_lua_multi(lua);
            };
            let mut minimum = *first;
            let mut maximum = *first;
            for point in &points {
                minimum = Vec3::new(
                    minimum.x.min(point.x),
                    minimum.y.min(point.y),
                    minimum.z.min(point.z),
                );
                maximum = Vec3::new(
                    maximum.x.max(point.x),
                    maximum.y.max(point.y),
                    maximum.z.max(point.z),
                );
            }
            let padding: f64 = options.get_or("Padding", 0.0)?;
            let padding = Vec3::new(padding, padding, padding);
            (
                make_vector(lua, minimum.subtract(padding))?,
                make_vector(lua, maximum.plus(padding))?,
            )
                .into_lua_multi(lua)
        })?,
    )?;
    Ok(module)
}
