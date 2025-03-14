macro_rules! component {
    ($name:ident : $inner:ident { $($key:ident : $ty:ty = $value:expr,)* }) => {
        #[derive(Clone)]
        pub struct $name(::std::sync::Arc<($crate::session::SessionWeak, ::parking_lot::Mutex<$inner>)>);
        impl $name {
            #[allow(dead_code)]
            pub(crate) fn new(session: $crate::session::SessionWeak) -> $name {
                debug!(target:"spotipi::component", "new {}", stringify!($name));

                $name(::std::sync::Arc::new((session, ::parking_lot::Mutex::new($inner {
                    $($key : $value,)*
                }))))
            }

            #[allow(dead_code)]
            fn lock<F: FnOnce(&mut $inner) -> R, R>(&self, f: F) -> R {
                let mut inner = (self.0).1.lock();
                f(&mut inner)
            }

            #[allow(dead_code)]
            fn session(&self) -> $crate::session::Session {
                (self.0).0.upgrade()
            }
        }

        struct $inner {
            $($key : $ty,)*
        }

        impl Drop for $inner {
            fn drop(&mut self) {
                debug!(target:"spotipi::component", "drop {}", stringify!($name));
            }
        }
    }
}
