use bevy::prelude::*;

const CHUNK_DIMENSION: usize = 32; 
const CHUNK_DATA_SIZE: usize = CHUNK_DIMENSION * CHUNK_DIMENSION * CHUNK_DIMENSION;
const VOXEL_SIZE: f32 = 1.0; 

//TODO: implement padding afterwards, need to have a neighbouring chunk argument. 

/*
    3D array -> 1D array. 
    X, Z, Y order (Y is vertical axis)

    e.g. if you want to access (10, 0, 0)
        data[10] = data[10 + 0 * 32 + 0 * 32 * 32] 
        data[10] = data[to_idx(10, 0, 0)]

    e.g. if you want to access (10, 9, 10)
        data[10538] = data[10 + 9 * 32 + 10 * 32 * 32]
*/
#[derive(Clone, Copy)]
struct Voxel{ 
    id: u8, // id 0 = air
}


// Chunk position is left bottom corner
struct Chunk {
    chunk_id : u32,
    pos: Pos, 
    data: [Voxel; CHUNK_DATA_SIZE]
}

struct Pos { 
    x: usize,
    y: usize,
    z: usize,
}

#[derive(Copy, Clone)]
struct Basis { 
    u: [f32; 3],
    v: [f32; 3]
}


/*
    Only contains 0 (air) and 1 (block), does not contain material info. 
    Used for meshing.
    TODO: Make this 1D
*/
struct ChunkViews {
    pos_x_faces: [[u32; CHUNK_DIMENSION]; CHUNK_DIMENSION],
    pos_z_faces: [[u32; CHUNK_DIMENSION]; CHUNK_DIMENSION],
    pos_y_faces: [[u32; CHUNK_DIMENSION]; CHUNK_DIMENSION],
    neg_x_faces: [[u32; CHUNK_DIMENSION]; CHUNK_DIMENSION],
    neg_z_faces: [[u32; CHUNK_DIMENSION]; CHUNK_DIMENSION],
    neg_y_faces: [[u32; CHUNK_DIMENSION]; CHUNK_DIMENSION],
    chunk_id: u32 
}

// full chunk
const FULL_DUMBY_CHUNK: Chunk = Chunk { 
    chunk_id: 0,
    pos: Pos {x: 0, y: 0, z: 0},
    data: [Voxel { id: 1}; CHUNK_DATA_SIZE]
};

//empty chunk 
const EMPTY_DUMBY_CHUNK: Chunk = Chunk { 
    chunk_id: 1,
    pos: Pos {x: 32, y: 32, z: 32},
    data: [ Voxel {id: 0}; CHUNK_DATA_SIZE]
};

fn generate_no_padding_dumby_chunk() -> Chunk { 
    let mut data = [Voxel {id: 0}; CHUNK_DATA_SIZE];
    
    for x in 0..CHUNK_DIMENSION {for y in 0..CHUNK_DIMENSION {for z in 0..CHUNK_DIMENSION {
        if x == 0 || y == 0 || z == 0 || x == 31 || y == 31 || z == 31 { 
            data[to_idx(x, y, z)].id = 0
        } else {
            data[to_idx(x, y ,z)].id = 1
        }   
    }}}
    Chunk {
        chunk_id: 1,
        pos: Pos {x: 0, y: 0, z: 0},
        data
    }
}

// converts (x, z, y) to 1D array index
fn to_idx(x: usize, z: usize, y: usize) -> usize { 
    x + z * CHUNK_DIMENSION + y * CHUNK_DIMENSION * CHUNK_DIMENSION
}

