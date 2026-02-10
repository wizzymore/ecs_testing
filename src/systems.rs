use bevy_ecs::prelude::*;
use rustyray::prelude::*;

use crate::components::*;
use crate::resources::*;
use crate::spatial_hash::SpatialHash;
#[cfg(feature = "trace")]
use tracing::info_span;

// Iterative system for propagating GlobalTransforms
#[allow(clippy::type_complexity)]
pub fn update_global_transforms_system(
    mut parents: Query<(&mut GlobalTransform, &Transform, Option<&Children>), Without<ChildOf>>,
    mut children: Query<(&mut GlobalTransform, &Transform, Option<&Children>), With<ChildOf>>,
) {
    let mut stack: Vec<(GlobalTransform, Entity)> = Vec::new();

    for (mut global, local, maybe_children) in parents.iter_mut() {
        *global = GlobalTransform::from_root(local);

        if let Some(children) = maybe_children {
            for &child in children {
                stack.push((*global, child));
            }
        }
    }

    while let Some((parent_gt, entity)) = stack.pop() {
        let Ok((mut global, local, maybe_children)) = children.get_mut(entity) else {
            continue;
        };

        *global = GlobalTransform::from_local(&parent_gt, local);

        if let Some(children) = maybe_children {
            for &child in children {
                stack.push((*global, child));
            }
        }
    }
}

pub fn move_player_system(
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
pub fn apply_velocity_system(
    mut movers_q: Query<(
        &mut Transform,
        &GlobalTransform,
        &Velocity,
        Option<&Collider>,
    )>,
    static_colliders: Query<(&Collider, &GlobalTransform), Without<Velocity>>,
    spatial_hash: Res<SpatialHash>,
    mut metrics: ResMut<Metrics>,
) {
    let start = std::time::Instant::now();

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
        // Precompute all static colliders
        let static_rects = spatial_hash
            .query(*player_rect)
            .iter()
            .filter_map(|&e| {
                if let Ok((collider, collider_gt)) = static_colliders.get(e) {
                    return match collider.kind {
                        ColliderKind::Rectangle(size) => Some(CollisionShape::Rect(Rectangle {
                            x: collider_gt.position.x - collider.offset.x,
                            y: collider_gt.position.y - collider.offset.y,
                            width: size.x * collider_gt.scale.x,
                            height: size.y * collider_gt.scale.y,
                        })),
                    };
                }

                None
            })
            .collect::<Vec<_>>();

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

        // Update the actual position based on resolved rectangle (world space)
        let delta = player_rect.position() - original_position;
        transform.position.x += delta.x;
        transform.position.y += delta.y;
    }

    // let duration = start.elapsed();
    // println!("Collision detection took: {duration:?}");
    metrics.apply_velocity_system_time = start.elapsed();
}

pub fn move_camera_to_target_system(
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

pub fn update_count_text_system(
    mut text: Query<&mut Text, With<CountText>>,
    entities: Query<&Sprite, Without<Player>>,
) {
    let count = entities.iter().count();
    for mut t in text.iter_mut() {
        t.content = format!("Count: {count}"); // Replace 42 with the actual count
    }
}

pub fn update_on_screen_text_system(
    mut text: Query<&mut Text, With<OnScreenText>>,
    entities: Query<&Sprite, With<OnScreen>>,
    camera: Single<&Camera, With<ActiveCamera>>,
) {
    let count = entities.iter().count();
    for mut t in text.iter_mut() {
        t.content = format!("On Screen: {count} {:.2}", camera.0.zoom);
    }
}

pub fn update_on_screen_system(
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

pub fn check_for_resize_system(
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

pub fn debug_toggle_system(
    mut debug_settings: ResMut<DebugSettings>,
    mut window: ResMut<WindowResource>,
) {
    if window.is_key_pressed(KeyboardKey::O) {
        debug_settings.origins = !debug_settings.origins;
    }
    if window.is_key_pressed(KeyboardKey::C) {
        debug_settings.colliders = !debug_settings.colliders;
    }
    if window.is_key_pressed(KeyboardKey::F) {
        window.set_target_fps(50000);
    }
}

pub fn update_render_textures_size_system(
    mut ev_resize: MessageReader<ResizeEvent>,
    mut render_textures: ResMut<LayerTextures>,
) {
    for ev in ev_resize.read() {
        for (_, rt) in render_textures.0.iter_mut() {
            *rt = OwnedRenderTexture::new(ev.to.x, ev.to.y).unwrap();
        }
    }
}

pub fn update_camera_offset(
    mut ev_resize: MessageReader<ResizeEvent>,
    mut camera: Single<&mut Camera>,
) {
    for ev in ev_resize.read() {
        camera.offset = Vector2::new(ev.to.x as f32 / 2.0, ev.to.y as f32 / 2.0);
    }
}

pub fn ensure_global_transform_system(
    q: Query<(Entity, &Transform), Without<GlobalTransform>>,
    mut commands: Commands,
) {
    for (e, t) in q.iter() {
        commands
            .entity(e)
            .insert_if_new(GlobalTransform::from_root(t));
    }
}

#[allow(clippy::type_complexity)]
pub fn sync_collider_with_sprite_system(
    mut q: Query<
        (&mut Collider, &Sprite, &GlobalTransform),
        (
            With<SyncColliderWithSprite>,
            Or<(Changed<Sprite>, Changed<GlobalTransform>)>,
        ),
    >,
) {
    for (mut collider, sprite, global_transform) in q.iter_mut() {
        let origin = sprite.get_origin_vector();
        collider.offset = match collider.kind {
            ColliderKind::Rectangle(size) => Vector2::new(
                (size.x * global_transform.scale.x) * origin.x,
                (size.y * global_transform.scale.y) * origin.y,
            ),
        }
    }
}

pub fn update_spatial_hash_system(
    mut spatial_hash: ResMut<SpatialHash>,
    query: Query<(Entity, &Sprite, &GlobalTransform), Changed<GlobalTransform>>,
) {
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
}
