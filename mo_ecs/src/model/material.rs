use bevy_math::prelude::*;

#[repr(u32)]
#[derive(Copy, Clone)]
pub enum AlphaMode {
    Opaque = 0,
    Mask = 1,
    Blend = 2,
}

/// Note: indexes into the Model specific texture array, not bindless indexes.
pub struct Material {
    // 贴图索引
    pub base_color_map: u32,
    pub normal_map: u32,
    pub metallic_roughness_map: u32,
    pub occlusion_map: u32,
    pub emissive_map: u32,

    // UV集配置
    pub base_color_uv_set: u32,
    pub normal_uv_set: u32,
    pub metallic_roughness_uv_set: u32,
    pub occlusion_uv_set: u32,
    pub emissive_uv_set: u32,

    // 混合模式
    pub alpha_mode: AlphaMode,
    pub alpha_cutoff: f32,

    // 材质参数
    pub base_color_factor: Vec4,
    pub metallic_factor: f32,
    pub roughness_factor: f32,
    pub emissive_factor: Vec3,

    // 扩展字段
    pub material_type: u32,
    pub material_property: f32,
}
