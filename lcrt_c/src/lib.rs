#![allow(unsafe_code)]

pub mod etf_c;
pub mod lcrt_c;

// #[cfg(not(feature = "c_unwind"))]
// macro_rules! extern_fn {
//     {
//         $name:ident($( $arg:ident: $type:ty ),* $(,)?) $(-> $ret:ty )?
//             $body:block
//     } => {
//         #[unsafe(no_mangle)]
//         pub unsafe extern "C" fn $name($( $arg: $type ),*) $(-> $ret )?
//             $body
//     };
// }
// #[cfg(feature = "c_unwind")]
// macro_rules! extern_fn {
//     {
//         $name:ident($( $arg:ident: $type:ty ),* $(,)?) $(-> $ret:ty )?
//             $body:block
//     } => {
//         #[unsafe(no_mangle)]
//         pub unsafe extern "C-unwind" fn $name($( $arg: $type ),*) $(-> $ret )?
//             $body
//     };
// }
