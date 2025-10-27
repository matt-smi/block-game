const CHUNK_DIMENSION: usize = 32; 
const CHUNK_DATA_SIZE: usize = CHUNK_DIMENSION * CHUNK_DIMENSION * CHUNK_DIMENSION;
const VOXEL_SIZE: f32 = 1.0; 

/*
    3D array -> 1D array. 
    X, Z, Y order (Y is vertical axis)

    e.g. if you want to access (10, 0, 0)
        data[10] = data[10 + 0 * 32 + 0 * 32 * 32] 
        data[10] = data[to_idx(10, 0, 0)]

    e.g. if you want to access (10, 9, 10)
        data[10538] = data[10 + 9 * 32 + 10 * 32 * 32]
*/

struct Voxel{ 
    id: u8, 
}


// Chunk position is left bottom corner
struct Chunk {
    chunk_id : u32,
    pos: Pos, 
    data: [Voxel; CHUNK_DATA_SIZE]

}

struct Pos { 
    x: i32,
    y: i32,
    z: i32,
}

/*
    Only contains 0 (air) and 1 (block), does not contain material info. 
    Used for meshing.
*/
struct ChunkViews {
    x_faces: [[u32; CHUNK_DIMENSION]; CHUNK_DIMENSION],
    z_faces: [[u32; CHUNK_DIMENSION]; CHUNK_DIMENSION],
    y_faces: [[u32; CHUNK_DIMENSION]; CHUNK_DIMENSION],
    chunk_id: u32 
}

// full chunk
const FULL_DUMBY_CHUNK: Chunk = Chunk { 
    data: [1; CHUNK_DATA_SIZE]
};

//empty chunk 
const EMPTY_DUMBY_CHUNK: Chunk = Chunk { 
    data: [0; CHUNK_DATA_SIZE]
};

// converts (x, z, y) to 1D array index
fn to_idx(pos: Pos) -> usize { 
    return pos.x + pos.z * CHUNK_DIMENSION + pos.y * CHUNK_DIMENSION * CHUNK_DIMENSION;
}

/*
    returns XY (z-faces), ZY (x-faces), XZ (y-faces) plane views
*/
fn chunk_view_generator(chunk: &Chunk) -> ChunkViews {
    let mut x_faces= [[0u32; CHUNK_DIMENSION]; CHUNK_DIMENSION]; // z, y 
    let mut z_faces = [[0u32; CHUNK_DIMENSION]; CHUNK_DIMENSION]; // x, y 
    let mut y_faces = [[0u32; CHUNK_DIMENSION]; CHUNK_DIMENSION]; // x, z 

    // Z-faces
    for x in 0..CHUNK_DIMENSION {
        for y in 0..CHUNK_DIMENSION {
            for z in 0..CHUNK_DIMENSION {
                if chunk.data[to_idx(x, z, y)] != 0 {
                    z_faces[y][x] += 1;
                    x_faces[y][z] += 1; 
                    y_faces[z][x] += 1; 
                }
            } 
        }
    }
   
    return ChunkViews {
        x_faces,
        z_faces,
        y_faces,
    };
}

// dumby method for now should be O(1) when implemented
fn get_block(pos: Pos) -> Voxel { 
    return Voxel {id: 0}; 
}

//bitwise operations
fn prune_interior_blocks(chunk_views: &mut ChunkViews) -> Void { 
 
}

fn get_chunk(chunk_id: u32) -> Chunk {
    return FULL_DUMBY_CHUNK;
}


// mark 0 for faces that are adjacent to neighbour blocks
fn prune_exterior_faces(chunk_views: &mut ChunkViews) -> Void {
   
    // X FACE
    chunk = get_chunk(chunk_views.id); // right chunk
    let x_left_adj = chunk.pos.x - 1;  
    let x_right_adj = chunk.pos.x + CHUNK_DIMENSION as i32;

    for y in range(CHUNK_DIMENSION){
        for z in range(CHUNK_DIMENSION) { 
            if !get_block(Pos {x_left_adj, y, z}) {
                chunk_views.x_faces[y][z] -= ;
            }

            if !get_block(Pos {x_right_adj, y, z}) {

            }

            }
        }

    }

    // Y FACE 


    // Z FACE

}



fn get_chunk_mesh(chunk: &chunk, neighbour_chunks: [&Chunk; 6]) -> Mesh3d { 
    //chunk view 
    //get chunk faces 
    // prune_exterior_faces 
    // create mesh from faces. 
    // return 
}
