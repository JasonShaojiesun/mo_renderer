use crate::component::Transform;
use bevy_ecs::prelude::*;
use bevy_math::prelude::*;

#[derive(Component)]
pub struct DirectionalLight {
    pub transform: Transform,
    pub color: Vec3,
    pub intensity: f32,
    pub is_shadow_caster: bool,
    pub shadow_width: f32,
    pub shadow_height: f32,
}

impl Default for DirectionalLight {
    fn default() -> Self {
        Self {
            color: Vec3::new(1.0, 1.0, 1.0),
            intensity: 1.0,
            is_shadow_caster: false,
            shadow_width: 8.0,
            shadow_height: 8.0,
            transform: Transform::default(),
        }
    }
}

impl DirectionalLight {
    pub fn new(transform: Transform, color: Vec3, intensity: f32, is_shadow_caster: bool) -> Self {
        Self {
            color,
            intensity,
            is_shadow_caster,
            transform,
            ..Default::default()
        }
    }

    pub fn proj_view(&self) -> Mat4 {
        let direction = -self.transform.direction();
        let right = direction.cross(Vec3::Y).normalize();
        let up = right.cross(direction).normalize();

        // 构建光源的投影视图矩阵
        let view = Mat4::look_at_rh(
            self.transform.translation,             // Eye
            self.transform.translation + direction, // Center
            up,                                     // Up
        );

        // Vulkan的NDC坐标系中，Y轴向下，而Bevy的orthographic_rh可能生成Y轴向的投影矩阵，这可能导致渲染的深度图上下颠倒。为了解决这个问题，可以在投影矩阵中翻转Y轴。例如，将orthographic_rh的上下参数交换，或者在投影矩阵之后乘以一个Y轴翻转的矩阵。
        let ortho_proj = Mat4::orthographic_rh(
            -self.shadow_width,  // 左
            self.shadow_width,   // 右
            self.shadow_height,  // 下
            -self.shadow_height, // 上
            0.1,                 // 近平面
            100.0,               // 远平面
        );

        ortho_proj * view
    }
}
