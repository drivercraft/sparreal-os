/// Defines a driver type that wraps a boxed trait object.
///
/// $name: driver name
///
/// $tr: driver trait path
#[macro_export]
macro_rules! def_driver {
    ($name:ident, $tr:path) => {
        $crate::paste! {
            pub mod [<$name:lower>]{
                use super::*;
                pub struct $name(alloc::boxed::Box<dyn $tr>);

                impl $name {
                    pub fn new<T: $tr>(driver: T) -> Self {
                        Self(alloc::boxed::Box::new(driver))
                    }

                    pub fn typed_ref<T: $tr>(&self) -> Option<&T> {
                        self.raw_any()?.downcast_ref()
                    }

                    pub fn typed_mut<T: $tr>(&mut self) -> Option<&mut T> {
                        self.raw_any_mut()?.downcast_mut()
                    }
                }

                impl $crate::DriverGeneric for $name {
                    fn open(&mut self) -> Result<(), rdif_base::KError> {
                        self.0.open()
                    }

                    fn close(&mut self) -> Result<(), rdif_base::KError> {
                        self.0.close()
                    }

                    fn raw_any(&self) -> Option<&dyn core::any::Any> {
                        Some( self.0.as_ref() as &dyn core::any::Any )
                    }

                    fn raw_any_mut(&mut self) -> Option<&mut dyn core::any::Any> {
                        Some( self.0.as_mut() as &mut dyn core::any::Any )
                    }
                }

                impl core::ops::Deref for $name {
                    type Target = dyn $tr;

                    fn deref(&self) -> &Self::Target {
                        self.0.as_ref()
                    }
                }

                impl core::ops::DerefMut for $name {
                    fn deref_mut(&mut self) -> &mut Self::Target {
                        self.0.as_mut()
                    }
                }
            }
            pub use [<$name:lower>]::$name;
        }
    };
}

// / Defines a driver type that wraps a boxed trait object.
// /
// / $name: driver name
// /
// / $t: driver trait path
// #[macro_export(local_inner_macros)]
// macro_rules! def_driver_rdif {
//     ($name:ident) => {
//         paste::paste! {
//             def_driver!($name, [<rdif_ $name:lower>]::Interface, [<rdif_ $name:lower>]);
//         }
//     };
// }
