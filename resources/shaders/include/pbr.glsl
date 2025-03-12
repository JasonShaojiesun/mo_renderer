#ifndef _PBR_H_
#define _PBR_H_

const float PI = 3.14159265358979323846;// pi
const float TWO_PI = 6.28318530717958648;// 2*pi
const float PI_2 = 1.57079632679489661923;// pi/2
const float PI_4 = 0.785398163397448309616;// pi/4
const float One_OVER_PI = 0.318309886183790671538;// 1/pi
const float Two_OVER_PI = 0.636619772367581343076;// 2/pi

float luminance(vec3 rgb)
{
    // Coefficents from the BT.709 standard
    return dot(rgb, vec3(0.2126f, 0.7152f, 0.0722f));
}

float linearToSrgb(float linearColor)
{
    if (linearColor < 0.0031308f) {
        return linearColor * 12.92f;
    }
    else {
        return 1.055f * float(pow(linearColor, 1.0f / 2.4f)) - 0.055f;
    }
}

vec3 linearToSrgb(vec3 linearColor)
{
    return vec3(linearToSrgb(linearColor.x), linearToSrgb(linearColor.y), linearToSrgb(linearColor.z));
}

vec3 extract_camera_position(mat4 viewMatrix) {
    mat4 inverseViewMatrix = inverse(viewMatrix);
    vec3 cameraPosition = vec3(inverseViewMatrix[3]);
    return cameraPosition;
}

vec3 world_dir_from_ndc(vec3 ndc, mat4 view, mat4 projection)
{
    vec4 clipSpace = vec4(ndc, 1.0);
    vec4 viewSpace = inverse(projection) * clipSpace;
    viewSpace.w = 0.0;
    vec4 worldSpace = inverse(view) * viewSpace;
    vec3 worldDir = normalize(worldSpace.xyz);

    return worldDir;
}

vec3 world_dir_from_uv(vec2 uv, mat4 view, mat4 projection)
{
    return world_dir_from_ndc(vec3(uv, 0.0) * 2.0 - 1.0, view, projection);
}

// Clever offset_ray function from Ray Tracing Gems chapter 6
// Offsets the ray origin from current position p, along normal n (which must be geometric normal)
// so that no self-intersection can occur.
vec3 offsetRay(const vec3 p, const vec3 n)
{
    const float origin = 1.0f / 32.0f;
    const float float_scale = 1.0f / 65536.0f;
    const float int_scale = 256.0f;

    ivec3 of_i = ivec3(int_scale * n.x, int_scale * n.y, int_scale * n.z);

    vec3 p_i = vec3(
    intBitsToFloat(floatBitsToInt(p.x) + ((p.x < 0) ? -of_i.x : of_i.x)),
    intBitsToFloat(floatBitsToInt(p.y) + ((p.y < 0) ? -of_i.y : of_i.y)),
    intBitsToFloat(floatBitsToInt(p.z) + ((p.z < 0) ? -of_i.z : of_i.z)));

    return vec3(abs(p.x) < origin ? p.x + float_scale * n.x : p_i.x,
    abs(p.y) < origin ? p.y + float_scale * n.y : p_i.y,
    abs(p.z) < origin ? p.z + float_scale * n.z : p_i.z);
}

float DistributionGGX(vec3 N, vec3 H, float roughness)
{
    float a = roughness * roughness;
    float a2 = a * a;
    float NdotH = max(dot(N, H), 0.0);
    float NdotH2 = NdotH * NdotH;

    float num = a2;
    float denom = (NdotH2 * (a2 - 1.0) + 1.0);
    denom = PI * denom * denom;

    return num / denom;
}

float GeometrySchlickGGX(float NdotV, float roughness)
{
    float r = (roughness + 1.0);
    float k = (r * r) / 8.0;

    float num = NdotV;
    float denom = NdotV * (1.0 - k) + k;

    return num / denom;
}

