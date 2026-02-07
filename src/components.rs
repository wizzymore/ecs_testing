#![allow(dead_code)]

use bevy_ecs::prelude::*;
use rustyray::prelude::*;

#[derive(Component)]
pub struct Camera(pub Camera2D);

impl std::ops::Deref for Camera {
    type Target = Camera2D;

    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

impl std::ops::DerefMut for Camera {
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.0
    }
}

#[derive(Component)]
pub struct ActiveCamera;

#[derive(Component)]
pub struct CameraTarget;

#[allow(dead_code)]
pub enum SpriteKind {
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
pub enum SpriteOrigin {
    TopLeft,
    Top,
    TopRight,
    Left,
    Center,
    Right,
    BottomLeft,
    #[default]
    Bottom,
    BottomRight,
    Custom(Vector2),
}

#[derive(Component)]
pub struct Sprite {
    pub kind: SpriteKind,
    pub origin: SpriteOrigin,
    pub color: Color,
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
    pub fn get_origin_vector(&self) -> Vector2 {
        match self.origin {
            SpriteOrigin::TopLeft => Vector2::new(0.0, 0.0),
            SpriteOrigin::Top => Vector2::new(0.5, 0.0),
            SpriteOrigin::TopRight => Vector2::new(1.0, 0.0),
            SpriteOrigin::Left => Vector2::new(0.0, 0.5),
            SpriteOrigin::Center => Vector2::new(0.5, 0.5),
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
#[component(storage = "SparseSet")]
pub struct OnScreen;

#[derive(Debug)]
pub enum ColliderKind {
    Rectangle(Vector2),
}

impl Default for ColliderKind {
    fn default() -> Self {
        Self::Rectangle(Vector2::new(32.0, 32.0))
    }
}

#[derive(Debug, Component, Default)]
pub struct Collider {
    pub kind: ColliderKind,
    pub offset: Vector2,
}

#[derive(Bundle, Default)]
pub struct ColliderBundle {
    pub collider: Collider,
    pub transform: Transform,
}

#[derive(Debug, Clone, Copy, Component)]
pub struct Transform {
    pub position: Vector2,
    pub rotation: f32,
    pub scale: Vector2,
}

impl Transform {
    pub fn with_position(mut self, position: Vector2) -> Self {
        self.position = position;
        self
    }
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

#[derive(Debug, Clone, Copy, Component)]
pub struct GlobalTransform {
    pub position: Vector2,
    pub rotation: f32,
    pub scale: Vector2,
}

impl GlobalTransform {
    pub fn from_local(parent: &GlobalTransform, local: &Transform) -> Self {
        Self {
            position: parent.position + local.position,
            rotation: (parent.rotation + local.rotation).rem_euclid(360.0),
            scale: parent.scale * local.scale,
        }
    }

    pub fn from_root(local: &Transform) -> Self {
        Self {
            position: local.position,
            rotation: local.rotation.rem_euclid(360.0),
            scale: local.scale,
        }
    }
}

impl Default for GlobalTransform {
    fn default() -> Self {
        Self {
            position: Vector2 { x: 0.0, y: 0.0 },
            rotation: 0.0,
            scale: Vector2 { x: 1.0, y: 1.0 },
        }
    }
}

#[derive(Component)]
pub struct Text {
    pub content: String,
    pub font_size: u32,
    pub color: Color,
}

#[derive(Component)]
pub struct CountText;
#[derive(Component)]
pub struct OnScreenText;

#[derive(Component)]
pub struct Player;

#[derive(Component, Default)]
pub struct Layer(pub u32);

#[derive(Bundle, Default)]
pub struct SpriteBundle {
    pub sprite: Sprite,
    pub transform: Transform,
    pub layer: Layer,
}

#[derive(Debug, Component, Default, Clone, Copy)]
pub struct Velocity(pub Vector2);

impl std::ops::Deref for Velocity {
    type Target = Vector2;

    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

impl std::ops::DerefMut for Velocity {
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.0
    }
}
