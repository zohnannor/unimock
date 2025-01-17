use std::collections::HashSet;

use quote::quote;
use syn::spanned::Spanned;
use syn::visit_mut::VisitMut;

use super::attr::MockApi;
use super::output;
use super::util::{GenericParamsWithBounds, IsTypeGeneric};
use super::Attr;

use crate::doc;

pub struct MockMethod<'t> {
    pub method: &'t syn::TraitItemMethod,
    pub adapted_sig: syn::Signature,
    pub is_type_generic: IsTypeGeneric,
    pub generic_params_with_bounds: GenericParamsWithBounds,
    pub impl_trait_idents: HashSet<String>,
    pub non_generic_mock_entry_ident: Option<syn::Ident>,
    pub mock_fn_ident: syn::Ident,
    pub mock_fn_name: syn::LitStr,
    pub output_structure: output::OutputStructure,
    mirrored_attr_indexes: Vec<usize>,
}

impl<'t> MockMethod<'t> {
    pub fn mock_fn_path(&self, attr: &Attr) -> proc_macro2::TokenStream {
        let mock_fn_ident = &self.mock_fn_ident;

        match (&attr.mock_api, &self.non_generic_mock_entry_ident) {
            (MockApi::MockMod(ident), None) => {
                quote! { #ident::#mock_fn_ident }
            }
            _ => quote! { #mock_fn_ident },
        }
    }

    pub fn mirrored_attrs(&self) -> impl Iterator<Item = &'_ syn::Attribute> {
        self.mirrored_attr_indexes
            .iter()
            .map(|index| &self.method.attrs[*index])
    }

    pub fn inputs_destructuring(&self) -> InputsDestructuring {
        InputsDestructuring { method: self }
    }

    pub fn generate_debug_inputs_fn(&self, attr: &Attr) -> proc_macro2::TokenStream {
        let prefix = &attr.prefix;
        let first_param = self
            .method
            .sig
            .inputs
            .iter()
            .find(|fn_arg| matches!(fn_arg, syn::FnArg::Typed(_)));

        let body = if first_param.is_some() {
            let inputs_try_debug_exprs = self.inputs_try_debug_exprs();
            quote! {
                use #prefix::macro_api::{ProperDebug, NoDebug};
                #prefix::macro_api::format_inputs(&[#(#inputs_try_debug_exprs),*])
            }
        } else {
            quote! {
                #prefix::macro_api::format_inputs(&[])
            }
        };

        let inputs_destructuring = self.inputs_destructuring();

        quote! {
            fn debug_inputs((#inputs_destructuring): &Self::Inputs<'_>) -> String {
                #body
            }
        }
    }

    pub fn inputs_try_debug_exprs(&self) -> impl Iterator<Item = proc_macro2::TokenStream> + 't {
        self.method
            .sig
            .inputs
            .iter()
            .enumerate()
            .filter_map(|(index, fn_arg)| match fn_arg {
                syn::FnArg::Receiver(_) => None,
                syn::FnArg::Typed(pat_type) => match (index, pat_type.pat.as_ref()) {
                    (0, syn::Pat::Ident(pat_ident)) if pat_ident.ident == "self" => None,
                    (_, syn::Pat::Ident(pat_ident)) => {
                        Some(try_debug_expr(pat_ident, &pat_type.ty))
                    }
                    _ => Some(
                        syn::Error::new(pat_type.span(), "Unprocessable argument")
                            .to_compile_error(),
                    ),
                },
            })
    }

    pub fn mockfn_doc_attrs(&self, trait_ident: &syn::Ident) -> Vec<proc_macro2::TokenStream> {
        let sig_string = doc::signature_documentation(&self.method.sig, doc::SkipReceiver(true));

        let doc_string = if self.non_generic_mock_entry_ident.is_some() {
            format!("Generic mock interface for `{trait_ident}::{sig_string}`. Get a MockFn instance by calling `with_types()`.")
        } else {
            format!("MockFn for `{trait_ident}::{sig_string}`.")
        };

        let doc_lit = syn::LitStr::new(&doc_string, proc_macro2::Span::call_site());

        vec![quote! {
            #[doc = #doc_lit]
        }]
    }
}

pub fn extract_methods<'s>(
    prefix: &syn::Path,
    item_trait: &'s syn::ItemTrait,
    is_trait_type_generic: IsTypeGeneric,
    attr: &Attr,
) -> syn::Result<Vec<Option<MockMethod<'s>>>> {
    item_trait
        .items
        .iter()
        .filter_map(|item| match item {
            syn::TraitItem::Method(method) => Some(method),
            _ => None,
        })
        .enumerate()
        .map(|(index, method)| {
            match determine_mockable(method) {
                Mockable::Yes => {}
                Mockable::Skip => return Ok(None),
                Mockable::Err(err) => return Err(err),
            };

            let mock_fn_name = syn::LitStr::new(
                &format!("{}::{}", &item_trait.ident, method.sig.ident),
                item_trait.ident.span(),
            );

            let mut adapted_sig = method.sig.clone();
            let adapt_sig_result = adapt_sig(&mut adapted_sig);
            let is_type_generic =
                IsTypeGeneric(is_trait_type_generic.0 || adapt_sig_result.is_type_generic.0);

            let output_structure =
                output::determine_output_structure(prefix, item_trait, &adapted_sig);

            let mirrored_attr_indexes = method
                .attrs
                .iter()
                .enumerate()
                .filter_map(|(index, attr)| {
                    if attr.path.is_ident("cfg") {
                        Some(index)
                    } else {
                        None
                    }
                })
                .collect();

            let generic_params_with_bounds =
                GenericParamsWithBounds::new(&adapted_sig.generics, false);

            Ok(Some(MockMethod {
                method,
                adapted_sig,
                is_type_generic: adapt_sig_result.is_type_generic,
                generic_params_with_bounds,
                impl_trait_idents: adapt_sig_result.impl_trait_idents,
                non_generic_mock_entry_ident: if is_type_generic.0 {
                    Some(generate_mock_fn_ident(
                        method,
                        index,
                        IsTypeGeneric(false),
                        attr,
                    )?)
                } else {
                    None
                },
                mock_fn_ident: generate_mock_fn_ident(method, index, is_type_generic, attr)?,
                mock_fn_name,
                output_structure,
                mirrored_attr_indexes,
            }))
        })
        .collect()
}

