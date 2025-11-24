use crate::{
    level::{
        DebugPickingColor, Key, Layer, Serialize, SerializedColliderConstructor, Transient,
        rectangle,
    },
    player::{AimVector, Attack, Grounded, Player, WeaponVelocity},
};
use avian2d::prelude::*;
use bevy::{
    color::palettes::css::PURPLE,
    ecs::{lifecycle::HookContext, world::DeferredWorld},
    prelude::*,
};
use bevy_enhanced_input::prelude::Fire;
use bevy_rand::{global::GlobalRng, prelude::WyRand};
use bevy_tween::{
    bevy_time_runner::TimeRunnerEnded, component_tween_system, prelude::*, tween::AnimationTarget,
};
use rand::Rng;
use std::f32::consts::PI;

pub fn plugin(app: &mut App) {
    app.add_systems(Update, (despawn_bullets, laser, weapon_pickup))
        .add_tween_systems(component_tween_system::<BulletVelocityLength>())
        .add_observer(reload)
        .add_observer(insert_fire)
        .add_observer(remove_fire)
        .add_observer(shotgun)
        .add_observer(assault_rifle)
        .add_observer(gravity_gun)
        .add_observer(rocket);
}

#[derive(Component, Reflect)]
#[component(on_insert = Self::insert)]
#[reflect(Component)]
pub struct MaxAmmo(pub usize);

impl MaxAmmo {
    fn insert(mut world: DeferredWorld, ctx: HookContext) {
        let max = world.get::<Self>(ctx.entity).unwrap().0;
        world.commands().entity(ctx.entity).insert_if_new(Ammo(max));
    }
}

#[derive(Component)]
pub struct Ammo(pub usize);

fn reload(_: On<Insert, Grounded>, ammo: Single<(&mut Ammo, &MaxAmmo), With<SelectedWeapon>>) {
    let (mut ammo, max_ammo) = ammo.into_inner();
    ammo.0 = max_ammo.0;
}

#[derive(Component)]
struct FireWeapon;

fn insert_fire(
    _attack: On<Fire<Attack>>,
    mut commands: Commands,
    weapon: Single<(Entity, &mut Ammo), With<SelectedWeapon>>,
    is_grounded: Single<Has<Grounded>, With<Player>>,
) {
    let (entity, mut ammo) = weapon.into_inner();
    if !*is_grounded && ammo.0 == 0 {
        return;
    }
    commands.entity(entity).insert(FireWeapon);
    if !*is_grounded {
        ammo.0 -= 1;
    }
}

fn remove_fire(insert: On<Insert, FireWeapon>, mut commands: Commands) {
    commands.entity(insert.entity).remove::<FireWeapon>();
}

#[derive(Default, Component, Reflect)]
#[require(Serialize)]
#[reflect(Component)]
pub struct Weapon;

#[derive(Default, Component, Reflect)]
#[reflect(Component)]
pub struct SelectedWeapon;

#[derive(Default, Clone, Copy, Component, Reflect)]
#[require(Weapon, MaxAmmo(1), Name::new("Shotgun"))]
#[reflect(Default, Component)]
pub struct Shotgun;

fn shotgun(
    _fire: On<Insert, FireWeapon>,
    mut commands: Commands,
    player: Single<(&mut WeaponVelocity, &GlobalTransform, &AimVector), With<Player>>,
    _shotgun: Single<(), (With<Shotgun>, With<SelectedWeapon>)>,
    mut rng: Single<&mut WyRand, With<GlobalRng>>,
) {
    let (mut player_velocity, player_transform, aim_vector) = player.into_inner();

    let dir = -aim_vector.0;
    let force = dir * 2_000.0;
    player_velocity.0 += force;

    for _ in 0..12 {
        let velocity = random_direction_in_arc(aim_vector.0, 0.9, &mut rng);
        let starting_velocity = rng.random_range(1_000.0..1_300.0);

        let target = AnimationTarget.into_target();
        commands
            .spawn((
                Bullet,
                AnimationTarget,
                LinearVelocity(velocity),
                Transform::from_translation(player_transform.translation().xy().extend(0.0)),
                Collider::circle(5.0),
                Sprite::from_color(Color::WHITE, Vec2::splat(10.0)),
                GravityScale(0.0),
            ))
            .animation()
            .insert_tween_here(
                Duration::from_secs_f32(0.8),
                EaseKind::QuadraticOut,
                target.with(bullet_velocity(starting_velocity, 100.0)),
            );
    }
}

