use std::collections::BTreeMap;

use darling::{ast, FromDeriveInput, FromField, FromMeta};
use proc_macro2::TokenStream;
use quote::{format_ident, quote};
use syn::{parse_macro_input, DeriveInput, Ident, Type};

#[derive(FromDeriveInput)]
#[darling(attributes(error), supports(struct_any))]
struct Opts {
    display: Option<String>,
    exit_code: Option<u8>,
    url: Option<String>,
    code: Option<String>,
    severity: Option<syn::Path>,

    data: ast::Data<(), StructFields>,
}

#[proc_macro_derive(Error, attributes(error, label))]
pub fn derive_errful(input: proc_macro::TokenStream) -> proc_macro::TokenStream {
    let input = parse_macro_input!(input);
    let opts = match Opts::from_derive_input(&input) {
        Ok(r) => r,
        Err(e) => return e.write_errors().into(),
    };

    let DeriveInput { ident, .. } = input;

    let (source_fields_to_provide, source_method) = match generate_source(&opts.data) {
        Ok(r) => r,
        Err(e) => return e.write_errors().into(),
    };

    let (label_fields_to_provide, labels) = match find_labels(&opts.data) {
        Ok(r) => r,
        Err(e) => return e.write_errors().into(),
    };

    let mut fields_to_provide = source_fields_to_provide;
    for (ix, field) in label_fields_to_provide {
        fields_to_provide.insert(ix, field);
    }

    let (field_names, field_provides) = match provide_fields(fields_to_provide, &opts.data) {
        Ok(r) => r,
        Err(e) => return e.write_errors().into(),
    };

    let source_code = match find_source_code(&opts.data) {
        Ok(r) => r,
        Err(e) => return e.write_errors().into(),
    };

    let display_impl = opts.display.map(|display| {
        quote! {
            #[automatically_derived]
            impl ::core::fmt::Display for #ident {
                fn fmt(&self, __formatter: &mut ::core::fmt::Formatter<'_>) -> ::core::fmt::Result {
                    write!(__formatter, #display)
                }
            }
        }
    });

    let mut provisions = Vec::new();

    provisions.extend(field_provides);

    if let Some(exit_code) = opts.exit_code {
        provisions.push(quote! {
            .provide_value(::std::process::ExitCode::from(#exit_code))
        });
    };

    if let Some(url) = opts.url {
        provisions.push(quote! {
            .provide_value(::errful::protocol::Url(#url))
        });
    };

    if let Some(code) = opts.code {
        provisions.push(quote! {
            .provide_value(::errful::protocol::Code(#code))
        });
    };

    if let Some(severity) = opts.severity {
        provisions.push(quote! {
            .provide_ref::<dyn ::errful::PrintableSeverity>(&#severity)
        });
    }

    if let Some(labels) = labels {
        provisions.push(labels);
    }

    if let Some(source_code) = source_code {
        provisions.push(source_code);
    }

    let output = quote! {
        #[automatically_derived]
        impl ::core::error::Error for #ident {
            #source_method

            #[allow(non_camel_case_types)]
            fn provide<'a>(&'a self, __request: &mut ::core::error::Request<'a>) {
                use ::std::borrow::Borrow;

                #(#field_names)*

                __request
                    #(#provisions)*
                ;
            }
        }

        #display_impl
    };

    output.into()
}

#[derive(Debug, FromField)]
#[darling(attributes(error))]
struct StructFields {
    // -- magic fields:
    ident: Option<syn::Ident>,
    ty: syn::Type,

    // -- actual options

    // is this the source of the error?
    #[darling(default)]
    source: bool,

    // labels
    label: Option<LabelTarget>,
    source_id: Option<String>,

    // source code
    #[darling(default)]
    source_code: bool,
}

#[derive(Debug)]
enum LabelTarget {
    Field(syn::Ident),
    Literal(String),
}

impl From<syn::Ident> for LabelTarget {
    fn from(ident: syn::Ident) -> Self {
        LabelTarget::Field(ident)
    }
}

impl From<String> for LabelTarget {
    fn from(literal: String) -> Self {
        LabelTarget::Literal(literal)
    }
}

impl FromMeta for LabelTarget {
    fn from_meta(item: &syn::Meta) -> darling::Result<Self> {
        String::from_meta(item)
            .map(Into::into)
            .or_else(|_| syn::Ident::from_meta(item).map(Into::into))
    }
}

fn provide_fields(
    fields_to_provide: BTreeMap<Ident, TokenStream>,
    data: &ast::Data<(), StructFields>,
) -> darling::Result<(Vec<TokenStream>, Vec<TokenStream>)> {
    let fields = data.as_ref().take_struct().expect("struct").fields;

    let mut field_names = Vec::new();
    let mut provides = Vec::new();

    for (ix, field) in fields.iter().enumerate() {
        let fieldname = field
            .ident
            .as_ref()
            .cloned()
            .unwrap_or_else(|| format_ident!("{}", ix));

        let Some(type_to_provide) = fields_to_provide.get(&fieldname) else {
            continue;
        };

        let field_struct = format_ident!("__Field__{}", fieldname);
        field_names.push(quote! {
            // access the field #fieldname
            struct #field_struct {}
            impl ::errful::protocol::ErrField for #field_struct {
                type T = #type_to_provide;
            }
        });

        provides.push(quote! {
            .provide_ref(::errful::protocol::Field::<#field_struct, #type_to_provide>::new(
                self.#fieldname.borrow()
                //(&&&::errful::RefWrapper(&self.#fieldname)).maybe_deref()
            ))
        });
    }

    Ok((field_names, provides))
}

fn generate_source(
    data: &ast::Data<(), StructFields>,
) -> darling::Result<(BTreeMap<Ident, TokenStream>, TokenStream)> {
    let fields = data.as_ref().take_struct().expect("struct").fields;

    let mut sources = Vec::new();
    let mut fields_to_provide = BTreeMap::new();

    for (ix, field) in fields.iter().enumerate() {
        let fieldname = field
            .ident
            .as_ref()
            .cloned()
            .unwrap_or_else(|| format_ident!("{}", ix));

        if field.source
            || field
                .ident
                .as_ref()
                .map(|i| i == "source")
                .unwrap_or_default()
        {
            fields_to_provide.insert(
                fieldname.clone(),
                quote! { dyn ::core::error::Error + 'static },
            );

            if is_optional(&field.ty) {
                sources.push(quote! {
                    if let Some(source) = self.#fieldname {
                        use std::borrow::Borrow;
                        return Some(source.borrow());
                    }
                });
            } else {
                sources.push(quote! {
                    use std::borrow::Borrow;
                    return Some(self.#fieldname.borrow());
                });
            }
        }
    }

    let result = quote! {
        #[allow(unreachable_code)]
        fn source(&self) -> Option<&(dyn ::core::error::Error + 'static)> {
            #(#sources)*
            None
        }
    };

    Ok((fields_to_provide, result))
}

fn find_labels(
    data: &ast::Data<(), StructFields>,
) -> darling::Result<(BTreeMap<Ident, TokenStream>, Option<TokenStream>)> {
    let fields = data.as_ref().take_struct().expect("struct").fields;

    let mut fields_to_provide = BTreeMap::new();
    let mut labels = Vec::new();

    for (ix, field) in fields.iter().enumerate() {
        let Some(label) = &field.label else { continue };

        let fieldname = name_for_field((ix, field));

        let source_id = match &field.source_id {
            Some(id) => quote! { Some(#id) },
            None => quote! { None },
        };

        let value = match label {
            LabelTarget::Field(ident) => {
                fields_to_provide
                    .insert(ident.clone(), quote! { dyn ::core::error::Error + 'static });

                let field_struct = format_ident!("__Field__{}", ident);
                quote! {
                   ::errful::protocol::RawLabel::new_error(
                       #source_id,
                       Box::new(#field_struct {}),
                       self.#fieldname)
                }
            }
            LabelTarget::Literal(label) => {
                quote! {
                    ::errful::protocol::RawLabel::new_literal(#source_id, #label, self.#fieldname)
                }
            }
        };

        labels.push(value);
    }

    if labels.is_empty() {
        return Ok((fields_to_provide, None));
    }

    let result = Some(quote! {
        .provide_value_with::<::std::vec::Vec<::errful::protocol::RawLabel>>(|| {
            vec![
                #(#labels),*
            ]
        })
    });

    Ok((fields_to_provide, result))
}

fn find_source_code(data: &ast::Data<(), StructFields>) -> darling::Result<Option<TokenStream>> {
    let fields = data.as_ref().take_struct().expect("struct").fields;

    // TODO error if specified more than once

    for (ix, field) in fields.iter().enumerate() {
        if field.source_code {
            let fieldname = name_for_field((ix, field));
            return Ok(Some(quote! {
                .provide_ref(::errful::protocol::SourceCode::new(&self.#fieldname))
            }));
        }
    }

    Ok(None)
}

fn name_for_field(field: (usize, &StructFields)) -> TokenStream {
    if let Some(ident) = &field.1.ident {
        quote! { #ident }
    } else {
        let ix = field.0;
        quote! { #ix }
    }
}

fn is_optional(ty: &Type) -> bool {
    // TODO: bad
    // instead use a trait for source_code
    // maybe also castaway
    let Type::Path(p) = ty else { return false };
    let Some(last) = p.path.segments.last() else {
        return false;
    };

    last.ident == "Option"
}
