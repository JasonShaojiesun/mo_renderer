#version 460
#extension GL_EXT_nonuniform_qualifier: require
#extension GL_GOOGLE_include_directive: require

#include "include/bindless.glsl"

layout (location = 0) in vec4 position;
layout (location = 1) in vec4 normal;
layout (location = 2) in vec4 color;
layout (location = 3) in vec2 uv0;
layout (location = 4) in vec2 uv1;
layout (location = 5) in vec4 tangent;

layout (location = 0) out vec2 outTexCoord0;
layout (location = 1) out vec2 outTexCoord1;
layout (location = 2) out vec3 outNormal;
layout (location = 3) out vec4 outTangent;
layout (location = 4) out vec3 outModelSpacePos;
layout (location = 5) out vec4 outClipSpacePos;
layout (location = 6) out vec4 outPrevClipSpacePos;
layout (location = 7) out vec3 outBitangent;
layout (location = 8) out mat3 outTBN;

layout (push_constant) uniform PushConsts {
    mat4 world;
    mat4 normal_matrix;
    uint mat_index;
    ivec3 pad;
} pushConsts;

void main() {
    gl_Position = projview.projection * projview.view * pushConsts.world * vec4(position.xyz, 1.0);

    outTexCoord0 = uv0;
    outTexCoord1 = uv1;

    vec3 bitangentL = cross(normal.xyz, tangent.xyz) * tangent.w;

    mat3 normal_matrix = mat3(pushConsts.normal_matrix);
    // 使用预计算的 Normal Matrix
    vec3 T = normalize(normal_matrix * tangent.xyz);
    vec3 B = normalize(normal_matrix * bitangentL);
    vec3 N = normalize(normal_matrix * normal.xyz);
    outTBN = mat3(T, B, N);
    outNormal = N;

    outTangent = tangent;
    outBitangent = bitangentL;

    outModelSpacePos = position.xyz;
    outClipSpacePos = gl_Position;
    outPrevClipSpacePos = projview.projection * projview.prev_view * pushConsts.world * vec4(position.xyz, 1.0);
}