enum Mockable {
    Yes,
    Skip,
    Err(syn::Error),
}

fn determine_mockable(method: &syn::TraitItemMethod) -> Mockable {
    fn is_receiver(first_fn_arg: Option<&syn::FnArg>) -> bool {
        match first_fn_arg {
            None => false,
            Some(syn::FnArg::Receiver(_)) => true,
            Some(syn::FnArg::Typed(pat_type)) => match pat_type.pat.as_ref() {
                syn::Pat::Ident(pat_ident) => pat_ident.ident == "self",
                // Probably not mockable, but try, then generate compile error later:
                _ => true,
            },
        }
    }

    let first_fn_arg = method.sig.inputs.first();

    if is_receiver(first_fn_arg) {
        Mockable::Yes
    } else if method.default.is_some() {
        // method is provided, skip
        Mockable::Skip
    } else {
        Mockable::Err(syn::Error::new(
            method.sig.ident.span(),
            "Method has no self receiver and no default body. Mocking will not work.",
        ))
    }
}

fn generate_mock_fn_ident(
    method: &syn::TraitItemMethod,
    method_index: usize,
    generic: IsTypeGeneric,
    attr: &Attr,
) -> syn::Result<syn::Ident> {
    if generic.0 {
        match &attr.mock_api {
            MockApi::Flattened(flat_mocks) => Ok(quote::format_ident!(
                "__Generic{}",
                flat_mocks.get_mock_ident(method_index)?
            )),
            MockApi::MockMod(_) | MockApi::Hidden => {
                Ok(quote::format_ident!("__Generic{}", method.sig.ident))
            }
        }
    } else {
        match &attr.mock_api {
            MockApi::Flattened(flat_mocks) => Ok(flat_mocks.get_mock_ident(method_index)?.clone()),
            MockApi::MockMod(_) => Ok(method.sig.ident.clone()),
            MockApi::Hidden => Ok(quote::format_ident!("UnimockHidden__{}", method.sig.ident)),
        }
    }
}

