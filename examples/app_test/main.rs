use bevy_math::{Quat, Vec3};
use mo_core::App;
use mo_ecs::resource::{GlobalSamplers, IBLResource};
use mo_ecs::{
    component::{DirectionalLight, Transform},
    model::Model,
    resource::{Camera, DefaultTextures, Input, Timer},
};
use std::f32::consts::PI;
use winit::event_loop::EventLoop;

fn main() {
    let event_loop = EventLoop::new().unwrap();
    let mut app = App::new(&event_loop, Default::default());

    // app.init_resource::<AssetManager>();
    app.init_resource::<Timer>();
    app.init_resource::<Input>();

    let camera = Camera::new(
        Vec3::new(-3.0, 0.0, 3.0),
        Quat::from_axis_angle(Vec3::Y, PI * 0.75),
    );
    app.insert_resource::<Camera>(camera);
    app.init_resource::<DefaultTextures>();
    app.init_resource::<IBLResource>();
    app.init_resource::<GlobalSamplers>();

    app.add_runtime_system(Timer::update_timer);
    app.add_runtime_system(Camera::update_camera);

    app.add_entity((
        Transform::from_xyz(0.0, 0.0, 0.0),
        Model::load_gltf("DamagedHelmet.glb"),
    ));

    app.add_entity((DirectionalLight::new(
        Transform::from_xyz(10.0, 10.0, 10.0).looking_at(Vec3::ZERO, Vec3::Y),
        Vec3::new(1.0, 1.0, 1.0),
        4.0,
        true,
    ),));

    app.add_entity((DirectionalLight::new(
        Transform::from_xyz(-10.0, 10.0, 10.0).looking_at(Vec3::ZERO, Vec3::Y),
        Vec3::new(1.0, 1.0, 1.0),
        2.0,
        false,
    ),));

    event_loop
        .run_app(&mut app)
        .expect("Run Application Failed");
}
