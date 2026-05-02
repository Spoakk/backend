use std::ffi::c_int;

pub const MC_1_21: c_int = 29;
pub const MC_1_20: c_int = 26;
pub const MC_1_19: c_int = 25;
pub const MC_1_18: c_int = 24;
pub const MC_1_17: c_int = 23;
pub const MC_1_16: c_int = 22;

pub const DIM_OVERWORLD: c_int = 0;
pub const NO_FLAGS: u32 = 0;

pub const DESERT_PYRAMID: c_int = 1;
pub const JUNGLE_TEMPLE:  c_int = 2;
pub const SWAMP_HUT:      c_int = 3;
pub const IGLOO:          c_int = 4;
pub const VILLAGE:        c_int = 5;
pub const OCEAN_RUIN:     c_int = 6;
pub const SHIPWRECK:      c_int = 7;
pub const MONUMENT:       c_int = 8;
pub const MANSION:        c_int = 9;
pub const OUTPOST:        c_int = 10;
pub const RUINED_PORTAL:  c_int = 11;
pub const ANCIENT_CITY:   c_int = 13;
pub const TRIAL_CHAMBERS: c_int = 24;

#[repr(C)]
#[derive(Clone, Copy)]
pub struct Range {
    pub scale: c_int,
    pub x: c_int,
    pub z: c_int,
    pub sx: c_int,
    pub sz: c_int,
    pub y: c_int,
    pub sy: c_int,
}

#[repr(C)]
#[derive(Clone, Copy)]
pub struct Pos {
    pub x: c_int,
    pub z: c_int,
}

#[repr(C)]
pub struct StructureConfig {
    pub salt: i32,
    pub region_size: i8,
    pub chunk_range: i8,
    pub struct_type: u8,
    pub dim: i8,
    pub rarity: f32,
}

pub enum Generator {}

extern "C" {
    fn cubiomes_alloc_generator() -> *mut Generator;
    fn cubiomes_free_generator(g: *mut Generator);
    fn setupGenerator(g: *mut Generator, mc: c_int, flags: u32);
    fn applySeed(g: *mut Generator, dim: c_int, seed: u64);
    fn allocCache(g: *const Generator, r: Range) -> *mut c_int;
    fn genBiomes(g: *const Generator, cache: *mut c_int, r: Range) -> c_int;
    fn getStructureConfig(struct_type: c_int, mc: c_int, conf: *mut StructureConfig) -> c_int;
    fn getStructurePos(struct_type: c_int, mc: c_int, seed: u64, reg_x: c_int, reg_z: c_int, pos: *mut Pos) -> c_int;
    fn isViableStructurePos(struct_type: c_int, g: *mut Generator, block_x: c_int, block_z: c_int, flags: u32) -> c_int;
    fn free(ptr: *mut std::ffi::c_void);
}

pub struct BiomeGenerator {
    ptr: *mut Generator,
    pub mc: c_int,
    pub seed: u64,
}

unsafe impl Send for BiomeGenerator {}

impl BiomeGenerator {
    #[inline]
    pub fn new(mc_version: c_int, seed: i64, flags: u32) -> Self {
        let ptr = unsafe { cubiomes_alloc_generator() };
        assert!(!ptr.is_null());
        unsafe {
            setupGenerator(ptr, mc_version, flags);
            applySeed(ptr, DIM_OVERWORLD, seed as u64);
        }
        Self { ptr, mc: mc_version, seed: seed as u64 }
    }

    #[inline]
    pub fn get_biomes(&self, x: i32, z: i32, sx: i32, sz: i32, scale: i32, y: i32) -> Vec<i32> {
        unsafe {
            let r = Range { scale, x, z, sx, sz, y, sy: 0 };
            let cache = allocCache(self.ptr, r);
            assert!(!cache.is_null());
            genBiomes(self.ptr, cache, r);
            let len = (sx * sz) as usize;
            let result = std::slice::from_raw_parts(cache, len).to_vec();
            free(cache as *mut _);
            result
        }
    }

    #[inline]
    pub fn find_structures(&mut self, struct_type: c_int, center_x: i32, center_z: i32, radius_blocks: i32) -> Vec<(i32, i32)> {
        let mut conf = StructureConfig { salt: 0, region_size: 0, chunk_range: 0, struct_type: 0, dim: 0, rarity: 0.0 };
        let ok = unsafe { getStructureConfig(struct_type, self.mc, &mut conf) };
        if ok == 0 || conf.region_size == 0 { return Vec::new(); }

        let region_blocks = (conf.region_size as i32) * 16;
        let reg_cx = center_x.div_euclid(region_blocks);
        let reg_cz = center_z.div_euclid(region_blocks);
        let reg_r = (radius_blocks / region_blocks).max(2) + 1;

        let estimated_capacity = ((2 * reg_r + 1) as usize).pow(2) / 4;
        let mut results = Vec::with_capacity(estimated_capacity.min(256));
        let mut pos = Pos { x: 0, z: 0 };

        for rx in (reg_cx - reg_r)..=(reg_cx + reg_r) {
            for rz in (reg_cz - reg_r)..=(reg_cz + reg_r) {
                let valid = unsafe { getStructurePos(struct_type, self.mc, self.seed, rx, rz, &mut pos) };
                if valid == 0 { continue; }
                let viable = unsafe { isViableStructurePos(struct_type, self.ptr, pos.x, pos.z, 0) };
                if viable != 0 {
                    results.push((pos.x, pos.z));
                }
            }
        }
        results
    }
}

impl Drop for BiomeGenerator {
    fn drop(&mut self) {
        unsafe { cubiomes_free_generator(self.ptr) };
    }
}
