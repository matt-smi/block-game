use bevy::asset::RenderAssetUsages;
use bevy::platform::collections::HashMap;
use bevy::prelude::*;
use bevy::render::render_resource::PrimitiveTopology;
use bevy_mesh::*;

use crate::world::*;

// TODO: Add chunk exterior face pruning

pub struct WorldPlugin;
impl Plugin for WorldPlugin {
    fn build(&self, app: &mut App) {
        app.insert_resource(ChunkEntities {
            chunks: HashMap::new(),
        });
    }
}

#[derive(Resource)]
pub struct VoxelResource {
    pub materials: Vec<Handle<StandardMaterial>>,
    pub mesh: Handle<Mesh>,
}

const VOXEL_SIZE: f32 = 1.0;

#[derive(Copy, Clone)]
struct Basis {
    u: Vec3,
    v: Vec3,
}

struct MeshBuffers<'a> {
    vertices: &'a mut Vec<Vec3>,
    indices: &'a mut Vec<u16>,
    normals: &'a mut Vec<Vec3>,
    colours: &'a mut Vec<[f32; 4]>,
    base_idx: &'a mut u16,
}

struct QuadParams {
    u_start: u32,
    v_start: u32,
    u_dimension: u32,
    v_dimension: u32,
    depth: u32,
}

struct FaceParams {
    normal: Vec3,
    basis: Basis,
}

/// Contains face visibility data for each direction.
/// Only contains 0 (air) and 1 (block), does not contain material info.
/// Used for meshing.
pub struct ChunkViews {
    pos_x_faces: [[u32; CHUNK_DIMENSION as usize]; CHUNK_DIMENSION as usize],
    pos_z_faces: [[u32; CHUNK_DIMENSION as usize]; CHUNK_DIMENSION as usize],
    pos_y_faces: [[u32; CHUNK_DIMENSION as usize]; CHUNK_DIMENSION as usize],
    neg_x_faces: [[u32; CHUNK_DIMENSION as usize]; CHUNK_DIMENSION as usize],
    neg_z_faces: [[u32; CHUNK_DIMENSION as usize]; CHUNK_DIMENSION as usize],
    neg_y_faces: [[u32; CHUNK_DIMENSION as usize]; CHUNK_DIMENSION as usize],
}

// Just a random chunk to test renderer.
pub fn generate_no_padding_dumby_chunk() -> VoxelData {
    let voxels = vec![VoxelId::Air; CHUNK_DATA_SIZE];
    let mut chunk = VoxelData {
        voxels,
        size: UVec3::new(CHUNK_DIMENSION, CHUNK_DIMENSION, CHUNK_DIMENSION),
    };

    let center_x = CHUNK_DIMENSION / 2;
    let center_z = CHUNK_DIMENSION / 2;

    const MAX_HEIGHT: u32 = 20;
    const HILL_RADIUS: f32 = 15.0;
    const MIN_HEIGHT: f32 = 5.0;

    for x in 0..CHUNK_DIMENSION {
        for z in 0..CHUNK_DIMENSION {
            let dx = (x as i32 - center_x as i32).abs();
            let dz = (z as i32 - center_z as i32).abs();
            let dist = ((dx * dx + dz * dz) as f32).sqrt();

            let base_height = (MAX_HEIGHT as f32 * (1.0 - (dist / HILL_RADIUS).min(1.0))) as u32;
            let wave = ((x as f32 * 0.3).sin() + (z as f32 * 0.3).cos()) * 2.0;
            let height = (base_height as f32 + wave).max(MIN_HEIGHT) as u32;

            for y in 0..CHUNK_DIMENSION {
                if y < height {
                    if y < height - 1 {
                        chunk.set(x, y, z, VoxelId::Dirt);
                    } else {
                        chunk.set(x, y, z, VoxelId::Grass);
                    }
                }
            }
        }
    }

    // Add a stone tower in the center
    const TOWER_HEIGHT: u32 = 28;
    const TOWER_RADIUS: i32 = 3;
    for y in 0..TOWER_HEIGHT {
        for offset_x in -TOWER_RADIUS..=TOWER_RADIUS {
            for offset_z in -TOWER_RADIUS..=TOWER_RADIUS {
                let tx = (center_x as i32 + offset_x) as u32;
                let tz = (center_z as i32 + offset_z) as u32;

                if tx < CHUNK_DIMENSION && tz < CHUNK_DIMENSION {
                    let tower_dist = ((offset_x * offset_x + offset_z * offset_z) as f32).sqrt();

                    if tower_dist <= TOWER_RADIUS as f32 && tower_dist >= (TOWER_RADIUS - 1) as f32
                    {
                        chunk.set(tx, y, tz, VoxelId::Stone);
                    }

                    if y == TOWER_HEIGHT - 1
                        && tower_dist <= TOWER_RADIUS as f32
                        && (offset_x + offset_z) % 2 == 0
                    {
                        chunk.set(tx, y, tz, VoxelId::Stone);
                        if y + 1 < CHUNK_DIMENSION {
                            chunk.set(tx, y + 1, tz, VoxelId::Stone);
                        }
                    }
                }
            }
        }
    }
    const PILLAR_HEIGHT: u32 = 15;
    let pillar_positions = [
        (5, 5),
        (CHUNK_DIMENSION - 6, 5),
        (5, CHUNK_DIMENSION - 6),
        (CHUNK_DIMENSION - 6, CHUNK_DIMENSION - 6),
    ];

    for (px, pz) in pillar_positions {
        for y in 0..PILLAR_HEIGHT {
            chunk.set(px, y, pz, VoxelId::Stone);
        }
    }

    chunk
}

