use hex::{cgmath::Vector2, cid, components::Transform, ecs::component_manager::Component};

#[derive(Clone)]
pub struct Collider {
    pub points: Vec<Vector2<f32>>,
    pub collisions: Vec<usize>,
    pub active: bool,
}

impl Collider {
    pub fn new(points: Vec<Vector2<f32>>, active: bool) -> Self {
        Self {
            points,
            collisions: Vec::new(),
            active,
        }
    }

    pub fn rect(dims: Vector2<f32>, active: bool) -> Self {
        let dims = dims / 2.0;

        Self::new(
            vec![
                Vector2::new(-dims.x, -dims.y),
                Vector2::new(-dims.x, dims.y),
                Vector2::new(dims.x, dims.y),
                Vector2::new(dims.x, -dims.y),
            ],
            active,
        )
    }

    pub fn intersecting(&self, transform: &Transform, b: &Self, b_transform: &Transform) -> bool {
        let a_points = self
            .points
            .iter()
            .cloned()
            .map(|p| (transform.matrix() * p.extend(1.0)).truncate())
            .collect::<Vec<_>>();
        let b_points = b
            .points
            .iter()
            .cloned()
            .map(|p| (b_transform.matrix() * p.extend(1.0)).truncate())
            .collect::<Vec<_>>();

        for i in 0..a_points.len() {
            let p1 = a_points[i];
            let p2 = a_points[(i + 1) % a_points.len()];

            let normal = Vector2::new(p2.y - p1.y, p1.x - p2.x);

            let mut a_min = None;
            let mut a_max = None;

            for p in &a_points {
                let projected = normal.x * p.x + normal.y * p.y;

                if a_min.map(|a| projected < a).unwrap_or(true) {
                    a_min = Some(projected);
                }

                if a_max.map(|a| projected > a).unwrap_or(true) {
                    a_max = Some(projected);
                }
            }

            let mut b_min = None;
            let mut b_max = None;

            for p in &b_points {
                let projected = normal.x * p.x + normal.y * p.y;

                if b_min.map(|b| projected < b).unwrap_or(true) {
                    b_min = Some(projected);
                }

                if b_max.map(|b| projected > b).unwrap_or(true) {
                    b_max = Some(projected);
                }
            }

            if a_max.and_then(|a| b_min.map(|b| a < b)).unwrap_or(true)
                || b_max.and_then(|b| a_min.map(|a| b < a)).unwrap_or(true)
            {
                return false;
            }
        }

        true
    }
}

impl Component for Collider {
    fn id() -> usize {
        cid!()
    }
}
