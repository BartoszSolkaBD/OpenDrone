//! A tiny `.glb` writer for the Map checks: just enough glTF to hold objects
//! with tags, a mesh each and the Launch Spot, so each check builds the exact
//! file it needs, in plain Rust, without Blender.
//!
//! Positions are given in the Map's axes (east, north, up), as the Map
//! scripts think of them; the writer turns them into glTF's (x, z, -y).

#![allow(dead_code)]

use serde_json::{Value, json};

/// A box `size` across (east, north, up) around `centre`, as corners and
/// triangles facing outward.
pub fn box_mesh(centre: [f32; 3], size: [f32; 3]) -> (Vec<[f32; 3]>, Vec<[u32; 3]>) {
    let corners: Vec<[f32; 3]> = (0..8)
        .map(|i| {
            [
                centre[0] + size[0] / 2.0 * if i & 4 == 0 { -1.0 } else { 1.0 },
                centre[1] + size[1] / 2.0 * if i & 2 == 0 { -1.0 } else { 1.0 },
                centre[2] + size[2] / 2.0 * if i & 1 == 0 { -1.0 } else { 1.0 },
            ]
        })
        .collect();
    let triangles = vec![
        [0, 1, 3],
        [0, 3, 2],
        [4, 6, 7],
        [4, 7, 5],
        [0, 4, 5],
        [0, 5, 1],
        [2, 3, 7],
        [2, 7, 6],
        [0, 2, 6],
        [0, 6, 4],
        [1, 5, 7],
        [1, 7, 3],
    ];
    // The pairs above are the faces; turn every triangle round if together
    // they enclose a negative volume, which means they face inward.
    let volume: f32 = triangles
        .iter()
        .map(|[a, b, c]| {
            let (p, q, r) = (
                corners[*a as usize],
                corners[*b as usize],
                corners[*c as usize],
            );
            let cross = [
                q[1] * r[2] - q[2] * r[1],
                q[2] * r[0] - q[0] * r[2],
                q[0] * r[1] - q[1] * r[0],
            ];
            p[0] * cross[0] + p[1] * cross[1] + p[2] * cross[2]
        })
        .sum();
    let triangles = if volume < 0.0 {
        triangles.into_iter().map(|[a, b, c]| [a, c, b]).collect()
    } else {
        triangles
    };
    (corners, triangles)
}

/// A flat sheet over a square, facing up: an open mesh, never a closed solid.
pub fn sheet_mesh(centre: [f32; 3], side: f32) -> (Vec<[f32; 3]>, Vec<[u32; 3]>) {
    let h = side / 2.0;
    (
        vec![
            [centre[0] - h, centre[1] - h, centre[2]],
            [centre[0] + h, centre[1] - h, centre[2]],
            [centre[0] + h, centre[1] + h, centre[2]],
            [centre[0] - h, centre[1] + h, centre[2]],
        ],
        vec![[0, 1, 2], [0, 2, 3]],
    )
}

/// A Map's `.glb` under construction.
#[derive(Default)]
pub struct Glb {
    nodes: Vec<Value>,
    meshes: Vec<Value>,
    accessors: Vec<Value>,
    views: Vec<Value>,
    bin: Vec<u8>,
    extensions_used: Vec<String>,
}

impl Glb {
    pub fn new() -> Glb {
        Glb::default()
    }

    /// An object with a mesh, `extras` for its tags, and the material
    /// `material`.
    pub fn object(
        mut self,
        name: &str,
        extras: Value,
        mesh: (Vec<[f32; 3]>, Vec<[u32; 3]>),
        material: &str,
    ) -> Glb {
        let (corners, triangles) = mesh;
        let mut positions = Vec::new();
        for c in &corners {
            // East, north, up into glTF's x, y (up), z (south).
            for n in [c[0], c[2], -c[1]] {
                positions.extend_from_slice(&n.to_le_bytes());
            }
        }
        let position = self.add_view(&positions, corners.len(), "VEC3", 5126);
        let mut indices = Vec::new();
        for n in triangles.iter().flatten() {
            indices.extend_from_slice(&(*n as u16).to_le_bytes());
        }
        let index = self.add_view(&indices, triangles.len() * 3, "SCALAR", 5123);
        let material_number = self.materials_index(material);
        self.meshes.push(json!({
            "name": name,
            "primitives": [{
                "attributes": { "POSITION": position },
                "indices": index,
                "material": material_number,
            }]
        }));
        self.nodes.push(json!({
            "name": name,
            "mesh": self.meshes.len() - 1,
            "extras": extras,
        }));
        self
    }

    /// Gives the last object's mesh a second set of coordinates, into the
    /// Map's light atlas: `uvs` is one `[u, v]` per corner.
    pub fn light_uvs(mut self, uvs: &[[f32; 2]]) -> Glb {
        let bytes: Vec<u8> = uvs.iter().flatten().flat_map(|n| n.to_le_bytes()).collect();
        let accessor = self.add_view(&bytes, uvs.len(), "VEC2", 5126);
        let mesh = self
            .meshes
            .last_mut()
            .expect("an object to give coordinates to");
        mesh["primitives"][0]["attributes"]["TEXCOORD_1"] = json!(accessor);
        self
    }

