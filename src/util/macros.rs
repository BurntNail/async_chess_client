///Provides any number of unit structs that implement a unit type
#[macro_export]
macro_rules! generic_enum {
    (($trait_name:ident -> $trait_docs:literal) => $(($unit_struct_name:ident -> $docs:literal)),+) => {
        pub trait $trait_name : Sealed {}

        $(
            #[doc=$trait_docs]
            #[derive(Copy, Clone, Debug)]
            pub struct $unit_struct_name;
            impl Sealed for $unit_struct_name {}
            impl $trait_name for $unit_struct_name {}
        )+
    };
}