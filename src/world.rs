pub mod chunk;
pub mod setup;

pub use setup::WorldPlugin;
pub use setup::VoxelResource;
pub use chunk::VoxelMapping;
pub use chunk::{VoxelData, VoxelId, CHUNK_DIMENSION, CHUNK_DATA_SIZE};