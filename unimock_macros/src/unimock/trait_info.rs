use super::attr::Attr;
use super::method;
use super::output::OutputWrapping;

pub struct TraitInfo<'t> {
    pub input_trait: &'t syn::ItemTrait,
    pub output_trait: Option<&'t syn::ItemTrait>,
    pub trait_path: syn::Path,
    pub generic_params_with_bounds: syn::punctuated::Punctuated<syn::TypeParam, syn::token::Comma>,
    pub methods: Vec<Option<method::MockMethod<'t>>>,
    pub is_type_generic: bool,
}

impl<'t> TraitInfo<'t> {
    pub fn analyze(
        prefix: &syn::Path,
        input_trait: &'t syn::ItemTrait,
        attr: &Attr,
    ) -> syn::Result<Self> {
        let generics = &input_trait.generics;
        let is_type_generic = input_trait
            .generics
            .params
            .iter()
            .any(|param| matches!(param, syn::GenericParam::Type(_)));
        let generic_params = &generics.params;

        let methods = method::extract_methods(prefix, input_trait, is_type_generic, attr)?;

        let contains_async = methods.iter().filter_map(Option::as_ref).any(|method| {
            if method.method.sig.asyncness.is_some() {
                return true;
            }
            matches!(
                method.output_structure.wrapping,
                OutputWrapping::ImplTraitFuture(_)
            )
        });

        let mut generic_params_with_bounds: syn::punctuated::Punctuated<
            syn::TypeParam,
            syn::token::Comma,
        > = Default::default();

        // add 'static bounds
        // TODO(perhaps): should only be needed for generic params which are used as function outputs?
        if is_type_generic {
            for generic_param in generic_params.iter() {
                if let syn::GenericParam::Type(type_param) = generic_param {
                    let mut bounded_param = type_param.clone();

                    add_static_bound_if_not_present(&mut bounded_param);
                    if contains_async {
                        add_send_bound_if_not_present(&mut bounded_param);
                    }

                    generic_params_with_bounds.push(bounded_param);
                }
            }
        }

        if let Some(emulate) = &attr.emulate {
            Ok(Self {
                input_trait,
                output_trait: None,
                trait_path: emulate.clone(),
                generic_params_with_bounds,
                methods,
                is_type_generic,
            })
        } else {
            let trait_ident = &input_trait.ident;
            let trait_path = syn::parse_quote! {
                #trait_ident
            };

            Ok(Self {
                input_trait,
                output_trait: Some(input_trait),
                trait_path,
                generic_params_with_bounds,
                methods,
                is_type_generic,
            })
        }
    }

    pub fn generic_type_params(&self) -> impl Iterator<Item = &syn::TypeParam> {
        self.input_trait
            .generics
            .params
            .iter()
            .filter_map(|generic_param| match generic_param {
                syn::GenericParam::Type(type_param) => Some(type_param),
                _ => None,
            })
    }
}

fn add_static_bound_if_not_present(type_param: &mut syn::TypeParam) {
    let has_static_bound = type_param.bounds.iter().any(|bound| match bound {
        syn::TypeParamBound::Lifetime(lifetime) => lifetime.ident == "static",
        _ => false,
    });

    if !has_static_bound {
        type_param
            .bounds
            .push(syn::TypeParamBound::Lifetime(syn::parse_quote! { 'static }));
    }
}

fn add_send_bound_if_not_present(type_param: &mut syn::TypeParam) {
    let has_send_bound = type_param.bounds.iter().any(|bound| match bound {
        syn::TypeParamBound::Trait(trait_bound) => trait_bound
            .path
            .segments
            .last()
            .map(|segment| segment.ident == "Send")
            .unwrap_or(false),
        _ => false,
    });

    if !has_send_bound {
        type_param
            .bounds
            .push(syn::TypeParamBound::Trait(syn::parse_quote! { Send }));
    }
}
