use std::collections::HashMap;

use bevy_ecs::{prelude::*, schedule::ScheduleLabel};
use components::*;
use rayon::prelude::*;
use resources::*;
use rustyray::prelude::*;
use spatial_hash::SpatialHash;
#[cfg(feature = "trace")]
use tracing::{info, info_span};
#[cfg(feature = "trace")]
use tracing_subscriber::prelude::*;

mod components;
mod resources;
mod spatial_hash;

fn init_world(world: &mut World) {
    world.insert_resource(spatial_hash::SpatialHash::new(96.0));
    world.insert_resource(Messages::<ResizeEvent>::default());
    world.insert_resource(DebugSettings {
        origins: false,
        colliders: false,
    });
    world.init_resource::<LayerTextures>();

    world.insert_resource(WindowSize(Vector2i { x: 1024, y: 768 }));
    world.insert_resource(WindowResource(
        WindowBuilder::new(1024, 768, "RustyRay")
            .set_fps(60)
            .set_config_flags(
                ConfigFlag::WindowHighdpi | ConfigFlag::WindowResizable | ConfigFlag::VsyncHint,
            )
            .build()
            .unwrap(),
    ));
    world.spawn((
        Camera(Camera2D {
            offset: Vector2 {
                x: 1024. / 2.,
                y: 768. / 2.,
            },
            ..Default::default()
        }),
        ActiveCamera,
    ));
}

fn cleanup_world(world: &mut World) {
    // Make sure we remove this now, because we can't be sure when the WindowResource is removed and that will close out the window so this will fail
    world.remove_resource::<LayerTextures>();
}

#[derive(Component)]
struct SyncColliderWithSprite;

#[derive(ScheduleLabel, Hash, PartialEq, Eq, Debug, Clone)]
struct Update;
#[derive(ScheduleLabel, Hash, PartialEq, Eq, Debug, Clone)]
struct FirstPhysicsUpdate;
#[derive(ScheduleLabel, Hash, PartialEq, Eq, Debug, Clone)]
struct PrePhysicsUpdate;
#[derive(ScheduleLabel, Hash, PartialEq, Eq, Debug, Clone)]
struct PhysicsUpdate;
#[derive(ScheduleLabel, Hash, PartialEq, Eq, Debug, Clone)]
struct PostPhysicsUpdate;
#[derive(ScheduleLabel, Hash, PartialEq, Eq, Debug, Clone)]
struct LastPhysicsUpdate;
#[derive(ScheduleLabel, Hash, PartialEq, Eq, Debug, Clone)]
struct PreRender;
#[derive(ScheduleLabel, Hash, PartialEq, Eq, Debug, Clone)]
struct Render;

