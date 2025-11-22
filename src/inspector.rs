//! # Inspector features
//!
//! ## Level Geometry
//! - `drag`: moves the transform under the cursor.
//! - `<shift>drag`: vertical scale.
//! - `<cr>drag`: horizontal scale.
//! - `<alt>click`: create a new wall.
//!
//! ## Selection
//! - `click`: selects an entity.
//! - `<cr>v`: clones the selected entity under the cursor.
//!
//! ## Terminal
//! - `/mk ident`: makes a new level with `ident`.
//! - `/ld ident`: loads the level with `ident`.
//! - `/cp ident`: copies the current state into a new level with `ident`.
//! - `/door ident`: creates a new [`Door`] and [`DestructableKey`] leading to `ident`.
//! - `/setdoor ident`: assigns the level's [`Door`] to `ident`.
//! - `/destroy`: crates a new [`MustDestroy`] [`Key`].
//! - `/keep`: crates a new [`MustKeep`] [`Key`].
//! - `/door ident`: creates a new [`Door`] and [`DestructableKey`] leading to `ident`.
//! - `/ammo usize`: set the [`MaxAmmo`] of the current weapon.

use crate::{
    level::{
        self, Door, Key, KeyOf, KillBox, Level, LevelGeometry, MustDestroy, MustKeep, Wall,
        rectangle,
    },
    player::Player,
    weapon::{self, Ammo, MaxAmmo, SelectedWeapon, Weapon},
};
use avian2d::prelude::RigidBody;
use bevy::{
    log::{
        BoxedLayer,
        tracing::{self, Subscriber},
        tracing_subscriber::Layer,
    },
    prelude::*,
    window::PrimaryWindow,
};
use bevy_enhanced_input::prelude::ContextActivity;
use bevy_simple_text_input::{
    TextInput, TextInputInactive, TextInputPlugin, TextInputSubmitMessage, TextInputSystem,
    TextInputTextFont, TextInputValue,
};
use std::{
    collections::VecDeque,
    sync::{Arc, Mutex},
};

pub fn plugin(app: &mut App) {
    app.add_plugins((
        bevy_egui::EguiPlugin::default(),
        bevy_inspector_egui::quick::WorldInspectorPlugin::default()
            .run_if(|inspector: Option<Single<&Inspector>>| inspector.is_some()),
        term_plugin,
        debug_information_plugin,
    ))
    .add_systems(Startup, spawn_selection)
    .add_systems(
        Update,
        (
            disable_input.after(toggle_term),
            enter_exit_inspector,
            place_wall,
            select_weapon,
            paste_selection,
        ),
    )
    .register_required_components::<Player, Pickable>()
    .register_required_components::<Player, Selectable>()
    .register_required_components::<Player, DontCopy>()
    .register_required_components::<Door, Pickable>()
    .register_required_components::<Door, Selectable>()
    .register_required_components::<Door, DontCopy>()
    .register_required_components::<Wall, Pickable>()
    .register_required_components::<Wall, Selectable>()
    .register_required_components::<KillBox, Pickable>()
    .register_required_components::<KillBox, Selectable>()
    .register_required_components::<Key, Pickable>()
    .register_required_components::<Key, Selectable>()
    .add_observer(drag_transform)
    .add_observer(delete_selectable)
    .add_observer(horizontal_expand_selectable)
    .add_observer(vertical_expand_selectable)
    .add_observer(make_selection);
}

#[derive(Component)]
pub struct DisableInput;

fn disable_input(
    mut commands: Commands,
    disable_input: Query<&DisableInput>,
    ctx: Single<(Entity, &ContextActivity<Player>)>,
) {
    let (player, ctx) = ctx.into_inner();
    if **ctx && !disable_input.is_empty() {
        commands
            .entity(player)
            .insert(ContextActivity::<Player>::INACTIVE);
    } else if !**ctx && disable_input.is_empty() {
        commands
            .entity(player)
            .insert(ContextActivity::<Player>::ACTIVE);
    }
}

#[derive(Component)]
struct Inspector;