/// Returns XY (z-faces), ZY (x-faces), XZ (y-faces) plane views, leaving only faces.
/// TODO: Shift to bit operations for face detection.
pub fn chunk_view_generator(chunk: &VoxelData) -> ChunkViews {
    let mut pos_x_faces = [[0u32; CHUNK_DIMENSION as usize]; CHUNK_DIMENSION as usize]; // z, y
    let mut pos_z_faces = [[0u32; CHUNK_DIMENSION as usize]; CHUNK_DIMENSION as usize]; // x, y
    let mut pos_y_faces = [[0u32; CHUNK_DIMENSION as usize]; CHUNK_DIMENSION as usize]; // x, z
    let mut neg_x_faces = [[0u32; CHUNK_DIMENSION as usize]; CHUNK_DIMENSION as usize]; // z, y
    let mut neg_z_faces = [[0u32; CHUNK_DIMENSION as usize]; CHUNK_DIMENSION as usize]; // x, y
    let mut neg_y_faces = [[0u32; CHUNK_DIMENSION as usize]; CHUNK_DIMENSION as usize]; // x, z

    // X FACE = ZY PLANE
    for x in 0..CHUNK_DIMENSION {
        for y in 0..CHUNK_DIMENSION {
            for z in 0..CHUNK_DIMENSION {
                if chunk.get_id(x, y, z) != 0 && (x == 0 || chunk.get_id(x - 1, y, z) == 0) {
                    pos_x_faces[y as usize][z as usize] |= 1u32 << x;
                }
            }
        }
    }

    // Y FACE = XZ PLANE
    for y in 0..CHUNK_DIMENSION {
        for x in 0..CHUNK_DIMENSION {
            for z in 0..CHUNK_DIMENSION {
                if chunk.get_id(x, y, z) != 0 && (y == 0 || chunk.get_id(x, y - 1, z) == 0) {
                    pos_y_faces[z as usize][x as usize] |= 1u32 << y;
                }
            }
        }
    }

    // Z FACE = XY PLANE
    for z in 0..CHUNK_DIMENSION {
        for y in 0..CHUNK_DIMENSION {
            for x in 0..CHUNK_DIMENSION {
                if chunk.get_id(x, y, z) != 0 && (z == 0 || chunk.get_id(x, y, z - 1) == 0) {
                    pos_z_faces[y as usize][x as usize] |= 1u32 << z;
                }
            }
        }
    }

    // -X FACE
    for x in (0..CHUNK_DIMENSION).rev() {
        for y in 0..CHUNK_DIMENSION {
            for z in 0..CHUNK_DIMENSION {
                if chunk.get_id(x, y, z) != 0
                    && (x == CHUNK_DIMENSION - 1 || chunk.get_id(x + 1, y, z) == 0)
                {
                    neg_x_faces[y as usize][z as usize] |= 1u32 << x;
                }
            }
        }
    }

    // -Y FACE
    for y in (0..CHUNK_DIMENSION).rev() {
        for x in 0..CHUNK_DIMENSION {
            for z in 0..CHUNK_DIMENSION {
                if chunk.get_id(x, y, z) != 0
                    && (y == CHUNK_DIMENSION - 1 || chunk.get_id(x, y + 1, z) == 0)
                {
                    neg_y_faces[z as usize][x as usize] |= 1u32 << y;
                }
            }
        }
    }

    // -Z FACE
    for z in (0..CHUNK_DIMENSION).rev() {
        for y in 0..CHUNK_DIMENSION {
            for x in 0..CHUNK_DIMENSION {
                if chunk.get_id(x, y, z) != 0
                    && (z == CHUNK_DIMENSION - 1 || chunk.get_id(x, y, z + 1) == 0)
                {
                    neg_z_faces[y as usize][x as usize] |= 1u32 << z;
                }
            }
        }
    }

    ChunkViews {
        pos_x_faces,
        pos_z_faces,
        pos_y_faces,
        neg_x_faces,
        neg_z_faces,
        neg_y_faces,
    }
}

