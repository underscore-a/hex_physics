use crate::{Box2d, Collider, Physical, QuadTree};
use hex::{
    anyhow,
    components::Transform,
    ecs::{ev::Control, system_manager::System, ComponentManager, EntityManager, Ev, Id, Scene},
    glium::glutin::event::Event,
    math::Vec2d,
};
use rayon::prelude::*;
use std::{
    sync::{Arc, RwLock},
    time::{Duration, Instant},
};

pub type Collision = (bool, (Option<Vec2d>, Option<Vec2d>));
pub type Colliders = Vec<(Id, (Id, Collider), Id, Option<Physical>)>;

pub struct PhysicsManager {
    pub rate: u32,
    pub step_amount: u32,
    pub max_delta: Option<Duration>,
    pub bounds: (Box2d, usize),
    frame: Instant,
    count: u32,
}

impl PhysicsManager {
    pub fn new(
        rate: u32,
        step_amount: u32,
        max_delta: Option<Duration>,
        bounds: (Box2d, usize),
    ) -> Self {
        Self {
            rate,
            step_amount,
            bounds,
            max_delta,
            frame: Instant::now(),
            count: 0,
        }
    }

    pub fn detect(
        (ac, at, ap): (&Collider, &Transform, &Option<Physical>),
        (bc, bt, bp): (&Collider, &Transform, &Option<Physical>),
    ) -> Option<Collision> {
        if ac.layers.iter().any(|a| bc.layers.contains(a))
            && !ac.ignore.iter().any(|a| bc.layers.contains(a))
            && !bc.ignore.iter().any(|b| ac.layers.contains(b))
        {
            if let Some(min_translation) = ac.intersecting(at, bc, bt) {
                return Some((
                    ac.ghost || bc.ghost,
                    (
                        ap.as_ref().map(|_| -min_translation),
                        bp.as_ref().map(|_| min_translation),
                    ),
                ));
            }
        }

        None
    }

    pub fn resolve(
        ghost_col: bool,
        other_e: Id,
        cache_collider: Id,
        cache_transform: Id,
        tr: Option<Vec2d>,
        cm: &mut ComponentManager,
    ) {
        if let Some(collider) = cm
            .get_cache_mut::<Collider>(cache_collider)
            .and_then(|c| (!c.collisions.contains(&other_e)).then_some(c))
        {
            collider.collisions.push(other_e);
        }

        if !ghost_col {
            if let Some((tr, t)) =
                tr.and_then(|tr| Some((tr, cm.get_cache_mut::<Transform>(cache_transform)?)))
            {
                t.set_position(t.position() + tr);
            }
        }
    }

    pub fn check_collisions(&self, (em, cm): (&EntityManager, &mut ComponentManager)) {
        let (boundary, cap) = self.bounds.clone();
        let mut tree = QuadTree::new(boundary, cap);
        let entities: Vec<_> = em
            .entities
            .keys()
            .cloned()
            .filter_map(|e| {
                let ref e @ (be, _, (_, ref b_transform), _) = (
                    e,
                    cm.get_id::<Collider>(e, em).and_then(|c| {
                        cm.get_cache_mut::<Collider>(c)
                            .and_then(|col| col.active.then(|| (c, col.clone())))
                    })?,
                    cm.get_id::<Transform>(e, em).and_then(|t| {
                        cm.get_cache::<Transform>(t)
                            .and_then(|transform| transform.active.then(|| (t, transform.clone())))
                    })?,
                    cm.get::<Physical>(e, em).cloned(),
                );

                tree.insert((b_transform.position(), be), Arc::new(e.clone()))
                    .then(|| e.clone())
            })
            .collect();
        let checked = RwLock::new(Vec::new());

        for ((ae, ac, at), (be, bc, bt), (ghost, (atr, btr))) in entities
            .par_iter()
            .cloned()
            .filter_map(|(ae, (ac, a_col), (at, a_transform), a_physical)| {
                Some(
                    tree.query(Box2d::new(a_transform.position(), a_col.boundary))
                        .into_iter()
                        .filter_map(|(_, a)| {
                            let (be, (bc, b_col), (bt, b_transform), b_physical) = &*a;
                            let res = {
                                let res = {
                                    let checked = checked.read().ok()?;

                                    !checked.contains(&(ae, *be)) && !checked.contains(&(*be, ae))
                                };

                                if res {
                                    Some((
                                        (ae, ac, at),
                                        (*be, *bc, *bt),
                                        Self::detect(
                                            (&a_col, &a_transform, &a_physical),
                                            (b_col, b_transform, b_physical),
                                        )?,
                                    ))
                                } else {
                                    None
                                }
                            };

                            checked.write().ok()?.push((ae, *be));

                            res
                        })
                        .collect::<Vec<_>>(),
                )
            })
            .flatten()
            .collect::<Vec<_>>()
        {
            Self::resolve(ghost, ae, bc, bt, btr, cm);
            Self::resolve(ghost, be, ac, at, atr, cm);
        }
    }

    pub fn update_positions(
        &self,
        step_amount: Option<u32>,
        delta: Duration,
        (em, cm): (&mut EntityManager, &mut ComponentManager),
    ) {
        for e in em.entities.keys().cloned() {
            if let Some((pos, physical)) = cm
                .get::<Physical>(e, em)
                .cloned()
                .and_then(|p| {
                    let force = p.active.then_some(p.force)?;
                    let t = cm.get_id::<Transform>(e, em)?;
                    let pos = if let Some(step_amount) = step_amount {
                        if let Some(t) = cm.get_cache_mut::<Transform>(t) {
                            t.set_position(
                                t.position() + (force * delta.as_secs_f32()) / step_amount as f32,
                            );
                        }

                        self.check_collisions((em, cm));

                        cm.get_cache::<Transform>(t).map(|t| t.position())
                    } else if let Some(t) = cm.get_cache_mut::<Transform>(t) {
                        t.set_position(t.position() + force * delta.as_secs_f32());

                        Some(t.position())
                    } else {
                        None
                    };

                    pos
                })
                .and_then(|pos| Some((pos, cm.get_mut::<Physical>(e, em)?)))
            {
                if let Some(vel) = physical
                    .last_position()
                    .map(|l| (pos - l) / delta.as_secs_f32())
                {
                    physical.set_velocity(vel);
                }

                physical.set_last_position(pos);
            }
        }
    }

    pub fn clear_collisions(&self, (em, cm): (&mut EntityManager, &mut ComponentManager)) {
        for e in em.entities.keys().cloned() {
            if let Some(col) = cm
                .get_mut::<Collider>(e, em)
                .and_then(|col| col.active.then_some(col))
            {
                col.collisions.clear()
            }
        }
    }
}

impl<'a> System<'a> for PhysicsManager {
    fn update(
        &mut self,
        ev: &mut Ev,
        _: &mut Scene,
        (em, cm): (&mut EntityManager, &mut ComponentManager),
    ) -> anyhow::Result<()> {
        if let Ev::Event(Control {
            event: Event::MainEventsCleared,
            flow: _,
        }) = ev
        {
            let now = Instant::now();
            let delta = {
                let delta = now.duration_since(self.frame);

                if let Some(md) = self.max_delta {
                    delta.min(md)
                } else {
                    delta
                }
            };

            self.frame = now;

            if self.count >= self.rate {
                self.count = 0;

                self.clear_collisions((em, cm));

                for _ in 0..self.step_amount {
                    self.update_positions(Some(self.step_amount), delta, (em, cm));
                }
            } else {
                self.update_positions(None, delta, (em, cm));
            }

            self.count += 1;
        }

        Ok(())
    }
}
