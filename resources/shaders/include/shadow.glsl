#ifndef _SHADOW_H_
#define _SHADOW_H_

float computeShadow(vec4 clipSpaceCoordWrtLight, sampler2D shadowMap) {
    vec3 ndcCoordWrtLight = clipSpaceCoordWrtLight.xyz / clipSpaceCoordWrtLight.w;

    vec3 zeroToOneCoordWrtLight = ndcCoordWrtLight;

    // z in vulkan is already in 0 to 1 space
    zeroToOneCoordWrtLight.xy = (zeroToOneCoordWrtLight.xy + 1.0) / 2.0;

    // y needs to be inverted in vulkan
    zeroToOneCoordWrtLight.y = 1.0 - zeroToOneCoordWrtLight.y;

    const float depthBias = 0.00000005;
    zeroToOneCoordWrtLight.z = zeroToOneCoordWrtLight.z - depthBias;

    float depthFromShadowMap = texture(shadowMap, zeroToOneCoordWrtLight.xy).x;
    return step(zeroToOneCoordWrtLight.z, depthFromShadowMap);
}

float PCF_shadow(vec4 clipSpaceCoordWrtLight, sampler2D shadowMap) {
    vec2 texCoord = clipSpaceCoordWrtLight.xy / clipSpaceCoordWrtLight.w;

    if (texCoord.x > 1.0 || texCoord.y > 1.0 || texCoord.x < 0.0 || texCoord.y < 0.0) {
        return 1.0;
    }

    vec2 texSize = textureSize(shadowMap, 0);

    float result = 0.0;
    vec2 offset = (1.0 / texSize) * clipSpaceCoordWrtLight.w;

    // 对当前像素周围的16个像素（4x4区域）进行采样，然后计算平均值来减少阴影的走样现象
    for (float i = -1.5; i <= 1.5; i += 1.0) {
        for (float j = -1.5; j <= 1.5; j += 1.0) {
            result += computeShadow(clipSpaceCoordWrtLight + vec4(vec2(i, j) * offset, 0.0, 0.0), shadowMap);
        }
    }

    return result / 16.0;
}

#endif