/// Handles the position/vertice/indice addition for each view.
fn emit_quads(
    buffers: &mut MeshBuffers,
    params: QuadParams,
    normal: Vec3,
    basis: Basis,
    colour: [f32; 4],
) {
    let depth_f = params.depth as f32 * VOXEL_SIZE;
    let u_start_f = params.u_start as f32 * VOXEL_SIZE;
    let v_start_f = params.v_start as f32 * VOXEL_SIZE;
    let u_end_f = (params.u_start + params.u_dimension) as f32 * VOXEL_SIZE;
    let v_end_f = (params.v_start + params.v_dimension) as f32 * VOXEL_SIZE;

    let face_offset = if normal.x < 0. || normal.y < 0. || normal.z < 0. {
        1.0
    } else {
        0.0
    };

    let base_pos = Vec3::new(
        normal.x.abs() * (depth_f + face_offset),
        normal.y.abs() * (depth_f + face_offset),
        normal.z.abs() * (depth_f + face_offset),
    );

    let u_vec = basis.u;
    let v_vec = basis.v;

    let v0 = base_pos + u_vec * u_start_f + v_vec * v_start_f;
    let v1 = base_pos + u_vec * u_end_f + v_vec * v_start_f;
    let v2 = base_pos + u_vec * u_end_f + v_vec * v_end_f;
    let v3 = base_pos + u_vec * u_start_f + v_vec * v_end_f;

    buffers.vertices.push(v0);
    buffers.vertices.push(v1);
    buffers.vertices.push(v2);
    buffers.vertices.push(v3);

    buffers.normals.push(-normal);
    buffers.normals.push(-normal);
    buffers.normals.push(-normal);
    buffers.normals.push(-normal);

    let computed_normal = u_vec.cross(v_vec);
    let needs_flip = computed_normal.dot(normal) < 0.;

    if needs_flip {
        buffers.indices.push(*buffers.base_idx);
        buffers.indices.push(*buffers.base_idx + 1);
        buffers.indices.push(*buffers.base_idx + 2);
        buffers.indices.push(*buffers.base_idx);
        buffers.indices.push(*buffers.base_idx + 2);
        buffers.indices.push(*buffers.base_idx + 3);
    } else {
        buffers.indices.push(*buffers.base_idx);
        buffers.indices.push(*buffers.base_idx + 2);
        buffers.indices.push(*buffers.base_idx + 1);
        buffers.indices.push(*buffers.base_idx);
        buffers.indices.push(*buffers.base_idx + 3);
        buffers.indices.push(*buffers.base_idx + 2);
    }

    buffers.colours.push(colour);
    buffers.colours.push(colour);
    buffers.colours.push(colour);
    buffers.colours.push(colour);

    *buffers.base_idx += 4;
}

fn get_voxel_position(curr_u: u32, curr_v: u32, basis: Basis, normal: Vec3, depth: u32) -> Vec3 {
    let Vec3 {
        x: nx,
        y: ny,
        z: nz,
    } = normal.abs();
    let Vec3 {
        x: ux,
        y: uy,
        z: uz,
    } = basis.u;
    let Vec3 {
        x: vx,
        y: vy,
        z: vz,
    } = basis.v;

    Vec3::new(
        nx * depth as f32 + ux * curr_u as f32 + vx * curr_v as f32,
        ny * depth as f32 + uy * curr_u as f32 + vy * curr_v as f32,
        nz * depth as f32 + uz * curr_u as f32 + vz * curr_v as f32,
    )
}