float GeometrySmith(vec3 N, vec3 V, vec3 L, float roughness)
{
    float NdotV = max(dot(N, V), 0.0);
    float NdotL = max(dot(N, L), 0.0);
    float ggx2 = GeometrySchlickGGX(NdotV, roughness);
    float ggx1 = GeometrySchlickGGX(NdotL, roughness);

    return ggx1 * ggx2;
}

// Based omn http://byteblacksmith.com/improvements-to-the-canonical-one-liner-glsl-rand-for-opengl-es-2-0/
float random(vec2 co)
{
    float a = 12.9898;
    float b = 78.233;
    float c = 43758.5453;
    float dt = dot(co.xy, vec2(a, b));
    float sn = mod(dt, 3.14);
    return fract(sin(sn) * c);
}

// Radical inverse based on http://holger.dammertz.org/stuff/notes_HammersleyOnHemisphere.html
vec2 hammersley2d(uint i, uint N)
{
    uint bits = (i << 16u) | (i >> 16u);
    bits = ((bits & 0x55555555u) << 1u) | ((bits & 0xAAAAAAAAu) >> 1u);
    bits = ((bits & 0x33333333u) << 2u) | ((bits & 0xCCCCCCCCu) >> 2u);
    bits = ((bits & 0x0F0F0F0Fu) << 4u) | ((bits & 0xF0F0F0F0u) >> 4u);
    bits = ((bits & 0x00FF00FFu) << 8u) | ((bits & 0xFF00FF00u) >> 8u);
    float rdi = float(bits) * 2.3283064365386963e-10;
    return vec2(float(i) / float(N), rdi);
}

// Based on http://blog.selfshadow.com/publications/s2013-shading-course/karis/s2013_pbs_epic_slides.pdf
// https://github.com/SaschaWillems/Vulkan-glTF-PBR/blob/master/data/shaders/genbrdflut.frag
vec3 importanceSample_GGX(vec2 Xi, float roughness, vec3 normal)
{
    // Maps a 2D point to a hemisphere with spread based on roughness
    float alpha = roughness * roughness;
    float phi = 2.0 * PI * Xi.x + random(normal.xz) * 0.1;
    float cosTheta = sqrt((1.0 - Xi.y) / (1.0 + (alpha * alpha - 1.0) * Xi.y));
    float sinTheta = sqrt(1.0 - cosTheta * cosTheta);
    vec3 H = vec3(sinTheta * cos(phi), sinTheta * sin(phi), cosTheta);

    // Tangent space
    vec3 up = abs(normal.z) < 0.999 ? vec3(0.0, 0.0, 1.0) : vec3(1.0, 0.0, 0.0);
    vec3 tangentX = normalize(cross(up, normal));
    vec3 tangentY = normalize(cross(normal, tangentX));

    // Convert to world Space
    return normalize(tangentX * H.x + tangentY * H.y + normal * H.z);
}

vec3 fresnelSchlick(float cosTheta, vec3 F0)
{
    return F0 + (1.0 - F0) * pow(clamp(1.0 - cosTheta, 0.0, 1.0), 5.0);
}

vec3 fresnelSchlickRoughness(float cosTheta, vec3 F0, float roughness)
{
    return F0 + (max(vec3(1.0 - roughness), F0) - F0) * pow(clamp(1.0 - cosTheta, 0.0, 1.0), 5.0);
}

struct PixelParams
{
    vec3 position;
    vec3 baseColor;
    vec3 normal;
    float metallic;
    float roughness;
    float occlusion;
    uint padding;
};

struct GpuLight
{
    vec4 type_range_spot_id;
    vec4 position;
    vec4 color;
    vec4 direction;
    vec4 attenuation;
};

