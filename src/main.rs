use std::collections::HashMap;

use bevy_ecs::{prelude::*, schedule::ScheduleLabel};
use components::*;
use rayon::prelude::*;
use resources::*;
use rustyray::prelude::*;
use systems::*;
#[cfg(feature = "trace")]
use tracing::{info, info_span};
#[cfg(feature = "trace")]
use tracing_subscriber::prelude::*;

mod components;
mod resources;
mod spatial_hash;
mod systems;

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
                position: Vector2 { x: 50.0, y: 1000.0 },
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
                "Apply Velocity System: {:.2}ms",
                metrics.apply_velocity_system_time.as_secs_f32() * 1000.0,
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
