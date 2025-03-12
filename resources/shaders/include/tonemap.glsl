#ifndef _TONEMAP_INCLUDE_H_
#define _TONEMAP_INCLUDE_H_

#define GAMMA 2.2

vec3 ToneMappingUncharted2(vec3 color)
{
    float A = 0.22;//0.15;
    float B = 0.30;//0.50;
    float C = 0.10;
    float D = 0.20;
    float E = 0.01;//0.02;
    float F = 0.30;//0.30;
    float W = 11.2;
    float exposure = 2.;
    color *= exposure;
    color = ((color * (A * color + C * B) + D * E) / (color * (A * color + B) + D * F)) - E / F;
    float white = ((W * (A * W + C * B) + D * E) / (W * (A * W + B) + D * F)) - E / F;
    color /= white;
    color = pow(color, vec3(1. / GAMMA));
    return color;
}

vec3 ToneMappingReinhard(vec3 color)
{
    vec3 result = color / (color + vec3(1.0));
    result = pow(result, vec3(1.0 / GAMMA));

    return result;
}

// Unreal 3, Documentation: "Color Grading"
// Adapted to be close to Tonemap_ACES, with similar range
// Gamma 2.2 correction is baked in, don't use with sRGB conversion!
vec3 ToneMappingUnreal(vec3 x) {
    return x / (x + 0.155) * 1.019;
}

#endif
