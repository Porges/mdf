use darling::{FromAttributes, FromDeriveInput};
use proc_macro2::TokenStream;
use quote::quote;
use syn::{parse_macro_input, Data, DataStruct, DeriveInput, Field};

#[derive(FromDeriveInput, Default)]
#[darling(default, attributes(error))]
struct Opts {
    display: Option<String>,
    exit_code: Option<u8>,
}

#[proc_macro_derive(Error, attributes(error))]
pub fn derive_errful(input: proc_macro::TokenStream) -> proc_macro::TokenStream {
    let input = parse_macro_input!(input);
    let opts = match Opts::from_derive_input(&input) {
        Ok(r) => r,
        Err(e) => return e.write_errors().into(),
    };

    let DeriveInput { ident, data, .. } = input;

    let source_method = match generate_source(&data) {
        Ok(r) => r,
        Err(e) => return e.write_errors().into(),
    };

    let display_impl = opts.display.map(|display| {
        quote! {
            impl ::core::fmt::Display for #ident {
                fn fmt(&self, __formatter: &mut ::core::fmt::Formatter<'_>) -> ::core::fmt::Result {
                    write!(__formatter, #display)
                }
            }
        }
    });

    let mut provisions = Vec::new();

    if let Some(exit_code) = opts.exit_code {
        provisions.push(quote! {
            .provide_value(ExitCode::from(#exit_code))
        });
    };

    let output = quote! {
        impl ::core::error::Error for #ident {
            #source_method

            fn provide<'a>(&'a self, __request: &mut ::core::error::Request<'a>) {
                __request
                    #(#provisions)*
                ;
            }
        }

        #display_impl
    };

    output.into()
}

fn generate_source(data: &Data) -> darling::Result<TokenStream> {
    match data {
        Data::Struct(s) => source_for_struct(s),
        Data::Enum(_) => todo!(),
        Data::Union(_) => todo!(),
    }
}

#[derive(FromAttributes, Default)]
#[darling(default, attributes(error))]
struct FieldOpts {
    source: bool,
}

fn source_for_struct(s: &DataStruct) -> darling::Result<TokenStream> {
    let mut errors = darling::Error::accumulator();

    let mut sources = Vec::new();
    for field in &s.fields {
        let Some(opts) = errors.handle(FieldOpts::from_attributes(&field.attrs)) else {
            continue;
        };

        if opts.source {
            sources.push(field.ident.as_ref().unwrap());
        } else if let Some(ident) = &field.ident {
            if ident == "source" {
                sources.push(field.ident.as_ref().unwrap());
            }
        }
    }

    errors.finish()?;

    Ok(quote! {
        fn source(&self) -> Option<&(dyn ::core::error::Error + 'static)> {
            #(
                if let Some(source) = self.#sources.source() {
                    return Some(source);
                }
            )*
            None
        }
    })
}