fn enter_exit_inspector(
    mut commands: Commands,
    input: Res<ButtonInput<KeyCode>>,
    mut enabled: Local<bool>,
    inspector: Query<Entity, With<Inspector>>,
    term: Single<&TextInputInactive>,
) {
    if input.just_pressed(KeyCode::KeyI) && term.0 {
        if !*enabled {
            commands.spawn((Inspector, DisableInput));
        } else {
            for entity in inspector.iter() {
                commands.entity(entity).despawn();
            }
        }
        *enabled = !*enabled;
    }
}

// LEVEL EDITOR

#[derive(Default, Component)]
struct DontCopy;

#[derive(Default, Component)]
struct Selectable;

fn drag_transform(
    pick: On<Pointer<Drag>>,
    mut transforms: Query<&mut Transform, With<Selectable>>,
    input: Res<ButtonInput<KeyCode>>,
    _enable: Single<&Inspector>,
) {
    if input.get_pressed().next().is_some() {
        return;
    }

    if let Ok(mut transform) = transforms.get_mut(pick.entity) {
        let delta = pick.delta;
        transform.translation.x += delta.x;
        transform.translation.y -= delta.y;
    }
}

#[derive(Component)]
struct Selection(Entity);

fn spawn_selection(mut commands: Commands) {
    commands.spawn(Selection(Entity::PLACEHOLDER));
}

fn make_selection(
    press: On<Pointer<Press>>,
    mut selection: Single<&mut Selection, Without<DontCopy>>,
    selectable: Query<(), With<Selectable>>,
) {
    if selectable.get(press.entity).is_ok() {
        selection.0 = press.entity;
    }
}

fn paste_selection(
    mut commands: Commands,
    key_input: Res<ButtonInput<KeyCode>>,
    window: Single<&Window, With<PrimaryWindow>>,
    camera: Single<(&Camera, &GlobalTransform)>,
    selection: Single<&Selection>,
    transforms: Query<&Transform>,
    _enable: Single<&Inspector>,
) -> Result {
    if !key_input.pressed(KeyCode::ControlLeft) || !key_input.just_pressed(KeyCode::KeyV) {
        return Ok(());
    }

    let (camera, camera_transform) = camera.into_inner();
    if let Some(world_position) = window
        .cursor_position()
        .and_then(|cursor| camera.viewport_to_world_2d(camera_transform, cursor).ok())
        && let Ok(mut entity) = commands.get_entity(selection.0)
    {
        let mut transform = *transforms.get(selection.0)?;
        transform.translation.x = world_position.x;
        transform.translation.y = world_position.y;
        entity.clone_and_spawn().insert(transform);
    }
    Ok(())
}

fn place_wall(
    mut commands: Commands,
    mouse_input: Res<ButtonInput<MouseButton>>,
    key_input: Res<ButtonInput<KeyCode>>,
    window: Single<&Window, With<PrimaryWindow>>,
    camera: Single<(&Camera, &GlobalTransform)>,
    level_geometry: Single<Entity, With<LevelGeometry>>,
    _enable: Single<&Inspector>,
) {
    if !mouse_input.just_pressed(MouseButton::Left) || !key_input.pressed(KeyCode::AltLeft) {
        return;
    }

    let (camera, camera_transform) = camera.into_inner();
    if let Some(world_position) = window
        .cursor_position()
        .and_then(|cursor| camera.viewport_to_world_2d(camera_transform, cursor).ok())
    {
        let width = 200.0;
        let height = 25.0;
        commands.spawn((
            ChildOf(*level_geometry),
            RigidBody::Static,
            Transform::from_translation(world_position.extend(0.0)),
            rectangle(width, height),
            Name::new("Inspector Wall"),
            Wall,
        ));
    }
}

fn delete_selectable(
    pick: On<Pointer<Press>>,
    mut commands: Commands,
    walls: Query<(), With<Selectable>>,
    _enable: Single<&Inspector>,
) {
    if pick.button != PointerButton::Secondary {
        return;
    }
    if walls.get(pick.entity).is_ok() {
        commands.entity(pick.entity).despawn();
    }
}

