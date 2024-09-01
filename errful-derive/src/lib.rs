use darling::{ast, FromDeriveInput, FromField, FromMeta};
use proc_macro2::TokenStream;
use quote::{format_ident, quote};
use syn::{parse_macro_input, DeriveInput, Type};

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

    let source_method = match generate_source(&opts.data) {
        Ok(r) => r,
        Err(e) => return e.write_errors().into(),
    };

    let labels = match generate_labels(&opts.data) {
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

    // exit code is provided
    if let Some(exit_code) = opts.exit_code {
        provisions.push(quote! {
            .provide_value(::std::process::ExitCode::from(#exit_code))
        });
    };

    // TODO: backtrace is provided

    let url = opts.url.map(|url| {
        quote! {
            fn url(&self) -> Option<::url::Url> {
                Some(::errful::protocol::url!(#url))
            }
        }
    });

    let code = opts.code.map(|code| {
        quote! {
            fn code(&self) -> Option<&'static str> {
                Some(#code)
            }
        }
    });

    let severity = opts.severity.map(|severity| {
        quote! {
            fn severity(&self) -> Option<&dyn ::errful::PrintableSeverity> {
                Some(&#severity)
            }
        }
    });

    let source_code = source_code.map(|source_code| {
        quote! {
            fn source_code(&self) -> Option<&str> {
                Some(#source_code)
            }
        }
    });

    let output = quote! {
        #[automatically_derived]
        impl ::core::error::Error for #ident {
            #source_method

            fn provide<'a>(&'a self, __request: &mut ::core::error::Request<'a>) {
                use ::std::borrow::Borrow;

                __request
                    .provide_ref::<dyn ::errful::Errful>(self)
                    #(#provisions)*
                ;
            }
        }

        #[automatically_derived]
        impl ::errful::Errful for #ident {
            #code
            #labels
            #severity
            #source_code
            #url
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

fn generate_source(data: &ast::Data<(), StructFields>) -> darling::Result<TokenStream> {
    let fields = data.as_ref().take_struct().expect("struct").fields;

    let mut sources = Vec::new();

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

    Ok(result)
}

fn generate_labels(data: &ast::Data<(), StructFields>) -> darling::Result<Option<TokenStream>> {
    let fields = data.as_ref().take_struct().expect("struct").fields;

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
                quote! {
                   ::errful::protocol::Label::new_error(
                       #source_id,
                       self.#ident.borrow(),
                       self.#fieldname)
                }
            }
            LabelTarget::Literal(label) => {
                quote! {
                    ::errful::protocol::Label::new_literal(#source_id, #label, self.#fieldname)
                }
            }
        };

        labels.push(value);
    }

    if labels.is_empty() {
        return Ok(None);
    }

    let result = Some(quote! {
        fn labels(&self) -> Option<::std::vec::Vec<::errful::protocol::Label>> {
            use ::std::borrow::Borrow;
            Some(vec![
                #(#labels),*
            ])
        }
    });

    Ok(result)
}

fn find_source_code(data: &ast::Data<(), StructFields>) -> darling::Result<Option<TokenStream>> {
    let fields = data.as_ref().take_struct().expect("struct").fields;

    // TODO error if specified more than once

    for (ix, field) in fields.iter().enumerate() {
        if field.source_code {
            let fieldname = name_for_field((ix, field));
            return Ok(Some(quote! {
                &self.#fieldname
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