#[derive(Default, Clone, Copy, Component, Reflect)]
#[require(Weapon, MaxAmmo(3), Name::new("Assault Rifle"))]
#[reflect(Default, Component)]
pub struct AssaultRifle;

fn assault_rifle(
    _fire: On<Insert, FireWeapon>,
    mut commands: Commands,
    player: Single<(&mut WeaponVelocity, &GlobalTransform, &AimVector), With<Player>>,
    _assault_rifle: Single<(), (With<AssaultRifle>, With<SelectedWeapon>)>,
    mut rng: Single<&mut WyRand, With<GlobalRng>>,
) {
    let (mut player_velocity, player_transform, aim_vector) = player.into_inner();

    let dir = -aim_vector.0;
    let force = dir * 500.0;
    player_velocity.0 += force;

    let velocity = random_direction_in_arc(aim_vector.0, PI * 0.1, &mut rng);
    let starting_velocity = rng.random_range(1_000.0..1_300.0);

    commands
        .spawn((
            Bullet,
            LinearVelocity(velocity * starting_velocity),
            Transform::from_translation(player_transform.translation().xy().extend(0.0)),
            Collider::circle(5.0),
            Sprite::from_color(Color::WHITE, Vec2::splat(10.0)),
            GravityScale(0.0),
            CollisionEventsEnabled,
        ))
        .observe(|target: On<CollisionStart>, mut commands: Commands| {
            commands.entity(target.collider1).despawn();
        });
}

#[derive(Default, Clone, Copy, Component, Reflect)]
#[require(Weapon, MaxAmmo(2), Name::new("Gravity Gun"))]
#[reflect(Default, Component)]
pub struct GravityGun;

fn gravity_gun(
    _fire: On<Insert, FireWeapon>,
    mut commands: Commands,
    player: Single<Entity, With<Player>>,
    _gravity_gun: Single<&GravityGun, With<SelectedWeapon>>,
    mut gravity: ResMut<Gravity>,
) {
    gravity.0.y = -gravity.0.y;
    if gravity.0.y > 0.0 {
        commands.entity(*player).insert(Player::ceiling_caster());
    } else {
        commands.entity(*player).insert(Player::ground_caster());
    }
}

#[derive(Default, Clone, Copy, Component, Reflect)]
#[require(Weapon, MaxAmmo(1), Name::new("Rocket"))]
#[reflect(Default, Component)]
pub struct Rocket;

fn rocket(
    _fire: On<Insert, FireWeapon>,
    mut commands: Commands,
    player: Single<(&GlobalTransform, &AimVector), With<Player>>,
    _rocket: Single<(), (With<Rocket>, With<SelectedWeapon>)>,
) {
    let (player_transform, aim_vector) = player.into_inner();
    let dir = aim_vector.0;
    let velocity = dir * 1_000.0;

    commands
        .spawn((
            Bullet,
            RocketBullet,
            LinearVelocity(velocity),
            Transform::from_translation(player_transform.translation().xy().extend(0.0)),
            Collider::circle(5.0),
            Sprite::from_color(Color::WHITE, Vec2::splat(10.0)),
            GravityScale(0.5),
            CollisionEventsEnabled,
        ))
        .observe(rocket_bullet);
}

#[derive(Component)]
pub struct RocketBullet;

fn rocket_bullet(
    start: On<CollisionStart>,
    mut commands: Commands,
    player: Single<(&mut WeaponVelocity, &GlobalTransform), With<Player>>,
    _rocket: Single<(), (With<Rocket>, With<SelectedWeapon>)>,
    transforms: Query<&GlobalTransform>,
) -> Result {
    let (mut velocity, player_transform) = player.into_inner();
    let transform = transforms.get(start.collider1)?;
    let diff = transform.translation().xy() - player_transform.translation().xy();
    let dist = diff.length();
    let angle = diff.normalize_or(Vec2::NEG_Y);

    let falloff_rate = 0.003;
    let force = 5_000.0 * (-falloff_rate * (dist - 300.0).max(0.0)).exp();
    velocity.0 = velocity.0.max(-angle * force);

    commands.entity(start.collider1).despawn();
    Ok(())
}

#[derive(Default, Clone, Copy, Component, Reflect)]
#[require(Weapon, Name::new("Laser"))]
#[component(on_insert = Laser::insert)]
#[reflect(Default, Component)]
pub struct Laser;