fn main() {
    #[cfg(feature = "trace")]
    tracing_subscriber::registry()
        .with(tracing_tracy::TracyLayer::default())
        .init();

    let mut world = World::default();
    init_world(&mut world);

    let mut update_schedule = bevy_ecs::schedule::Schedule::new(Update);
    let mut first_physics_update_schedule = bevy_ecs::schedule::Schedule::new(FirstPhysicsUpdate);
    let mut pre_physics_update_schedule = bevy_ecs::schedule::Schedule::new(PrePhysicsUpdate);
    let mut physics_update_schedule = bevy_ecs::schedule::Schedule::new(PhysicsUpdate);
    let mut post_physics_update_schedule = bevy_ecs::schedule::Schedule::new(PostPhysicsUpdate);
    let mut last_physics_update_schedule = bevy_ecs::schedule::Schedule::new(LastPhysicsUpdate);
    let mut pre_render_schedule = bevy_ecs::schedule::Schedule::new(PreRender);
    let mut render_schedule = bevy_ecs::schedule::Schedule::new(Render);
    render_schedule.set_executor_kind(bevy_ecs::schedule::ExecutorKind::SingleThreaded);
    // pre_render_schedule.set_executor_kind(bevy_ecs::schedule::ExecutorKind::SingleThreaded);
    render_schedule.add_systems((
        update_render_textures_size_system,
        render_layers,
        render_system,
    ));
    pre_render_schedule.add_systems(update_camera_offset);
    first_physics_update_schedule.add_systems(ensure_global_transform_system);
    pre_physics_update_schedule.add_systems(
        (
            update_global_transforms_system,
            sync_collider_with_sprite_system,
        )
            .chain(),
    );
    physics_update_schedule.add_systems(move_player_system);
    post_physics_update_schedule.add_systems(apply_velocity_system);
    last_physics_update_schedule.add_systems(
        (
            update_global_transforms_system,
            (update_spatial_hash_system, update_on_screen_system).chain(),
        )
            .chain(),
    );
    update_schedule.add_systems((
        move_camera_to_target_system,
        update_count_text_system,
        update_on_screen_text_system,
        check_for_resize_system,
        debug_toggle_system,
    ));

    world.spawn((
        SpriteBundle {
            sprite: Sprite {
                kind: SpriteKind::Circle { radius: 40.0 },
                color: Color::RED,
                origin: SpriteOrigin::Bottom,
            },
            transform: Transform {
                position: Vector2 { x: 50.0, y: 50.0 },
                ..Default::default()
            },
            ..Default::default()
        },
        Velocity::default(),
        OnScreen,
        Collider::default(),
        SyncColliderWithSprite,
    ));

    world.spawn((
        SpriteBundle {
            sprite: Sprite {
                kind: SpriteKind::Rectangle {
                    size: (32.0, 32.0),
                    lines: false,
                },
                color: Color::RED,
                origin: SpriteOrigin::Custom(Vector2::new(0.5, 0.75)),
            },
            transform: Transform {
                position: Vector2 { x: 50.0, y: 50.0 },
                ..Default::default()
            },
            ..Default::default()
        },
        Velocity::default(),
        Player,
        OnScreen,
        CameraTarget,
        Collider::default(),
        SyncColliderWithSprite,
    ));

    // world.spawn((
    //     SpriteBundle {
    //         transform: Transform {
    //             position: Vector2 { x: 100., y: 100. },
    //             scale: Vector2 { x: 40.0, y: 1.0 },
    //             ..Default::default()
    //         },
    //         ..Default::default()
    //     },
    //     Velocity(Vector2::new(2.0, -2.0)),
    //     OnScreen,
    //     Collider::default(),
    //     SyncColliderWithSprite,
    // ));

    const TO_SPAWN: usize = 100_000 / 100;

    (0..TO_SPAWN).for_each(|i| {
        (0..100).for_each(|j| {
            world.spawn((
                SpriteBundle {
                    sprite: Sprite {
                        origin: SpriteOrigin::Custom((0.0, 0.0).into()),
                        ..Default::default()
                    },
                    transform: Transform {
                        position: Vector2 {
                            x: 200. + (35 * j) as f32,
                            y: 100. + (35 * i) as f32,
                        },
                        ..Default::default()
                    },
                    ..Default::default()
                },
                Collider::default(),
                SyncColliderWithSprite,
            ));
        });
    });

    world.spawn((
        Transform::default(),
        Text {
            content: String::new(),
            font_size: 24,
            color: Color::WHITE,
        },
        CountText,
    ));
    world.spawn((
        Transform::default().with_position(Vector2::new(0.0, 20.0)),
        Text {
            content: String::new(),
            font_size: 24,
            color: Color::WHITE,
        },
        OnScreenText,
    ));

    let mut time = Time::new(60.0);
    world.insert_resource(Metrics::default());
    let mut window = world.resource::<WindowResource>();
    loop {
        let frame_time = window.frame_time();
        time.accumulator += frame_time;
        while time.accumulator >= time.delta {
            #[cfg(feature = "trace")]
            let _span = info_span!("physics loop").entered();
            world.insert_resource(time);
            first_physics_update_schedule.run(&mut world);
            pre_physics_update_schedule.run(&mut world);
            physics_update_schedule.run(&mut world);
            post_physics_update_schedule.run(&mut world);
            last_physics_update_schedule.run(&mut world);
            time.accumulator -= time.delta
        }
        world.insert_resource(Time {
            delta: frame_time,
            accumulator: 0.0,
        });
        {
            #[cfg(feature = "trace")]
            let _span = info_span!("update").entered();
            update_schedule.run(&mut world);
        }
        {
            #[cfg(feature = "trace")]
            let _span = info_span!("render").entered();
            pre_render_schedule.run(&mut world);
            render_schedule.run(&mut world);
        }

        window = world.resource::<WindowResource>();

        #[cfg(feature = "once")]
        break;

        if window.should_close() {
            break;
        }
    }

    cleanup_world(&mut world);
}

