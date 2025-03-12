use crate::{
    component::Transform,
    resource::{
        input::{EInputButton, EInputState}, Input,
        Timer,
    },
};
use bevy_ecs::prelude::*;
use bevy_math::{Mat4, Quat, Vec3};
use std::f32::consts::PI;
use winit::keyboard::{KeyCode, PhysicalKey};

const ROTATE_SPEED: f32 = PI / 10.0;
const MOVE_SPEED: f32 = 0.01;
const MOUSE_SENSITIVITY: f32 = 0.005;

/// The orthographic camera size settings. Since we can not fix the screen aspect ratio,
/// we must choose to either set the width or height, or set the minimum width and height.
#[derive(Debug, Clone, Copy)]
pub enum OrthographicCameraSize {
    /// Set a width and calculate height by width / aspect_ratio.
    FixedWidth = 0,
    /// Set a height and calculate with by height * aspect_ratio.
    FixedHeight = 1,
    /// Set a min width and a min height.
    MinWidthHeight = 2,
}

impl From<i32> for OrthographicCameraSize {
    fn from(value: i32) -> Self {
        match value {
            0 => OrthographicCameraSize::FixedWidth,
            1 => OrthographicCameraSize::FixedHeight,
            2 => OrthographicCameraSize::MinWidthHeight,
            _ => OrthographicCameraSize::FixedHeight,
        }
    }
}

impl OrthographicCameraSize {
    /// Use with [crate::data::Limit::Int32Enum].
    pub fn enum_vector() -> Vec<(i32, String)> {
        vec![
            (0, "FixedWidth".into()),
            (1, "FixedHeight".into()),
            (2, "MinWidthHeight".into()),
        ]
    }
}

/// The camera settings.
#[derive(Debug, Clone, Copy)]
pub enum CameraSettings {
    Orthographic {
        width: f32,
        height: f32,
        size: OrthographicCameraSize,
        near: f32,
        far: f32,
    },
    Perspective {
        /// The y fov radians.
        fov: f32,
        /// Must be greater than zero.
        near: f32,
        /// Must be greater than zero.
        far: f32,
    },
}

impl CameraSettings {
    /// Create a new orthographic camera settings with default value.
    pub fn new_orthographic() -> Self {
        CameraSettings::Orthographic {
            width: 20.0,
            height: 20.0,
            size: OrthographicCameraSize::FixedHeight,
            near: -1000000.0,
            far: 1000000.0,
        }
    }

    /// Create a new perspective camera settings with default value.
    pub fn new_perspective() -> Self {
        CameraSettings::Perspective {
            fov: 45.0_f32.to_radians(),
            near: 0.1,
            far: 1000.0,
        }
    }
}

/// Camera management service
#[derive(Resource)]
pub struct Camera {
    transform: Transform,
    pub settings: CameraSettings,
    aspect: f32,
    view: Mat4,
    prev_view: Mat4,
    proj: Mat4,
    is_dirty: bool,
}

impl Default for Camera {
    fn default() -> Self {
        Camera {
            transform: Default::default(),
            settings: CameraSettings::new_perspective(),
            aspect: 1.0,
            view: Mat4::IDENTITY,
            prev_view: Mat4::IDENTITY,
            proj: Mat4::IDENTITY,
            is_dirty: true,
        }
    }
}

impl Camera {
    /// Creates new Camera instance
    pub fn new(position: Vec3, rotation: Quat) -> Self {
        Self {
            transform: Transform::from_xyz(position.x, position.y, position.z)
                .with_rotation(rotation),
            ..Default::default()
        }
    }

    /// Returns view matrix with zero transition
    ///
    /// It is useful for sky boxes and domes
    pub fn view_matrix_static(&self) -> Mat4 {
        let mut view_static = self.view;
        view_static.w_axis.x = 0.0;
        view_static.w_axis.y = 0.0;
        view_static.w_axis.z = 0.0;
        view_static
    }

    pub fn transform(&self) -> &Transform {
        &self.transform
    }

    /// Returns calculated camera position
    pub fn position(&self) -> Vec3 {
        self.transform.translation
    }

    pub fn direction(&self) -> Vec3 {
        self.transform.direction()
    }

    pub fn up(&self) -> Vec3 {
        self.transform.up().into()
    }

    pub fn right(&self) -> Vec3 {
        self.transform.right().into()
    }

    pub fn near_p(&self) -> f32 {
        match self.settings {
            CameraSettings::Orthographic { near, .. } => near,
            CameraSettings::Perspective { near, .. } => near,
        }
    }

    pub fn fov(&self) -> f32 {
        match self.settings {
            CameraSettings::Orthographic { .. } => 0.0,
            CameraSettings::Perspective { fov, .. } => fov,
        }
    }

    /// Calculate the (projection * view) matrix of camera.
    pub fn projection_view(&self) -> Mat4 {
        self.proj * self.view
    }

    pub fn projection(&self) -> Mat4 {
        self.proj
    }
    pub fn view(&self) -> Mat4 {
        self.view
    }

    pub fn aspect(&self) -> f32 {
        self.aspect
    }

    pub fn inverse_projection(&self) -> Mat4 {
        self.proj.inverse()
    }

    pub fn inverse_view(&self) -> Mat4 {
        self.view.inverse()
    }

    pub fn prev_view(&self) -> Mat4 {
        self.prev_view
    }

