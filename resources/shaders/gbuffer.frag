#version 460
#extension GL_EXT_nonuniform_qualifier: require
#extension GL_GOOGLE_include_directive: require

#include "include/bindless.glsl"
#include "include/utility.glsl"

layout (location = 0) in vec2 uv0;
layout (location = 1) in vec2 uv1;
layout (location = 2) in vec3 in_normal;
layout (location = 3) in vec4 in_tangent;
layout (location = 4) in vec3 in_model_space_pos;
layout (location = 5) in vec4 in_clip_space_pos;
layout (location = 6) in vec4 in_prev_clip_space_pos;
layout (location = 7) in vec3 in_bitangent;
layout (location = 8) in mat3 in_tbn;

layout (location = 0) out vec4 out_gbuffer_base_color;
layout (location = 1) out vec4 out_gbuffer_position;
layout (location = 2) out vec4 out_gbuffer_normal;
layout (location = 3) out vec4 out_gbuffer_emissive;
layout (location = 4) out vec4 out_gbuffer_pbr;
layout (location = 5) out vec2 out_gbuffer_velocity;

layout (push_constant) uniform PushConsts {
    mat4 world;
    mat4 normal_matrix;
    uint mat_index;
    ivec3 pad;
} pushConsts;

void main() {
    GltfMaterialGPU material = materialsSSBO.materials[pushConsts.mat_index];

    vec2 base_color_uv = material.base_color_uv_set == 0 ? uv0 : uv1;
    vec4 diffuse_color = texture(nonuniformEXT(sampledTextures[material.base_color_map]), base_color_uv);

    vec2 normal_uv = material.normal_uv_set == 0 ? uv0 : uv1;
    vec4 normal_map = texture(nonuniformEXT(sampledTextures[material.normal_map]), normal_uv);

    vec2 metallic_roughness_uv = material.metallic_roughness_uv_set == 0 ? uv0 : uv1;
    float metallic = texture(nonuniformEXT(sampledTextures[material.metallic_roughness_map]), metallic_roughness_uv).b;
    float roughness = texture(nonuniformEXT(sampledTextures[material.metallic_roughness_map]), metallic_roughness_uv).g;

    vec2 occlusion_uv = material.occlusion_uv_set == 0 ? uv0 : uv1;
    float occlusion = texture(nonuniformEXT(sampledTextures[material.occlusion_map]), occlusion_uv).r;

    diffuse_color *= material.base_color_factor;
    roughness *= material.roughness_factor;
    metallic *= material.metallic_factor;

    // From sRGB space to Linear space, since in gltf, we loaded everything in linear space.
    diffuse_color.rgb = toLinear(diffuse_color.rgb);

    vec3 normal = normalize(in_normal);
    if (in_tangent.xyz != vec3(0.0f))
    {
        normal = normalize(normal_map.xyz * 2.0 - 1.0);
        normal = normalize(in_tbn * normal);
    }

    out_gbuffer_base_color = vec4(diffuse_color.rgb, 1.0);
    out_gbuffer_position = in_clip_space_pos;
    out_gbuffer_normal = vec4(normal, 1.0);
    out_gbuffer_pbr = vec4(occlusion, roughness, metallic, 1.0);

    vec2 emissive_uv = material.emissive_uv_set == 0 ? uv0 : uv1;
    out_gbuffer_emissive = texture(nonuniformEXT(sampledTextures[uint(material.emissive_map)]), emissive_uv) * vec4(material.emissive_factor);
    out_gbuffer_position = vec4(in_model_space_pos.xyz, 1.0);

    {
        vec2 a = (in_clip_space_pos.xy / in_clip_space_pos.w);
        a = (a + 1.0f) / 2.0f;
        a.y = 1.0 - a.y;
        vec2 b = (in_prev_clip_space_pos.xy / in_prev_clip_space_pos.w);
        b = (b + 1.0f) / 2.0f;
        b.y = 1.0 - b.y;
        out_gbuffer_velocity = (a - b);
    }
}