/*
    returns XY (z-faces), ZY (x-faces), XZ (y-faces) plane views, leaving only faces.
    TODO: Shift to bit operations for face detection.
*/
fn chunk_view_generator(chunk: &Chunk) -> ChunkViews {
    let mut pos_x_faces= [[0u32; CHUNK_DIMENSION]; CHUNK_DIMENSION]; // z, y 
    let mut pos_z_faces = [[0u32; CHUNK_DIMENSION]; CHUNK_DIMENSION]; // x, y 
    let mut pos_y_faces = [[0u32; CHUNK_DIMENSION]; CHUNK_DIMENSION]; // x, z 
    let mut neg_x_faces= [[0u32; CHUNK_DIMENSION]; CHUNK_DIMENSION]; // z, y 
    let mut neg_z_faces = [[0u32; CHUNK_DIMENSION]; CHUNK_DIMENSION]; // x, y 
    let mut neg_y_faces = [[0u32; CHUNK_DIMENSION]; CHUNK_DIMENSION]; // x, z 
    //can look at combining loops...
    // X FACE = ZY PLANE 
    for x in 0..CHUNK_DIMENSION {
        for y in 0..CHUNK_DIMENSION {
            for z in 0..CHUNK_DIMENSION {
                if chunk.data[to_idx(x, z, y)].id != 0 && (x == 0 || chunk.data[to_idx(x - 1 , z, y)].id == 0){
                    pos_x_faces[y][z] |= (1u32 << (x as u32));   
                }
            } 
        }
    }

    // Y FACE = XZ PLANE
    for y in 0..CHUNK_DIMENSION {
        for x in 0..CHUNK_DIMENSION {
            for z in 0..CHUNK_DIMENSION {
                if chunk.data[to_idx(x, z, y)].id != 0 && (y == 0 || chunk.data[to_idx(x, z, y - 1)].id == 0){
                    pos_y_faces[z][x] |= (1u32 << (y as u32));   
                }
              
            } 
        }
    }

    // Z FACE = XY PLANE
    for z in 0..CHUNK_DIMENSION {
        for y in 0..CHUNK_DIMENSION {
            for x in 0..CHUNK_DIMENSION {
                if chunk.data[to_idx(x, z, y)].id != 0 && (z == 0 || chunk.data[to_idx(x, z - 1, y)].id == 0){
                    pos_z_faces[y][x] |= (1u32 << (z as u32));   
                }
            } 
        }
    }

    // -X FACE 
    for x in (0..CHUNK_DIMENSION).rev() {
        for y in 0..CHUNK_DIMENSION {
            for z in 0..CHUNK_DIMENSION {
                if chunk.data[to_idx(x, z, y)].id != 0 && (x == CHUNK_DIMENSION - 1 || chunk.data[to_idx(x + 1, z, y)].id == 0){
                    neg_x_faces[y][z] |= (1u32 << (x as u32));   
                }
            } 
        }
    }

    // -Y FACE
    for y in (0..CHUNK_DIMENSION).rev() {
        for x in 0..CHUNK_DIMENSION {
            for z in 0..CHUNK_DIMENSION {
                if chunk.data[to_idx(x, z, y)].id != 0 && (y == CHUNK_DIMENSION - 1 || chunk.data[to_idx(x, z, y + 1)].id == 0){
                    neg_y_faces[z][x] |= (1u32 << (y as u32));   
                }
              
            } 
        }
    }

    // -Z FACE
    for z in (0..CHUNK_DIMENSION).rev() {
        for y in 0..CHUNK_DIMENSION {
            for x in 0..CHUNK_DIMENSION {
                if chunk.data[to_idx(x, z, y)].id != 0 && (z == CHUNK_DIMENSION - 1 || chunk.data[to_idx(x, z + 1, y)].id == 0){
                    neg_z_faces[y][x] |= (1u32 << (z as u32));   
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
        chunk_id: chunk.chunk_id,
    }
}

// dumby method for now should be O(1) when implemented
fn get_block(_pos: Pos) -> Voxel { 
    Voxel {id: 0}
}

//bitwise operations
fn prune_interior_blocks(_chunk_views: &mut ChunkViews) { 
 
}

fn get_chunk(_chunk_id: u32) -> Chunk {
    FULL_DUMBY_CHUNK
}

// handles the position/vertice/indice addition for each view. 
fn emit_quads(vertices: &mut Vec<Vec3>, indices: &mut Vec<u16>, normals: &mut Vec<Vec3>, normal: Vec3, depth: u32, 
            u_start: u32, 
            v_start: u32,
            v_dimension: u32, 
            u_dimension: u32,
            base_idx: &mut u16, 
        basis: Basis) {
    let depth_f = depth as f32 * VOXEL_SIZE;
    let u_start_f = u_start as f32 * VOXEL_SIZE;
    let v_start_f = v_start as f32 * VOXEL_SIZE;
    let u_end_f = (u_start + u_dimension) as f32 * VOXEL_SIZE;
    let v_end_f = (v_start + v_dimension) as f32 * VOXEL_SIZE;
    
    let base_pos = Vec3::new(
        normal.x * depth_f,
        normal.y * depth_f,
        normal.z * depth_f
    );
    
    let u_vec = Vec3::from_array(basis.u);
    let v_vec = Vec3::from_array(basis.v);
    
    let v0 = base_pos + u_vec * u_start_f + v_vec * v_start_f;
    let v1 = base_pos + u_vec * u_end_f + v_vec * v_start_f;
    let v2 = base_pos + u_vec * u_end_f + v_vec * v_end_f;
    let v3 = base_pos + u_vec * u_start_f + v_vec * v_end_f;
    
    vertices.push(v0);
    vertices.push(v1);
    vertices.push(v2);
    vertices.push(v3);
    
    normals.push(normal);
    normals.push(normal);
    normals.push(normal);
    normals.push(normal);
    
    indices.push(*base_idx);
    indices.push(*base_idx + 1);
    indices.push(*base_idx + 2);
    indices.push(*base_idx);
    indices.push(*base_idx + 2);
    indices.push(*base_idx + 3);
    
    *base_idx += 4;
}


// // TODO: Do trailing one pruning, so we no longer need to precompute faces and then we don't have to precompute faces + we can then generate views in one loop... 
// // Also may be able to generate a view orthogonal to the x/y/z view which then makes it so we no longer have to sweep the plane.
fn greedy_mesher(
    face: &mut [[u32; CHUNK_DIMENSION]; CHUNK_DIMENSION], 
    vertices: &mut Vec<Vec3>, 
    indices: &mut Vec<u16>, 
    normals: &mut Vec<Vec3>, 
    normal: Vec3, 
    base_idx: &mut u16, 
    basis: Basis
) { 
    for u in 0..CHUNK_DIMENSION { 
        for v in 0..CHUNK_DIMENSION {
            while face[u][v] != 0 { 
                let depth = face[u][v].trailing_zeros();
                let u_start = u as u32;
                let v_start = v as u32;
                
                let mut v_dimension = 1u32;
                face[u][v] ^= 1u32 << depth;
                
                let mut curr_v = v + 1;
                while curr_v < CHUNK_DIMENSION && ((face[u][curr_v] >> depth) & 1) == 1 { 
                    v_dimension += 1;
                    face[u][curr_v] ^= 1u32 << depth;
                    curr_v += 1;
                }
                
                let mut u_dimension = 1u32;
                let mut curr_u = u + 1;
                
                'outer: while curr_u < CHUNK_DIMENSION {
                    for check_v in v..(v + v_dimension as usize) {
                        if ((face[curr_u][check_v] >> depth) & 1) != 1 {
                            break 'outer;
                        }
                    }
                    
                    for clear_v in v..(v + v_dimension as usize) {
                        face[curr_u][clear_v] ^= 1u32 << depth;
                    }
                    
                    u_dimension += 1;
                    curr_u += 1;
                }
                
                emit_quads(
                    vertices, 
                    indices, 
                    normals, 
                    normal, 
                    depth, 
                    u_start, 
                    v_start, 
                    v_dimension, 
                    u_dimension, 
                    base_idx, 
                    basis
                );
            }
        }
    }
}


fn generate_mesh(chunk_views: &mut ChunkViews) {
    let mut base_idx = 0u16;
    let mut vertice_buffer: Vec<Vec3> = vec![];
    let mut indice_buffer: Vec<u16> = vec![]; 
    let mut normal_buffer: Vec<Vec3> = vec![]; 
    // view, normal, basis
    let faces = [ 
        (&mut chunk_views.pos_x_faces, Vec3::new(1., 0., 0.), Basis { u: [0., 1., 0.], v: [0., 0., 1.0]}),
        (&mut chunk_views.pos_z_faces, Vec3::new(0., 0., 1.), Basis { u: [0., 1., 0.], v: [1., 0., 0.]}),
        (&mut chunk_views.pos_y_faces, Vec3::new(0., 1., 0.), Basis { u: [0., 0., 1.], v: [1., 0., 0.]}), 
        (&mut chunk_views.neg_x_faces, Vec3::new(-1., 0., 0.), Basis { u: [0., 1., 0.], v: [0., 0., 1.0]}),
        (&mut chunk_views.neg_z_faces, Vec3::new(0., 0., -1.), Basis { u: [0., 1., 0.], v: [1., 0., 0.]}),
        (&mut chunk_views.neg_y_faces, Vec3::new(0., -1., 0.), Basis { u: [0., 0., 1.], v: [1., 0., 0.]}),
    ];

    for (face, normal, basis) in faces { 
        greedy_mesher(face, &mut vertice_buffer, &mut indice_buffer, &mut normal_buffer, normal, &mut base_idx, basis);
    }

    // let mut mesh = Mesh::new(PrimitiveTopology::TriangleList);
    // mesh.insert_attribute(Mesh::ATTRIBUTE_POSITION, vertices);
    // mesh.insert_attribute(Mesh::ATTRIBUTE_NORMAL, normals);
    // mesh.set_indices(Some(Indices::U16(indices)));
    // mesh
}

fn main(){
    let interior_chunk = generate_no_padding_dumby_chunk();
    let mut chunk_views = chunk_view_generator(&interior_chunk);
    generate_mesh(&mut chunk_views);
    chunk_view_generator(&EMPTY_DUMBY_CHUNK);
}