fn try_debug_expr(pat_ident: &syn::PatIdent, ty: &syn::Type) -> proc_macro2::TokenStream {
    fn count_references(ty: &syn::Type) -> usize {
        match ty {
            syn::Type::Reference(type_reference) => 1 + count_references(&type_reference.elem),
            _ => 0,
        }
    }

    let ref_count = count_references(ty);
    let ident = &pat_ident.ident;

    if ref_count > 0 {
        // insert as many * as there are references
        let derefs = (0..ref_count).map(|_| quote! { * });

        quote! {
            (#(#derefs)* #ident).unimock_try_debug()
        }
    } else {
        quote! {
            #ident.unimock_try_debug()
        }
    }
}

pub struct InputsDestructuring<'t> {
    method: &'t MockMethod<'t>,
}

impl<'t> quote::ToTokens for InputsDestructuring<'t> {
    fn to_tokens(&self, tokens: &mut proc_macro2::TokenStream) {
        let inputs = &self.method.method.sig.inputs;
        if inputs.is_empty() {
            return;
        }

        let last_index = self.method.method.sig.inputs.len() - 1;
        for (index, pair) in self.method.method.sig.inputs.pairs().enumerate() {
            if let syn::FnArg::Typed(pat_type) = pair.value() {
                match (index, pat_type.pat.as_ref()) {
                    (0, syn::Pat::Ident(pat_ident)) if pat_ident.ident == "self" => {}
                    (_, syn::Pat::Ident(pat_ident)) => {
                        pat_ident.to_tokens(tokens);
                    }
                    _ => {
                        syn::Error::new(pat_type.span(), "Unprocessable argument")
                            .to_compile_error()
                            .to_tokens(tokens);
                    }
                };
                if index < last_index {
                    syn::token::Comma::default().to_tokens(tokens);
                }
            }
        }
    }
}

struct AdaptSigResult {
    is_type_generic: IsTypeGeneric,
    impl_trait_idents: HashSet<String>,
}

// TODO: Rewrite impl Trait to normal param
fn adapt_sig(sig: &mut syn::Signature) -> AdaptSigResult {
    let mut generics: syn::Generics = Default::default();
    let mut impl_trait_idents: HashSet<String> = HashSet::new();
    std::mem::swap(&mut sig.generics, &mut generics);

    struct ImplTraitConverter<'s> {
        generics: &'s mut syn::Generics,
        impl_trait_idents: &'s mut HashSet<String>,
        impl_trait_count: usize,
    }

    impl<'s> syn::visit_mut::VisitMut for ImplTraitConverter<'s> {
        fn visit_type_mut(&mut self, ty: &mut syn::Type) {
            if let syn::Type::ImplTrait(impl_trait) = ty {
                let generic_ident = quote::format_ident!("ImplTrait{}", self.impl_trait_count);

                self.impl_trait_idents.insert(generic_ident.to_string());

                self.generics
                    .params
                    .push(syn::GenericParam::Type(syn::TypeParam {
                        attrs: vec![],
                        ident: generic_ident.clone(),
                        colon_token: Some(syn::token::Colon::default()),
                        bounds: impl_trait.bounds.clone(),
                        eq_token: None,
                        default: None,
                    }));

                *ty = syn::parse_quote!( #generic_ident );

                self.impl_trait_count += 1;
            }
        }
    }

    let mut converter = ImplTraitConverter {
        generics: &mut generics,
        impl_trait_idents: &mut impl_trait_idents,
        impl_trait_count: 0,
    };
    converter.visit_signature_mut(sig);

    // write back generics
    std::mem::swap(&mut generics, &mut sig.generics);

    let mut is_type_generic = IsTypeGeneric(false);
    for generic_param in &sig.generics.params {
        if matches!(generic_param, syn::GenericParam::Type(_)) {
            is_type_generic.0 = true;
        }
    }

    AdaptSigResult {
        is_type_generic,
        impl_trait_idents,
    }
}
