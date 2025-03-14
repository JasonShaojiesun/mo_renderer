#ifndef SHADER_INCLUDE_BINDLESS
#define SHADER_INCLUDE_BINDLESS

struct GltfMaterialGPU
{
    uint base_color_map;
    uint normal_map;
    uint metallic_roughness_map;
    uint occlusion_map;
    uint emissive_map;
    uint base_color_uv_set;
    uint normal_uv_set;
    uint metallic_roughness_uv_set;
    uint occlusion_uv_set;
    uint emissive_uv_set;
    vec2 padding;
    vec4 base_color_factor;
    vec4 emissive_factor;
    float metallic_factor;
    float roughness_factor;
    /// 0 = opaque, 1 = mask, 2 = blend
    uint alpha_mode;
    float alpha_cutoff;

    // Ray tracing properties
    // x = type (0 = lambertian, 1 = metal, 2 = dielectric, 3 = diffuse light)
    // y = metal -> fuzz, dielectric -> index of refractions
    vec4 raytrace_properties;
};

layout (set = 0, binding = 0) uniform sampler2D sampledTextures[];

layout (set = 1, binding = 0) readonly buffer MaterialsSSBO
{
    GltfMaterialGPU materials[];
} materialsSSBO;

layout (std140, set = 2, binding = 0) uniform UBO_projview
{
    mat4 projection;
    mat4 view;
    mat4 prev_view;
} projview;

#endif
