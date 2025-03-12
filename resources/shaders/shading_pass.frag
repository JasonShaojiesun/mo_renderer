#version 460
#extension GL_EXT_nonuniform_qualifier: require
#extension GL_GOOGLE_include_directive: require
#extension GL_EXT_scalar_block_layout: enable

#include "include/pbr.glsl"
#include "include/shadow.glsl"
#include "include/tonemap.glsl"

layout (scalar, set = 0, binding = 0) readonly buffer LightsSSBO
{
    GpuLight lights[];
} lightsSSBO;

layout (set = 1, binding = 0) uniform sampler2D inBaseColor;
layout (set = 1, binding = 1) uniform sampler2D inNormal;
layout (set = 1, binding = 2) uniform sampler2D inEmissive;
layout (set = 1, binding = 3) uniform sampler2D inORM;
layout (set = 1, binding = 4) uniform sampler2D inPosition;
layout (set = 1, binding = 5) uniform sampler2D inVelocity;
// On MacOS, depth comparison is not enabled, so we cannot use sampler2DShadow here
layout (set = 1, binding = 6) uniform sampler2D inShadowMap;
layout (set = 1, binding = 7) uniform sampler2D inSSAO;
layout (set = 1, binding = 8) uniform samplerCube inIrradianceMap;
layout (set = 1, binding = 9) uniform samplerCube inPrefilterMap;
layout (set = 1, binding = 10) uniform sampler2D inBRDFLUT;

layout (set = 2, binding = 0) uniform UBO_view
{
    mat4 proj_view;
    mat4 inverse_view;
    mat4 inverse_projection;
    mat4 light_proj_view;
    vec3 eye_pos;
    uint num_lights;
} view;

layout (location = 0) in vec2 fragTexCoord;
layout (location = 0) out vec4 outColor;

// PCF 采样参数
#define PCF_SAMPLES 9
const vec2 poissonDisk[9] = vec2[](
vec2(-0.94201624, -0.39906216),
vec2(0.94558609, -0.76890725),
vec2(-0.094184101, -0.92938870),
vec2(0.34495938, 0.29387760),
vec2(-0.91588581, 0.45771432),
vec2(-0.81544232, -0.87912464),
vec2(-0.38277543, 0.27676845),
vec2(0.97484398, 0.75648379),
vec2(0.44323325, -0.97511554)
);

float CalculateShadow(vec3 worldPos) {
    // 转换到光源空间
    vec4 lightSpacePos = view.light_proj_view * vec4(worldPos, 1.0);
    vec3 projCoords = lightSpacePos.xyz / lightSpacePos.w;

    // 适配Vulkan坐标系 [0,1]范围
    projCoords.xy = projCoords.xy * 0.5 + 0.5;
    //    projCoords.y = 1.0 - projCoords.y; // Vulkan Y 轴翻转

    if (projCoords.z > 1.0) return 1.0;// 超出远裁剪面

    // 获取当前深度
    float currentDepth = projCoords.z - 0.000005;// Shadow Bias

    // PCF采样
    float shadow = 0.0;
    vec2 texelSize = 1.0 / textureSize(inShadowMap, 0);

    for (int i = 0; i < PCF_SAMPLES; i++) {
        float closestDepth = texture(
            inShadowMap,
            projCoords.xy + poissonDisk[i] * texelSize
        ).r;
        shadow += currentDepth > closestDepth ? 1.0 : 0.0;
    }
    shadow /= float(PCF_SAMPLES);

    return mix(1.0 - shadow, 1.0, 0.25);// Last parameter is shadow intensity
}

void main() {
    vec3 position = texture(inPosition, fragTexCoord).rgb;
    vec3 normal = texture(inNormal, fragTexCoord).rgb;
    vec3 diffuse_color = texture(inBaseColor, fragTexCoord).rgb;
    float metallic = texture(inORM, fragTexCoord).b;
    float roughness = texture(inORM, fragTexCoord).g;
    float occlusion = texture(inORM, fragTexCoord).r;
    float ssao = texture(inSSAO, fragTexCoord).r;
    vec3 emissive_color = texture(inEmissive, fragTexCoord).rgb;

    PixelParams pixel;
    pixel.position = position;
    pixel.baseColor = diffuse_color;
    pixel.normal = normal;
    pixel.metallic = metallic;
    pixel.roughness = roughness;
    pixel.occlusion = occlusion;

    vec3 Lo = vec3(0.0);

    for (int i = 0; i < view.num_lights; i++)
    {
        Lo += surfaceShading(pixel, lightsSSBO.lights[i], view.eye_pos.xyz, 1.0f);
    }

    vec3 ambient = imageBasedLighting(pixel, view.eye_pos.xyz, inIrradianceMap, inPrefilterMap, inBRDFLUT);

    float shadow = CalculateShadow(position);
    vec3 color = (ambient + Lo * shadow) * ssao;

    color += emissive_color;
    color = ToneMappingUnreal(color);

    outColor = vec4(color, 1.0f);
}
