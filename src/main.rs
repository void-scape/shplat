#![allow(clippy::type_complexity)]
#![allow(clippy::too_many_arguments)]

use avian2d::prelude::*;
use bevy::log::LogPlugin;
use bevy::prelude::*;
#[cfg(feature = "debug")]
use bevy::window::PrimaryWindow;

#[cfg(feature = "debug")]
mod inspector;
mod level;
mod player;
mod weapon;

pub const WIDTH: f32 = 1280.0;
pub const HEIGHT: f32 = 720.0;
pub const GRAVITY: f32 = 2000.0;

fn main() {
    let mut app = App::default();

    #[cfg(feature = "debug")]
    let log = LogPlugin {
        custom_layer: inspector::term_layer,
        ..Default::default()
    };
    #[cfg(not(feature = "debug"))]
    let log = LogPlugin::default();

    app.add_plugins((
        DefaultPlugins
            .set(ImagePlugin::default_nearest())
            .set(WindowPlugin {
                primary_window: Some(Window {
                    resolution: (WIDTH as u32, HEIGHT as u32).into(),
                    ..Default::default()
                }),
                ..Default::default()
            })
            .set(log),
        bevy_tween::DefaultTweenPlugins,
        bevy_rand::prelude::EntropyPlugin::<bevy_rand::prelude::WyRand>::with_seed(
            69u64.to_le_bytes(),
        ),
        #[cfg(feature = "debug")]
        inspector::plugin,
    ))
    .add_plugins((
        avian2d::PhysicsPlugins::default().with_length_unit(20.0),
        #[cfg(feature = "debug")]
        avian2d::debug_render::PhysicsDebugPlugin,
        bevy_enhanced_input::EnhancedInputPlugin,
        level::plugin,
        player::plugin,
        weapon::plugin,
    ))
    .insert_resource(Gravity(Vec2::NEG_Y * GRAVITY));

    #[cfg(not(feature = "debug"))]
    app.set_error_handler(bevy::ecs::error::warn);

    app.add_systems(
        Startup,
        (
            camera,
            #[cfg(feature = "debug")]
            maximize,
        ),
    )
    .run();
}

#[cfg(not(debug_assertions))]
pub fn name(_: impl Into<std::borrow::Cow<'static, str>>) -> () {}
#[cfg(debug_assertions)]
pub fn name(name: impl Into<std::borrow::Cow<'static, str>>) -> Name {
    Name::new(name)
}

#[cfg(feature = "debug")]
fn maximize(mut window: Single<&mut Window, With<PrimaryWindow>>) {
    window.set_maximized(true);
}

fn camera(mut commands: Commands) {
    commands.spawn(Camera2d);
}
