use proc_macro::TokenStream;
use quote::quote;
use syn::{parse_macro_input, Attribute, Data, DeriveInput, Field, Fields};

/// Derive macro for automatically implementing QueryCapability
///
/// # Example
/// ```rust
/// #[derive(Query)]
/// struct GetUserName {
///     client: FancyClient,
/// }
///
/// impl GetUserName {
///     async fn run(&self, user_id: &usize) -> Result<String, ()> {
///         // Your async logic here
///     }
/// }
/// ```
#[proc_macro_derive(Query, attributes(query))]
pub fn derive_query(input: TokenStream) -> TokenStream {
    let input = parse_macro_input!(input as DeriveInput);
    let name = &input.ident;

    // Extract the struct fields to understand the captured context
    let fields = match &input.data {
        Data::Struct(data) => match &data.fields {
            Fields::Named(fields) => &fields.named,
            _ => {
                return syn::Error::new_spanned(
                    &input,
                    "Query derive macro only supports structs with named fields",
                )
                .to_compile_error()
                .into();
            }
        },
        _ => {
            return syn::Error::new_spanned(&input, "Query derive macro only supports structs")
                .to_compile_error()
                .into();
        }
    };

    // Find the key type from attributes or default to usize
    let key_type = extract_key_type(&input.attrs).unwrap_or_else(|| quote! { usize });
    let ok_type = extract_ok_type(&input.attrs).unwrap_or_else(|| quote! { String });
    let err_type = extract_err_type(&input.attrs).unwrap_or_else(|| quote! { () });

    // Generate the captured fields initialization
    let captured_fields = generate_captured_fields(fields);

    let expanded = quote! {
        impl ::dioxus_query::query::QueryCapability for #name {
            type Ok = #ok_type;
            type Err = #err_type;
            type Keys = #key_type;

            async fn run(&self, key: &Self::Keys) -> Result<Self::Ok, Self::Err> {
                self.run(key).await
            }
        }

        impl ::std::clone::Clone for #name {
            fn clone(&self) -> Self {
                Self {
                    #captured_fields
                }
            }
        }

        impl ::std::cmp::PartialEq for #name {
            fn eq(&self, other: &Self) -> bool {
                true // For simplicity, consider all instances equal
            }
        }

        impl ::std::cmp::Eq for #name {}

        impl ::std::hash::Hash for #name {
            fn hash<H: ::std::hash::Hasher>(&self, state: &mut H) {
                stringify!(#name).hash(state);
            }
        }
    };

    TokenStream::from(expanded)
}

fn extract_key_type(attrs: &[Attribute]) -> Option<proc_macro2::TokenStream> {
    for attr in attrs {
        if attr.path().is_ident("query") {
            if let Ok(meta) = attr.parse_args::<syn::Meta>() {
                if let syn::Meta::NameValue(nv) = meta {
                    if nv.path.is_ident("key") {
                        if let syn::Expr::Path(path) = nv.value {
                            return Some(quote! { #path });
                        }
                    }
                }
            }
        }
    }
    None
}

fn extract_ok_type(attrs: &[Attribute]) -> Option<proc_macro2::TokenStream> {
    for attr in attrs {
        if attr.path().is_ident("query") {
            if let Ok(meta) = attr.parse_args::<syn::Meta>() {
                if let syn::Meta::NameValue(nv) = meta {
                    if nv.path.is_ident("ok") {
                        if let syn::Expr::Path(path) = nv.value {
                            return Some(quote! { #path });
                        }
                    }
                }
            }
        }
    }
    None
}

fn extract_err_type(attrs: &[Attribute]) -> Option<proc_macro2::TokenStream> {
    for attr in attrs {
        if attr.path().is_ident("query") {
            if let Ok(meta) = attr.parse_args::<syn::Meta>() {
                if let syn::Meta::NameValue(nv) = meta {
                    if nv.path.is_ident("err") {
                        if let syn::Expr::Path(path) = nv.value {
                            return Some(quote! { #path });
                        }
                    }
                }
            }
        }
    }
    None
}

fn generate_captured_fields(
    fields: &syn::punctuated::Punctuated<Field, syn::token::Comma>,
) -> proc_macro2::TokenStream {
    let field_clones = fields.iter().map(|field| {
        let field_name = &field.ident;
        quote! { #field_name: self.#field_name.clone() }
    });

    quote! { #(#field_clones),* }
}
