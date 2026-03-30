use std::ptr;

use etf::geo;
use lcrt::NodeInfo;

use crate::lcrt_c::{LcrtArea, LcrtPosition};

#[repr(C)]
pub struct EtfPath {
    inner: std::vec::IntoIter<(glam::DVec3, std::net::Ipv4Addr)>,
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn etf_find_path(area: *const LcrtArea, to: LcrtPosition) -> *mut EtfPath {
    let area = &unsafe { &*area }.0;
    let Some((nodes, network)) = area.get_network() else {
        todo!("handle incomplete network");
    };

    let position = area.get_node_info().position();
    let radius = area.get_config().radius;

    let line = geo::Line::new(position, to.into());
    if let Some(slt) = etf::get_straight_trajectory(
        line,
        nodes
            .iter()
            .map(|(id, data)| (*id, geo::Sphere::new(data.position, radius))),
    ) {
        return Box::leak(Box::new(EtfPath {
            inner: slt.into_iter(),
        }));
    }

    let Some(parent) = area.get_parent() else {
        return ptr::null_mut();
    };

    // TODO: rewrite everything to not require this
    let mapped = network.map(
        |_, id| (*id, geo::Sphere::new(nodes[id].position, radius)),
        |_, ()| (),
    );

    let path = 'path: {
        if let Some(path) = etf::get_ancestor_path(&mapped, nodes[&parent].index, to.into()) {
            break 'path path;
        }

        let intersections = etf::Intersections::new(&mapped);
        let start_ix = intersections.get_ix(&parent);
        let Some(path) = intersections.get_path(start_ix, to.into()) else {
            return ptr::null_mut();
        };
        path
    };

    #[expect(
        clippy::needless_collect,
        reason = "collect used to remove dependency on borrow"
    )]
    let v = path.segments(to.into()).collect::<Vec<_>>();
    println!("{v:?}");
    Box::leak(Box::new(EtfPath {
        inner: v.into_iter(),
    }))
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn etf_path_next(
    path: *mut EtfPath,
    position: *mut LcrtPosition,
    forwarder: *mut u32,
) -> bool {
    let p = unsafe { &mut *path };
    let Some((p, f)) = p.inner.next() else {
        return false;
    };

    unsafe { position.write(p.into()) };
    unsafe { forwarder.write(f.into()) };

    true
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn etf_path_drop(path: *mut EtfPath) {
    drop(unsafe { Box::from_raw(path) });
}