fn horizontal_expand_selectable(
    pick: On<Pointer<Drag>>,
    mut transforms: Query<&mut Transform, With<Selectable>>,
    input: Res<ButtonInput<KeyCode>>,
    _enable: Single<&Inspector>,
) {
    if !input.pressed(KeyCode::ControlLeft) {
        return;
    }

    if let Ok(mut transform) = transforms.get_mut(pick.entity) {
        let delta = pick.delta;
        transform.scale.x += delta.x * 0.01;
    }
}

fn vertical_expand_selectable(
    pick: On<Pointer<Drag>>,
    mut transforms: Query<&mut Transform, With<Selectable>>,
    input: Res<ButtonInput<KeyCode>>,
    _enable: Single<&Inspector>,
) {
    if !input.pressed(KeyCode::ShiftLeft) {
        return;
    }

    if let Ok(mut transform) = transforms.get_mut(pick.entity) {
        let delta = pick.delta;
        transform.scale.y += delta.y * 0.1;
    }
}

// SELECT WEAPONS

fn select_weapon(
    mut commands: Commands,
    input: Res<ButtonInput<KeyCode>>,
    player: Single<Entity, With<Player>>,
    term: Single<&TextInputInactive>,
) {
    if !term.0 {
        return;
    }

    for input in input.get_just_pressed() {
        match input {
            KeyCode::Digit1 => {
                commands
                    .entity(*player)
                    .despawn_children()
                    .with_child(weapon::Shotgun);
            }
            KeyCode::Digit2 => {
                commands
                    .entity(*player)
                    .despawn_children()
                    .with_child(weapon::AssaultRifle);
            }
            KeyCode::Digit3 => {
                commands
                    .entity(*player)
                    .despawn_children()
                    .with_child(weapon::GravityGun);
            }
            _ => {}
        }
    }
}

// TERMINAL

pub fn term_layer(app: &mut App) -> Option<BoxedLayer> {
    let logs = Logs::default();
    app.insert_resource(logs.clone());
    Some(logs.boxed())
}

fn term_plugin(app: &mut App) {
    app.add_plugins(TextInputPlugin)
        .add_systems(Startup, spawn_term)
        .add_systems(
            Update,
            (
                toggle_term.after(TextInputSystem),
                parse_commands.after(TextInputSystem),
                auto_scroll_on_new_items,
                log_tracing,
            ),
        )
        .add_observer(background_node_click);
}

fn parse_commands(
    mut commands: Commands,
    mut events: MessageReader<TextInputSubmitMessage>,
    mut level: ResMut<Level>,
    mut selected_weapon: Option<Single<(&mut MaxAmmo, &mut Ammo), With<SelectedWeapon>>>,
    mut door: Option<Single<(Entity, &mut Door)>>,
) {
    for event in events.read() {
        if let Some(level_ident) = event.value.strip_prefix("/mk ") {
            info!("creating {level_ident}");
            level.0 = level_ident.to_string();
            commands.run_system_cached(level::despawn_level);
            commands.run_system_cached(level::new_level);
            commands.run_system_cached(level::serialize_level);
        } else if let Some(level_ident) = event.value.strip_prefix("/ld ") {
            info!("loading {level_ident}");
            level.0 = level_ident.to_string();
            commands.run_system_cached(level::reset_level);
        } else if let Some(level_ident) = event.value.strip_prefix("/cp ") {
            info!("saving current state to {level_ident}");
            level.0 = level_ident.to_string();
            commands.run_system_cached(level::serialize_level);
            commands.run_system_cached(level::reset_level);
        } else if let Some(level_ident) = event.value.strip_prefix("/door ") {
            info!("creating key and door to {level_ident}");
            commands.spawn(Door(level_ident.to_string()));
        } else if let Some(level_ident) = event.value.strip_prefix("/setdoor ") {
            if let Some(door) = &mut door {
                info!("setting door to {level_ident}");
                door.1.0 = level_ident.to_string();
            } else {
                error!("there is not door to set {level_ident} to");
            }
        } else if event.value == "/keep" {
            if let Some(door) = &door {
                info!("creating keep lock");
                commands.spawn((Key, MustKeep, KeyOf(door.0)));
            } else {
                error!("there is not door to make a lock for");
            }
        } else if event.value == "/destroy" {
            if let Some(door) = &door {
                info!("creating destroy lock");
                commands.spawn((Key, MustDestroy, KeyOf(door.0)));
            } else {
                error!("there is not door to make a lock for");
            }
        } else if let Some(value) = event.value.strip_prefix("/ammo ") {
            if let Some(selected_weapon) = selected_weapon.as_mut() {
                let Ok(amount) = value.parse::<usize>() else {
                    error!("{value} is not a usize");
                    return;
                };
                info!("setting max ammo to {value}");
                selected_weapon.0.0 = amount;
                selected_weapon.1.0 = amount;
            }
        } else {
            error!("[Usage] /[mk|ld|cp] lvl-ident");
        }
    }
}

