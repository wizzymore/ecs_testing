use std::collections::HashMap;

use bevy_ecs::{
    bundle::Bundle,
    component::Component,
    entity::Entity,
    event::{Event, EventReader, EventWriter, Events},
    query::{With, Without},
    resource::Resource,
    schedule::IntoScheduleConfigs,
    system::{Commands, Query, Res, ResMut, Single},
    world::World,
};
use rayon::prelude::*;
use rustyray::prelude::{
    Camera2D, Color, ConfigFlag, OwnedRenderTexture, OwnedTexture, Rectangle, Vector2, Vector2i,
    Window, WindowBuilder,
};

use crate::spatial_hash::SpatialHash;
mod spatial_hash;

#[derive(Component)]
struct Camera(Camera2D);

#[derive(Component)]
struct ActiveCamera;

#[derive(Component)]
struct CameraTarget;

#[derive(Resource)]
struct WindowResource(Window);

impl Drop for LayerTextures {
    fn drop(&mut self) {
        println!("LayerTextures dropped");
    }
}

impl Drop for WindowResource {
    fn drop(&mut self) {
        println!("WindowResource dropped");
    }
}

#[derive(Resource, Default)]
struct LayerTextures(HashMap<u32, OwnedRenderTexture>);

#[allow(dead_code)]
enum SpriteKind {
    Rectangle { size: (f32, f32), lines: bool },
    Circle { radius: f32 },
    Texture { texture: OwnedTexture },
}

impl Default for SpriteKind {
    fn default() -> Self {
        Self::Rectangle {
            size: (32.0, 32.0),
            lines: false,
        }
    }
}

#[derive(Default)]
#[allow(dead_code)]
enum SpriteOrigin {
    TopLeft,
    Top,
    TopRight,
    Left,
    Middle,
    Right,
    BottomLeft,
    #[default]
    Bottom,
    BottomRight,
    Custom(Vector2),
}

#[derive(Component)]
struct Sprite {
    kind: SpriteKind,
    origin: SpriteOrigin,
    color: Color,
}

impl Default for Sprite {
    fn default() -> Self {
        Self {
            kind: SpriteKind::default(),
            origin: SpriteOrigin::default(),
            color: Color::GREEN,
        }
    }
}

impl Sprite {
    fn get_origin_vector(&self) -> Vector2 {
        match self.origin {
            SpriteOrigin::TopLeft => Vector2::new(0.0, 0.0),
            SpriteOrigin::Top => Vector2::new(0.5, 0.0),
            SpriteOrigin::TopRight => Vector2::new(1.0, 0.0),
            SpriteOrigin::Left => Vector2::new(0.0, 0.5),
            SpriteOrigin::Middle => Vector2::new(0.5, 0.5),
            SpriteOrigin::Right => Vector2::new(1.0, 0.5),
            SpriteOrigin::BottomLeft => Vector2::new(0.0, 1.0),
            SpriteOrigin::Bottom => Vector2::new(0.5, 1.0),
            SpriteOrigin::BottomRight => Vector2::new(1.0, 1.0),
            SpriteOrigin::Custom(vec) => {
                debug_assert!(vec.x >= 0.0 && vec.x <= 1.0);
                debug_assert!(vec.y >= 0.0 && vec.y <= 1.0);
                vec
            }
        }
    }
}

#[derive(Component)]
struct OnScreen;

#[derive(Clone, Component)]
struct Transform {
    position: Vector2,
    rotation: f32,
    scale: Vector2,
}

impl Default for Transform {
    fn default() -> Self {
        Self {
            position: Vector2 { x: 0.0, y: 0.0 },
            rotation: 0.0,
            scale: Vector2 { x: 1.0, y: 1.0 },
        }
    }
}

#[derive(Component)]
struct Text {
    content: String,
    font_size: u32,
    color: Color,
}

#[derive(Component)]
struct CountText;

#[derive(Component)]
struct Player;

#[derive(Component, Default)]
struct Layer(u32);

#[derive(Bundle, Default)]
struct SpriteBundle {
    sprite: Sprite,
    transform: Transform,
    layer: Layer,
}

#[derive(Resource)]
struct WindowSize(Vector2i);

#[derive(Event)]
struct ResizeEvent {
    from: Vector2i,
    to: Vector2i,
}