    pub fn resize(&mut self, window_size: [f32; 2]) {
        self.aspect = window_size[0] / window_size[1];

        let mut projection = match self.settings {
            CameraSettings::Orthographic {
                width,
                height,
                size,
                near,
                far,
            } => {
                let (half_width, half_height) = match size {
                    OrthographicCameraSize::FixedWidth => Self::fixed_width(width, window_size),
                    OrthographicCameraSize::FixedHeight => Self::fixed_height(height, window_size),
                    OrthographicCameraSize::MinWidthHeight => {
                        if width / height > window_size[0] / window_size[1] {
                            Self::fixed_width(width, window_size)
                        } else {
                            Self::fixed_height(height, window_size)
                        }
                    }
                };
                Mat4::orthographic_rh(
                    -half_width,
                    half_width,
                    -half_height,
                    half_height,
                    near,
                    far,
                )
            }
            CameraSettings::Perspective { fov, near, far } => {
                Mat4::perspective_rh(fov, window_size[0] / window_size[1], near, far)
            }
        };
        projection.y_axis.y *= -1.0;

        self.proj = projection;
    }

    fn fixed_width(width: f32, window_size: [f32; 2]) -> (f32, f32) {
        let half_width = width / 2.0;
        let half_height = half_width * window_size[1] / window_size[0];
        (half_width, half_height)
    }

    fn fixed_height(height: f32, window_size: [f32; 2]) -> (f32, f32) {
        let half_height = height / 2.0;
        let half_width = half_height * window_size[1] / window_size[0];
        (half_width, half_height)
    }

    /// System controlling camera with mouse.
    pub fn update_camera(mut camera: ResMut<Camera>, input: Res<Input>, frame: Res<Timer>) {
        let time_delta = frame.delta().as_secs_f32();

        // 处理相机旋转
        if input
            .button_state(EInputButton::MouseRight)
            .is_some_and(|state| state == EInputState::Activated)
        {
            let mouse_delta = input.mouse_delta();

            // 计算旋转量（包含帧时间补偿和灵敏度）
            let yaw_amount = -mouse_delta.x * ROTATE_SPEED * time_delta * MOUSE_SENSITIVITY;
            let pitch_amount = mouse_delta.y * ROTATE_SPEED * time_delta * MOUSE_SENSITIVITY;

            // 创建旋转四元数
            let yaw_rot = Quat::from_rotation_y(yaw_amount);
            let pitch_rot = Quat::from_rotation_x(pitch_amount);

            // 应用旋转：先偏航（世界Y轴），后俯仰（本地X轴）
            camera.transform.rotation = yaw_rot * camera.transform.rotation * pitch_rot;

            // 转换为YXZ欧拉角（yaw, pitch, roll）
            let (yaw, mut pitch, _roll) =
                camera.transform.rotation.to_euler(bevy_math::EulerRot::YXZ);

            // 限制俯仰角在±89.9度之间
            pitch = pitch.clamp(-PI / 2.0 + 0.001, PI / 2.0 - 0.001);

            // 重建四元数并清除滚转（保持相机直立）
            camera.transform.rotation = Quat::from_euler(bevy_math::EulerRot::YXZ, yaw, pitch, 0.0);

            // 确保四元数规范化
            camera.transform.rotation = camera.transform.rotation.normalize();
            camera.is_dirty = true;
        }

        let direction = camera.direction();
        let right = direction.cross(Vec3::Y).normalize();

        // 处理 WASD 按键移动相机
        let mut movement = Vec3::ZERO;
        if input
            .button_state(EInputButton::Key(PhysicalKey::Code(KeyCode::KeyW)))
            .is_some_and(|state| state == EInputState::Activated)
        {
            movement += direction * time_delta;
        }
        if input
            .button_state(EInputButton::Key(PhysicalKey::Code(KeyCode::KeyA)))
            .is_some_and(|state| state == EInputState::Activated)
        {
            movement -= right * time_delta;
        }
        if input
            .button_state(EInputButton::Key(PhysicalKey::Code(KeyCode::KeyS)))
            .is_some_and(|state| state == EInputState::Activated)
        {
            movement -= direction * time_delta;
        }
        if input
            .button_state(EInputButton::Key(PhysicalKey::Code(KeyCode::KeyD)))
            .is_some_and(|state| state == EInputState::Activated)
        {
            movement += right * time_delta;
        }
        // E 键上移
        if input
            .button_state(EInputButton::Key(PhysicalKey::Code(KeyCode::KeyE)))
            .is_some_and(|state| state == EInputState::Activated)
        {
            movement += Vec3::Y * time_delta;
        }
        // Q 键下移
        if input
            .button_state(EInputButton::Key(PhysicalKey::Code(KeyCode::KeyQ)))
            .is_some_and(|state| state == EInputState::Activated)
        {
            movement -= Vec3::Y * time_delta;
        }

        if movement.length() > 0.0 {
            movement = movement.normalize() * MOVE_SPEED;
            camera.transform.translation += movement;
            camera.is_dirty = true;
        }

        if !camera.is_dirty {
            return;
        }

        let up = right.cross(direction).normalize();
        let view = Mat4::look_at_rh(camera.position(), camera.position() + direction, up);

        camera.prev_view = camera.view;
        camera.view = view;
    }
}