#[derive(Component)]
pub struct Term;

fn toggle_term(
    mut commands: Commands,
    term: Single<(Entity, &mut Node), With<Term>>,
    text_input: Single<(&mut TextInputValue, &mut TextInputInactive), With<TermStdIn>>,
    input: Res<ButtonInput<KeyCode>>,
) {
    let (mut text_value, mut input_inactive) = text_input.into_inner();
    let slash = input.just_pressed(KeyCode::Slash);
    if !input.just_pressed(KeyCode::Escape) && (!slash || !input_inactive.0) {
        return;
    }
    let (entity, mut term) = term.into_inner();
    term.display = match term.display {
        Display::Flex => {
            commands.entity(entity).remove::<DisableInput>();
            input_inactive.0 = true;
            text_value.0.clear();
            Display::None
        }
        _ => {
            commands.entity(entity).insert(DisableInput);
            input_inactive.0 = false;
            if slash {
                text_value.0.push('/');
            }
            Display::Flex
        }
    };
}

#[derive(Component)]
struct TermStdIn;

#[derive(Component)]
struct TermStdOut;

const FONT_SIZE: f32 = 17.;

fn spawn_term(mut commands: Commands) {
    commands.spawn((
        Term,
        Pickable::default(),
        Node {
            display: Display::None,
            flex_direction: FlexDirection::Column,
            justify_content: JustifyContent::Center,
            align_items: AlignItems::Center,
            width: percent(50),
            height: percent(50),
            left: percent(50),
            ..default()
        },
        children![
            (
                TermStdOut,
                BackgroundColor(Color::srgba(0.30, 0.30, 0.30, 0.75)),
                Node {
                    flex_direction: FlexDirection::Column,
                    align_self: AlignSelf::Stretch,
                    width: percent(100),
                    height: percent(100),
                    overflow: Overflow::scroll_y(),
                    ..default()
                },
            ),
            (
                TermStdIn,
                BackgroundColor(Color::srgba(0.40, 0.40, 0.40, 0.9)),
                Pickable::default(),
                Node {
                    width: percent(100),
                    ..default()
                },
                TextInput,
                TextInputTextFont(TextFont::from_font_size(FONT_SIZE)),
                TextInputInactive(true),
            ),
        ],
    ));
}

fn auto_scroll_on_new_items(
    mut scroll_position: Single<&mut ScrollPosition, With<TermStdOut>>,
    _stdout_changed: Single<&Children, (With<TermStdOut>, Changed<Children>)>,
) {
    // TODO: This resets position even if user scrolls up to look at something
    scroll_position.y = f32::MAX;
}

fn background_node_click(
    mut trigger: On<Pointer<Click>>,
    term: Single<Entity, With<Term>>,
    input: Single<Entity, With<TermStdIn>>,
    mut input_inactive: Single<&mut TextInputInactive, With<TermStdIn>>,
) {
    trigger.propagate(false);
    input_inactive.0 = trigger.entity != *term && trigger.entity != *input;
}