vec3 surfaceShading(const PixelParams pixel, GpuLight light, const vec3 eyePos, float lightColorFactor)
{
    light.direction = vec4(-light.direction.x, light.direction.y, -light.direction.z, 1.0);
    vec3 color = vec3(0.0f);

/* Implementation from https://learnopengl.com/PBR/Theory */
    vec3 N = pixel.normal;
    vec3 V = normalize(eyePos - pixel.position);
    vec3 R = reflect(V, N);

    vec3 F0 = vec3(0.04);
    F0 = mix(F0, pixel.baseColor, pixel.metallic);

    vec3 L = vec3(0.0);
    float attenuation = 1.0f;
    vec3 posToLight = light.position.xyz - pixel.position;

    if (light.type_range_spot_id.x == 0.0f)// Directional light
    {
        L = normalize(light.direction.xyz * vec3(-1, 1, -1));
        attenuation = light.attenuation.x;
    }
    else if (light.type_range_spot_id.x == 1.0f)// Point light
    {
        L = normalize(posToLight);
        float d = length(posToLight);
        attenuation = 1.0f / dot(light.attenuation.xyz, vec3(1.0f, d, d * d));
    }
    else if (light.type_range_spot_id.x == 2.0f)// Spot light
    {
        L = normalize(posToLight);
        float d = length(posToLight);
        float spot = pow(max(dot(L, normalize(light.direction.xyz)), 0.0f), light.type_range_spot_id.z);
        attenuation = spot / dot(light.attenuation.xyz, vec3(1.0f, d, d * d));
    }

    // Reflectance equation
    vec3 Lo = vec3(0.0);

    vec3 H = normalize(V + L);
    vec3 radiance = light.color.rgb * attenuation * lightColorFactor;

    // Cook-torrance brdf
    float NDF = DistributionGGX(N, H, pixel.roughness);
    float G = GeometrySmith(N, V, L, pixel.roughness);
    vec3 F = fresnelSchlick(max(dot(H, V), 0.0), F0);

    vec3 kS = F;
    vec3 kD = vec3(1.0) - kS;
    kD *= 1.0 - pixel.metallic;

    vec3 numerator = NDF * G * F;
    float denominator = 4.0 * max(dot(N, V), 0.0) * max(dot(N, L), 0.0) + 0.0001;
    vec3 specular = numerator / denominator;

    // Add to outgoing radiance Lo
    float NdotL = max(dot(N, L), 0.0);
    color = (kD * pixel.baseColor / PI + specular) * radiance * NdotL;

    return color;
}

vec3 imageBasedLighting(const PixelParams pixel, const vec3 eyePos, samplerCube in_irradiance_map, samplerCube in_specular_map, sampler2D in_brdf_lut)
{
    vec3 V = normalize(eyePos - pixel.position);
    vec3 R = reflect(V, pixel.normal.xyz);// Note: -1 indicates that the specular cubemp not being as expected

    vec3 F0 = vec3(0.04);
    F0 = mix(F0, pixel.baseColor, pixel.metallic);

    vec3 F = fresnelSchlickRoughness(max(dot(pixel.normal.xyz, V), 0.0), F0, pixel.roughness);
    vec3 kS = F;
    vec3 kD = 1.0 - kS;
    kD *= 1.0 - pixel.metallic;

    vec3 irradiance = texture(in_irradiance_map, pixel.normal.xyz).rgb;
    vec3 diffuse = irradiance * pixel.baseColor;

    // Sample both the pre-filter map and the BRDF lut and combine them together as per the Split-Sum approximation to get the IBL specular part.
    // Note: 1 - roughness, same as Vulkan-glTF-PBR but differs from LearnOpenGL
    const float MAX_REFLECTION_LOD = 7.0;
    vec3 prefilteredColor = textureLod(in_specular_map, R, pixel.roughness * MAX_REFLECTION_LOD).rgb;
    vec2 brdf = texture(in_brdf_lut, vec2(max(dot(pixel.normal.xyz, V), 0.0), 1.0f - pixel.roughness)).rg;
    vec3 specular = prefilteredColor * (F * brdf.x + brdf.y);

    vec3 ambient = (kD * diffuse + specular) * pixel.occlusion;

    return ambient;
}

#endif
