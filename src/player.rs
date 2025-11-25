use crate::level::{DebugPickingColor, Layer, Serialize, Wall};
use avian2d::prelude::*;
use bevy::{
    color::palettes::css::ORANGE, input::mouse::MouseMotion, prelude::*, window::PrimaryWindow,
};
use bevy_enhanced_input::{prelude::Cancel, prelude::Press, prelude::*};
use bevy_tween::prelude::EaseKind;

pub fn plugin(app: &mut App) {
    app.add_input_context::<Player>()
        .add_systems(
            FixedPostUpdate,
            (grounded, apply_movement)
                .chain()
                .in_set(PhysicsSystems::Last),
        )
        .add_systems(Update, aim_with_mouse_input)
        .add_observer(inject_bindings)
        .add_observer(handle_movement)
        .add_observer(stop_movement)
        .add_observer(start_jump)
        .add_observer(handle_jump)
        .add_observer(cancel_jump)
        .add_observer(end_jump)
        .add_observer(handle_aim)
        .add_observer(handle_attack);
}

/// The player marker component.
#[derive(Component, Reflect)]
#[require(
    Serialize,
    Transform,
    DebugPickingColor::new(ORANGE),
    // Avian Components
    RigidBody::Dynamic,
    LockedAxes::ROTATION_LOCKED,
    Collider = Self::collider(),
    ShapeCaster = Self::ground_caster(),
    Friction = Friction::ZERO.with_combine_rule(CoefficientCombine::Min),
    Restitution = Restitution::ZERO.with_combine_rule(CoefficientCombine::Min),
    // Bounce???
    // Restitution::PERFECTLY_ELASTIC,
    CollisionLayers::new(Layer::Player, [Layer::Default, Layer::Wall, Layer::KillBox]),
    // Input Components
    OrientationMethod,
    MoveVector,
    AimVector,
    // Physics Parameters
    InputVelocity(300.0),
    WeaponVelocity,
    WeaponVelocityDamp(10.0),
    JumpImpulse {
        impulse_range: Vec2::new(500.0, 700.0),
        duration: 0.2,
    },
)]
#[reflect(Component)]
pub struct Player;

impl Player {
    pub fn collider() -> Collider {
        Collider::rectangle(12.5 * 2.0, 20.0 * 2.0)
    }

    pub fn ground_caster() -> ShapeCaster {
        let mut shape = Self::collider();
        shape.set_scale(Vec2::splat(0.99), 10);
        ShapeCaster::new(shape, Vec2::ZERO, 0.0, Dir2::NEG_Y).with_max_distance(10.0)
    }

    pub fn ceiling_caster() -> ShapeCaster {
        let mut shape = Self::collider();
        shape.set_scale(Vec2::splat(0.99), 10);
        ShapeCaster::new(shape, Vec2::ZERO, 0.0, Dir2::Y).with_max_distance(10.0)
    }
}

#[derive(Component)]
pub struct Grounded;

fn grounded(
    mut commands: Commands,
    player: Single<(Entity, &ShapeHits, Has<Grounded>), With<Player>>,
    walls: Query<&Wall>,
) {
    let (entity, hits, has_grounded) = player.into_inner();
    let is_grounded = hits.iter().any(|data| walls.contains(data.entity));
    if is_grounded && !has_grounded {
        commands.entity(entity).insert(Grounded);
    } else if !is_grounded && has_grounded {
        commands.entity(entity).remove::<Grounded>();
    }
}

/// X-axis velocity applied to the player from input.
#[derive(Default, Component)]
pub struct InputVelocity(pub f32);

#[derive(Default, Component)]
pub struct WeaponVelocity(pub Vec2);

#[derive(Component)]
pub struct WeaponVelocityDamp(pub f32);

#[derive(Component)]
pub struct JumpImpulse {
    pub impulse_range: Vec2,
    pub duration: f32,
}

#[derive(Component, Default)]
pub enum OrientationMethod {
    #[default]
    Stick,
    Mouse,
}

fn aim_with_mouse_input(
    window: Single<&Window, With<PrimaryWindow>>,
    camera: Single<(&Camera, &GlobalTransform)>,
    player: Single<(&mut AimVector, &GlobalTransform, &mut OrientationMethod), With<Player>>,
    input_ctx: Single<&ContextActivity<Player>>,
    mut motion: MessageReader<MouseMotion>,
) {
    let (mut aim_vector, player_transform, mut orientation) = player.into_inner();
    if !***input_ctx {
        return;
    }

    if let OrientationMethod::Stick = *orientation {
        if motion.read().last().is_none() {
            *orientation = OrientationMethod::Mouse;
        } else {
            return;
        }
    }

    let (camera, camera_transform) = camera.into_inner();
    if let Some(Ok(cursor_translation)) = window
        .cursor_position()
        .map(|cursor| camera.viewport_to_world_2d(camera_transform, cursor))
    {
        let target = cursor_translation - player_transform.translation().xy();
        let normalized_translation = target.normalize_or(Vec2::X);

        if normalized_translation != Vec2::ZERO {
            aim_vector.0 = normalized_translation;
        }
    }
}