#[derive(Resource, Clone, Copy)]
struct Time {
    delta: f32,
    accumulator: f32,
}

#[derive(Component, Default)]
struct Velocity(f32, f32);

#[derive(Event)]
struct UpdateSpatialHash(Entity);

impl Time {
    pub fn new(framerate: f32) -> Self {
        let timestep = 1.0 / framerate;
        Self {
            delta: timestep,
            accumulator: 0.0,
        }
    }

    pub fn delta(&self) -> f32 {
        self.delta
    }
}

fn init_world(world: &mut World) {
    world.insert_resource(spatial_hash::SpatialHash::new(96.0));
    world.insert_resource(Events::<ResizeEvent>::default());
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

fn main() {
    let mut world = World::default();
    init_world(&mut world);

    let mut update_schedule = bevy_ecs::schedule::Schedule::default();
    let mut physics_update_schedule = bevy_ecs::schedule::Schedule::default();
    let mut render_schedule = bevy_ecs::schedule::Schedule::default();
    render_schedule.set_executor_kind(bevy_ecs::schedule::ExecutorKind::SingleThreaded);
    render_schedule.add_systems((
        update_camera_offset,
        update_render_textures_size_system,
        render_system,
    ));
    physics_update_schedule.add_systems((move_player_system, apply_velocity_system).chain());
    update_schedule.add_systems((
        move_camera_to_target_system,
        update_count_text_system,
        check_for_resize_system,
        // detect_new_entities_system,
        update_spatial_hash_system,
        update_on_screen_system.after(update_spatial_hash_system),
    ));

    world.init_resource::<Events<UpdateSpatialHash>>();
    let mut spawn_events = world
        .remove_resource::<Events<UpdateSpatialHash>>()
        .unwrap();

    {
        let player = world.spawn((
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
        ));

        spawn_events.send(UpdateSpatialHash(player.id()));
    }

    {
        let platform = world.spawn((
            SpriteBundle {
                transform: Transform {
                    position: Vector2 { x: 0., y: 100. },
                    scale: Vector2 { x: 40.0, y: 1.0 },
                    ..Default::default()
                },
                ..Default::default()
            },
            Velocity(2.0, -2.0),
            OnScreen,
        ));
        spawn_events.send(UpdateSpatialHash(platform.id()));
    }

    const TO_SPAWN: usize = 20_000 / 5;

    (0..5).for_each(|i| {
        (0..TO_SPAWN).for_each(|j| {
            let entity = world.spawn(SpriteBundle {
                sprite: Sprite {
                    origin: SpriteOrigin::Custom((0.5, 0.75).into()),
                    ..Default::default()
                },
                transform: Transform {
                    position: Vector2 {
                        x: 200. + (35 * j) as f32,
                        y: 500. + (35 * i) as f32,
                    },
                    ..Default::default()
                },
                ..Default::default()
            });

            spawn_events.send(UpdateSpatialHash(entity.id()));
        });
    });

    world.insert_resource(spawn_events);

    world.spawn((
        Transform::default(),
        Text {
            content: String::new(),
            font_size: 24,
            color: Color::WHITE,
        },
        CountText,
    ));

    let mut time = Time::new(60.0);
    let mut window = world.resource::<WindowResource>();
    while !window.0.should_close() {
        let frame_time = window.0.frame_time();
        time.accumulator += frame_time;
        while time.accumulator >= time.delta {
            world.insert_resource(time);
            physics_update_schedule.run(&mut world);
            time.accumulator -= time.delta
        }
        world.insert_resource(Time {
            delta: frame_time,
            accumulator: 0.0,
        });
        update_schedule.run(&mut world);
        render_schedule.run(&mut world);

        window = world.resource::<WindowResource>();
    }

    cleanup_world(&mut world);
}

fn move_player_system(
    window: Res<WindowResource>,
    time: Res<Time>,
    mut velocity: Single<&mut Velocity, With<Player>>,
) {
    let move_left = window.0.is_key_down(rustyray::prelude::KeyboardKey::A);
    let move_right = window.0.is_key_down(rustyray::prelude::KeyboardKey::D);
    let move_up = window.0.is_key_down(rustyray::prelude::KeyboardKey::W);
    let move_down = window.0.is_key_down(rustyray::prelude::KeyboardKey::S);

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
    velocity.0 = m.x;
    velocity.1 = m.y;
}

#[allow(clippy::type_complexity)]
fn apply_velocity_system(
    mut update_spatial_hash: EventWriter<UpdateSpatialHash>,
    mut movers: Query<(Entity, &Sprite, &mut Transform, &Velocity)>,
    mut others: Query<(&mut Sprite, &Transform), (With<OnScreen>, Without<Velocity>)>,
) {
    // Record how much time this took
    let start = std::time::Instant::now();

    // Precompute all movers rects
    let mut moving_rects: Vec<_> = movers
        .iter_mut()
        .filter_map(|(m_entity, s, t, v)| {
            if let SpriteKind::Rectangle { size, .. } = s.kind {
                let mut r = Rectangle {
                    x: t.position.x,
                    y: t.position.y,
                    width: size.0 * t.scale.x,
                    height: size.1 * t.scale.y,
                };
                let o = s.get_origin_vector();
                r.x -= r.width * o.x;
                r.y -= r.height * o.y;
                Some((m_entity, r, s, t, v))
            } else {
                None
            }
        })
        .collect();

    let static_rects: Vec<_> = others
        .iter_mut()
        .filter_map(|(s, t)| {
            if let SpriteKind::Rectangle { size, .. } = s.kind {
                let mut r = Rectangle {
                    x: t.position.x,
                    y: t.position.y,
                    width: size.0 * t.scale.x,
                    height: size.1 * t.scale.y,
                };
                let o = s.get_origin_vector();
                r.x -= r.width * o.x;
                r.y -= r.height * o.y;
                Some(r)
            } else {
                None
            }
        })
        .collect();

    for i in 0..moving_rects.len() {
        let (left, right) = moving_rects.split_at_mut(i);
        let ((p_entity, player_rect, sprite, transform, velocity), rest) =
            right.split_first_mut().unwrap();
        let mut did_move = false;
        let origin = sprite.get_origin_vector();

        if velocity.0 != 0.0 || velocity.1 != 0.0 {
            did_move = true;
            player_rect.x += velocity.0;
            for static_rect in static_rects.iter() {
                if player_rect.collides_rect(static_rect) {
                    if velocity.0 > 0.0 {
                        player_rect.x = static_rect.x - player_rect.width; // stop right before left wall
                    } else if velocity.0 < 0.0 {
                        player_rect.x = static_rect.x + static_rect.width; // stop right before right wall
                    }
                }
            }

            for (_, moving_rect, ..) in left.iter().chain(rest.iter()) {
                if player_rect.collides_rect(moving_rect) {
                    if velocity.0 > 0.0 {
                        player_rect.x = moving_rect.x - player_rect.width; // stop right before left wall
                    } else if velocity.0 < 0.0 {
                        player_rect.x = moving_rect.x + moving_rect.width; // stop right before right wall
                    }
                }
            }

            player_rect.y += velocity.1;
            for static_rect in static_rects.iter() {
                if player_rect.collides_rect(static_rect) {
                    if velocity.1 > 0.0 {
                        player_rect.y = static_rect.y - player_rect.height; // stop above floor
                    } else if velocity.1 < 0.0 {
                        player_rect.y = static_rect.y + static_rect.height; // stop below ceiling
                    }
                }
            }

            for (_, moving_rect, ..) in left.iter().chain(rest.iter()) {
                if player_rect.collides_rect(moving_rect) {
                    if velocity.1 > 0.0 {
                        player_rect.y = moving_rect.y - player_rect.height; // stop above floor
                    } else if velocity.1 < 0.0 {
                        player_rect.y = moving_rect.y + moving_rect.height; // stop below ceiling
                    }
                }
            }
        } else {
            // No velocity position check
            for static_rect in static_rects.iter() {
                if player_rect.collides_rect(static_rect) {
                    // Compute overlap along X and Y
                    let delta_x = (player_rect.x + player_rect.width / 2.0)
                        - (static_rect.x + static_rect.width / 2.0);
                    let delta_y = (player_rect.y + player_rect.height / 2.0)
                        - (static_rect.y + static_rect.height / 2.0);
                    let intersect_x = (player_rect.width + static_rect.width) / 2.0 - delta_x.abs();
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

            for (_, moving_rect, _, _, mover_velocity) in left.iter().chain(rest.iter()) {
                // If the other entity has velocity, we will handle the collision then
                if mover_velocity.0 == 0.0
                    && mover_velocity.1 == 0.0
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
                    did_move = true;
                }
            }
        }

        if did_move {
            update_spatial_hash.write(UpdateSpatialHash(*p_entity));
        }

        // Update the actual position based on resolved rectangle
        transform.position.x = player_rect.x + player_rect.width * origin.x;
        transform.position.y = player_rect.y + player_rect.height * origin.y;
    }

    let duration = start.elapsed();
    println!("Collision detection took: {duration:?}");
}

fn move_camera_to_target_system(
    mut camera: Single<&mut Camera, With<ActiveCamera>>,
    target: Single<&Transform, With<CameraTarget>>,
) {
    camera.0.target = target.position;
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
    let screen_size = window.0.get_screen_size().to_vector2();
    let (min_x, min_y) = (
        camera.0.target.x - camera.0.offset.x,
        camera.0.target.y - camera.0.offset.y,
    );

    let on_screen_entities = spatial_hash.query(Rectangle {
        x: min_x,
        y: min_y,
        width: screen_size.x,
        height: screen_size.y,
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
    if window.0.get_screen_size() != current_size.0 {
        let new_size = window.0.get_screen_size();
        println!("Window resized to {}x{}", new_size.x, new_size.y);
        ev_resize.write(ResizeEvent {
            from: current_size.0,
            to: new_size,
        });
        current_size.0 = new_size;
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
        camera.0.offset = Vector2::new(ev.to.x as f32 / 2.0, ev.to.y as f32 / 2.0);
    }
}

#[allow(clippy::type_complexity)]
fn update_spatial_hash_system(
    mut spatial_hash: ResMut<SpatialHash>,
    mut event_r: EventReader<UpdateSpatialHash>,
    query: Query<(Entity, &Sprite, &Transform)>,
) {
    for ev in event_r.read() {
        if let Ok((entity, sprite, transform)) = query.get(ev.0) {
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
}

fn render_system(
    window: Res<WindowResource>,
    mut layer_rt: ResMut<LayerTextures>,
    sprite_q: Query<(&Sprite, &Transform, &Layer), With<OnScreen>>,
    camera: Query<(&Camera, &ActiveCamera)>,
    text: Query<(&Text, &Transform)>,
) {
    render_layers(&window, &mut layer_rt, sprite_q.iter().collect(), &camera);

    window
        .0
        .draw(|d| {
            d.clear(Color::CORNFLOWERBLUE);
            let screen_size = window.0.get_screen_size();

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
        })
        .expect("Failed to draw the frame");
}

fn render_layers(
    window: &Res<'_, WindowResource>,
    layer_rt: &mut ResMut<LayerTextures>,
    sprite_q: Vec<(&Sprite, &Transform, &Layer)>,
    camera: &Query<(&Camera, &ActiveCamera)>,
) {
    let camera = camera.single().unwrap().0;
    let sprites: Vec<_> = sprite_q.iter().collect();
    // This created a map for each thread with all the sprites in that thread
    // then we merge all the small maps into a big one with all the sprites
    let mut sprites_map = sprites
        .par_iter()
        .fold(
            HashMap::<u32, Vec<(&Sprite, &Transform)>>::new,
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
        window.0.draw_render_texture(render_texture, |d| {
            d.clear(Color::BLANK);
            let _ch = d.begin_mode_2d(camera.0);
            for (sprite, transform) in sprites.iter() {
                match &sprite.kind {
                    SpriteKind::Rectangle { size, lines } => {
                        let origin = sprite.get_origin_vector();
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
                        d.draw_circle(transform.position, 2.0, Color::BLUE);
                    }
                    SpriteKind::Circle { radius } => {
                        d.draw_circle(
                            Vector2 {
                                x: transform.position.x,
                                y: transform.position.y,
                            },
                            radius * transform.scale.x,
                            sprite.color,
                        );
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
                            Vector2 { x: 0.0, y: 0.0 },
                            transform.rotation,
                            sprite.color,
                        );
                    }
                }
            }
        });
    }
}
