use bevy::asset::RenderAssetUsages;
use bevy::platform::collections::HashMap;
use bevy::prelude::*;
use bevy::render::render_resource::PrimitiveTopology;
use bevy_mesh::*;

use crate::world::WORLD_VOXEL_SIZE;
use crate::world::*;

/*
TODOs for chunk system:
    should make it so mesh doesnt go outside of chunk (can happen if near boundary and lod scaling is used)
    all chunks outside of 32 should be transient load and get rid of 
    1. Boundary chunk face culling
    2. World oct-tree (potentially vary compression as well for voxelData)
*/
pub struct WorldPlugin;
impl Plugin for WorldPlugin {
    fn build(&self, app: &mut App) {
        app.insert_resource(ChunkEntities {
            chunks: HashMap::new(),
        })
        .insert_resource(ChunkVoxels {
            chunks: HashMap::new(),
        });
    }
}

#[derive(Resource)]
pub struct VoxelResource {
    pub materials: Vec<Handle<StandardMaterial>>,
    pub mesh: Handle<Mesh>,
}

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

/// LOD meshing: full voxel buffer stays 32 x 32 x 32; stride samples macro-cells.
#[derive(Copy, Clone)]
struct LodMeshParams {
    lod_scale: u32,
    chunk_dimension: u32,
}

fn lod_mesh_params(lod: u8) -> LodMeshParams {
    let lod = lod.min(3);
    let lod_scale = 1u32 << lod;
    LodMeshParams {
        lod_scale,
        chunk_dimension: 1u32 << (5 - lod),
    }
}