    /// Gives the last object's mesh picture coordinates (`TEXCOORD_0`), one
    /// `[u, v]` per corner.
    pub fn uvs(mut self, uvs: &[[f32; 2]]) -> Glb {
        let bytes: Vec<u8> = uvs.iter().flatten().flat_map(|n| n.to_le_bytes()).collect();
        let accessor = self.add_view(&bytes, uvs.len(), "VEC2", 5126);
        let mesh = self
            .meshes
            .last_mut()
            .expect("an object to give coordinates to");
        mesh["primitives"][0]["attributes"]["TEXCOORD_0"] = json!(accessor);
        self
    }

    /// Gives the last object's mesh vertex colours (`COLOR_0`) as 32-bit
    /// decimal numbers, red, green, blue and alpha, one per corner.
    pub fn colours(mut self, colours: &[[f32; 4]]) -> Glb {
        let bytes: Vec<u8> = colours
            .iter()
            .flatten()
            .flat_map(|n| n.to_le_bytes())
            .collect();
        let accessor = self.add_view(&bytes, colours.len(), "VEC4", 5126);
        self.set_colours(accessor)
    }

    /// Gives the last object's mesh vertex colours (`COLOR_0`) as whole
    /// numbers that stand for 0 to 1 (`normalized` unsigned bytes), red,
    /// green and blue, one per corner.
    pub fn byte_colours(mut self, colours: &[[u8; 3]]) -> Glb {
        let bytes: Vec<u8> = colours.iter().flatten().copied().collect();
        let accessor = self.add_view(&bytes, colours.len(), "VEC3", 5121);
        self.accessors[accessor]["normalized"] = json!(true);
        self.set_colours(accessor)
    }

    fn set_colours(mut self, accessor: usize) -> Glb {
        let mesh = self
            .meshes
            .last_mut()
            .expect("an object to give colours to");
        mesh["primitives"][0]["attributes"]["COLOR_0"] = json!(accessor);
        self
    }

    /// An object with tags and no mesh, such as the Launch Spot: an empty at
    /// `at` (east, north, up).
    pub fn empty(mut self, name: &str, extras: Value, at: [f32; 3]) -> Glb {
        self.nodes.push(json!({
            "name": name,
            "translation": [at[0], at[2], -at[1]],
            "extras": extras,
        }));
        self
    }

    pub fn launch_spot(self, name: &str, at: [f32; 3], heading: f64) -> Glb {
        self.empty(name, json!({ "od_launch_spot": heading }), at)
    }

    /// Lists an extension the file uses, as a compressed export would.
    pub fn uses_extension(mut self, name: &str) -> Glb {
        self.extensions_used.push(name.to_string());
        self
    }

    fn materials_index(&mut self, _name: &str) -> usize {
        0
    }

    fn add_view(&mut self, bytes: &[u8], count: usize, kind: &str, component: u32) -> usize {
        while !self.bin.len().is_multiple_of(4) {
            self.bin.push(0);
        }
        self.views.push(json!({
            "buffer": 0,
            "byteOffset": self.bin.len(),
            "byteLength": bytes.len(),
        }));
        self.bin.extend_from_slice(bytes);
        self.accessors.push(json!({
            "bufferView": self.views.len() - 1,
            "componentType": component,
            "count": count,
            "type": kind,
        }));
        self.accessors.len() - 1
    }

    /// The file's bytes.
    pub fn build(self) -> Vec<u8> {
        let mut doc = json!({
            "asset": { "version": "2.0", "generator": "OpenDrone's own Map check" },
            "scene": 0,
            "scenes": [{ "nodes": (0..self.nodes.len()).collect::<Vec<_>>() }],
            "nodes": self.nodes,
            "meshes": self.meshes,
            "materials": [{
                "name": "concrete_plaza",
                "pbrMetallicRoughness": { "baseColorFactor": [0.4, 0.4, 0.38, 1.0] }
            }],
            "accessors": self.accessors,
            "bufferViews": self.views,
            "buffers": [{ "byteLength": self.bin.len() }],
        });
        if !self.extensions_used.is_empty() {
            doc["extensionsUsed"] = json!(self.extensions_used);
        }
        let mut text = serde_json::to_vec(&doc).unwrap();
        while !text.len().is_multiple_of(4) {
            text.push(b' ');
        }
        let mut bin = self.bin;
        while !bin.len().is_multiple_of(4) {
            bin.push(0);
        }
        let total = 12 + 8 + text.len() + 8 + bin.len();
        let mut out = Vec::new();
        out.extend_from_slice(b"glTF");
        out.extend_from_slice(&2u32.to_le_bytes());
        out.extend_from_slice(&(total as u32).to_le_bytes());
        out.extend_from_slice(&(text.len() as u32).to_le_bytes());
        out.extend_from_slice(b"JSON");
        out.extend_from_slice(&text);
        out.extend_from_slice(&(bin.len() as u32).to_le_bytes());
        out.extend_from_slice(b"BIN\0");
        out.extend_from_slice(&bin);
        out
    }
}
