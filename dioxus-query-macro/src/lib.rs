use proc_macro::TokenStream;
use quote::quote;
use syn::{parse_macro_input, Data, DeriveInput, Field, Fields, Lit, Meta, MetaNameValue};

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
    let derive_input = parse_macro_input!(input as DeriveInput);
    let (name, fields) = match extract_name_and_fields(&derive_input) {
        Ok(val) => val,
        Err(err) => return err.to_compile_error().into(),
    };

    let DeriveAttributeValues {
        key_type,
        ok_type,
        err_type,
    } = match extract_attribute_values(&derive_input.attrs, "query", quote! {String}) {
        Ok(val) => val,
        Err(err) => return err.to_compile_error().into(),
    };

    let captured_fields = generate_captured_fields(&fields);

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

fn generate_captured_fields(
    fields: &syn::punctuated::Punctuated<Field, syn::token::Comma>,
) -> proc_macro2::TokenStream {
    let field_clones = fields.iter().map(|field| {
        let field_name = &field.ident;
        quote! { #field_name: self.#field_name.clone() }
    });

    quote! { #(#field_clones),* }
}

#[proc_macro_derive(Mutation, attributes(mutation))]
pub fn derive_mutation(input: TokenStream) -> TokenStream {
    let derive_input = parse_macro_input!(input as DeriveInput);
    let (name, fields) = match extract_name_and_fields(&derive_input) {
        Ok(val) => val,
        Err(err) => return err.to_compile_error().into(),
    };

    let DeriveAttributeValues {
        key_type,
        ok_type,
        err_type,
    } = match extract_attribute_values(&derive_input.attrs, "mutation", quote! {()}) {
        Ok(val) => val,
        Err(err) => return err.to_compile_error().into(),
    };

    let captured_fields = generate_captured_fields(&fields);

    let expanded = quote! {
        impl ::dioxus_query::mutation::MutationCapability for #name {
            type Ok = #ok_type;
            type Err = #err_type;
            type Keys = #key_type;

            async fn run(&self, key: &Self::Keys) -> Result<Self::Ok, Self::Err> {
                self.run(key).await
            }

            // Add forwarding for on_settled
            async fn on_settled(&self, keys: &Self::Keys, result: &Result<Self::Ok, Self::Err>) {
                // This assumes the user has an inherent method `on_settled` with the same signature.
                // If not, this will cause a compile error, which is a way to enforce the contract.
                // A more advanced macro could check for the method's existence and provide a true default if not found.
                self.on_settled(keys, result).await
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
                // TODO: Compare fields if they are PartialEq
                // For now, to ensure proper re-rendering on state change in captured values,
                // we should compare the captured fields if possible.
                // However, the original Query derive had `true`, so we'll start there.
                // This might need refinement based on how Captured<T>'s PartialEq works.
                true
            }
        }

        impl ::std::cmp::Eq for #name {}

        impl ::std::hash::Hash for #name {
            fn hash<H: ::std::hash::Hasher>(&self, state: &mut H) {
                stringify!(#name).hash(state);
                // TODO: Hash fields if they are Hash
            }
        }
    };

    TokenStream::from(expanded)
}

// Helper function to extract struct name and fields
fn extract_name_and_fields(
    input: &DeriveInput,
) -> Result<
    (
        &syn::Ident,
        &syn::punctuated::Punctuated<Field, syn::token::Comma>,
    ),
    syn::Error,
> {
    let name = &input.ident;
    let fields = match &input.data {
        Data::Struct(data) => match &data.fields {
            Fields::Named(fields) => &fields.named,
            _ => {
                return Err(syn::Error::new_spanned(
                    input,
                    "This derive macro only supports structs with named fields",
                ));
            }
        },
        _ => {
            return Err(syn::Error::new_spanned(
                input,
                "This derive macro only supports structs",
            ));
        }
    };
    Ok((name, fields))
}

struct DeriveAttributeValues {
    key_type: proc_macro2::TokenStream,
    ok_type: proc_macro2::TokenStream,
    err_type: proc_macro2::TokenStream,
}

// Helper function to extract attribute values (key, ok, err)
fn extract_attribute_values(
    attrs: &[syn::Attribute],
    attribute_name: &str, // "query" or "mutation"
    default_ok_type: proc_macro2::TokenStream,
) -> Result<DeriveAttributeValues, syn::Error> {
    let mut key_type = quote! { usize };
    let mut ok_type = default_ok_type;
    let mut err_type = quote! { () };

    for attr in attrs {
        if attr.path().is_ident(attribute_name) {
            match attr.parse_args_with(
                syn::punctuated::Punctuated::<Meta, syn::Token![,]>::parse_terminated,
            ) {
                Ok(meta_list) => {
                    for meta_item in meta_list {
                        if let Meta::NameValue(MetaNameValue { path, value, .. }) = meta_item {
                            let ident_name = path.get_ident().map(|i| i.to_string());
                            match ident_name.as_deref() {
                                Some("key") => {
                                    if let syn::Expr::Path(expr_path) = value {
                                        key_type = quote! { #expr_path };
                                    } else if let syn::Expr::Lit(lit) = value {
                                        if let Lit::Str(lit_str) = lit.lit {
                                            let type_ident: syn::Type =
                                                syn::parse_str(&lit_str.value()).map_err(|e| {
                                                    syn::Error::new_spanned(
                                                        lit_str,
                                                        format!(
                                                            "Failed to parse key type string: {}",
                                                            e
                                                        ),
                                                    )
                                                })?;
                                            key_type = quote! { #type_ident };
                                        }
                                    }
                                }
                                Some("ok") => {
                                    if let syn::Expr::Path(expr_path) = value {
                                        ok_type = quote! { #expr_path };
                                    } else if let syn::Expr::Lit(lit) = value {
                                        if let Lit::Str(lit_str) = lit.lit {
                                            let type_ident: syn::Type =
                                                syn::parse_str(&lit_str.value()).map_err(|e| {
                                                    syn::Error::new_spanned(
                                                        lit_str,
                                                        format!(
                                                            "Failed to parse ok type string: {}",
                                                            e
                                                        ),
                                                    )
                                                })?;
                                            ok_type = quote! { #type_ident };
                                        }
                                    }
                                }
                                Some("err") => {
                                    if let syn::Expr::Path(expr_path) = value {
                                        err_type = quote! { #expr_path };
                                    } else if let syn::Expr::Lit(lit) = value {
                                        if let Lit::Str(lit_str) = lit.lit {
                                            let type_ident: syn::Type =
                                                syn::parse_str(&lit_str.value()).map_err(|e| {
                                                    syn::Error::new_spanned(
                                                        lit_str,
                                                        format!(
                                                            "Failed to parse err type string: {}",
                                                            e
                                                        ),
                                                    )
                                                })?;
                                            err_type = quote! { #type_ident };
                                        }
                                    }
                                }
                                _ => {}
                            }
                        }
                    }
                }
                Err(e) => return Err(e),
            }
        }
    }
    Ok(DeriveAttributeValues {
        key_type,
        ok_type,
        err_type,
    })
}
