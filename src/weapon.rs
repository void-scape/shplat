use crate::{
    Serialize,
    player::{AimVector, Attack, Grounded, Player},
};
use avian2d::prelude::Gravity;
use bevy::{
    ecs::{lifecycle::HookContext, world::DeferredWorld},
    prelude::*,
};
use bevy_enhanced_input::prelude::Fire;

pub fn plugin(app: &mut App) {
    app.add_systems(Update, weapon_velocity.in_set(WeaponSystems))
        .add_observer(reload)
        .add_observer(insert_fire)
        .add_observer(remove_fire)
        .add_observer(shotgun)
        .add_observer(assault_rifle)
        .add_observer(gravity_gun);
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, SystemSet)]
pub struct WeaponSystems;

#[derive(Default, Component)]
pub struct WeaponAcceleration(pub Vec2);

#[derive(Default, Component)]
pub struct WeaponVelocity(pub Vec2);

#[derive(Component)]
pub struct WeaponDamping(pub Vec2);

impl Default for WeaponDamping {
    fn default() -> Self {
        Self(Vec2::new(10.0, 120.0))
    }
}

fn weapon_velocity(
    time: Res<Time>,
    mut weapons: Query<(&mut WeaponVelocity, &mut WeaponAcceleration, &WeaponDamping)>,
) {
    let dt = time.delta_secs();
    for (mut velocity, mut acceleration, damping) in weapons.iter_mut() {
        velocity.0 += acceleration.0 * dt;
        velocity.0 *= 1.0 / (1.0 + damping.0 * dt);
        acceleration.0 = Vec2::ZERO;
    }
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
) {
    let (entity, mut ammo) = weapon.into_inner();
    if ammo.0 == 0 {
        return;
    }
    commands.entity(entity).insert(FireWeapon);
    ammo.0 -= 1;
}

fn remove_fire(insert: On<Insert, FireWeapon>, mut commands: Commands) {
    commands.entity(insert.entity).remove::<FireWeapon>();
}

#[derive(Default, Component, Reflect)]
#[require(
    Serialize,
    // TODO: move
    SelectedWeapon,
    WeaponAcceleration,
    WeaponVelocity,
    WeaponDamping
)]
#[reflect(Component)]
pub struct Weapon;

#[derive(Default, Component, Reflect)]
#[reflect(Component)]
pub struct SelectedWeapon;

#[derive(Component, Reflect)]
#[require(Weapon, MaxAmmo(1), Name::new("Shotgun"))]
#[reflect(Component)]
pub struct Shotgun;

fn shotgun(
    _fire: On<Insert, FireWeapon>,
    aim_vector: Single<&AimVector, With<Player>>,
    mut velocity: Single<&mut WeaponVelocity, (With<Shotgun>, With<SelectedWeapon>)>,
) {
    let dir = -aim_vector.0;
    let force = dir * 1_000.0;
    velocity.0 += force;
}

#[derive(Component, Reflect)]
#[require(Weapon, MaxAmmo(4), Name::new("Assault Rifle"))]
#[reflect(Component)]
pub struct AssaultRifle;

fn assault_rifle(
    _fire: On<Insert, FireWeapon>,
    aim_vector: Single<&AimVector, With<Player>>,
    mut velocity: Single<&mut WeaponVelocity, (With<AssaultRifle>, With<SelectedWeapon>)>,
) {
    let dir = -aim_vector.0;
    let force = dir * 800.0;
    velocity.0 += force;
}

#[derive(Component, Reflect)]
#[require(Weapon, MaxAmmo(2), Name::new("Gravity Gun"))]
#[reflect(Component)]
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