impl Laser {
    fn insert(mut world: DeferredWorld, ctx: HookContext) {
        let mut shape_caster = ShapeCaster::new(Collider::circle(0.5), Vec2::ZERO, 0.0, Dir2::X);
        shape_caster.query_filter = shape_caster
            .query_filter
            .with_mask([Layer::Wall, Layer::Key]);
        world.commands().entity(ctx.entity).insert(shape_caster);
    }
}

fn laser(
    mut commands: Commands,
    aim_vector: Single<&AimVector, With<Player>>,
    laser: Single<(&mut ShapeCaster, &ShapeHits), (With<Laser>, With<SelectedWeapon>)>,
    keys: Query<Entity, With<Key>>,
) {
    let (mut caster, hits) = laser.into_inner();
    for entity in keys.iter_many(hits.iter().map(|data| data.entity)) {
        commands.entity(entity).despawn();
    }
    if let Ok(direction) = Dir2::new(aim_vector.0) {
        caster.direction = direction;
    }
}

#[derive(Component)]
#[require(
    Transient,
    RigidBody::Dynamic,
    LockedAxes::ROTATION_LOCKED,
    Restitution {
        coefficient: 0.1,
        combine_rule: CoefficientCombine::Average,
    },
    CollisionLayers::new(Layer::Bullet, Layer::Default.to_bits() | Layer::Wall.to_bits() | Layer::Key.to_bits()),
)]
pub struct Bullet;

#[derive(Component)]
struct BulletVelocityLength {
    start: f32,
    end: f32,
}

fn bullet_velocity(start: f32, end: f32) -> BulletVelocityLength {
    BulletVelocityLength { start, end }
}

impl Interpolator for BulletVelocityLength {
    type Item = LinearVelocity;
    fn interpolate(
        &self,
        item: &mut Self::Item,
        value: interpolate::CurrentValue,
        _: interpolate::PreviousValue,
    ) {
        if item.0 != Vec2::ZERO {
            let new_length = self.start.lerp(self.end, value);
            item.0 = item.0.normalize() * new_length;
        }
    }
}

fn despawn_bullets(
    mut commands: Commands,
    mut reader: MessageReader<TimeRunnerEnded>,
    bullets: Query<(), With<Bullet>>,
) {
    for event in reader.read() {
        if event.is_completed() && bullets.contains(event.entity) {
            commands.entity(event.entity).despawn();
        }
    }
}

/// Returns a random unit vector whose direction lies within an arc of `arc_radians`
/// centered around the given direction vector.
///
/// `dir` does not have to be normalized; this function normalizes it internally.
/// `arc_radians` is the full width of the arc (e.g. PI/4 is ±PI/8 around dir).
fn random_direction_in_arc(dir: Vec2, arc_radians: f32, rng: &mut impl Rng) -> Vec2 {
    // Normalize the input direction
    let dir = dir.normalize_or_zero();

    // Convert direction to angle
    let base_angle = dir.y.atan2(dir.x); // atan2(y, x)

    // Half-width of the arc
    let half_arc = arc_radians * 0.5;

    // Sample angle uniformly in [base_angle - half_arc, base_angle + half_arc]
    let offset: f32 = rng.random_range(-half_arc..=half_arc);
    let final_angle = base_angle + offset;

    // Convert back to a unit vector
    Vec2 {
        x: final_angle.cos(),
        y: final_angle.sin(),
    }
}

#[derive(Default, Clone, Copy, Component, Reflect)]
#[require(
    Transform, 
    SerializedColliderConstructor = rectangle(50.0, 50.0),
    DebugPickingColor::new(PURPLE),
)]
#[reflect(Default, Component)]
pub struct WeaponPickup;

fn weapon_pickup(
    mut commands: Commands,
    player: Single<(Entity, &GlobalTransform), With<Player>>,
    weapon: Query<Entity, With<SelectedWeapon>>,
    pickups: Query<(Entity, &GlobalTransform), With<WeaponPickup>>,
) {
    let radius = 100.0;
    let (player, player_transform) = player.into_inner();
    let player_translation = player_transform.translation().xy();
    for (pickup, pickup_transform) in pickups.iter() {
        if pickup_transform
            .translation()
            .xy()
            .distance_squared(player_translation)
            < radius * radius
        {
            commands
                .entity(pickup)
                .remove::<(
                    Transform, 
                    WeaponPickup, 
                    Collider, 
                    SerializedColliderConstructor,
                    ColliderConstructor, 
                    Sprite, 
                    DebugPickingColor,
                )>()
                .insert((SelectedWeapon, ChildOf(player)));
            for entity in weapon.iter() {
                commands.entity(entity).despawn();
            }
        }
    }
}
