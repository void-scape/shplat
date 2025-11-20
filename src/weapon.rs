use crate::player::{AimVector, Attack, Grounded, Player};
use bevy::{
    ecs::{lifecycle::HookContext, world::DeferredWorld},
    prelude::*,
};
use bevy_enhanced_input::prelude::Fire;

pub fn plugin(app: &mut App) {
    app.add_systems(Update, weapon_velocity.in_set(WeaponSystems))
        .add_observer(ammo)
        .add_observer(shotgun);
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
        Self(Vec2::new(10.0, 180.0))
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

fn ammo(insert: On<Insert, Grounded>, mut ammo: Query<(&mut Ammo, &MaxAmmo)>) {
    if let Ok((mut ammo, max_ammo)) = ammo.get_mut(insert.entity) {
        ammo.0 = max_ammo.0;
    }
}

#[derive(Component, Reflect)]
#[require(WeaponAcceleration, WeaponVelocity, WeaponDamping, MaxAmmo(1))]
#[reflect(Component)]
pub struct Shotgun;

fn shotgun(
    _attack: On<Fire<Attack>>,
    player: Single<(&AimVector, &mut WeaponVelocity, &mut Ammo), (With<Player>, With<Shotgun>)>,
) {
    let (aim_vector, mut velocity, mut ammo) = player.into_inner();
    if ammo.0 == 0 {
        return;
    }
    ammo.0 -= 1;
    let dir = -aim_vector.0;
    let force = dir * 3_000.0;
    velocity.0 += force;
}
