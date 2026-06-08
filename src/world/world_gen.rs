use crate::world::{CHUNK_DATA_SIZE, CHUNK_DIMENSION, VoxelData, VoxelId};
use bevy::math::IVec3;

// ─── Minimal Perlin Noise (no external crate needed) ──────────────────────────

const PERM: [u8; 512] = {
    const P: [u8; 256] = [
        151, 160, 137, 91, 90, 15, 131, 13, 201, 95, 96, 53, 194, 233, 7, 225, 140, 36, 103, 30,
        69, 142, 8, 99, 37, 240, 21, 10, 23, 190, 6, 148, 247, 120, 234, 75, 0, 26, 197, 62, 94,
        252, 219, 203, 117, 35, 11, 32, 57, 177, 33, 88, 237, 149, 56, 87, 174, 20, 125, 136, 171,
        168, 68, 175, 74, 165, 71, 134, 139, 48, 27, 166, 77, 146, 158, 231, 83, 111, 229, 122, 60,
        211, 133, 230, 220, 105, 92, 41, 55, 46, 245, 40, 244, 102, 143, 54, 65, 25, 63, 161, 1,
        216, 80, 73, 209, 76, 132, 187, 208, 89, 18, 169, 200, 196, 135, 130, 116, 188, 159, 86,
        164, 100, 109, 198, 173, 186, 3, 64, 52, 217, 226, 250, 124, 123, 5, 202, 38, 147, 118,
        126, 255, 82, 85, 212, 207, 206, 59, 227, 47, 16, 58, 17, 182, 189, 28, 42, 223, 183, 170,
        213, 119, 248, 152, 2, 44, 154, 163, 70, 221, 153, 101, 155, 167, 43, 172, 9, 129, 22, 39,
        253, 19, 98, 108, 110, 79, 113, 224, 232, 178, 185, 112, 104, 218, 246, 97, 228, 251, 34,
        242, 193, 238, 210, 144, 12, 191, 179, 162, 241, 81, 51, 145, 235, 249, 14, 239, 107, 49,
        192, 214, 31, 181, 199, 106, 157, 184, 84, 204, 176, 115, 121, 50, 45, 127, 4, 150, 254,
        138, 236, 205, 93, 222, 114, 67, 29, 24, 72, 243, 141, 128, 195, 78, 66, 215, 61, 156, 180,
    ];
    let mut out = [0u8; 512];
    let mut i = 0;
    while i < 256 {
        out[i] = P[i];
        out[i + 256] = P[i];
        i += 1;
    }
    out
};

#[inline]
fn fade(t: f64) -> f64 {
    t * t * t * (t * (t * 6.0 - 15.0) + 10.0)
}
#[inline]
fn lerp(a: f64, b: f64, t: f64) -> f64 {
    a + t * (b - a)
}
#[inline]
fn grad(hash: u8, x: f64, y: f64) -> f64 {
    match hash & 7 {
        0 => x + y,
        1 => -x + y,
        2 => x - y,
        3 => -x - y,
        4 => x,
        5 => -x,
        6 => y,
        _ => -y,
    }
}

/// 2-D Perlin noise in [-1, 1].
pub fn perlin2(x: f64, y: f64) -> f64 {
    let xi = x.floor() as i32 & 255;
    let yi = y.floor() as i32 & 255;
    let xf = x - x.floor();
    let yf = y - y.floor();
    let u = fade(xf);
    let v = fade(yf);

    let aa = PERM[(PERM[xi as usize] as usize + yi as usize) & 255];
    let ab = PERM[(PERM[xi as usize] as usize + yi as usize + 1) & 255];
    let ba = PERM[(PERM[(xi + 1) as usize & 255] as usize + yi as usize) & 255];
    let bb = PERM[(PERM[(xi + 1) as usize & 255] as usize + yi as usize + 1) & 255];

    lerp(
        lerp(grad(aa, xf, yf), grad(ba, xf - 1.0, yf), u),
        lerp(grad(ab, xf, yf - 1.0), grad(bb, xf - 1.0, yf - 1.0), u),
        v,
    )
}

/// Fractal Brownian Motion — stacks `octaves` layers of Perlin noise.
pub fn fbm(mut x: f64, mut y: f64, octaves: u32, lacunarity: f64, gain: f64) -> f64 {
    let mut value = 0.0;
    let mut amplitude = 0.5;
    let mut frequency = 1.0;
    for _ in 0..octaves {
        value += amplitude * perlin2(x * frequency, y * frequency);
        frequency *= lacunarity;
        amplitude *= gain;
    }
    value
}

// ─── World Generation ─────────────────────────────────────────────────────────

/// Absolute Y voxel coordinate → world height at which each layer sits.
/// chunk_y=0 → voxels 0‥31, chunk_y=1 → 32‥63, etc.
fn world_y(chunk_y: i32, local_y: u32) -> i32 {
    chunk_y * CHUNK_DIMENSION as i32 + local_y as i32
}

/// Height of the terrain surface in *world voxels* at a given (wx, wz) column.
fn surface_height(wx: i32, wz: i32) -> i32 {
    // Scale down to keep features broader than one chunk.
    let nx = wx as f64 * 0.018;
    let nz = wz as f64 * 0.018;

    // Primary rolling hills (large scale)
    let hills = fbm(nx, nz, 5, 2.0, 0.5); // range ≈ [-1, 1]

    // Secondary detail ridges (medium scale)
    let detail = fbm(nx * 3.1 + 5.3, nz * 3.1 + 2.7, 3, 2.0, 0.5) * 0.25;

    let combined = hills + detail; // ≈ [-1.25, 1.25]

    // Map to [SEA_LEVEL - 12, SEA_LEVEL + 48]
    const SEA_LEVEL: i32 = 40;
    let height = SEA_LEVEL + (combined * 30.0) as i32;
    height.max(1) // never below y=1
}

/// Depth below the surface at which stone begins (in voxels).
const STONE_DEPTH: i32 = 4;

/// Replaces `generate_no_padding_dumby_chunk`.
/// Takes the chunk grid coordinate so every chunk tiles seamlessly.
pub fn generate_chunk(chunk_pos: IVec3, lod: u8) -> VoxelData {
    let mut chunk = VoxelData {
        voxels: vec![VoxelId::Air; CHUNK_DATA_SIZE],
        lod,
    };

    let chunk_x = chunk_pos.x;
    let chunk_y = chunk_pos.y;
    let chunk_z = chunk_pos.z;

    for lx in 0..CHUNK_DIMENSION {
        for lz in 0..CHUNK_DIMENSION {
            let wx = chunk_x * CHUNK_DIMENSION as i32 + lx as i32;
            let wz = chunk_z * CHUNK_DIMENSION as i32 + lz as i32;

            let surface = surface_height(wx, wz);

            for ly in 0..CHUNK_DIMENSION {
                let wy = world_y(chunk_y, ly);

                let voxel = if wy > surface {
                    VoxelId::Air
                } else if wy == surface {
                    VoxelId::Grass
                } else if wy > surface - STONE_DEPTH {
                    VoxelId::Dirt
                } else {
                    VoxelId::Stone
                };

                chunk.set(lx, ly, lz, voxel);
            }
        }
    }

    chunk
}
