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
    Middle,
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
pub struct OnScreen;

#[derive(Clone, Component)]
pub struct Transform {
    pub position: Vector2,
    pub rotation: f32,
    pub scale: Vector2,
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
pub struct Text {
    pub content: String,
    pub font_size: u32,
    pub color: Color,
}

#[derive(Component)]
pub struct CountText;

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

#[derive(Component, Default)]
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
