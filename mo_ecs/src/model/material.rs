use bevy_math::prelude::*;

#[repr(u32)]
#[derive(Copy, Clone, Debug)]
pub enum MaterialType {
    None = 0,
    Unlit = 0x80,
    MetallicRoughness = 0x1,
    SpecularGlossiness = 0x2,
    Sheen = 0x4,
    Clearcoat = 0x8,
    Specular = 0x10,
    Transparent = 0x20,
    Volume = 0x40,
}

#[repr(u32)]
#[derive(Copy, Clone, Debug)]
pub enum AlphaMode {
    Opaque = 0,
    Mask = 1,
    Blend = 2,
}

/// Note: these are not bindless indexes, but rather the index into the Model specific texture array.
#[derive(Clone, Debug)]
pub struct GltfMaterialCPU {
    pub base_color_factor: Vec4,
    pub ormn: Vec4, // occlusion, roughness, metallic, normal strength
    pub specular_glossiness: Vec4,
    pub sheen_factors: Vec4,

    pub clearcoat_transmission_thickness: Vec4,
    pub specular_factors: Vec4,
    pub attenuation: Vec4,

    pub emissive_factor_alpha_cutoff: Vec4,

    pub base_color_texture: u32,
    pub base_color_sampler: u32,
    pub base_color_uv: u32,

    // MetallicRoughness / SpecularGlossiness
    pub surface_properties_texture: u32,
    pub surface_properties_sampler: u32,
    pub surface_properties_uv: u32,

    pub normal_texture: u32,
    pub normal_sampler: u32,
    pub normal_uv: u32,

    pub occlusion_texture: u32,
    pub occlusion_sampler: u32,
    pub occlusion_uv: u32,

    pub emissive_texture: u32,
    pub emissive_sampler: u32,
    pub emissive_uv: u32,

    pub sheen_texture: u32,
    pub sheen_sampler: u32,
    pub sheen_uv: u32,
    pub sheen_roughness_texture: u32,
    pub sheen_roughness_sampler: u32,
    pub sheen_roughness_uv: u32,

    pub clearcoat_texture: u32,
    pub clearcoat_sampler: u32,
    pub clearcoat_uv: u32,
    pub clearcoat_roughness_texture: u32,
    pub clearcoat_roughness_sampler: u32,
    pub clearcoat_roughness_uv: u32,
    pub clearcoat_normal_texture: u32,
    pub clearcoat_normal_sampler: u32,
    pub clearcoat_normal_uv: u32,

    pub specular_texture: u32,
    pub specular_sampler: u32,
    pub specular_uv: u32,
    pub specular_color_texture: u32,
    pub specular_color_sampler: u32,
    pub specular_color_uv: u32,

    pub transmission_texture: u32,
    pub transmission_sampler: u32,
    pub transmission_uv: u32,

    pub thickness_texture: u32,
    pub thickness_sampler: u32,
    pub thickness_uv: u32,

    pub iridescence_texture: u32,
    pub iridescence_sampler: u32,
    pub iridescence_uv: u32,
    pub iridescence_thickness_texture: u32,
    pub iridescence_thickness_sampler: u32,
    pub iridescence_thickness_uv: u32,

    pub anisotropy_texture: u32,
    pub anisotropy_sampler: u32,
    pub anisotropy_uv: u32,

    pub alpha_mode: AlphaMode,
    pub material_type: MaterialType,
    pub ior: f32,
}

impl Default for GltfMaterialCPU {
    fn default() -> Self {
        Self {
            base_color_factor: Vec4::ONE,
            ormn: Vec4::new(0.0, 0.5, 0.5, 1.0),
            specular_glossiness: Vec4::ONE,
            sheen_factors: Vec4::ONE,

            clearcoat_transmission_thickness: Vec4::ONE,
            specular_factors: Vec4::ONE,
            attenuation: Vec4::ONE,

            emissive_factor_alpha_cutoff: Vec4::ZERO,

            base_color_texture: u32::MAX,
            base_color_sampler: 0,
            base_color_uv: 0,

            surface_properties_texture: u32::MAX,
            surface_properties_sampler: 0,
            surface_properties_uv: 0,

            normal_texture: u32::MAX,
            normal_sampler: 0,
            normal_uv: 0,

            occlusion_texture: u32::MAX,
            occlusion_sampler: 0,
            occlusion_uv: 0,

            emissive_texture: u32::MAX,
            emissive_sampler: 0,
            emissive_uv: 0,

            sheen_texture: u32::MAX,
            sheen_sampler: 0,
            sheen_uv: 0,

            sheen_roughness_texture: u32::MAX,
            sheen_roughness_sampler: 0,
            sheen_roughness_uv: 0,

            clearcoat_texture: u32::MAX,
            clearcoat_sampler: 0,
            clearcoat_uv: 0,
            clearcoat_roughness_texture: u32::MAX,
            clearcoat_roughness_sampler: 0,
            clearcoat_roughness_uv: 0,
            clearcoat_normal_texture: u32::MAX,
            clearcoat_normal_sampler: 0,
            clearcoat_normal_uv: 0,

            specular_texture: u32::MAX,
            specular_sampler: 0,
            specular_uv: 0,
            specular_color_texture: u32::MAX,
            specular_color_sampler: 0,
            specular_color_uv: 0,

            transmission_texture: u32::MAX,
            transmission_sampler: 0,
            transmission_uv: 0,

            thickness_texture: u32::MAX,
            thickness_sampler: 0,
            thickness_uv: 0,

            iridescence_texture: u32::MAX,
            iridescence_sampler: 0,
            iridescence_uv: 0,
            iridescence_thickness_texture: u32::MAX,
            iridescence_thickness_sampler: 0,
            iridescence_thickness_uv: 0,

            anisotropy_texture: u32::MAX,
            anisotropy_sampler: 0,
            anisotropy_uv: 0,

            alpha_mode: AlphaMode::Opaque,
            material_type: MaterialType::None,
            ior: 1.5,
        }
    }
}
