#[cfg(feature = "debug")]
use crate::inspector;
use crate::{HEIGHT, WIDTH, player::Player, weapon::Bullet};
use avian2d::prelude::{
    Collider, ColliderConstructor, CollisionEventsEnabled, CollisionLayers, CollisionStart,
    LayerMask, PhysicsLayer, RigidBody, Sensor,
};
use bevy::{
    color::palettes::css::{BLUE, GREEN, RED, YELLOW},
    ecs::{lifecycle::HookContext, world::DeferredWorld},
    prelude::*,
    scene::SceneInstance,
    tasks::IoTaskPool,
};
use std::{fs::File, io::Write};

pub fn plugin(app: &mut App) {
    app.init_resource::<Level>()
        .add_systems(Startup, deserialize_level)
        .add_systems(
            Update,
            (
                add_pickable_sprites,
                remove_dynamic_scene_root,
                #[cfg(feature = "debug")]
                user_serialize_level,
                user_reset_level,
            ),
        )
        .add_observer(killbox)
        .add_observer(door)
        .add_observer(must_keep)
        .add_observer(destroy_key);
}

#[derive(Default, PhysicsLayer, Component)]
pub enum Layer {
    #[default]
    Default,
    Player,
    Bullet,
    Wall,
}

#[derive(Default, Clone, Copy, Component, Reflect)]
#[reflect(Component)]
pub struct Serialize;

#[derive(Resource)]
pub struct Level(pub String);

impl Default for Level {
    fn default() -> Self {
        Self("shotgun_1".to_string())
    }
}

#[derive(Component, Reflect)]
#[reflect(Component)]
pub struct LevelGeometry;

#[derive(Clone, Copy, Component, Reflect)]
#[require(
    Serialize,
    RigidBody::Static,
    DebugPickingColor::new(BLUE),
    CollisionLayers::new(Layer::Wall, LayerMask::ALL)
)]
#[reflect(Component)]
pub struct Wall;

#[derive(Clone, Copy, Component, Reflect)]
#[require(
    Serialize,
    RigidBody::Static,
    Sensor,
    CollisionEventsEnabled,
    DebugPickingColor::new(RED)
)]
#[reflect(Component)]
pub struct KillBox;

fn killbox(
    enter: On<CollisionStart>,
    mut commands: Commands,
    player: Single<Entity, With<Player>>,
    killboxes: Query<&KillBox>,
) {
    if killboxes.contains(enter.collider1) && enter.collider2 == *player {
        commands.run_system_cached(reset_level);
    }
}

#[derive(Component, Reflect)]
#[require(
    Serialize,
    Transform,
    RigidBody::Static,
    Sensor,
    CollisionEventsEnabled,
    SerializedColliderConstructor = rectangle(20.0, 20.0),
    DebugPickingColor::new(GREEN),
)]
#[reflect(Component)]
pub struct Door(pub String);

#[derive(Component)]
pub struct Locked;

fn door(
    start: On<CollisionStart>,
    mut commands: Commands,
    player: Single<Entity, With<Player>>,
    doors: Query<(&Door, Option<&Keys>), Without<Locked>>,
    must_destroy: Query<&MustDestroy>,
    mut level: ResMut<Level>,
) {
    if *player == start.collider2
        && let Ok((door, keys)) = doors.get(start.collider1)
        && keys.is_none_or(|keys| keys.iter().all(|entity| !must_destroy.contains(entity)))
    {
        level.0 = door.0.clone();
        commands.run_system_cached(despawn_level);
        commands.run_system_cached(reset_level);
    }
}

#[derive(Component, Reflect)]
#[relationship_target(relationship = KeyOf)]
#[reflect(Component)]
pub struct Keys(Vec<Entity>);

#[derive(Component, Reflect)]
#[relationship(relationship_target = Keys)]
#[reflect(Component)]
pub struct KeyOf(pub Entity);

#[derive(Default, Clone, Copy, Component, Reflect)]
#[require(
    Serialize,
    Transform,
    RigidBody::Static,
    Sensor,
    CollisionEventsEnabled,
    SerializedColliderConstructor = rectangle(20.0, 20.0),
    DebugPickingColor::new(YELLOW),
)]
#[reflect(Component)]
pub struct Key;

#[derive(Clone, Copy, Component, Reflect)]
#[require(Key)]
#[reflect(Component)]
pub struct MustDestroy;

#[derive(Clone, Copy, Component, Reflect)]
#[require(Key)]
#[reflect(Component)]
pub struct MustKeep;