fn move_player_system(
    window: Res<WindowResource>,
    time: Res<Time>,
    mut velocity: Single<&mut Velocity, With<Player>>,
) {
    let move_left = window.is_key_down(KeyboardKey::A);
    let move_right = window.is_key_down(KeyboardKey::D);
    let move_up = window.is_key_down(KeyboardKey::W);
    let move_down = window.is_key_down(KeyboardKey::S);

    const SPEED: f32 = 300.0;
    const RUN_SPEED: f32 = SPEED * 3.0;
    let mut dir = Vector2::ZERO;
    if move_left {
        dir.x -= 1.0;
    }
    if move_right {
        dir.x += 1.0;
    }
    if move_up {
        dir.y -= 1.0;
    }
    if move_down {
        dir.y += 1.0;
    }

    let m = if window
        .0
        .is_key_down(rustyray::prelude::KeyboardKey::LeftShift)
    {
        RUN_SPEED
    } else {
        SPEED
    } * dir.normalized()
        * time.delta();
    velocity.0 = m;
}

enum CollisionShape {
    Rect(Rectangle),
}

#[allow(clippy::type_complexity)]
fn apply_velocity_system(
    mut movers_q: Query<(
        &mut Transform,
        &GlobalTransform,
        &Velocity,
        Option<&Collider>,
    )>,
    static_colliders: Query<(&Collider, &GlobalTransform), (With<OnScreen>, Without<Velocity>)>,
) {
    // Record how much time this took
    // let start = std::time::Instant::now();

    // Precompute all static colliders
    let static_rects = static_colliders
        .iter()
        .map(|(collider, collider_gt)| match collider.kind {
            ColliderKind::Rectangle(size) => CollisionShape::Rect(Rectangle {
                x: collider_gt.position.x - collider.offset.x,
                y: collider_gt.position.y - collider.offset.y,
                width: size.x * collider_gt.scale.x,
                height: size.y * collider_gt.scale.y,
            }),
        })
        .collect::<Vec<_>>();

    let mut moving_rects = movers_q
        .iter_mut()
        .filter_map(|(mut t, gt, v, collider)| {
            let Some(collider) = collider else {
                t.position += v.0;
                return None;
            };

            let r = match collider.kind {
                ColliderKind::Rectangle(size) => Rectangle {
                    x: gt.position.x - collider.offset.x,
                    y: gt.position.y - collider.offset.y,
                    width: size.x * gt.scale.x,
                    height: size.y * gt.scale.y,
                },
            };

            Some((r, t, v))
        })
        .collect::<Vec<_>>();

    for i in 0..moving_rects.len() {
        let (left, right) = moving_rects.split_at_mut(i);
        let ((player_rect, transform, velocity), rest) = right.split_first_mut().unwrap();
        let original_position = player_rect.position();

        if velocity.x != 0.0 || velocity.y != 0.0 {
            player_rect.x += velocity.x;
            for static_rect in static_rects.iter() {
                match static_rect {
                    CollisionShape::Rect(static_rect) => {
                        if player_rect.collides_rect(static_rect) {
                            if velocity.x > 0.0 {
                                player_rect.x = static_rect.x - player_rect.width; // stop right before left wall
                            } else if velocity.x < 0.0 {
                                player_rect.x = static_rect.x + static_rect.width; // stop right before right wall
                            }
                        }
                    }
                }
            }

            for (moving_rect, ..) in left.iter().chain(rest.iter()) {
                if player_rect.collides_rect(moving_rect) {
                    if velocity.x > 0.0 {
                        player_rect.x = moving_rect.x - player_rect.width; // stop right before left wall
                    } else if velocity.x < 0.0 {
                        player_rect.x = moving_rect.x + moving_rect.width; // stop right before right wall
                    }
                }
            }

            player_rect.y += velocity.y;
            for static_rect in static_rects.iter() {
                match static_rect {
                    CollisionShape::Rect(static_rect) => {
                        if player_rect.collides_rect(static_rect) {
                            if velocity.y > 0.0 {
                                player_rect.y = static_rect.y - player_rect.height; // stop above floor
                            } else if velocity.y < 0.0 {
                                player_rect.y = static_rect.y + static_rect.height; // stop below ceiling
                            }
                        }
                    }
                }
            }

            for (moving_rect, ..) in left.iter().chain(rest.iter()) {
                if player_rect.collides_rect(moving_rect) {
                    if velocity.y > 0.0 {
                        player_rect.y = moving_rect.y - player_rect.height; // stop above floor
                    } else if velocity.y < 0.0 {
                        player_rect.y = moving_rect.y + moving_rect.height; // stop below ceiling
                    }
                }
            }
        } else {
            // No velocity position check
            for static_rect in static_rects.iter() {
                match static_rect {
                    CollisionShape::Rect(static_rect) => {
                        if player_rect.collides_rect(static_rect) {
                            // Compute overlap along X and Y
                            let delta_x = (player_rect.x + player_rect.width / 2.0)
                                - (static_rect.x + static_rect.width / 2.0);
                            let delta_y = (player_rect.y + player_rect.height / 2.0)
                                - (static_rect.y + static_rect.height / 2.0);
                            let intersect_x =
                                (player_rect.width + static_rect.width) / 2.0 - delta_x.abs();
                            let intersect_y =
                                (player_rect.height + static_rect.height) / 2.0 - delta_y.abs();

                            // Only push along the axis of least penetration
                            if intersect_x < intersect_y {
                                // X axis
                                if delta_x > 0.0 {
                                    player_rect.x += intersect_x;
                                } else {
                                    player_rect.x -= intersect_x;
                                }
                            } else {
                                // Y axis
                                if delta_y > 0.0 {
                                    player_rect.y += intersect_y;
                                } else {
                                    player_rect.y -= intersect_y;
                                }
                            }
                        }
                    }
                }
            }

            for (moving_rect, _, mover_velocity) in left.iter().chain(rest.iter()) {
                // If the other entity has velocity, we will handle the collision then
                if mover_velocity.x == 0.0
                    && mover_velocity.y == 0.0
                    && player_rect.collides_rect(moving_rect)
                {
                    // Compute overlap along X and Y
                    let delta_x = (player_rect.x + player_rect.width / 2.0)
                        - (moving_rect.x + moving_rect.width / 2.0);
                    let delta_y = (player_rect.y + player_rect.height / 2.0)
                        - (moving_rect.y + moving_rect.height / 2.0);
                    let intersect_x = (player_rect.width + moving_rect.width) / 2.0 - delta_x.abs();
                    let intersect_y =
                        (player_rect.height + moving_rect.height) / 2.0 - delta_y.abs();

                    // Only push along the axis of least penetration
                    if intersect_x < intersect_y {
                        // X axis
                        if delta_x > 0.0 {
                            player_rect.x += intersect_x;
                        } else {
                            player_rect.x -= intersect_x;
                        }
                    } else {
                        // Y axis
                        if delta_y > 0.0 {
                            player_rect.y += intersect_y;
                        } else {
                            player_rect.y -= intersect_y;
                        }
                    }
                }
            }
        }

        // Update the actual position based on resolved rectangle
        transform.position -= original_position - player_rect.position();
    }

    // let duration = start.elapsed();
    // println!("Collision detection took: {duration:?}");
}

