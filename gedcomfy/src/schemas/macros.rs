use miette::SourceSpan;
use vec1::Vec1;

use super::SchemaError;

// embedded structures can only have 0:1 or 1:1 cardinality
macro_rules! structure_cardinality {
    ($ty:ty, 0, 1) => {
        Option<$ty>
    };
    ($ty:ty, 1, 1) => {
        $ty
    };
}

macro_rules! from_struct_cardinality {
    ($parent_span:expr, $value:expr, 0, 1) => {
        match $value {
            Some(x) => Some(x.complete($parent_span)?),
            None => None,
        }
    };
    ($parent_span:expr, $value:expr, 1, 1) => {
        match $value {
            Some(x) => x.complete($parent_span)?,
            None => todo!("required but not found"),
        }
    };
}

macro_rules! collection_for_cardinality {
    ($ty:ty, 0, 1) => {
        Option<$ty>
    };
    ($ty:ty, 1, 1) => {
        $ty
    };
    ($ty:ty, 0, N) => {
        Vec< $ty >
    };
    ($ty:ty, 1, N) => {
        vec1::Vec1< $ty >
    };
    ($ty:ty, 0, $max:literal) => {
        Vec< $ty >
    };
    ($ty:ty, 1, $max:literal) => {
        vec1::Vec1< $ty >
    };
}

pub(super) fn c_vec2opt<T>(tag: &'static str, v: Vec<T>) -> Result<Option<T>, SchemaError> {
    match v.len() {
        0 => Ok(None),
        1 => Ok(v.into_iter().next()),
        n => Err(SchemaError::TooManyRecords { tag, expected: 1, received: n }),
    }
}

pub(super) fn c_vec2one<T>(
    parent_span: SourceSpan,
    tag: &'static str,
    v: Vec<T>,
) -> Result<T, SchemaError> {
    match v.len() {
        0 => Err(SchemaError::MissingRecord { parent_span, tag }),
        1 => Ok(v.into_iter().next().unwrap()),
        n => Err(SchemaError::TooManyRecords { tag, expected: 1, received: n }),
    }
}

pub(super) fn c_vec2vec1<T>(
    parent_span: SourceSpan,
    tag: &'static str,
    v: Vec<T>,
) -> Result<Vec1<T>, SchemaError> {
    Vec1::try_from_vec(v).map_err(|_| SchemaError::MissingRecord { parent_span, tag })
}

macro_rules! from_cardinality {
    ($parent_span:expr, $tag:literal, $x:expr, 0, 1) => {{ crate::schemas::macros::c_vec2opt($tag, $x)? }};
    ($parent_span:expr, $tag:literal, $x:expr, 1, 1) => {{ crate::schemas::macros::c_vec2one($parent_span, $tag, $x)? }};
    ($parent_span:expr, $tag:literal, $x:expr, 1, N) => {{ crate::schemas::macros::c_vec2vec1($parent_span, $tag, $x)? }};
    ($parent_span:expr, $tag:literal, $x:expr, 0, N) => {{ $x }};
    ($parent_span:expr, $tag:literal, $x:expr, 1, $max:literal) => {{
        // TODO: enforce max
        c_vec2vec1($parent_span, $tag, $x)?
    }};
    ($parent_span:expr, $tag:literal, $x:expr, 0, $max:literal) => {{
        // TODO: enforce max
        $x
    }};
}

macro_rules! define_enum {
    (enum $name:ident { $($struct_ty:ident),+ $(,)? }) => {
        #[derive(Debug, Eq, PartialEq, Clone)]
        pub enum $name {
            $(
                /// $tag
                $struct_ty($struct_ty),
            )+
        }

        impl $name {
            #[inline]
            pub fn matches_tag(tag: &str) -> bool {
                $(
                    $struct_ty::matches_tag(tag) ||
                )+ false
            }

            fn build_from(record: Sourced<RawRecord>) -> Result<$name, SchemaError> {
                debug_assert!($name::matches_tag(record.line.tag.as_str()));
                match record.line.tag.as_str() {
                    $(tag if $struct_ty::matches_tag(tag) => {
                        Ok($struct_ty::try_from(record)?.into())
                    })*
                    _ => unreachable!(),
                }
            }
        }

        $(
            impl From<$struct_ty> for $name {
                fn from(e: $struct_ty) -> Self {
                    Self::$struct_ty(e)
                }
            }
        )+
    };
}

macro_rules! define_structure {
    ($name:ident {
        $(.. $struct_field:ident: $struct_ty:ty {$struct_min:tt : $struct_max:tt},)*
        $($tag:literal $field:ident: $ty:ty {$min:tt : $max:tt},)*
    }) => {
        #[derive(Debug, Eq, PartialEq, Clone)]
        pub struct $name {
            $(
                $struct_field: $crate::schemas::macros::structure_cardinality!($struct_ty, $struct_min, $struct_max),
            )*
            $(
                $field: $crate::schemas::macros::collection_for_cardinality!($ty, $min, $max),
            )*
        }

        paste::paste! {
            #[derive(Default)]
            struct [< $name Builder >] {
                $(
                    $struct_field: Option< [< $struct_ty Builder >] >,
                )*
                $(
                    $field: Vec<$ty>,
                )*
            }

            impl [< $name Builder >] {
                fn build_from(&mut self, record: Sourced<RawRecord>) -> Result<(), SchemaError> {
                    debug_assert!($name::matches_tag(record.line.tag.as_str()));
                    match record.line.tag.as_str() {
                        $($tag => {
                            let $field: $ty = <$ty>::try_from(record)?;
                            self.$field.push($field);
                        })*
                        tag => {
                            $(
                                if $struct_ty::matches_tag(tag) {
                                    self.$struct_field.get_or_insert_with(Default::default).build_from(record)?;
                                } else
                            )*
                            {
                                unreachable!("{tag} should have been handled")
                            }
                        }
                    }
                    Ok(())
                }

                #[allow(unused)]
                fn complete(self, parent_span: SourceSpan) -> Result<$name, SchemaError> {
                    Ok($name {
                        $(
                            $struct_field: $crate::schemas::macros::from_struct_cardinality!(parent_span, self.$struct_field, $struct_min, $struct_max),
                        )*
                        $(
                            $field: $crate::schemas::macros::from_cardinality!(parent_span, $tag, self.$field, $min, $max),
                        )*
                    })
                }
            }
        }

        impl $name {
            #[inline]
            #[allow(unused)]
            pub fn matches_tag(tag: &str) -> bool {
                match tag {
                    $($tag => true,)*
                    t => {
                        $( <$struct_ty>::matches_tag(t) || )* false
                    }
                }
            }
        }
    };
}