fn must_keep(remove: On<Remove, MustKeep>, mut commands: Commands, key_ofs: Query<&KeyOf>) {
    if let Ok(key_of) = key_ofs.get(remove.entity) {
        commands.entity(key_of.0).insert(Locked);
    }
}

fn destroy_key(
    enter: On<CollisionStart>,
    mut commands: Commands,
    keys: Query<&Key>,
    bullets: Query<&Bullet>,
) {
    if keys.contains(enter.collider1) && bullets.contains(enter.collider2) {
        commands.entity(enter.collider1).despawn();
    }
}

#[derive(Component)]
struct DebugPickingColor(Color);

impl DebugPickingColor {
    fn new(color: impl Into<Color>) -> Self {
        Self(color.into())
    }
}

fn add_pickable_sprites(
    mut commands: Commands,
    walls: Query<(Entity, &Collider, &DebugPickingColor), Without<Sprite>>,
) {
    for (entity, collider, color) in walls.iter() {
        let shape = collider.shape().as_cuboid().unwrap();
        commands.entity(entity).insert(Sprite::from_color(
            color.0,
            Vec2::new(shape.half_extents.x * 2.0, shape.half_extents.y * 2.0),
        ));
    }
}

#[derive(Clone, Component, Reflect)]
#[component(on_insert = Self::insert)]
#[reflect(Component)]
pub struct SerializedColliderConstructor(pub ColliderConstructor);

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

pub fn rectangle(width: f32, height: f32) -> SerializedColliderConstructor {
    SerializedColliderConstructor(ColliderConstructor::Rectangle {
        x_length: width,
        y_length: height,
    })
}

#[cfg(feature = "debug")]
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
    use crate::weapon::*;
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
        .allow_component::<Door>()
        .allow_component::<MustDestroy>()
        .allow_component::<MustKeep>()
        .allow_component::<Keys>()
        .allow_component::<KeyOf>()
        .allow_component::<Wall>()
        .allow_component::<KillBox>()
        .allow_component::<Sensor>()
        .allow_component::<CollisionEventsEnabled>()
        .allow_component::<RigidBody>()
        .allow_component::<SerializedColliderConstructor>()
        .extract_entities(serialize.iter())
        .build();
    let type_registry = world.resource::<AppTypeRegistry>().read();
    let serialized_scene = scene.serialize(&type_registry).unwrap();

    let level_ident = level.0.clone();
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
        Name::from(level.0.clone()),
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

fn user_reset_level(
    mut commands: Commands,
    input: Res<ButtonInput<KeyCode>>,
    #[cfg(feature = "debug")] disable_input: Query<&inspector::DisableInput>,
) {
    #[cfg(feature = "debug")]
    if !disable_input.is_empty() {
        return;
    }
    if !input.just_pressed(KeyCode::KeyR) {
        return;
    }
    commands.run_system_cached(reset_level);
}

pub fn reset_level(mut commands: Commands) {
    commands.run_system_cached(despawn_level);
    commands.run_system_cached(deserialize_level);
}

pub fn new_level(mut commands: Commands) {
    commands.spawn((
        Player,
        Name::new("Player"),
        Transform::from_xyz(-400.0, 0.0, 0.0),
    ));
    let child = commands
        .spawn((
            KillBox,
            Transform::from_xyz(0.0, -200.0, 0.0),
            rectangle(WIDTH / 10.0, 25.0),
        ))
        .id();
    let mut entity = commands.spawn((
        Serialize,
        LevelGeometry,
        Transform::default(),
        Visibility::default(),
        Name::new("Level Geometry"),
    ));
    entity.add_child(child);
    entity.with_child((
        Transform::from_xyz(0.0, -HEIGHT / 2.0, 0.0),
        rectangle(WIDTH, 25.0),
        Name::new("Bottom Wall"),
        Wall,
    ));
    entity.with_child((
        Transform::from_xyz(-WIDTH / 2.0, 0.0, 0.0),
        rectangle(25.0, HEIGHT),
        Name::new("Left Wall"),
        Wall,
    ));
    entity.with_child((
        Transform::from_xyz(WIDTH / 2.0, 0.0, 0.0),
        rectangle(25.0, HEIGHT),
        Name::new("Right Wall"),
        Wall,
    ));
    entity.with_child((
        Transform::from_xyz(0.0, HEIGHT / 2.0, 0.0),
        rectangle(WIDTH, 25.0),
        Name::new("Top Wall"),
        Wall,
    ));
}
