#![allow(unused_macros)]

#[rustversion::not(nightly)]
/// A macro that implements a trait for a type. If it detects that it is being
/// compiled on nightly, it will implement the trait as `const`, assuming that
/// the functions are `const` as well. If it is not being compiled on nightly,
/// it will implement the trait as normal.
macro_rules! const_impl {
    (
        impl $trt:tt $(< $opt_typ:ty >)?
        for $typ:ty { $($body:tt)* }
    ) => {
        impl $trt$(<$opt_typ>)? for $typ { $($body)* }
    };
}

#[rustversion::nightly]
/// A macro that implements a trait for a type. If it detects that it is being
/// compiled on nightly, it will implement the trait as `const`, assuming that
/// the functions are `const` as well. If it is not being compiled on nightly,
/// it will implement the trait as normal.
macro_rules! const_impl {
    (
        impl $trt:tt $(< $opt_typ:ty >)?
        for $typ:ty { $($body:tt)* }
    ) => {
        impl const $trt$(<$opt_typ>)? for $typ {
            $($body)*
        }
    };
}

#[rustversion::not(nightly)]
/// A macro to conditionally make a function constant
/// based on whether or not it is being compiled on nightly.
macro_rules! nightly_const {
    (pub fn $name:ident($($arg:ident $(: $typ:ty)?),*) -> $ret:ty $body:block) => {
        pub fn $name($($arg$(: $typ)?),*) -> $ret $body
    };
    (pub fn $name:ident($(&$arg:ident $(: $typ:ty)?),*) -> $ret:ty $body:block) => {
        pub fn $name($(&$arg$(: $typ)?),*) -> $ret $body
    };
    (pub fn $name:ident($(&mut $arg:ident $(: $typ:ty)?),*) -> $ret:ty $body:block) => {
        pub fn $name($(&mut $arg$(: $typ)?),*) -> $ret $body
    };
    (fn $name:ident($($arg:ident $(: $typ:ty)?),*) -> $ret:ty $body:block) => {
        fn $name($($arg: $typ),*) -> $ret $body
    };
    (fn $name:ident($(&$arg:ident $(: $typ:ty)?),*) -> $ret:ty $body:block) => {
        fn $name($(&$arg$(: $typ)?),*) -> $ret $body
    };
    (fn $name:ident($(&mut $arg:ident $(: $typ:ty)?),*) -> $ret:ty $body:block) => {
        fn $name($(&mut $arg$(: $typ)?),*) -> $ret $body
    };
}

#[rustversion::nightly]
/// A macro to conditionally make a function constant
/// based on whether or not it is being compiled on nightly.
macro_rules! nightly_const {
    (pub fn $name:ident($($arg:ident $(: $typ:ty)?),*) -> $ret:ty $body:block) => {
        pub const fn $name($($arg$(: $typ)?),*) -> $ret $body
    };
    (pub fn $name:ident($(&$arg:ident $(: $typ:ty)?),*) -> $ret:ty $body:block) => {
        pub const fn $name($(&$arg$(: $typ)?),*) -> $ret $body
    };
    (pub fn $name:ident($(&mut $arg:ident $(: $typ:ty)?),*) -> $ret:ty $body:block) => {
        pub const fn $name($(&mut $arg$(: $typ)?),*) -> $ret $body
    };
    (fn $name:ident($($arg:ident $(: $typ:ty)?),*) -> $ret:ty $body:block) => {
        const fn $name($($arg: $typ),*) -> $ret $body
    };
    (fn $name:ident($(&$arg:ident $(: $typ:ty)?),*) -> $ret:ty $body:block) => {
        const fn $name($(&$arg$(: $typ)?),*) -> $ret $body
    };
    (fn $name:ident($(&mut $arg:ident $(: $typ:ty)?),*) -> $ret:ty $body:block) => {
        const fn $name($(&mut $arg$(: $typ)?),*) -> $ret $body
    };
}

/* macro_rules! if_with_session {
    (
        $session:ident,
        $($other_ident:ident).*,
        $fn_name:ident($($args:tt)*),
        true
    ) => {
        use paste::paste;
        paste! {
            if let Some(ref mut s) = $session {
                $($other_ident).*.[<$fn_name _with_session>](s, $($args)*).await?;
            } else {
                $($other_ident).*.$fn_name($($args)*).await?;
            }
        };
    };
} */
pub(crate) use const_impl;
// pub(crate) use nightly_const;
// pub(crate) use if_with_session;
