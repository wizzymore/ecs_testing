use std::collections::HashMap;

use bevy_ecs::prelude::*;
use rustyray::prelude::*;

#[derive(Resource)]
pub struct WindowResource(pub Window);

impl std::ops::Deref for WindowResource {
    type Target = Window;

    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

impl std::ops::DerefMut for WindowResource {
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.0
    }
}

impl Drop for WindowResource {
    fn drop(&mut self) {
        println!("WindowResource dropped");
    }
}

#[derive(Resource, Default)]
pub struct LayerTextures(pub HashMap<u32, OwnedRenderTexture>);

impl Drop for LayerTextures {
    fn drop(&mut self) {
        println!("LayerTextures dropped");
    }
}

#[derive(Resource)]
pub struct WindowSize(pub Vector2i);

#[derive(Event)]
pub struct ResizeEvent {
    pub from: Vector2i,
    pub to: Vector2i,
}

#[derive(Resource, Clone, Copy)]
pub struct Time {
    pub delta: f32,
    pub accumulator: f32,
}

impl Time {
    pub fn new(framerate: f32) -> Self {
        let timestep = 1.0 / framerate;
        Self {
            delta: timestep,
            accumulator: timestep,
        }
    }

    pub fn delta(&self) -> f32 {
        self.delta
    }
}

#[derive(Event)]
pub struct UpdateSpatialHash(pub Entity);