fn move_camera_to_target_system(
    mut camera: Single<&mut Camera, With<ActiveCamera>>,
    target: Single<&Transform, With<CameraTarget>>,
    window: Res<WindowResource>,
) {
    camera.target = target.position;

    let mouse_scroll = window.mouse_wheel_move() / 10.0;
    if mouse_scroll != 0.0 {
        camera.zoom = (camera.zoom + mouse_scroll).clamp(0.3, 5.0);
    }
}

fn update_count_text_system(
    mut text: Query<&mut Text, With<CountText>>,
    entities: Query<&Sprite, Without<Player>>,
) {
    let count = entities.iter().count();
    for mut t in text.iter_mut() {
        t.content = format!("Count: {count}"); // Replace 42 with the actual count
    }
}

fn update_on_screen_text_system(
    mut text: Query<&mut Text, With<OnScreenText>>,
    entities: Query<&Sprite, With<OnScreen>>,
    camera: Single<&Camera, With<ActiveCamera>>,
) {
    let count = entities.iter().count();
    for mut t in text.iter_mut() {
        t.content = format!("On Screen: {count} {:.2}", camera.0.zoom);
    }
}

fn update_on_screen_system(
    spatial_hash: Res<SpatialHash>,
    window: Res<WindowResource>,
    camera: Single<&Camera, With<ActiveCamera>>,
    on_screen_q: Query<Entity, With<OnScreen>>,
    mut commands: Commands,
    mut metrics: ResMut<Metrics>,
) {
    let extra_offset = 0.0f32;
    let screen_size = window.screen_size().to_vector2();
    let (min_x, min_y, max_x, max_y) = (
        camera.target.x - screen_size.x / camera.zoom,
        camera.target.y - screen_size.y / camera.zoom,
        camera.target.x + screen_size.x / camera.zoom,
        camera.target.y + screen_size.y / camera.zoom,
    );
    let start = std::time::Instant::now();
    let on_screen_entities = spatial_hash.query(Rectangle {
        x: min_x - extra_offset,
        y: min_y - extra_offset,
        width: max_x - min_x + extra_offset * 2.0,
        height: max_y - min_y + extra_offset * 2.0,
    });
    metrics.update_on_screen_system_time = start.elapsed();

    {
        #[cfg(feature = "trace")]
        let _span = info_span!("add_onscreen_component").entered();
        on_screen_entities.iter().for_each(|&entity| {
            commands.entity(entity).insert(OnScreen);
        });
    }

    // Remove OnScreen from entities that are no longer on screen
    {
        #[cfg(feature = "trace")]
        let _span = info_span!("remove_onscreen_component").entered();
        for entity in on_screen_q.iter() {
            if !on_screen_entities.contains(&entity) {
                commands.entity(entity).remove::<OnScreen>();
            }
        }
    }
}