fn macro_cell_solid(chunk: &VoxelData, mx: u32, my: u32, mz: u32, scale: u32) -> bool {
    for dz in 0..scale {
        for dy in 0..scale {
            for dx in 0..scale {
                if chunk.get_id(mx * scale + dx, my * scale + dy, mz * scale + dz) != 0 {
                    return true;
                }
            }
        }
    }
    false
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

pub fn chunk_view_generator(chunk: &VoxelData) -> ChunkViews {
    let params = lod_mesh_params(chunk.lod);
    let scale = params.lod_scale;
    let dim = params.chunk_dimension as usize;

    let mut pos_x_faces = [[0u32; CHUNK_DIMENSION as usize]; CHUNK_DIMENSION as usize];
    let mut neg_x_faces = [[0u32; CHUNK_DIMENSION as usize]; CHUNK_DIMENSION as usize];
    let mut pos_y_faces = [[0u32; CHUNK_DIMENSION as usize]; CHUNK_DIMENSION as usize];
    let mut neg_y_faces = [[0u32; CHUNK_DIMENSION as usize]; CHUNK_DIMENSION as usize];
    let mut pos_z_faces = [[0u32; CHUNK_DIMENSION as usize]; CHUNK_DIMENSION as usize];
    let mut neg_z_faces = [[0u32; CHUNK_DIMENSION as usize]; CHUNK_DIMENSION as usize];

    // Solid column buffers — one per axis pair
    // solid_x[y][z]: bits along x axis
    // solid_y[x][z]: bits along y axis
    // solid_z[y][x]: bits along z axis
    let mut solid_x = [[0u32; CHUNK_DIMENSION as usize]; CHUNK_DIMENSION as usize];
    let mut solid_y = [[0u32; CHUNK_DIMENSION as usize]; CHUNK_DIMENSION as usize];
    let mut solid_z = [[0u32; CHUNK_DIMENSION as usize]; CHUNK_DIMENSION as usize];

    // Single pass: build all 3 solid column buffers
    for x in 0..dim {
        for y in 0..dim {
            for z in 0..dim {
                if macro_cell_solid(chunk, x as u32, y as u32, z as u32, scale) {
                    solid_x[y][z] |= 1u32 << x;
                    solid_y[x][z] |= 1u32 << y;
                    solid_z[y][x] |= 1u32 << z;
                }
            }
        }
    }

    // Derive all 6 face masks from solid columns — pure bitwise, no neighbor calls
    for y in 0..dim {
        for z in 0..dim {
            let col = solid_x[y][z];
            pos_x_faces[y][z] = col & !(col << 1);
            neg_x_faces[y][z] = col & !(col >> 1);
        }
    }

    for x in 0..dim {
        for z in 0..dim {
            let col = solid_y[x][z];
            pos_y_faces[z][x] = col & !(col << 1);
            neg_y_faces[z][x] = col & !(col >> 1);
        }
    }

    for y in 0..dim {
        for x in 0..dim {
            let col = solid_z[y][x];
            pos_z_faces[y][x] = col & !(col << 1);
            neg_z_faces[y][x] = col & !(col >> 1);
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
    lod_scale: u32,
) {
    let u_start_f = params.u_start as f32 * lod_scale as f32 * WORLD_VOXEL_SIZE;
    let v_start_f = params.v_start as f32 * lod_scale as f32 * WORLD_VOXEL_SIZE;
    let u_end_f = (params.u_start + params.u_dimension) as f32 * lod_scale as f32 * WORLD_VOXEL_SIZE;
    let v_end_f = (params.v_start + params.v_dimension) as f32 * lod_scale as f32 * WORLD_VOXEL_SIZE;

    // Match pre-LOD convention: +normal at depth·scale, −normal at (depth+1)·scale.
    let depth_f = params.depth as f32 * lod_scale as f32 * WORLD_VOXEL_SIZE;
    let face_offset = if normal.x < 0. || normal.y < 0. || normal.z < 0. {
        lod_scale as f32 * WORLD_VOXEL_SIZE
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

/// Macro-cell (u, v, depth) on a face → corner voxel index in the 32³ buffer.
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

fn macro_cell_material_id(chunk: &VoxelData, macro_pos: Vec3, lod_scale: u32) -> u8 {
    let mx = macro_pos.x as u32;
    let my = macro_pos.y as u32;
    let mz = macro_pos.z as u32;
    for dz in 0..lod_scale {
        for dy in 0..lod_scale {
            for dx in 0..lod_scale {
                let id = chunk.get_id(mx * lod_scale + dx, my * lod_scale + dy, mz * lod_scale + dz);
                if id != 0 {
                    return id;
                }
            }
        }
    }
    0
}

/// TODO: Do trailing one pruning, so we no longer need to precompute faces + we can then generate views in one loop...
/// Also may be able to use an orthogonal view then make it so we no longer have to sweep the plane, and just grab trailing ones for width/height.
fn greedy_mesher(
    face: &mut [[u32; CHUNK_DIMENSION as usize]; CHUNK_DIMENSION as usize],
    buffers: &mut MeshBuffers,
    face_params: FaceParams,
    chunk: &VoxelData,
    params: &LodMeshParams,
) {
    let dim = params.chunk_dimension;
    for u in 0..dim {
        for v in 0..dim {
            while face[u as usize][v as usize] != 0 {
                let depth = face[u as usize][v as usize].trailing_zeros();
                let u_start = u;
                let v_start = v;

                let initial_pos = get_voxel_position(
                    u_start,
                    v_start,
                    face_params.basis,
                    face_params.normal,
                    depth,
                );
                let curr_id = macro_cell_material_id(chunk, initial_pos, params.lod_scale);

                face[u as usize][v as usize] ^= 1u32 << depth;

                let mut v_dimension = 1u32;
                let mut curr_v = v + 1;

                while curr_v < dim && ((face[u as usize][curr_v as usize] >> depth) & 1) == 1 {
                    let check_pos = get_voxel_position(
                        u_start,
                        curr_v,
                        face_params.basis,
                        face_params.normal,
                        depth,
                    );
                    let check_id = macro_cell_material_id(chunk, check_pos, params.lod_scale);

                    if check_id != curr_id {
                        break;
                    }

                    v_dimension += 1;
                    face[u as usize][curr_v as usize] ^= 1u32 << depth;
                    curr_v += 1;
                }

                let mut u_dimension = 1u32;
                let mut curr_u = u + 1;

                'outer: while curr_u < dim {
                    for check_v in v..(v + v_dimension) {
                        if ((face[curr_u as usize][check_v as usize] >> depth) & 1) != 1 {
                            break 'outer;
                        }

                        let check_pos = get_voxel_position(
                            curr_u,
                            check_v,
                            face_params.basis,
                            face_params.normal,
                            depth,
                        );
                        let check_id = macro_cell_material_id(chunk, check_pos, params.lod_scale);

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
                    params.lod_scale,
                );
            }
        }
    }
}

pub fn generate_mesh(chunk_views: &mut ChunkViews, chunk: &VoxelData) -> Option<Mesh> {
    let mesh_params = lod_mesh_params(chunk.lod);
    let mut base_idx = 0u16;
    let mut vertex_buffer = Vec::new();
    let mut index_buffer = Vec::new();
    let mut normal_buffer = Vec::new();
    let mut colour_buffer = Vec::new();

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
        greedy_mesher(
            face,
            &mut buffers,
            FaceParams { normal, basis },
            chunk,
            &mesh_params,
        );
    }

    if vertex_buffer.is_empty() {
        return None;
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
