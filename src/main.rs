use std::collections::HashMap;

use bevy_ecs::prelude::*;
use components::*;
use rayon::prelude::*;
use resources::*;
use rustyray::prelude::*;
use spatial_hash::SpatialHash;

mod components;
mod resources;
mod spatial_hash;

fn init_world(world: &mut World) {
    world.insert_resource(spatial_hash::SpatialHash::new(96.0));
    world.insert_resource(Events::<ResizeEvent>::default());
    world.insert_resource(DebugSettings {
        origins: false,
        colliders: true,
    });
    world.init_resource::<LayerTextures>();

    world.insert_resource(WindowSize(Vector2i { x: 1024, y: 768 }));
    world.insert_resource(WindowResource(
        WindowBuilder::new(1024, 768, "RustyRay")
            .set_fps(1000)
            .set_config_flags(ConfigFlag::WindowResizable)
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

fn main() {
    let mut world = World::default();
    init_world(&mut world);

    let mut update_schedule = bevy_ecs::schedule::Schedule::default();
    let mut first_physics_update_schedule = bevy_ecs::schedule::Schedule::default();
    let mut pre_physics_update_schedule = bevy_ecs::schedule::Schedule::default();
    let mut physics_update_schedule = bevy_ecs::schedule::Schedule::default();
    let mut post_physics_update_schedule = bevy_ecs::schedule::Schedule::default();
    let mut last_physics_update_schedule = bevy_ecs::schedule::Schedule::default();
    last_physics_update_schedule
        .set_executor_kind(bevy_ecs::schedule::ExecutorKind::SingleThreaded);
    let mut render_schedule = bevy_ecs::schedule::Schedule::default();
    render_schedule.set_executor_kind(bevy_ecs::schedule::ExecutorKind::SingleThreaded);
    render_schedule.add_systems((
        update_camera_offset,
        update_render_textures_size_system,
        render_layers,
        render_system,
    ));
    first_physics_update_schedule.add_systems(ensure_global_transform_system);
    pre_physics_update_schedule.add_systems((
        update_global_transforms_system,
        sync_collider_with_sprite_system,
    ));
    physics_update_schedule.add_systems(move_player_system);
    post_physics_update_schedule.add_systems(apply_velocity_system);
    last_physics_update_schedule.add_systems((
        update_global_transforms_system,
        update_spatial_hash_system,
        update_on_screen_system.after(update_spatial_hash_system),
    ));
    update_schedule.add_systems((
        move_camera_to_target_system,
        update_count_text_system,
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
                origin: SpriteOrigin::BottomRight,
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

    const TO_SPAWN: usize = 500_000 / 5;

    (0..5).for_each(|i| {
        (0..TO_SPAWN).for_each(|j| {
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

    let once = cfg!(feature = "once");
    let mut time = Time::new(60.0);
    let mut window = world.resource::<WindowResource>();
    loop {
        let frame_time = window.frame_time();
        time.accumulator += frame_time;
        while time.accumulator >= time.delta {
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
        update_schedule.run(&mut world);
        render_schedule.run(&mut world);

        window = world.resource::<WindowResource>();
        if window.should_close() || once {
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
        .map(|(collider, collider_gt)| {
            let r = match collider.kind {
                ColliderKind::Rectangle(size) => CollisionShape::Rect(Rectangle {
                    x: collider_gt.position.x - collider.offset.x,
                    y: collider_gt.position.y - collider.offset.y,
                    width: size.x * collider_gt.scale.x,
                    height: size.y * collider_gt.scale.y,
                }),
            };

            r
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

    let mouse_scroll = window.get_mouse_wheel_move() / 10.0;
    if mouse_scroll != 0.0 {
        camera.zoom = (camera.zoom + mouse_scroll).clamp(0.0, 5.0);
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

fn update_on_screen_system(
    spatial_hash: Res<SpatialHash>,
    window: Res<WindowResource>,
    camera: Single<&Camera, With<ActiveCamera>>,
    on_screen_q: Query<Entity, With<OnScreen>>,
    mut commands: Commands,
) {
    let extra_offset = 1000.0f32;
    let screen_size = window.screen_size().to_vector2();
    let (min_x, min_y) = (
        camera.target.x - camera.offset.x,
        camera.target.y - camera.offset.y,
    );

    let on_screen_entities = spatial_hash.query(Rectangle {
        x: min_x - extra_offset,
        y: min_y - extra_offset,
        width: screen_size.x + extra_offset * 2.0,
        height: screen_size.y + extra_offset * 2.0,
    });

    on_screen_q.iter().for_each(|entity| {
        commands.entity(entity).remove::<OnScreen>();
    });
    on_screen_entities.iter().for_each(|entity| {
        commands.entity(*entity).insert(OnScreen);
    });
}

fn check_for_resize_system(
    window: Res<WindowResource>,
    mut current_size: ResMut<WindowSize>,
    mut ev_resize: EventWriter<ResizeEvent>,
) {
    if window.screen_size() != current_size.0 {
        let new_size = window.screen_size();
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
}

fn update_render_textures_size_system(
    mut ev_resize: EventReader<ResizeEvent>,
    mut render_textures: ResMut<LayerTextures>,
) {
    for ev in ev_resize.read() {
        println!(
            "Updating render textures size from {}x{} to {}x{}",
            ev.from.x, ev.from.y, ev.to.x, ev.to.y
        );
        for (_, rt) in render_textures.0.iter_mut() {
            *rt = OwnedRenderTexture::new(ev.to.x, ev.to.y).unwrap();
        }
    }
}

fn update_camera_offset(mut ev_resize: EventReader<ResizeEvent>, mut camera: Single<&mut Camera>) {
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

#[allow(clippy::type_complexity)]
fn update_spatial_hash_system(
    mut spatial_hash: ResMut<SpatialHash>,
    query: Query<(Entity, &Sprite, &Transform), Changed<Transform>>,
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

fn render_system(
    mut window: ResMut<WindowResource>,
    layer_rt: Res<LayerTextures>,
    text: Query<(&Text, &GlobalTransform)>,
) {
    let screen_size = window.screen_size();
    window.draw(|d| {
        d.clear(Color::CORNFLOWERBLUE);

        // Draw GAME entities and other stuff on the layers
        for (_, rt) in layer_rt.0.iter() {
            d.draw_render_texture(rt);
        }

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
        d.draw_text("Hello, RustyRay!", 20, 20, 32, Color::WHITE);
        for (text, transform) in text.iter() {
            d.draw_text(
                &text.content,
                transform.position.x as i32,
                transform.position.y as i32,
                text.font_size as i32,
                text.color,
            );
        }
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
    let mut sprites_map = sprites
        .par_iter()
        .fold(
            HashMap::<u32, Vec<(&Sprite, &GlobalTransform)>>::new,
            |mut map, (sprite, transform, layer)| {
                map.entry(layer.0).or_default().push((*sprite, *transform));
                map
            },
        )
        .reduce(HashMap::new, |mut acc, item| {
            for (k, mut v) in item {
                acc.entry(k).or_default().append(&mut v);
            }
            acc
        });

    // parallel sort per-layer if needed
    sprites_map.par_iter_mut().for_each(|(_, sprites)| {
        sprites.par_sort_unstable_by(|a, b| (a.1.position.y).partial_cmp(&b.1.position.y).unwrap());
    });

    for (layer, sprites) in sprites_map.iter() {
        let render_texture = layer_rt
            .0
            .entry(*layer)
            .or_insert_with(|| OwnedRenderTexture::new(1024, 768).unwrap());
        window.draw_texture_mode(render_texture, |mut d| {
            d.clear(Color::BLANK);
            {
                let d = d.begin_mode_2d(&camera);
                for (sprite, transform) in sprites.iter() {
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
                        d.draw_circle(transform.position, 2.0, Color::BLUE);
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
            }
        });
    }
}