fn check_for_resize_system(
    window: Res<WindowResource>,
    mut current_size: ResMut<WindowSize>,
    mut ev_resize: MessageWriter<ResizeEvent>,
) {
    let new_size = window.screen_size();
    if new_size != current_size.0 {
        println!("Window resized to {}x{}", new_size.x, new_size.y);
        ev_resize.write(ResizeEvent {
            from: current_size.0,
            to: new_size,
        });
        current_size.0 = new_size;
    }
}

fn debug_toggle_system(mut debug_settings: ResMut<DebugSettings>, window: Res<WindowResource>) {
    if window.is_key_pressed(KeyboardKey::O) {
        debug_settings.origins = !debug_settings.origins;
    }
    if window.is_key_pressed(KeyboardKey::C) {
        debug_settings.colliders = !debug_settings.colliders;
    }
}

fn update_render_textures_size_system(
    mut ev_resize: MessageReader<ResizeEvent>,
    mut render_textures: ResMut<LayerTextures>,
) {
    for ev in ev_resize.read() {
        for (_, rt) in render_textures.0.iter_mut() {
            *rt = OwnedRenderTexture::new(ev.to.x, ev.to.y).unwrap();
        }
    }
}

fn update_camera_offset(
    mut ev_resize: MessageReader<ResizeEvent>,
    mut camera: Single<&mut Camera>,
) {
    for ev in ev_resize.read() {
        camera.offset = Vector2::new(ev.to.x as f32 / 2.0, ev.to.y as f32 / 2.0);
    }
}

fn ensure_global_transform_system(
    q: Query<(Entity, &Transform), Without<GlobalTransform>>,
    mut commands: Commands,
) {
    for (e, t) in q.iter() {
        commands
            .entity(e)
            .insert_if_new(GlobalTransform::from_root(t));
    }
}

// Iterative system for propagating GlobalTransforms
#[allow(clippy::type_complexity)]
fn update_global_transforms_system(
    mut query: ParamSet<(
        Query<(&Transform, &mut GlobalTransform, Option<&Children>), Changed<Transform>>,
        Query<(&Transform, &mut GlobalTransform, Option<&Children>)>,
    )>,
) {
    // Stack for iterative traversal: (parent_global, child_entity)
    let mut stack = Vec::new();

    // Push roots onto stack
    for (local, mut global, children) in &mut query.p0() {
        *global = GlobalTransform::from_root(local);

        if let Some(children) = children {
            for &child in children {
                stack.push((Some(*global), child));
            }
        }
    }

    // Traverse hierarchy iteratively
    while let Some((parent_gt_opt, entity)) = stack.pop() {
        let mut p1 = query.p1();
        let Ok((local, mut gt, children)) = p1.get_mut(entity) else {
            continue;
        };

        *gt = if let Some(parent_gt) = parent_gt_opt {
            GlobalTransform::from_local(&parent_gt, local)
        } else {
            GlobalTransform::from_root(local)
        };

        if let Some(children) = children {
            for &child in children {
                stack.push((Some(*gt), child));
            }
        }
    }
}