/// TODO: Do trailing one pruning, so we no longer need to precompute faces + we can then generate views in one loop...
/// Also may be able to use an orthogonal view then make it so we no longer have to sweep the plane, and just grab trailing ones for width/height.
fn greedy_mesher(
    face: &mut [[u32; CHUNK_DIMENSION as usize]; CHUNK_DIMENSION as usize],
    buffers: &mut MeshBuffers,
    face_params: FaceParams,
    chunk: &VoxelData,
) {
    for u in 0..CHUNK_DIMENSION {
        for v in 0..CHUNK_DIMENSION {
            while face[u as usize][v as usize] != 0 {
                let depth = face[u as usize][v as usize].trailing_zeros();
                let u_start = u;
                let v_start = v;

                // Get the initial voxel ID that we're trying to merge
                let initial_pos = get_voxel_position(
                    u_start,
                    v_start,
                    face_params.basis,
                    face_params.normal,
                    depth,
                );
                let curr_id = chunk.get_id(
                    initial_pos.x as u32,
                    initial_pos.y as u32,
                    initial_pos.z as u32,
                );

                face[u as usize][v as usize] ^= 1u32 << depth;

                let mut v_dimension = 1u32;
                let mut curr_v = v + 1;

                // Expand in V direction, checking voxel ID matches
                while curr_v < CHUNK_DIMENSION
                    && ((face[u as usize][curr_v as usize] >> depth) & 1) == 1
                {
                    let check_pos = get_voxel_position(
                        u_start,
                        curr_v,
                        face_params.basis,
                        face_params.normal,
                        depth,
                    );
                    let check_id =
                        chunk.get_id(check_pos.x as u32, check_pos.y as u32, check_pos.z as u32);

                    if check_id != curr_id {
                        break;
                    }

                    v_dimension += 1;
                    face[u as usize][curr_v as usize] ^= 1u32 << depth;
                    curr_v += 1;
                }

                let mut u_dimension = 1u32;
                let mut curr_u = u + 1;

                // Expand in U direction, checking all voxels in the strip match
                'outer: while curr_u < CHUNK_DIMENSION {
                    for check_v in v..(v + v_dimension) {
                        if ((face[curr_u as usize][check_v as usize] >> depth) & 1) != 1 {
                            break 'outer;
                        }

                        // Check voxel ID matches
                        let check_pos = get_voxel_position(
                            curr_u,
                            check_v,
                            face_params.basis,
                            face_params.normal,
                            depth,
                        );
                        let check_id = chunk.get_id(
                            check_pos.x as u32,
                            check_pos.y as u32,
                            check_pos.z as u32,
                        );

                        if check_id != curr_id {
                            break 'outer;
                        }
                    }

                    for clear_v in v..(v + v_dimension) {
                        face[curr_u as usize][clear_v as usize] ^= 1u32 << depth;
                    }

                    u_dimension += 1;
                    curr_u += 1;
                }

                let colour = VOXEL_MAPPING.colours[curr_id as usize];
                emit_quads(
                    buffers,
                    QuadParams {
                        u_start,
                        v_start,
                        v_dimension,
                        u_dimension,
                        depth,
                    },
                    face_params.normal,
                    face_params.basis,
                    colour,
                );
            }
        }
    }
}

pub fn generate_mesh(chunk_views: &mut ChunkViews, chunk: &VoxelData) -> Option<Mesh> {
    let mut base_idx = 0u16;
    let mut vertex_buffer = Vec::new();
    let mut index_buffer = Vec::new();
    let mut normal_buffer = Vec::new();
    let mut colour_buffer = Vec::new();

    // view, normal, basis
    let faces = [
        (
            &mut chunk_views.pos_x_faces,
            Vec3::new(1., 0., 0.),
            Basis {
                u: Vec3::new(0., 1., 0.),
                v: Vec3::new(0., 0., 1.0),
            },
        ),
        (
            &mut chunk_views.pos_z_faces,
            Vec3::new(0., 0., 1.),
            Basis {
                u: Vec3::new(0., 1., 0.),
                v: Vec3::new(1., 0., 0.),
            },
        ),
        (
            &mut chunk_views.pos_y_faces,
            Vec3::new(0., 1., 0.),
            Basis {
                u: Vec3::new(0., 0., 1.),
                v: Vec3::new(1., 0., 0.),
            },
        ),
        (
            &mut chunk_views.neg_x_faces,
            Vec3::new(-1., 0., 0.),
            Basis {
                u: Vec3::new(0., 1., 0.),
                v: Vec3::new(0., 0., 1.0),
            },
        ),
        (
            &mut chunk_views.neg_z_faces,
            Vec3::new(0., 0., -1.),
            Basis {
                u: Vec3::new(0., 1., 0.),
                v: Vec3::new(1., 0., 0.),
            },
        ),
        (
            &mut chunk_views.neg_y_faces,
            Vec3::new(0., -1., 0.),
            Basis {
                u: Vec3::new(0., 0., 1.),
                v: Vec3::new(1., 0., 0.),
            },
        ),
    ];

    let mut buffers = MeshBuffers {
        vertices: &mut vertex_buffer,
        indices: &mut index_buffer,
        normals: &mut normal_buffer,
        colours: &mut colour_buffer,
        base_idx: &mut base_idx,
    };

    for (face, normal, basis) in faces {
        greedy_mesher(face, &mut buffers, FaceParams { normal, basis }, chunk);
        if buffers.vertices.is_empty() {
            return None;
        }
    }

    let mut mesh = Mesh::new(
        PrimitiveTopology::TriangleList,
        RenderAssetUsages::default(),
    );
    mesh.insert_attribute(Mesh::ATTRIBUTE_POSITION, vertex_buffer);
    mesh.insert_attribute(Mesh::ATTRIBUTE_NORMAL, normal_buffer);
    mesh.insert_attribute(Mesh::ATTRIBUTE_COLOR, colour_buffer);
    mesh.insert_indices(Indices::U16(index_buffer));

    Some(mesh)
}
