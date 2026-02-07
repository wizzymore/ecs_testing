use std::collections::{HashMap, HashSet};

use bevy_ecs::{entity::Entity, resource::Resource};
use rustyray::prelude::Rectangle;
#[cfg(feature = "trace")]
use tracing::info_span;

#[derive(Default, Resource)]
pub struct SpatialHash {
    pub cell_size: f32,
    pub cells: HashMap<(i32, i32), Vec<Entity>>,
    pub entities: HashMap<Entity, Vec<(i32, i32)>>,
}

impl SpatialHash {
    pub fn new(cell_size: f32) -> Self {
        Self {
            cell_size,
            cells: HashMap::new(),
            entities: HashMap::new(),
        }
    }

    pub fn cell_coords(&self, x: f32, y: f32) -> (i32, i32) {
        let cx = (x / self.cell_size).floor() as i32;
        let cy = (y / self.cell_size).floor() as i32;
        (cx, cy)
    }

    pub fn cell_coords_rect(&self, rect: Rectangle) -> Vec<(i32, i32)> {
        debug_assert!(rect.width >= 0.0 && rect.height >= 0.0);
        let (min_cx, min_cy) = self.cell_coords(rect.x, rect.y);
        let (max_cx, max_cy) = self.cell_coords(rect.x + rect.width, rect.y + rect.height);

        let mut out = Vec::new();
        for cy in min_cy..=max_cy {
            for cx in min_cx..=max_cx {
                out.push((cx, cy));
            }
        }
        out
    }

    pub fn insert(&mut self, entity: Entity, rect: Rectangle) {
        let cells = self.cell_coords_rect(rect);
        for cell in &cells {
            self.cells.entry(*cell).or_default().push(entity);
        }
        self.entities.insert(entity, cells);
    }

    pub fn update(&mut self, entity: Entity, new_rect: Rectangle) {
        #[cfg(feature = "trace")]
        let _span = info_span!("spatial_hash_update").entered();
        let cells = self.cell_coords_rect(new_rect);

        if let Some(old_cells) = self.entities.get(&entity) {
            // check if cells are the same
            if *old_cells == cells {
                return;
            }

            // remove from old cells
            for cell in old_cells {
                if let Some(bucket) = self.cells.get_mut(cell) {
                    bucket.retain(|&e| e != entity);
                    if bucket.is_empty() {
                        self.cells.remove(cell);
                    }
                }
            }
        }

        self.insert(entity, new_rect);
    }

    pub fn query(&self, query_rect: Rectangle) -> HashSet<Entity> {
        #[cfg(feature = "trace")]
        let _span = info_span!("spatial_hash_query").entered();
        let (min_cx, min_cy) = self.cell_coords(query_rect.x, query_rect.y);
        let (max_cx, max_cy) = self.cell_coords(
            query_rect.x + query_rect.width,
            query_rect.y + query_rect.height,
        );

        let num_cells = ((max_cx - min_cx + 1) * (max_cy - min_cy + 1)) as usize;

        // Estimate: total entities / used cells, times cells in this query
        let avg_per_cell = self.entities.len() / self.cells.len().max(1);
        let capacity = num_cells * avg_per_cell;

        let mut found = HashSet::with_capacity(capacity);
        for cy in min_cy..=max_cy {
            for cx in min_cx..=max_cx {
                if let Some(bucket) = self.cells.get(&(cx, cy)) {
                    found.extend(bucket);
                }
            }
        }
        found
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_spatial_hash() {
        let mut spatial_hash = SpatialHash::new(10.0);
        assert_eq!(spatial_hash.cell_coords(5.0, 5.0), (0, 0));
        assert_eq!(spatial_hash.cell_coords(15.0, 5.0), (1, 0));
        assert_eq!(spatial_hash.cell_coords(-5.0, -5.0), (-1, -1));

        let mut world = bevy_ecs::world::World::new();

        let entity = world.spawn(()).id();

        spatial_hash.insert(
            entity,
            Rectangle {
                x: 0.0,
                y: 0.0,
                width: 10.0,
                height: 10.0,
            },
        );

        let e = spatial_hash.query(Rectangle {
            x: 5.0,
            y: 5.0,
            width: 1.0,
            height: 1.0,
        });

        assert_eq!(e.len(), 1);

        spatial_hash.update(
            entity,
            Rectangle {
                x: 10.0,
                y: 10.0,
                width: 10.0,
                height: 10.0,
            },
        );

        let e = spatial_hash.query(Rectangle {
            x: 5.0,
            y: 5.0,
            width: 1.0,
            height: 1.0,
        });

        assert_eq!(e.len(), 0);
    }
}
