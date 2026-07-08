mod construction;
mod forward_join_request;
mod sending;
mod streaming;

pub use {
    construction::Construction, forward_join_request::ForwardJoinRequests, sending::Sending,
    streaming::Streaming,
};

// pub trait Behaviour {
//     fn handle_message<N>(&mut self, m: message::Message, node_info: &mut N) -> Response
//     where
//         N: NodeInfo;
// }