fn log_tracing(mut commands: Commands, logs: Res<Logs>, term: Single<Entity, With<TermStdOut>>) {
    let mut logs = logs.0.lock().unwrap();
    for log in logs.drain(..) {
        commands.spawn((
            ChildOf(*term),
            Node {
                width: percent(100),
                max_width: percent(100),
                ..default()
            },
            Text::from(log),
            TextFont::from_font_size(FONT_SIZE),
        ));
    }
}

#[derive(Clone, Resource)]
struct Logs(Arc<Mutex<VecDeque<String>>>);

impl Default for Logs {
    fn default() -> Self {
        Self(Arc::new(Mutex::new(VecDeque::default())))
    }
}

impl<S: Subscriber> Layer<S> for Logs {
    fn on_event(
        &self,
        event: &tracing::Event<'_>,
        _ctx: bevy::log::tracing_subscriber::layer::Context<'_, S>,
    ) {
        let mut logs = self.0.lock().unwrap();
        let mut message = None;
        event.record(&mut CaptureLayerVisitor(&mut message));
        if let Some(message) = message {
            let metadata = event.metadata();
            let msg = format!("[{}] {}", metadata.level(), message);
            logs.push_back(msg);
        }
    }
}

/// A [`Visit`](tracing::field::Visit)or that records log messages that are transferred to [`CaptureLayer`].
struct CaptureLayerVisitor<'a>(&'a mut Option<String>);
impl tracing::field::Visit for CaptureLayerVisitor<'_> {
    fn record_debug(&mut self, field: &tracing::field::Field, value: &dyn std::fmt::Debug) {
        // This if statement filters out unneeded events sometimes show up
        if field.name() == "message" {
            *self.0 = Some(format!("{value:?}"));
        }
    }
}

// DEBUG INFORMATION

fn debug_information_plugin(app: &mut App) {
    app.add_systems(Startup, spawn_debug_information)
        .add_systems(Update, (level_ident, weapon_ammo, weapons));
}

fn spawn_debug_information(mut commands: Commands) {
    commands.spawn((
        Node {
            position_type: PositionType::Absolute,
            flex_direction: FlexDirection::Column,
            bottom: percent(5),
            left: percent(0),
            ..default()
        },
        children![
            (
                LevelIdent,
                Text::default(),
                TextFont::from_font_size(FONT_SIZE),
            ),
            (
                WeaponAmmo(0, 0),
                Text::default(),
                TextFont::from_font_size(FONT_SIZE),
            ),
            (
                Weapons,
                Text::default(),
                TextFont::from_font_size(FONT_SIZE),
            )
        ],
    ));
}

#[derive(Component)]
struct LevelIdent;

fn level_ident(mut ident: Single<&mut Text, With<LevelIdent>>, level: Res<Level>) {
    if level.is_changed() {
        ident.0 = format!("Level: {}", level.0);
    }
}

#[derive(Component)]
struct WeaponAmmo(usize, usize);

fn weapon_ammo(
    ammo_text: Single<(&mut Text, &mut WeaponAmmo)>,
    selected_weapon: Single<(&MaxAmmo, &Ammo), Or<(Changed<MaxAmmo>, Changed<Ammo>)>>,
) {
    let (mut text, mut current) = ammo_text.into_inner();
    let (max_ammo, ammo) = selected_weapon.into_inner();
    if current.0 != max_ammo.0 || current.1 != ammo.0 {
        current.0 = max_ammo.0;
        current.1 = ammo.0;
        text.0 = format!("Ammo: {}/{}", current.1, current.0);
    }
}

#[derive(Component)]
struct Weapons;

fn weapons(
    mut weapons: Single<&mut Text, With<Weapons>>,
    player: Single<&Children, (Changed<Children>, With<Player>)>,
    player_weapons: Query<&Name, With<Weapon>>,
) {
    weapons.0 = format!(
        "Weapons: {}",
        player_weapons
            .iter_many(*player)
            .map(|weapon| weapon.as_str())
            .collect::<Vec<&str>>()
            .join(", ")
    );
}
