use proc_macro::TokenStream;
use quote::quote;
use syn::{parse_macro_input, Data, DeriveInput, Fields, Lit, Meta, MetaNameValue};

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
    derive_capability(input, CapabilityType::Query)
}

#[proc_macro_derive(Mutation, attributes(mutation))]
pub fn derive_mutation(input: TokenStream) -> TokenStream {
    derive_capability(input, CapabilityType::Mutation)
}

#[derive(Clone, Copy)]
enum CapabilityType {
    Query,
    Mutation,
}

impl CapabilityType {
    fn attribute_name(&self) -> &'static str {
        match self {
            CapabilityType::Query => "query",
            CapabilityType::Mutation => "mutation",
        }
    }

    fn default_ok_type(&self) -> proc_macro2::TokenStream {
        match self {
            CapabilityType::Query => quote! { String },
            CapabilityType::Mutation => quote! { () },
        }
    }

    fn trait_path(&self) -> proc_macro2::TokenStream {
        match self {
            CapabilityType::Query => quote! { ::dioxus_query::query::QueryCapability },
            CapabilityType::Mutation => quote! { ::dioxus_query::mutation::MutationCapability },
        }
    }

    fn additional_methods(&self) -> proc_macro2::TokenStream {
        match self {
            CapabilityType::Query => quote! {},
            CapabilityType::Mutation => quote! {
                // Add forwarding for on_settled
                async fn on_settled(&self, keys: &Self::Keys, result: &Result<Self::Ok, Self::Err>) {
                    // This assumes the user has an inherent method `on_settled` with the same signature.
                    // If not, this will cause a compile error, which is a way to enforce the contract.
                    // A more advanced macro could check for the method's existence and provide a true default if not found.
                    self.on_settled(keys, result).await
                }
            },
        }
    }
}

fn derive_capability(input: TokenStream, capability_type: CapabilityType) -> TokenStream {
    let derive_input = parse_macro_input!(input as DeriveInput);
    let (name, fields) = match extract_name_and_fields(&derive_input) {
        Ok(val) => val,
        Err(err) => return err.to_compile_error().into(),
    };

    let DeriveAttributeValues {
        key_type,
        ok_type,
        err_type,
    } = match extract_attribute_values(
        &derive_input.attrs,
        capability_type.attribute_name(),
        capability_type.default_ok_type(),
    ) {
        Ok(val) => val,
        Err(err) => return err.to_compile_error().into(),
    };

    let (_, clone_impl) = generate_clone_implementation(&name, fields);
    let trait_path = capability_type.trait_path();
    let additional_methods = capability_type.additional_methods();
    let common_trait_impls = generate_common_trait_impls(&name);

    let expanded = quote! {
        impl #trait_path for #name {
            type Ok = #ok_type;
            type Err = #err_type;
            type Keys = #key_type;

            async fn run(&self, key: &Self::Keys) -> Result<Self::Ok, Self::Err> {
                self.run(key).await
            }

            #additional_methods
        }

        #clone_impl
        #common_trait_impls
    };

    TokenStream::from(expanded)
}

/// Generate common trait implementations (PartialEq, Eq, Hash) for both Query and Mutation
fn generate_common_trait_impls(name: &syn::Ident) -> proc_macro2::TokenStream {
    quote! {
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
    }
}

fn extract_name_and_fields(
    input: &DeriveInput,
) -> Result<
    (
        &syn::Ident,
        Option<Fields>, // Changed to return Fields directly
    ),
    syn::Error,
> {
    let name = &input.ident;
    match &input.data {
        Data::Struct(data) => match &data.fields {
            Fields::Named(fields) => Ok((name, Some(Fields::Named(fields.clone())))),
            Fields::Unnamed(fields) => Ok((name, Some(Fields::Unnamed(fields.clone())))), // Handle unnamed fields
            Fields::Unit => Ok((name, None)),
        },
        _ => Err(syn::Error::new_spanned(
            input,
            "This derive macro only supports structs",
        )),
    }
}

fn generate_clone_implementation(
    name: &syn::Ident,
    fields_option: Option<Fields>,
) -> (proc_macro2::TokenStream, proc_macro2::TokenStream) {
    match fields_option {
        Some(Fields::Named(fields)) => {
            let field_clones = fields.named.iter().map(|field| {
                let field_name = &field.ident;
                quote! { #field_name: self.#field_name.clone() }
            });
            let captured_fields = quote! { #(#field_clones),* };
            let clone_impl = quote! {
                impl ::std::clone::Clone for #name {
                    fn clone(&self) -> Self {
                        Self {
                            #captured_fields
                        }
                    }
                }
            };
            (captured_fields, clone_impl)
        }
        Some(Fields::Unnamed(fields)) => {
            let field_clones = fields.unnamed.iter().enumerate().map(|(i, _field)| {
                let index = syn::Index::from(i);
                quote! { self.#index.clone() }
            });
            let captured_fields = quote! { #(#field_clones),* };
            let clone_impl = quote! {
                impl ::std::clone::Clone for #name {
                    fn clone(&self) -> Self {
                        Self(#captured_fields)
                    }
                }
            };
            (captured_fields, clone_impl)
        }
        Some(Fields::Unit) | None => {
            let captured_fields = quote! {};
            let clone_impl = quote! {
                impl ::std::clone::Clone for #name {
                    fn clone(&self) -> Self {
                        Self
                    }
                }
            };
            (captured_fields, clone_impl)
        }
    }
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
                                Some("key") => key_type = parse_type_value(value)?,
                                Some("ok") => ok_type = parse_type_value(value)?,
                                Some("err") => err_type = parse_type_value(value)?,
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

/// Parse a type value from either a path expression or a string literal
fn parse_type_value(value: syn::Expr) -> Result<proc_macro2::TokenStream, syn::Error> {
    match value {
        syn::Expr::Path(expr_path) => Ok(quote! { #expr_path }),
        syn::Expr::Tuple(tuple_expr) => {
            // Handle unit type () and tuple types
            Ok(quote! { #tuple_expr })
        }
        syn::Expr::Lit(lit) => {
            if let Lit::Str(lit_str) = lit.lit {
                let type_ident: syn::Type = syn::parse_str(&lit_str.value()).map_err(|e| {
                    syn::Error::new_spanned(lit_str, format!("Failed to parse type string: {}", e))
                })?;
                Ok(quote! { #type_ident })
            } else {
                Err(syn::Error::new_spanned(
                    lit,
                    "Expected string literal for type",
                ))
            }
        }
        _ => Err(syn::Error::new_spanned(value, "Expected path, tuple, or string literal")),
    }
}
