use crate::{Wall, player::Player};
use bevy::{input::common_conditions::input_toggle_active, prelude::*};
use bevy_enhanced_input::prelude::ContextActivity;

pub fn plugin(app: &mut App) {
    app.add_plugins((
        bevy_egui::EguiPlugin::default(),
        bevy_inspector_egui::quick::WorldInspectorPlugin::default()
            .run_if(input_toggle_active(false, KeyCode::KeyI)),
    ))
    .add_systems(Update, enter_exit_inspector)
    .register_required_components::<Player, Pickable>()
    .register_required_components::<Wall, Pickable>()
    .add_observer(drag_transform);
}

#[derive(Component)]
struct Inspector;

fn enter_exit_inspector(
    mut commands: Commands,
    input: Res<ButtonInput<KeyCode>>,
    ctx: Single<(Entity, &ContextActivity<Player>)>,
    mut enabled: Local<bool>,
    inspector: Query<Entity, With<Inspector>>,
) {
    if input.just_pressed(KeyCode::KeyI) {
        let (entity, context) = ctx.into_inner();
        commands.entity(entity).insert(context.toggled());
        if !*enabled {
            commands.spawn(Inspector);
        } else {
            for entity in inspector.iter() {
                commands.entity(entity).despawn();
            }
        }
        *enabled = !*enabled;
    }
}

fn drag_transform(
    pick: On<Pointer<Drag>>,
    mut transforms: Query<&mut Transform, With<Pickable>>,
    _enable: Single<&Inspector>,
) {
    if let Ok(mut transform) = transforms.get_mut(pick.entity) {
        let delta = pick.delta;
        transform.translation.x += delta.x;
        transform.translation.y -= delta.y;
    }
}
