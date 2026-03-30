use crate::geo::{Line, Secant, Sphere};

#[must_use]
pub fn get_straight_trajectory<Id, N>(line: Line, nodes: N) -> Option<Vec<(glam::DVec3, Id)>>
where
    N: IntoIterator<Item = (Id, Sphere)>,
{
    let mut intersecting: Vec<_> = nodes
        .into_iter()
        .map(|(id, sphere)| (id, line.sphere_intersection(&sphere)))
        .filter_map(|(id, result)| result.try_get().map(|secant| (id, secant)))
        .filter(|(_, secant)| line.intersects_with_secant(secant))
        .collect();

    intersecting.sort_unstable_by(|(_, a), (_, b)| a.first_total_cmp(b));
    let mut intersecting = intersecting.into_iter().peekable();

    let mut t = 0.;
    let mut changes = Vec::new();
    loop {
        let (id, Secant { tc: _, td }) = std::iter::from_fn(|| {
            intersecting.peek().filter(|(_, secant)| secant.tc < t)?;
            intersecting.next()
        })
        .max_by(|(_, a), (_, b)| a.second_total_cmp(b))?;
        if td < t {
            // no sphere covers the current point
            return None;
        }

        // TODO: use t.midpoint(tc) except on the first?
        // changes.push((id, t, line.interpolate(t)));
        changes.push((line.interpolate(t), id));

        if td >= line.abl {
            return Some(changes);
        }

        t = td;
    }
}

#[cfg(test)]
mod test {
    use glam::DVec3;

    use crate::{
        geo::{Line, Sphere},
        get_straight_trajectory,
    };

    #[test]
    fn simple5() {
        let line = Line::new(DVec3::new(-2., -3., -1.), DVec3::new(4., 1., 2.));

        let spheres = [
            (0, Sphere::new(DVec3::new(-2., -2., -2.), 2.)),
            (1, Sphere::new(DVec3::new(1., -2., 0.), 2.)),
            (2, Sphere::new(DVec3::new(5., 3., 3.), 6.)),
            (3, Sphere::new(DVec3::new(1.5, -1., 0.5), 1.)),
            (4, Sphere::new(DVec3::new(-2., 2., 1.), 2.)),
        ];

        let changes = get_straight_trajectory(line, spheres)
            .expect("expected to find a covering set of spheres");

        assert_eq!(changes.len(), 3);
        assert!(
            changes
                .iter()
                .zip(spheres)
                .all(|(change, sphere)| change.1 == sphere.0)
        );
        assert!(
            changes
                .iter()
                .copied()
                // .map(|(_, t)| line.interpolate(t))
                .map(|(p, _)| p)
                .zip([
                    line.a,
                    DVec3::new(-0.810_766_901, -2.207_177_934, -0.405_383_451,),
                    DVec3::new(1.843_073_503, -0.437_950_998, 0.921_536_751)
                ])
                .all(|(a, b)| { a.distance(b) < 1e-9 })
        );
    }
}