fn inject_bindings(
    trigger: On<Insert, Player>,
    mut commands: Commands,
    jump_impulse: Query<&JumpImpulse>,
) -> Result {
    let jump_impulse = jump_impulse.get(trigger.entity)?;
    commands.entity(trigger.entity).insert(actions!(Player[
        (
            Action::<Move>::new(),
            DeadZone::default(),
            Bindings::spawn((
                Cardinal::wasd_keys(),
                Axial::left_stick(),
            )),
        ),
        (
            Action::<Aim>::new(),
            DeadZone {
                lower_threshold: 0.5,
                ..Default::default()
            },
            SmoothNudge::new(16.0),
            Bindings::spawn((
                Cardinal::arrows(),
                Axial::right_stick(),
            )),
        ),
        (
            Action::<Jump>::new(),
            Hold::new(jump_impulse.duration),
            bindings![KeyCode::Space, KeyCode::ShiftLeft, GamepadButton::South],
        ),
        (
            Action::<Attack>::new(),
            Press::default(),
            bindings![MouseButton::Left, GamepadButton::RightTrigger2],
        ),
        (
            Action::<PickUp>::new(),
            Press::default(),
            bindings![KeyCode::KeyF, KeyCode::Enter, GamepadButton::North],
        ),
    ]));
    Ok(())
}

#[derive(InputAction)]
#[action_output(Vec2)]
pub struct Move;

#[derive(Default, Component)]
pub struct MoveVector(pub Vec2);

fn handle_movement(movement: On<Fire<Move>>, mut player: Single<&mut MoveVector, With<Player>>) {
    player.0 = movement.value;
}

fn stop_movement(_movement: On<Complete<Move>>, mut player: Single<&mut MoveVector, With<Player>>) {
    player.0 = Vec2::ZERO;
}

fn apply_movement(
    time: Res<Time>,
    player: Single<
        (
            &mut LinearVelocity,
            &mut WeaponVelocity,
            &InputVelocity,
            &WeaponVelocityDamp,
            &MoveVector,
        ),
        With<Player>,
    >,
) {
    let dt = time.delta_secs();
    let (mut velocity, mut weapon_velocity, input_velocity, damping, move_vector) =
        player.into_inner();

    weapon_velocity.0 *= 1.0 / (1.0 + damping.0 * dt);
    let input_movement = input_velocity.0 * move_vector.0.x;
    if weapon_velocity.0.x.abs() < input_velocity.0 && move_vector.0.x != 0.0 {
        velocity.x = input_movement;
    } else {
        velocity.x = weapon_velocity.0.x;
    }
    if weapon_velocity.0.y.abs() > 200.0 {
        velocity.y = weapon_velocity.0.y;
    }
}

#[derive(InputAction)]
#[action_output(bool)]
pub struct Jump;

#[derive(Component)]
struct Jumping(f32);

fn start_jump(
    _jump: On<Start<Jump>>,
    mut commands: Commands,
    player: Single<Entity, (With<Player>, With<Grounded>)>,
) {
    commands.entity(*player).insert(Jumping(0.0));
}

fn handle_jump(
    _jump: On<Ongoing<Jump>>,
    player: Single<(&mut LinearVelocity, &JumpImpulse, &Jumping), (With<Player>, With<Jumping>)>,
    gravity: Res<Gravity>,
) {
    let (mut velocity, jump_impulse, duration) = player.into_inner();
    let t = EaseKind::CubicInOut.sample(duration.0 / jump_impulse.duration);
    let range = jump_impulse.impulse_range * gravity.0.signum().y * -1.0;
    velocity.0.y = range.x.lerp(range.y, t);
}

fn cancel_jump(
    _jump: On<Cancel<Jump>>,
    mut commands: Commands,
    player: Single<Entity, (With<Player>, With<Jumping>)>,
) {
    commands.entity(*player).remove::<Jumping>();
}

fn end_jump(
    _jump: On<Fire<Jump>>,
    mut commands: Commands,
    player: Single<Entity, (With<Player>, With<Jumping>)>,
) {
    commands.entity(*player).remove::<Jumping>();
}

#[derive(InputAction)]
#[action_output(Vec2)]
pub struct Aim;

#[derive(Default, Component)]
pub struct AimVector(pub Vec2);

fn handle_aim(
    aim: On<Fire<Aim>>,
    player: Single<(&mut AimVector, &mut OrientationMethod), With<Player>>,
) {
    let (mut aim_vector, mut method) = player.into_inner();
    *method = OrientationMethod::Stick;

    let angle = aim.value.normalize_or_zero();
    if angle.length_squared() != 0.0 {
        aim_vector.0 = angle;
    }
}

#[derive(InputAction)]
#[action_output(bool)]
pub struct Attack;

fn handle_attack(
    _attack: On<Fire<Attack>>,
    mut commands: Commands,
    player: Single<Entity, With<Player>>,
) {
    commands.entity(*player).remove::<Jumping>();
}

#[derive(InputAction)]
#[action_output(bool)]
pub struct PickUp;