#[allow(clippy::type_complexity)]
fn sync_collider_with_sprite_system(
    mut q: Query<
        (&mut Collider, &Sprite, &GlobalTransform),
        (
            With<SyncColliderWithSprite>,
            Or<(Changed<Sprite>, Changed<GlobalTransform>)>,
        ),
    >,
) {
    for (mut collider, sprite, transform) in q.iter_mut() {
        let origin = sprite.get_origin_vector();
        collider.offset = match collider.kind {
            ColliderKind::Rectangle(size) => size * origin * transform.scale,
        }
    }
}

fn update_spatial_hash_system(
    mut spatial_hash: ResMut<SpatialHash>,
    query: Query<(Entity, &Sprite, &Transform), Changed<Transform>>,
    mut metrics: ResMut<Metrics>,
) {
    let start = std::time::Instant::now();
    for (entity, sprite, transform) in query.iter() {
        let origin = sprite.get_origin_vector();
        let rect = match &sprite.kind {
            SpriteKind::Rectangle { size: shape, .. } => Rectangle {
                x: transform.position.x - (shape.0 * transform.scale.x) * origin.x,
                y: transform.position.y - (shape.1 * transform.scale.y) * origin.y,
                width: shape.0 * transform.scale.x,
                height: shape.1 * transform.scale.y,
            },
            SpriteKind::Circle { radius, .. } => Rectangle {
                x: transform.position.x - (radius * transform.scale.x) * origin.x,
                y: transform.position.y - (radius * transform.scale.y) * origin.y,
                width: radius * transform.scale.x,
                height: radius * transform.scale.y,
            },
            _ => continue,
        };

        spatial_hash.update(entity, rect);
    }
    metrics.spatial_hash_update_time = start.elapsed();
}

fn render_system(
    mut window: ResMut<WindowResource>,
    layer_rt: Res<LayerTextures>,
    text: Query<(&Text, &GlobalTransform)>,
    metrics: Res<Metrics>,
) {
    let screen_size = window.screen_size();
    window.draw(|d| {
        d.clear(Color::CORNFLOWERBLUE);

        // Draw GAME entities and other stuff on the layers
        let _draw_layers_span = tracing::span!(tracing::Level::DEBUG, "draw_layers").entered();
        for (_, rt) in layer_rt.0.iter() {
            let _draw_layer = tracing::span!(tracing::Level::DEBUG, "draw_layer").entered();
            d.draw_render_texture(rt);
        }
        drop(_draw_layers_span);

        // START OF UI RENDERING
        d.draw_rect(
            Rectangle {
                height: 30.0,
                width: screen_size.x as f32,
                x: 0.0,
                y: screen_size.y as f32 - 30.0,
            },
            Color::new(0, 0, 0, 255).fade(0.5),
        );
        d.draw_fps(10, screen_size.y - 25);
        let _draw_texts_span = tracing::span!(tracing::Level::DEBUG, "draw_texts").entered();
        for (text, transform) in text.iter() {
            let _draw_text_span = tracing::span!(tracing::Level::DEBUG, "draw_text").entered();
            d.draw_text(
                &text.content,
                transform.position.x as i32,
                transform.position.y as i32,
                text.font_size as i32,
                text.color,
            );
        }
        drop(_draw_texts_span);
        d.draw_text(
            &format!(
                "Spatial Hash Update Time: {:.2}ms",
                metrics.spatial_hash_update_time.as_secs_f32() * 1000.0,
            ),
            200,
            screen_size.y - 25,
            20,
            Color::WHITE,
        );
        d.draw_text(
            &format!(
                "On Screen Update Time: {:.2}ms",
                metrics.update_on_screen_system_time.as_secs_f32() * 1000.0,
            ),
            550,
            screen_size.y - 25,
            20,
            Color::WHITE,
        );
    });
}

