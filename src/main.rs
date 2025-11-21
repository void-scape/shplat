#![allow(clippy::type_complexity)]
#![allow(clippy::too_many_arguments)]

use crate::weapon::*;
use avian2d::prelude::*;
use bevy::log::LogPlugin;
#[cfg(feature = "debug")]
use bevy::window::PrimaryWindow;
use bevy::{
    color::palettes::css::BLUE,
    ecs::{lifecycle::HookContext, world::DeferredWorld},
    prelude::*,
    scene::SceneInstance,
    tasks::IoTaskPool,
};
use player::Player;
use std::{fs::File, io::Write};

#[cfg(feature = "debug")]
mod inspector;
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
        #[cfg(feature = "debug")]
        inspector::plugin,
    ))
    .add_plugins((
        avian2d::PhysicsPlugins::default().with_length_unit(20.0),
        #[cfg(feature = "debug")]
        avian2d::debug_render::PhysicsDebugPlugin,
        bevy_enhanced_input::EnhancedInputPlugin,
        player::plugin,
        weapon::plugin,
    ))
    .insert_resource(Gravity(Vec2::NEG_Y * GRAVITY));

    #[cfg(not(feature = "debug"))]
    app.set_error_handler(bevy::ecs::error::warn);

    app.init_resource::<Level>()
        .add_systems(
            Startup,
            (
                camera,
                deserialize_level,
                #[cfg(feature = "debug")]
                maximize,
            ),
        )
        .add_systems(
            Update,
            (
                add_wall_sprites,
                remove_dynamic_scene_root,
                #[cfg(feature = "debug")]
                (user_serialize_level, reset_level),
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

#[derive(Default, Component, Reflect)]
#[reflect(Component)]
pub struct Serialize;

#[derive(Resource)]
pub struct Level(&'static str);

impl Default for Level {
    fn default() -> Self {
        Self("shotgun_1")
    }
}

pub fn user_serialize_level(
    mut commands: Commands,
    input: Res<ButtonInput<KeyCode>>,
    disable_input: Query<&inspector::DisableInput>,
) {
    if !disable_input.is_empty() || !input.just_pressed(KeyCode::KeyP) {
        return;
    }
    commands.run_system_cached(serialize_level);
}

pub fn serialize_level(
    world: &World,
    serialize: Query<Entity, With<Serialize>>,
    level: Res<Level>,
) {
    let scene = DynamicSceneBuilder::from_world(world)
        .allow_component::<Serialize>()
        .allow_component::<Name>()
        .allow_component::<Transform>()
        .allow_component::<GlobalTransform>()
        .allow_component::<Visibility>()
        .allow_component::<Player>()
        .allow_component::<Children>()
        .allow_component::<ChildOf>()
        .allow_component::<SelectedWeapon>()
        .allow_component::<Shotgun>()
        .allow_component::<AssaultRifle>()
        .allow_component::<GravityGun>()
        .allow_component::<LevelGeometry>()
        .allow_component::<Wall>()
        .allow_component::<RigidBody>()
        .allow_component::<SerializedColliderConstructor>()
        .extract_entities(serialize.iter())
        .build();
    let type_registry = world.resource::<AppTypeRegistry>().read();
    let serialized_scene = scene.serialize(&type_registry).unwrap();

    let level_ident = level.0;
    IoTaskPool::get()
        .spawn(async move {
            File::create(format!("assets/scenes/{}.scn.ron", level_ident))
                .and_then(|mut file| file.write(serialized_scene.as_bytes()))
                .expect("error while writing scene to file");
        })
        .detach();
}

pub fn deserialize_level(mut commands: Commands, server: Res<AssetServer>, level: Res<Level>) {
    commands.spawn((
        Name::from(level.0),
        DynamicSceneRoot(server.load(format!("scenes/{}.scn.ron", level.0))),
    ));
}

fn remove_dynamic_scene_root(
    mut commands: Commands,
    dynamic_scenes: Query<(Entity, &Children), With<SceneInstance>>,
) {
    for (entity, children) in dynamic_scenes.iter() {
        for child in children.iter() {
            commands.entity(child).remove::<ChildOf>();
        }
        commands.entity(entity).despawn();
    }
}

pub fn despawn_level(
    mut commands: Commands,
    entities: Query<Entity, (With<Serialize>, Without<ChildOf>)>,
) {
    for entity in entities.iter() {
        commands.entity(entity).despawn();
    }
}

fn reset_level(
    mut commands: Commands,
    input: Res<ButtonInput<KeyCode>>,
    disable_input: Query<&inspector::DisableInput>,
) {
    if !disable_input.is_empty() {
        return;
    }

    if input.just_pressed(KeyCode::KeyR) {
        commands.run_system_cached(despawn_level);
        commands.run_system_cached(deserialize_level);
    }
}

#[derive(Component, Reflect)]
#[component(on_insert = Self::insert)]
#[reflect(Component)]
struct SerializedColliderConstructor(ColliderConstructor);

impl SerializedColliderConstructor {
    fn insert(mut world: DeferredWorld, ctx: HookContext) {
        let constructor = world
            .get::<SerializedColliderConstructor>(ctx.entity)
            .unwrap()
            .0
            .clone();
        world.commands().entity(ctx.entity).insert(constructor);
    }
}

fn rectangle(width: f32, height: f32) -> SerializedColliderConstructor {
    SerializedColliderConstructor(ColliderConstructor::Rectangle {
        x_length: width,
        y_length: height,
    })
}

#[derive(Component, Reflect)]
#[reflect(Component)]
pub struct LevelGeometry;

pub fn new_level(mut commands: Commands) {
    commands.spawn((
        Player,
        name("Player"),
        Transform::from_xyz(-400.0, 0.0, 0.0),
    ));
    let entity = commands.spawn((
        Serialize,
        LevelGeometry,
        Transform::default(),
        Visibility::default(),
        name("Level Geometry"),
    ));
    level_walls(entity);
}

fn level_walls(mut commands: EntityCommands) {
    commands.with_child((
        RigidBody::Static,
        Transform::from_xyz(0.0, -HEIGHT / 2.0, 0.0),
        rectangle(WIDTH, 25.0),
        name("Bottom Wall"),
        Wall,
    ));
    commands.with_child((
        RigidBody::Static,
        Transform::from_xyz(-WIDTH / 2.0, 0.0, 0.0),
        rectangle(25.0, HEIGHT),
        name("Left Wall"),
        Wall,
    ));
    commands.with_child((
        RigidBody::Static,
        Transform::from_xyz(WIDTH / 2.0, 0.0, 0.0),
        rectangle(25.0, HEIGHT),
        name("Right Wall"),
        Wall,
    ));
    commands.with_child((
        RigidBody::Static,
        Transform::from_xyz(0.0, HEIGHT / 2.0, 0.0),
        rectangle(WIDTH, 25.0),
        name("Top Wall"),
        Wall,
    ));
}

#[derive(Component, Reflect)]
#[require(Serialize)]
#[reflect(Component)]
pub struct Wall;

fn add_wall_sprites(
    mut commands: Commands,
    walls: Query<(Entity, &Collider), (With<Wall>, Without<Sprite>)>,
) {
    for (entity, collider) in walls.iter() {
        let shape = collider.shape().as_cuboid().unwrap();
        commands.entity(entity).insert(Sprite::from_color(
            BLUE,
            Vec2::new(shape.half_extents.x * 2.0, shape.half_extents.y * 2.0),
        ));
    }
}
