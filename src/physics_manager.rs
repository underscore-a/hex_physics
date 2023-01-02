use crate::polygon::Polygon;
use hex::{
    anyhow,
    components::Transform,
    ecs::{
        component_manager::ComponentManager,
        entity_manager::EntityManager,
        system_manager::{Ev, System},
    },
    glium::Display,
};

pub struct PhysicsManager;

impl<'a> System<'a> for PhysicsManager {
    fn update(
        &mut self,
        _: &Display,
        _: &mut Ev,
        entity_manager: &mut EntityManager,
        component_manager: &mut ComponentManager,
    ) -> anyhow::Result<()> {
        let callbacks = {
            let mut objects: Vec<_> = entity_manager
                .entities
                .keys()
                .filter_map(|e| {
                    Some((
                        e,
                        component_manager.get::<Polygon>(*e, entity_manager)?,
                        component_manager.get::<Transform>(*e, entity_manager)?,
                    ))
                })
                .collect();

            let mut callbacks = Vec::new();

            while let Some((ae, a, at)) = objects.pop() {
                for (be, b, bt) in &objects {
                    if a.intersecting(at, b, bt) {
                        callbacks.extend([
                            (*ae, **be, a.callback.clone()),
                            (**be, *ae, b.callback.clone()),
                        ]);
                    }
                }
            }

            callbacks
        };

        for (ae, be, c) in callbacks {
            c.try_borrow_mut()?(ae, be, entity_manager, component_manager);
        }

        Ok(())
    }
}