fn render_layers(
    mut window: ResMut<WindowResource>,
    mut layer_rt: ResMut<LayerTextures>,
    debug_settings: Res<DebugSettings>,
    sprite_q: Query<(&Sprite, &GlobalTransform, &Layer), With<OnScreen>>,
    camera: Single<&Camera, With<ActiveCamera>>,
    colliders: Query<(&Collider, &GlobalTransform), With<OnScreen>>,
) {
    let sprites: Vec<_> = sprite_q.iter().collect();
    // This created a map for each thread with all the sprites in that thread
    // then we merge all the small maps into a big one with all the sprites
    let mut sprites_map: HashMap<u32, Vec<(&Sprite, &GlobalTransform)>> =
        HashMap::with_capacity(layer_rt.0.len());
    {
        let _collection_sprites_spawn =
            tracing::span!(tracing::Level::DEBUG, "sort_sprites").entered();
        for (sprite, transform, layer) in &sprites {
            sprites_map
                .entry(layer.0)
                .or_default()
                .push((*sprite, *transform));
        }
    }

    // parallel sort per-layer if needed

    {
        let _sorting_sprites_span = tracing::span!(tracing::Level::DEBUG, "sort_sprites").entered();
        sprites_map.par_iter_mut().for_each(|(_, sprites)| {
            sprites.par_sort_unstable_by(|a, b| {
                (a.1.position.y).partial_cmp(&b.1.position.y).unwrap()
            });
        });
    }

    #[cfg(feature = "trace")]
    let _span = info_span!("draw layers").entered();
    for (layer, sprites) in sprites_map.iter() {
        #[cfg(feature = "trace")]
        let _span_layer = info_span!("draw layer").entered();
        let render_texture = layer_rt
            .0
            .entry(*layer)
            .or_insert_with(|| OwnedRenderTexture::new(1024, 768).unwrap());
        window.draw_texture_mode(render_texture, |mut d| {
            #[cfg(feature = "trace")]
            let _span_in = info_span!("draw layer sprites").entered();
            d.clear(Color::BLANK);
            let d = d.begin_mode_2d(&camera);
            for &(sprite, transform) in sprites {
                let origin = sprite.get_origin_vector();
                match &sprite.kind {
                    SpriteKind::Rectangle { size, lines } => {
                        let mut dest = Rectangle {
                            x: transform.position.x,
                            y: transform.position.y,
                            width: size.0 * transform.scale.x,
                            height: size.1 * transform.scale.y,
                        };
                        if *lines {
                            dest.x -= dest.width * origin.x;
                            dest.y -= dest.height * origin.y;
                            d.draw_rect_lines(dest, sprite.color);
                        } else {
                            d.draw_rect_pro(
                                dest,
                                origin * dest.size(),
                                transform.rotation,
                                sprite.color,
                            );
                        }
                    }
                    SpriteKind::Circle { radius } => {
                        let radius = *radius * transform.scale;
                        let diameter = radius * 2.0;
                        let center = transform.position + (radius - diameter * origin);

                        match transform.scale.x == transform.scale.y {
                            true => d.draw_circle(center, radius.x, sprite.color),
                            false => d.draw_ellipse(center.to_vector2i(), radius, sprite.color),
                        }
                    }
                    SpriteKind::Texture { texture } => {
                        let size = texture.size();
                        let dest = Rectangle {
                            x: transform.position.x,
                            y: transform.position.y,
                            width: size.x as f32 * transform.scale.x,
                            height: size.y as f32 * transform.scale.y,
                        };
                        d.draw_texture_pro(
                            texture,
                            Rectangle {
                                x: 0.0,
                                y: 0.0,
                                width: size.x as f32,
                                height: size.y as f32,
                            },
                            dest,
                            origin,
                            transform.rotation,
                            sprite.color,
                        );
                    }
                }
                if debug_settings.origins {
                    d.draw_circle(transform.position, 4.0, Color::BLUE);
                }
            }

            if debug_settings.colliders {
                for (collider, transform) in colliders.iter() {
                    let pos = transform.position - collider.offset;
                    match collider.kind {
                        ColliderKind::Rectangle(size) => {
                            d.draw_rect_lines(
                                Rectangle {
                                    x: pos.x,
                                    y: pos.y,
                                    width: size.x,
                                    height: size.y,
                                },
                                Color::ORANGE,
                            );
                        }
                    }
                }
            }
        });
    }
}