macro_rules! if_not_provided {
    (() $code:block) => {
        $code
    };
    (($($target:tt)+) $code:block) => {};
}

macro_rules! define_record {
    // Record with data attached and maybe children:
    // TODO it doesn't make sense for structure to have cardinality other than 0:1 or 1:1
    ($self_tag:literal $name:ident $(($value_name:ident : $value:ty))? {
        $(.. $struct_field:ident: $struct_ty:ident {$struct_min:tt : $struct_max:tt} ,)*
        $(enum $enum_field:ident: $enum_ty:ident {$enum_min:tt : $enum_max:tt} ,)*
        $($tag:literal $field:ident: $ty:ty {$min:tt : $max:tt} ,)*
    }) => {
        #[derive(Debug, Eq, PartialEq, Clone)]
        pub struct $name {
            $(
                pub $value_name: $value,
            )?
            $(
                pub $struct_field: $crate::schemas::macros::structure_cardinality!($struct_ty, $struct_min, $struct_max),
            )*
            $(
                pub $enum_field: $crate::schemas::macros::collection_for_cardinality!($enum_ty, $enum_min, $enum_max),
            )*
            $(
                pub $field: $crate::schemas::macros::collection_for_cardinality!($ty, $min, $max),
            )*
        }

        impl $name {
            #[inline]
            pub fn matches_tag(tag: &str) -> bool {
                tag == $self_tag
            }
        }

        impl<'a> TryFrom<Sourced<RawRecord<'a>>> for $name {
            type Error = SchemaError;

            fn try_from(mut source: Sourced<RawRecord<'a>>) -> Result<Self, Self::Error> {
                debug_assert_eq!(source.line.tag.as_str(), $self_tag);

                #[derive(Default)]
                struct Builder {
                    $(
                        $enum_field: Vec<$enum_ty>,
                    )*
                    $(
                        $field: Vec<$ty>,
                    )*
                }

                // TODO: need to read structures

                let mut unused_records = Vec::new();
                #[allow(unused)]
                let mut result = Builder::default();
                paste::paste! {
                    $(
                        let mut $struct_field : Option< [< $struct_ty Builder >] > = None;
                    )*
                }

                let parent_span = source.span;

                for record in source.sourced_value.records {
                    match record.line.tag.as_str() {
                        $(
                            $tag => {
                                let $field: $ty = <$ty>::try_from(record)?;
                                result.$field.push($field);
                            }
                        )*
                        "CONC" | "CONT" => {
                            // will be handled by line_value
                            // TODO: is CONC valid in other versions?
                            unused_records.push(record);
                        }
                        tag => {
                            $(
                                if $struct_ty::matches_tag(tag) {
                                    $struct_field.get_or_insert_with(Default::default).build_from(record)?;
                                } else
                            )*
                            $(
                                if $enum_ty::matches_tag(tag) {
                                    let $enum_field: $enum_ty = <$enum_ty>::build_from(record)?;
                                    result.$enum_field.push($enum_field);
                                } else
                            )*
                            if tag.starts_with("_") {
                                tracing::info!(tag, "Ignoring user-defined tag");
                            } else {
                                return Err(SchemaError::UnexpectedTag {
                                    parent_span,
                                    tag: tag.to_string(),
                                    span: record.line.tag.span });
                            }
                        }
                    }
                }

                source.sourced_value.records = unused_records;

                $crate::schemas::macros::if_not_provided!(($($value_name)?) {
                    if !source.sourced_value.records.is_empty() {
                        todo!("CONT not permitted here - no value expected")
                    }
                });

                Ok(Self {
                    $(
                        $value_name: <$value>::try_from(source)?,
                    )?
                    $(
                        $struct_field: $crate::schemas::macros::from_struct_cardinality!(parent_span, $struct_field, 0, 1),
                    )*
                    $(
                        $enum_field: $crate::schemas::macros::from_cardinality!(parent_span, "TODO", result.$enum_field, $enum_min, $enum_max),
                    )*
                    $(
                        $field: $crate::schemas::macros::from_cardinality!(parent_span, $tag, result.$field, $min, $max),
                    )*
                })
            }
        }
    };
}

pub(crate) use collection_for_cardinality;
pub(crate) use define_enum;
pub(crate) use define_record;
pub(crate) use define_structure;
pub(crate) use from_cardinality;
pub(crate) use from_struct_cardinality;
pub(crate) use if_not_provided;
pub(crate) use structure_cardinality;
