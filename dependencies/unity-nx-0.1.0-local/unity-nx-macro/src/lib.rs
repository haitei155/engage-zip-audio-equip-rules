use proc_macro::TokenStream;
use proc_macro2::TokenStream as TokenStream2;
use quote::{ToTokens, format_ident, quote};
mod from_pattern;
mod util;

#[proc_macro_attribute]
pub fn from_pattern(attr: TokenStream, item: TokenStream) -> TokenStream {
    translate(item, |body| from_pattern::expand(TokenStream2::from(attr), body))
}

mod data_models {
    pub mod class_attrs;
    pub mod field;
    pub mod method;
}

pub(crate) use data_models::class_attrs::ClassAttrs;
pub(crate) use data_models::field::Field;
pub(crate) use data_models::method::{Method, Resolution};

type ParseResult<T> = Result<T, venial::Error>;

// Lifted from godot-rust
fn translate<F>(input: TokenStream, transform: F) -> TokenStream
where
    F: FnOnce(venial::Item) -> ParseResult<TokenStream2>,
{
    let input2 = TokenStream2::from(input);

    let result2 = venial::parse_item(input2)
        .and_then(transform)
        .unwrap_or_else(|e| e.to_compile_error());

    TokenStream::from(result2)
}

#[proc_macro_attribute]
pub fn class(attr: TokenStream, item: TokenStream) -> TokenStream {
    translate(item, |body| class_inner(TokenStream2::from(attr), body))
}

#[proc_macro_attribute]
pub fn methods(attr: TokenStream, item: TokenStream) -> TokenStream {
    translate(item, |body| methods_inner(TokenStream2::from(attr), body))
}

#[proc_macro_attribute]
pub fn enumeration(attr: TokenStream, item: TokenStream) -> TokenStream {
    translate(item, |body| enum_inner(TokenStream2::from(attr), body))
}

#[proc_macro_attribute]
pub fn callback(attr: TokenStream, item: TokenStream) -> TokenStream {
    translate(item, |body| callback_inner(TokenStream2::from(attr), body))
}

#[proc_macro_attribute]
pub fn hook(attr: TokenStream, item: TokenStream) -> TokenStream {
    translate(item, |body| hook_inner(TokenStream2::from(attr), body))
}

#[proc_macro_attribute]
pub fn from_offset(attr: TokenStream, item: TokenStream) -> TokenStream {
    translate(item, |body| from_offset_inner(TokenStream2::from(attr), body))
}

#[proc_macro_attribute]
pub fn inject(attr: TokenStream, item: TokenStream) -> TokenStream {
    translate(item, |body| inject_inner(TokenStream2::from(attr), body))
}

#[proc_macro_attribute]
pub fn injected_methods(attr: TokenStream, item: TokenStream) -> TokenStream {
    translate(item, |body| injected_methods_inner(TokenStream2::from(attr), body))
}

fn class_inner(attr: TokenStream2, item: venial::Item) -> ParseResult<TokenStream2> {
    let class = match item {
        venial::Item::Struct(class) => class,
        _ => {
            return Err(venial::Error::new(
                "#[unity::class] can only be applied on Struct items",
            ));
        }
    };

    let class_ident = class.name.clone();
    let vis = class.vis_marker.clone();
    let class_attrs = ClassAttrs::parse(attr, class_ident.to_string(), &class.attributes)?;

    let passthrough_attrs: Vec<&venial::Attribute> = class
        .attributes
        .iter()
        .filter(|attr| !util::path_is_single(&attr.path, "parent"))
        .collect();

    let named_fields = match class.fields {
        venial::Fields::Named(named_fields) => named_fields.fields.inner,
        _ => {
            return Err(venial::Error::new(
                "#[unity::class] can only be used on Struct items with named fields",
            ));
        }
    };

    let fields = parse_fields(named_fields)?;
    let trait_ident = format_ident!("I{}", class_ident);

    // impl_generics goes on the left of every `impl`, type_generics goes on every type reference
    // A struct with type params needs a PhantomData<fn() -> (T, ...)> slot, variance-erased so
    // the wrapper stays Send + Sync regardless of T, at runtime ClassIdentity::class() calls
    // Class::make_generic with each T::class(), so the cached class is the runtime instantiation
    let (impl_generics, type_generics) = if let Some(gp) = class.generic_params.as_ref() {
        let inline = gp.as_inline_args();
        (quote! { #gp }, quote! { #inline })
    } else {
        (quote! {}, quote! {})
    };

    let type_param_idents: Vec<&proc_macro2::Ident> = class
        .generic_params
        .as_ref()
        .map(|gp| {
            gp.params
                .items()
                .filter(|p| p.tk_prefix.is_none())
                .map(|p| &p.name)
                .collect()
        })
        .unwrap_or_default();

    // Lifetimes and const generics have no IL2CPP analogue
    if let Some(gp) = class.generic_params.as_ref() {
        for param in gp.params.items() {
            if param.tk_prefix.is_some() {
                return Err(venial::Error::new(
                    "#[unity::class] only supports type generic parameters; \
                     lifetimes and const generics are not allowed here",
                ));
            }
        }
    }

    let (phantom_field_decl, phantom_init, class_resolver) = if type_param_idents.is_empty() {
        (
            quote! {},
            quote! {},
            quote! {
                static CACHE: ::std::sync::OnceLock<::unity::Class> =
                    ::std::sync::OnceLock::new();
                *CACHE.get_or_init(|| {
                    ::unity::Class::lookup(
                        <Self as ::unity::ClassIdentity>::NAMESPACE,
                        <Self as ::unity::ClassIdentity>::NAME,
                    )
                })
            },
        )
    } else {
        let type_args = &type_param_idents;
        (
            quote! { , ::core::marker::PhantomData<fn() -> (#(#type_args,)*)> },
            quote! { , ::core::marker::PhantomData },
            quote! {
                static CACHE: ::std::sync::OnceLock<
                    ::std::sync::Mutex<
                        ::std::collections::HashMap<u64, ::unity::Class>,
                    >,
                > = ::std::sync::OnceLock::new();
                use ::std::hash::{Hash as _, Hasher as _};
                let __args = [
                    #(<#type_args as ::unity::ClassIdentity>::class()),*
                ];
                let mut __h = ::std::collections::hash_map::DefaultHasher::new();
                for __a in __args.iter() {
                    (__a.raw() as *const _ as usize).hash(&mut __h);
                }
                let __key = __h.finish();
                let __map = CACHE.get_or_init(|| {
                    ::std::sync::Mutex::new(::std::collections::HashMap::new())
                });
                let mut __guard = __map.lock().unwrap();
                *__guard.entry(__key).or_insert_with(|| {
                    ::unity::Class::lookup(
                        <Self as ::unity::ClassIdentity>::NAMESPACE,
                        <Self as ::unity::ClassIdentity>::NAME,
                    )
                    .make_generic(&__args)
                    .unwrap_or_else(|| panic!(
                        "{}",
                        ::unity::Il2CppError::FailedGenericInstantiation {
                            class: ::core::stringify!(#class_ident).to_string(),
                        }
                    ))
                })
            },
        )
    };

    let ancestor_macro_ident = format_ident!("__{}_ancestor_impls", class_ident);

    let gen_meta_idents: Vec<proc_macro2::Ident> = (0..type_param_idents.len())
        .map(|i| format_ident!("__g{}", i))
        .collect();

    fn rewrite_tpidents(
        stream: TokenStream2,
        params: &[&proc_macro2::Ident],
        metas: &[proc_macro2::Ident],
    ) -> TokenStream2 {
        let mut out = TokenStream2::new();
        let dollar = proc_macro2::Punct::new('$', proc_macro2::Spacing::Alone);
        for tt in stream {
            match tt {
                proc_macro2::TokenTree::Ident(ref id) => {
                    if let Some(idx) = params.iter().position(|p| *p == id) {
                        out.extend(std::iter::once(proc_macro2::TokenTree::Punct(dollar.clone())));
                        out.extend(std::iter::once(proc_macro2::TokenTree::Ident(metas[idx].clone())));
                    } else {
                        out.extend(std::iter::once(tt));
                    }
                }
                proc_macro2::TokenTree::Group(g) => {
                    let inner = rewrite_tpidents(g.stream(), params, metas);
                    let mut new_group = proc_macro2::Group::new(g.delimiter(), inner);
                    new_group.set_span(g.span());
                    out.extend(std::iter::once(proc_macro2::TokenTree::Group(new_group)));
                }
                _ => out.extend(std::iter::once(tt)),
            }
        }
        out
    }

    fn starts_with_crate(stream: &TokenStream2) -> bool {
        let mut iter = stream.clone().into_iter();
        let first = iter.next();
        let second = iter.next();
        match (first, second) {
            (
                Some(proc_macro2::TokenTree::Punct(p1)),
                Some(proc_macro2::TokenTree::Punct(p2)),
            ) if p1.as_char() == ':' && p2.as_char() == ':' => {
                matches!(iter.next(), Some(proc_macro2::TokenTree::Ident(id)) if id == "crate")
            }
            (Some(proc_macro2::TokenTree::Ident(id)), _) => id == "crate",
            _ => false,
        }
    }

    fn strip_angles(ts: &TokenStream2) -> TokenStream2 {
        let toks: Vec<_> = ts.clone().into_iter().collect();
        if toks.len() < 2 {
            return TokenStream2::new();
        }
        toks[1..toks.len() - 1].iter().cloned().collect()
    }

    fn macroize_crate_prefix(prefix: &TokenStream2) -> TokenStream2 {
        let toks: Vec<proc_macro2::TokenTree> = prefix.clone().into_iter().collect();
        if let Some(proc_macro2::TokenTree::Ident(id)) = toks.first() {
            if *id == "crate" {
                let rest = &toks[1..];
                return quote! { $crate #(#rest)* };
            }
        }
        prefix.clone()
    }

    let parent_cascade_invocations: Vec<TokenStream2> = class_attrs
        .parents
        .iter()
        .take(1)
        .map(|p| {
            let base_macro = format_ident!("__{}_ancestor_impls", p.base);
            let inner = strip_angles(&p.generics);
            let rewritten = rewrite_tpidents(inner, &type_param_idents, &gen_meta_idents);
            let is_in_crate =
                p.path_prefix.is_empty() || starts_with_crate(&p.path_prefix);
            let prefix = if is_in_crate {
                quote! { $crate:: }
            } else {
                p.macro_path_prefix()
            };
            let p_prefix = macroize_crate_prefix(&p.path_prefix);
            let p_base = &p.base;
            let p_trait = format_ident!("I{}", p.base);
            if rewritten.is_empty() {
                quote! { #prefix #base_macro!($child, [#p_prefix #p_base], [#p_prefix #p_trait]); }
            } else {
                quote! { #prefix #base_macro!($child, [#p_prefix #p_base], [#p_prefix #p_trait], #rewritten); }
            }
        })
        .collect();

    let macro_param_slots: Vec<TokenStream2> = gen_meta_idents
        .iter()
        .map(|g| quote! { , $#g:ty })
        .collect();

    let own_trait_impl = if type_param_idents.is_empty() {
        quote! { impl $($__str)* for $child {} }
    } else {
        let gens = gen_meta_idents.iter().map(|g| quote! { $#g });
        quote! { impl $($__str)* <#(#gens),*> for $child {} }
    };
    let own_from_impl = if type_param_idents.is_empty() {
        quote! {
            #[doc(hidden)]
            impl ::core::convert::From<$child> for $($__sty)* {
                fn from(value: $child) -> Self {
                    <Self as ::unity::FromIlInstance>::from_il_instance(
                        <$child as ::core::convert::Into<::unity::IlInstance>>::into(value),
                    )
                }
            }
        }
    } else {
        let gens = gen_meta_idents.iter().map(|g| quote! { $#g });
        quote! {
            #[doc(hidden)]
            impl ::core::convert::From<$child> for $($__sty)* <#(#gens),*> {
                fn from(value: $child) -> Self {
                    <Self as ::unity::FromIlInstance>::from_il_instance(
                        <$child as ::core::convert::Into<::unity::IlInstance>>::into(value),
                    )
                }
            }
        }
    };

    let ancestor_macro_def = quote! {
        #[doc(hidden)]
        #[macro_export]
        macro_rules! #ancestor_macro_ident {
            ($child:ty, [$($__sty:tt)*], [$($__str:tt)*] #(#macro_param_slots)*) => {
                #(#parent_cascade_invocations)*
                #own_trait_impl
                #own_from_impl
            };
        }
    };

    let inherit_cascade: Vec<TokenStream2> = class_attrs
        .parents
        .iter()
        .take(1)
        .map(|p| {
            let inner = strip_angles(&p.generics);
            let rewritten = rewrite_tpidents(inner, &type_param_idents, &gen_meta_idents);
            let is_explicit_in_crate =
                !p.path_prefix.is_empty() && starts_with_crate(&p.path_prefix);
            let is_cross_crate =
                !p.path_prefix.is_empty() && !is_explicit_in_crate;
            let p_prefix = macroize_crate_prefix(&p.path_prefix);
            let p_base = &p.base;
            let p_trait = format_ident!("I{}", p.base);
            if is_explicit_in_crate {
                let parent_inherit = format_ident!("__{}_inherit", p.base);
                let prefix = &p.path_prefix;
                if rewritten.is_empty() {
                    quote! { #prefix #parent_inherit!($child, [#p_prefix #p_base], [#p_prefix #p_trait]); }
                } else {
                    quote! { #prefix #parent_inherit!($child, [#p_prefix #p_base], [#p_prefix #p_trait], #rewritten); }
                }
            } else if is_cross_crate {
                let crate_prefix = p.macro_path_prefix();
                let parent_export = format_ident!("__{}_ancestor_impls", p.base);
                if rewritten.is_empty() {
                    quote! { #crate_prefix #parent_export!($child, [#p_prefix #p_base], [#p_prefix #p_trait]); }
                } else {
                    quote! { #crate_prefix #parent_export!($child, [#p_prefix #p_base], [#p_prefix #p_trait], #rewritten); }
                }
            } else {
                let parent_export = format_ident!("__{}_ancestor_impls", p.base);
                if rewritten.is_empty() {
                    quote! { $crate::#parent_export!($child, [#p_prefix #p_base], [#p_prefix #p_trait]); }
                } else {
                    quote! { $crate::#parent_export!($child, [#p_prefix #p_base], [#p_prefix #p_trait], #rewritten); }
                }
            }
        })
        .collect();

    let inherit_macro_ident = format_ident!("__{}_inherit", class_ident);
    let inherit_wrapper_def = quote! {
        #[doc(hidden)]
        macro_rules! #inherit_macro_ident {
            ($child:ty, [$($__sty:tt)*], [$($__str:tt)*] #(#macro_param_slots)*) => {
                #(#inherit_cascade)*
                #own_trait_impl
                #own_from_impl
            };
        }
        #[doc(hidden)]
        #[allow(unused_imports)]
        pub(crate) use #inherit_macro_ident;
    };

    let parent_invocations_for_self: Vec<TokenStream2> = class_attrs
        .parents
        .iter()
        .map(|p| {
            let inner = strip_angles(&p.generics);
            let is_explicit_in_crate =
                !p.path_prefix.is_empty() && starts_with_crate(&p.path_prefix);
            let is_cross_crate =
                !p.path_prefix.is_empty() && !is_explicit_in_crate;
            let p_prefix = &p.path_prefix;
            let p_base = &p.base;
            let p_trait = format_ident!("I{}", p.base);
            if is_explicit_in_crate {
                let inherit_macro = format_ident!("__{}_inherit", p.base);
                let prefix = &p.path_prefix;
                if inner.is_empty() {
                    quote! { #prefix #inherit_macro!(#class_ident, [#p_prefix #p_base], [#p_prefix #p_trait]); }
                } else {
                    quote! { #prefix #inherit_macro!(#class_ident, [#p_prefix #p_base], [#p_prefix #p_trait], #inner); }
                }
            } else if is_cross_crate {
                let crate_prefix = p.macro_path_prefix();
                let base_macro = format_ident!("__{}_ancestor_impls", p.base);
                if inner.is_empty() {
                    quote! { #crate_prefix #base_macro!(#class_ident, [#p_prefix #p_base], [#p_prefix #p_trait]); }
                } else {
                    quote! { #crate_prefix #base_macro!(#class_ident, [#p_prefix #p_base], [#p_prefix #p_trait], #inner); }
                }
            } else {
                let base_macro = format_ident!("__{}_ancestor_impls", p.base);
                let alias = format_ident!(
                    "__unity2_anchor_{}_for_{}", p.base, class_ident
                );
                if inner.is_empty() {
                    quote! {
                        use crate::#base_macro as #alias;
                        #alias!(#class_ident, [#p_prefix #p_base], [#p_prefix #p_trait]);
                    }
                } else {
                    quote! {
                        use crate::#base_macro as #alias;
                        #alias!(#class_ident, [#p_prefix #p_base], [#p_prefix #p_trait], #inner);
                    }
                }
            }
        })
        .collect();

    let (parent_bound, parent_impls) = if class_attrs.parents.is_empty() {
        (quote! { ::unity::SystemObject }, quote! {})
    } else {
        let direct = &class_attrs.parents[0];
        let direct_base = &direct.base;
        let direct_generics = &direct.generics;
        let direct_prefix = &direct.path_prefix;
        let direct_trait_ident = format_ident!("I{}", direct_base);

        let impls = if type_param_idents.is_empty() {
            quote! { #(#parent_invocations_for_self)* }
        } else {
            let generic_ancestor_impls = class_attrs.parents.iter().map(|p| {
                let base = &p.base;
                let generics = &p.generics;
                let prefix = &p.path_prefix;
                let trait_ident = format_ident!("I{}", base);
                quote! {
                    impl #impl_generics #prefix #trait_ident #generics for #class_ident #type_generics {}
                    #[doc(hidden)]
                    impl #impl_generics ::core::convert::From<#class_ident #type_generics>
                        for #prefix #base #generics
                    {
                        fn from(value: #class_ident #type_generics) -> Self {
                            <Self as ::unity::FromIlInstance>::from_il_instance(
                                <#class_ident #type_generics as ::core::convert::Into<::unity::IlInstance>>::into(value),
                            )
                        }
                    }
                }
            });
            quote! { #(#generic_ancestor_impls)* }
        };

        (quote! { #direct_prefix #direct_trait_ident #direct_generics }, impls)
    };

    let base_block = if let Some(direct) = class_attrs.parents.first() {
        let direct_base = &direct.base;
        let direct_generics = &direct.generics;
        let direct_prefix = &direct.path_prefix;
        quote! {
            impl #impl_generics #class_ident #type_generics {
                #[inline]
                pub fn base(self) -> #direct_prefix #direct_base #direct_generics {
                    <#direct_prefix #direct_base #direct_generics as ::unity::FromIlInstance>::from_il_instance(
                        <#class_ident #type_generics as ::core::convert::Into<::unity::IlInstance>>::into(self),
                    )
                }
            }
        }
    } else {
        quote! {}
    };

    let instance_accessors = fields.iter().filter(|f| !f.is_static).map(|f| {
        let rust_name = &f.name;
        let ty = &f.ty;
        let il2cpp_name = &f.il2cpp_name;
        let setter_name = format_ident!("set_{}", rust_name);

        let resolve = match f.offset {
            Some(off) => {
                let off_lit = proc_macro2::Literal::usize_unsuffixed(off as usize);
                quote! {
                    const __OFFSET: usize = #off_lit;
                    #[cfg(debug_assertions)]
                    {
                        static __CHECK: ::std::sync::Once = ::std::sync::Once::new();
                        __CHECK.call_once(|| ::unity::verify_field_offset_instance(self, __OFFSET, #il2cpp_name));
                    }
                }
            }
            None => quote! {
                static OFFSET: ::std::sync::OnceLock<usize> = ::std::sync::OnceLock::new();
                let __OFFSET = ::unity::cached_field_offset_instance(&OFFSET, self, #il2cpp_name);
            },
        };

        quote! {
            #[inline]
            fn #rust_name(self) -> #ty {
                #resolve
                ::unity::field_get_value_at_offset(self, __OFFSET)
            }

            #[inline]
            fn #setter_name(self, value: #ty) {
                #resolve
                ::unity::field_set_value_at_offset(self, __OFFSET, value);
            }
        }
    });

    // Static accessors land as inherent associated functions, subclasses don't inherit them,
    // matching the common C# pattern of calling statics through the defining class name
    let static_fields: Vec<&Field> = fields.iter().filter(|f| f.is_static).collect();
    let static_accessors = static_fields.iter().map(|f| {
        let rust_name = &f.name;
        let ty = &f.ty;
        let il2cpp_name = &f.il2cpp_name;

        let setter_name = format_ident!("set_{}", rust_name);
        quote! {
            #[inline]
            #vis fn #rust_name() -> #ty {
                static OFFSET: ::std::sync::OnceLock<usize> = ::std::sync::OnceLock::new();
                let __offset = ::unity::cached_field_offset_static::<Self>(&OFFSET, #il2cpp_name);
                ::unity::static_field_get_value_at_offset(
                    <Self as ::unity::ClassIdentity>::class(),
                    __offset,
                )
            }

            #[inline]
            #vis fn #setter_name(value: #ty) {
                static OFFSET: ::std::sync::OnceLock<usize> = ::std::sync::OnceLock::new();
                let __offset = ::unity::cached_field_offset_static::<Self>(&OFFSET, #il2cpp_name);
                ::unity::static_field_set_value_at_offset(
                    <Self as ::unity::ClassIdentity>::class(),
                    __offset,
                    value,
                );
            }
        }
    });

    let statics_block = if static_fields.is_empty() {
        quote! {}
    } else {
        quote! {
            impl #impl_generics #class_ident #type_generics {
                #(#static_accessors)*
            }
        }
    };

    let namespace_lit = class_attrs.namespace.as_deref().unwrap_or("");
    let class_name_lit = class_attrs.name.as_str();

    // Build doc lines summarizing IL2CPP namespace + parent chain so rustdoc shows hierarchy
    let il2cpp_qualified = if namespace_lit.is_empty() {
        class_name_lit.to_string()
    } else {
        format!("{}.{}", namespace_lit, class_name_lit)
    };
    let inheritance_doc_lines: Vec<String> = {
        // Double-backtick so generic IL2CPP names like `ProcSceneSequence`1` render correctly
        let mut lines = vec![
            String::new(),
            format!("IL2CPP class: ``{}``", il2cpp_qualified),
        ];
        if !class_attrs.parents.is_empty() {
            let chain = class_attrs
                .parents
                .iter()
                .map(|p| {
                    let generics = p.generics.to_string().replace(' ', "");
                    if generics.is_empty() {
                        format!("``{}``", p.base)
                    } else {
                        format!("``{}{}``", p.base, generics)
                    }
                })
                .collect::<Vec<_>>()
                .join(" : ");
            lines.push(format!("Inherits: {}", chain));
        }
        lines
    };
    let inheritance_docs = inheritance_doc_lines
        .iter()
        .map(|line| quote! { #[doc = #line] });

    Ok(quote! {
        #(#passthrough_attrs)*
        #(#inheritance_docs)*
        #[repr(transparent)]
        #[derive(::core::clone::Clone, ::core::marker::Copy)]
        #vis struct #class_ident #impl_generics(::unity::IlInstance #phantom_field_decl);

        #[doc(hidden)]
        impl #impl_generics ::core::convert::From<#class_ident #type_generics> for ::unity::IlInstance {
            fn from(value: #class_ident #type_generics) -> Self {
                value.0
            }
        }

        #[doc(hidden)]
        impl #impl_generics ::core::convert::AsRef<::unity::IlInstance> for #class_ident #type_generics {
            fn as_ref(&self) -> &::unity::IlInstance {
                &self.0
            }
        }

        impl #impl_generics ::unity::ClassIdentity for #class_ident #type_generics {
            const NAMESPACE: &'static str = #namespace_lit;
            const NAME: &'static str = #class_name_lit;

            fn class() -> ::unity::Class {
                #class_resolver
            }
        }

        impl #impl_generics ::unity::FromIlInstance for #class_ident #type_generics {
            #[inline]
            fn from_il_instance(instance: ::unity::IlInstance) -> Self {
                Self(instance #phantom_init)
            }
        }

        impl #impl_generics ::unity::IlType for #class_ident #type_generics {
            fn il_type() -> &'static ::unity::il2cpp::Il2CppType {
                &<Self as ::unity::ClassIdentity>::class().raw()._1.byval_arg
            }
        }

        #vis trait #trait_ident #impl_generics: #parent_bound {
            #(#instance_accessors)*
        }

        impl #impl_generics #trait_ident #type_generics for #class_ident #type_generics {}
        #parent_impls
        #base_block

        #statics_block

        #ancestor_macro_def
        #[doc(hidden)]
        pub use #ancestor_macro_ident;
        #inherit_wrapper_def
    })
}

fn parse_fields(
    named_fields: Vec<(venial::NamedField, proc_macro2::Punct)>,
) -> ParseResult<Vec<Field>> {
    let mut all_fields = Vec::with_capacity(named_fields.len());
    for (named_field, _) in named_fields {
        all_fields.push(Field::parse(&named_field)?);
    }
    Ok(all_fields)
}

fn enum_inner(attr: TokenStream2, item: venial::Item) -> ParseResult<TokenStream2> {
    // When both namespace and name are given the generated impl includes a class() method,
    // otherwise the enum stays purely Rust-side
    let (il2cpp_namespace, il2cpp_name) = if attr.is_empty() {
        (None, None)
    } else {
        let mut parser = util::KvParser::parse_args("enumeration", attr, proc_macro2::Span::call_site())?;
        let ns = parser.handle_literal("namespace", "string")?
            .map(|lit| data_models::field::unquote_string_literal(&lit.to_string()));
        let name = parser.handle_literal("name", "string")?
            .map(|lit| data_models::field::unquote_string_literal(&lit.to_string()));
        parser.finish()?;
        (ns, name)
    };

    let enum_item = match &item {
        venial::Item::Enum(e) => e,
        _ => {
            return Err(venial::Error::new(
                "#[unity::enumeration] can only be applied to enum items",
            ));
        }
    };

    // #[repr(<int type>)] drives from_value arg type and value() return type, explicit so the
    // Rust side can't silently drift from the IL2CPP enum's underlying discriminant type
    let repr_ident = {
        let mut found: Option<proc_macro2::Ident> = None;
        for attr in &enum_item.attributes {
            if !util::path_is_single(&attr.path, "repr") {
                continue;
            }
            for tt in attr.value.get_value_tokens() {
                if let proc_macro2::TokenTree::Ident(id) = tt {
                    let s = id.to_string();
                    if matches!(
                        s.as_str(),
                        "i8" | "i16" | "i32" | "i64" | "isize"
                            | "u8" | "u16" | "u32" | "u64" | "usize"
                    ) {
                        found = Some(id.clone());
                        break;
                    }
                }
            }
            if found.is_some() {
                break;
            }
        }
        match found {
            Some(id) => id,
            None => {
                return Err(venial::Error::new(
                    "#[unity::enumeration] requires `#[repr(<int type>)]` with one of \
                     i8/i16/i32/i64/u8/u16/u32/u64 as the discriminant type",
                ));
            }
        }
    };

    let enum_name = &enum_item.name;

    // Reject variants with fields, generated from_value and VARIANTS and Display assume unit variants
    // with const discriminants, IL2CPP enums are pure discriminants anyway
    let mut variant_names: Vec<&proc_macro2::Ident> = Vec::new();
    for (variant, _) in enum_item.variants.inner.iter() {
        if !matches!(variant.fields, venial::Fields::Unit) {
            return Err(venial::Error::new(
                "#[unity::enumeration] requires unit variants (no tuple or struct data)",
            ));
        }
        variant_names.push(&variant.name);
    }

    let variant_names_str: Vec<String> =
        variant_names.iter().map(|n| n.to_string()).collect();

    // `Self::Name as #repr` isn't valid in match patterns but IS a valid const expression in an `if`
    let from_value_arms = variant_names.iter().map(|name| {
        quote! {
            if v == Self::#name as #repr_ident {
                return ::core::option::Option::Some(Self::#name);
            }
        }
    });

    let display_arms = variant_names
        .iter()
        .zip(variant_names_str.iter())
        .map(|(name, name_str)| quote! { Self::#name => #name_str, });

    let (identity_methods, identity_impls) = match (&il2cpp_namespace, &il2cpp_name) {
        (Some(ns), Some(name)) => {
            let ns_lit = ns.as_str();
            let name_lit = name.as_str();
            (
                quote! {
                    pub const IL2CPP_NAMESPACE: &'static str = #ns_lit;
                    pub const IL2CPP_NAME: &'static str = #name_lit;

                    pub fn class() -> ::unity::Class {
                        static CACHE: ::std::sync::OnceLock<::unity::Class> =
                            ::std::sync::OnceLock::new();
                        *CACHE.get_or_init(|| ::unity::Class::lookup(#ns_lit, #name_lit))
                    }
                },
                quote! {
                    impl ::unity::ClassIdentity for #enum_name {
                        const NAMESPACE: &'static str = #ns_lit;
                        const NAME: &'static str = #name_lit;
                        fn class() -> ::unity::Class {
                            #enum_name::class()
                        }
                    }
                    impl ::unity::IlType for #enum_name {
                        fn il_type() -> &'static ::unity::il2cpp::Il2CppType {
                            &<Self as ::unity::ClassIdentity>::class().raw()._1.byval_arg
                        }
                    }
                },
            )
        }
        _ => {
            (
                quote! {},
                quote! {
                    impl ::unity::IlType for #enum_name {
                        fn il_type() -> &'static ::unity::il2cpp::Il2CppType {
                            <#repr_ident as ::unity::IlType>::il_type()
                        }
                    }
                },
            )
        }
    };

    Ok(quote! {
        #[derive(
            ::core::clone::Clone,
            ::core::marker::Copy,
            ::core::fmt::Debug,
            ::core::cmp::PartialEq,
            ::core::cmp::Eq,
            ::core::hash::Hash,
        )]
        #item

        impl #enum_name {
            #identity_methods

            pub const VARIANTS: &'static [Self] = &[#(Self::#variant_names),*];

            // Returns None for unknown values, so a game patch that adds a variant doesn't UB
            pub const fn from_value(v: #repr_ident) -> ::core::option::Option<Self> {
                #(#from_value_arms)*
                ::core::option::Option::None
            }

            #[inline]
            pub const fn value(self) -> #repr_ident {
                self as #repr_ident
            }
        }

        impl ::core::fmt::Display for #enum_name {
            fn fmt(&self, f: &mut ::core::fmt::Formatter<'_>) -> ::core::fmt::Result {
                f.write_str(match self {
                    #(#display_arms)*
                })
            }
        }

        #identity_impls
    })
}

fn callback_inner(attr: TokenStream2, item: venial::Item) -> ParseResult<TokenStream2> {
    if !attr.is_empty() {
        return Err(venial::Error::new(
            "#[unity::callback] takes no arguments",
        ));
    }

    let func = match item {
        venial::Item::Function(f) => f,
        _ => {
            return Err(venial::Error::new(
                "#[unity::callback] can only be applied to extern \"C\" fn items",
            ));
        }
    };

    let fn_name = &func.name;
    let helper_name = format_ident!("{}_method_info", fn_name);
    let static_name = format_ident!("__UNITY2_MI_{}", fn_name);
    let params_static = format_ident!("__UNITY2_MI_PARAMS_{}", fn_name);

    // Drop leading `this`/`_this` (becomes `class` via its Rust type) and the trailing
    // `method_info`/`_method_info` / `OptionalMethod` hidden arg, neither counts toward
    // parameters_count or the parameters array
    let typed_params: Vec<&venial::FnTypedParam> = func
        .params
        .inner
        .iter()
        .filter_map(|(p, _)| match p {
            venial::FnParam::Typed(t) => Some(t),
            _ => None,
        })
        .collect();

    let is_receiver = |t: &venial::FnTypedParam| {
        let n = t.name.to_string();
        n == "this" || n == "_this"
    };
    let is_trailing_method_info = |t: &venial::FnTypedParam| {
        let n = t.name.to_string();
        n == "method_info" || n == "_method_info"
    };

    let receiver_ty: Option<&venial::TypeExpr> = typed_params
        .first()
        .filter(|t| is_receiver(t))
        .map(|t| &t.ty);

    let mut body_params: Vec<&venial::FnTypedParam> = typed_params.iter().copied().collect();
    if receiver_ty.is_some() {
        body_params.remove(0);
    }
    if body_params.last().map(|t| is_trailing_method_info(t)).unwrap_or(false) {
        body_params.pop();
    }

    let parameters_count: u8 = body_params.len().min(u8::MAX as usize) as u8;

    // ParameterInfo entries pull parameter_type from IlType at lazy-init, can't be const
    // because IL2CPP metadata is only populated after il2cpp_init
    let param_entries: Vec<TokenStream2> = body_params
        .iter()
        .enumerate()
        .map(|(i, t)| {
            let ty = &t.ty;
            let raw = t.name.to_string();
            let bytes: Vec<u8> = raw.trim_start_matches('_').bytes().chain(std::iter::once(0u8)).collect();
            let lit = proc_macro2::Literal::byte_string(&bytes);
            let pos = i as i32;
            quote! {
                ::unity::il2cpp::ParameterInfo {
                    name: (#lit).as_ptr(),
                    position: #pos,
                    token: 0,
                    parameter_type: <#ty as ::unity::IlType>::il_type(),
                }
            }
        })
        .collect();

    let return_ty_expr: TokenStream2 = match func.return_ty.as_ref() {
        Some(t) => {
            let ty = &t;
            quote! { <#ty as ::unity::IlType>::il_type() }
        }
        None => quote! { <() as ::unity::IlType>::il_type() },
    };

    let class_expr: TokenStream2 = match receiver_ty {
        Some(ty) => quote! { ::core::option::Option::Some(<#ty as ::unity::ClassIdentity>::class().raw()) },
        None => quote! { ::core::option::Option::None },
    };

    let fn_name_c_lit = {
        let s = fn_name.to_string();
        let bytes: Vec<u8> = s.bytes().chain(std::iter::once(0u8)).collect();
        let lit = proc_macro2::Literal::byte_string(&bytes);
        quote! { #lit }
    };

    const METHOD_ATTRIBUTE_STATIC: u16 = 0x0010;
    let flags: u16 = if receiver_ty.is_some() { 0 } else { METHOD_ATTRIBUTE_STATIC };

    // The MethodInfo lives as a plain `static` in the plugin's binary, no heap allocation, no
    // leak, no runtime init, every field is const-initializable and the record sits in .rodata
    // MethodInfo's unsafe impl Sync is honest here, the record's fields are immutable post-init
    // and all raw pointers are either 'static or stable-address fn pointers
    Ok(quote! {
        #func

        #[allow(non_upper_case_globals)]
        static #params_static: ::std::sync::LazyLock<[::unity::il2cpp::ParameterInfo; #parameters_count as usize]> =
            ::std::sync::LazyLock::new(|| [#(#param_entries),*]);

        #[allow(non_upper_case_globals)]
        static #static_name: ::std::sync::LazyLock<::unity::MethodInfo> =
            ::std::sync::LazyLock::new(|| ::unity::MethodInfo {
                method_ptr: #fn_name as *mut u8,
                invoker_method: ::core::ptr::null(),
                name: (#fn_name_c_lit).as_ptr(),
                class: #class_expr,
                return_type: #return_ty_expr as *const _ as *const u8,
                parameters: (&*#params_static).as_ptr(),
                info_or_definition: ::core::ptr::null(),
                generic_method_or_container: ::core::ptr::null(),
                token: 0,
                flags: #flags,
                iflags: 0,
                slot: u16::MAX,
                parameters_count: #parameters_count,
                bitflags: 0,
            });

        #[allow(non_snake_case)]
        fn #helper_name() -> &'static ::unity::MethodInfo {
            &*#static_name
        }
    })
}

struct IlHookArgs {
    namespace: String,
    class: String,
    method: String,
    args_override: Option<usize>,
}

fn parse_hook_args(attr: TokenStream2, attr_name: &str) -> ParseResult<IlHookArgs> {
    let tokens: Vec<proc_macro2::TokenTree> = attr.into_iter().collect();
    let mut strings: Vec<String> = Vec::new();
    let mut args_override: Option<usize> = None;
    let mut i = 0;

    while i < tokens.len() {
        match &tokens[i] {
            proc_macro2::TokenTree::Literal(lit) => {
                let s = lit.to_string();
                if let Some(stripped) = s.strip_prefix('"').and_then(|v| v.strip_suffix('"')) {
                    strings.push(stripped.to_string());
                } else if let Ok(n) = s.parse::<usize>() {
                    if args_override.is_some() {
                        return Err(venial::Error::new(format!(
                            "#[unity::{attr_name}] accepts at most one numeric args override"
                        )));
                    }
                    args_override = Some(n);
                } else {
                    return Err(venial::Error::new(format!(
                        "#[unity::{attr_name}] expects string literals for namespace/class/method (got {s})"
                    )));
                }
                i += 1;
            }
            proc_macro2::TokenTree::Punct(p) if p.as_char() == ',' => {
                i += 1;
            }
            other => {
                return Err(venial::Error::new(format!(
                    "#[unity::{attr_name}] unexpected token `{other}`; expected `(\"NS\", \"Class\", \"Method\")` or `(\"NS\", \"Class\", \"Method\", N)`"
                )));
            }
        }
    }

    if strings.len() != 3 {
        return Err(venial::Error::new(format!(
            "#[unity::{attr_name}] requires exactly 3 string literals (namespace, class, method); got {}",
            strings.len()
        )));
    }

    let method = strings.pop().unwrap();
    let class = strings.pop().unwrap();
    let namespace = strings.pop().unwrap();

    Ok(IlHookArgs {
        namespace,
        class,
        method,
        args_override,
    })
}

fn infer_il_arg_count(func: &venial::Function) -> usize {
    let typed: Vec<&venial::FnTypedParam> = func
        .params
        .inner
        .iter()
        .filter_map(|(p, _)| match p {
            venial::FnParam::Typed(t) => Some(t),
            _ => None,
        })
        .collect();

    let has_name = |t: &venial::FnTypedParam, a: &str, b: &str| {
        let n = t.name.to_string();
        n == a || n == b
    };

    let mut start = 0;
    let mut end = typed.len();

    if start < end && has_name(typed[start], "this", "_this") {
        start += 1;
    }
    if end > start && has_name(typed[end - 1], "method_info", "_method_info") {
        end -= 1;
    }

    end - start
}

fn hook_inner(attr: TokenStream2, item: venial::Item) -> ParseResult<TokenStream2> {
    let func = match item {
        venial::Item::Function(f) => f,
        _ => {
            return Err(venial::Error::new(
                "#[unity::hook] can only be applied to function items",
            ));
        }
    };

    if func.body.is_none() {
        return Err(venial::Error::new(
            "#[unity::hook] requires a function with a body; for a bare declaration use #[unity::from_offset]",
        ));
    }

    let parsed = parse_hook_args(attr, "hook")?;
    let args_count = parsed.args_override.unwrap_or_else(|| infer_il_arg_count(&func));

    let fn_name = &func.name;
    let lookup_mod_ident = format_ident!("__unity2_hook_lookup_{}", fn_name);
    let namespace = &parsed.namespace;
    let class = &parsed.class;
    let method = &parsed.method;

    Ok(quote! {
        #[doc(hidden)]
        #[allow(non_snake_case)]
        mod #lookup_mod_ident {
            static OFFSET: ::std::sync::LazyLock<::unity::Il2CppResult<usize>> =
                ::std::sync::LazyLock::new(|| {
                    ::unity::lookup::method_offset_by_name(#namespace, #class, #method, #args_count)
                });
            pub fn get_offset() -> usize {
                match &*OFFSET {
                    ::core::result::Result::Ok(o) => *o,
                    ::core::result::Result::Err(e) => panic!(
                        "#[unity::hook({:?}, {:?}, {:?})] install failed: {}",
                        #namespace, #class, #method, e
                    ),
                }
            }
        }

        #[::skyline::hook(offset = #lookup_mod_ident::get_offset())]
        #func
    })
}

fn from_offset_inner(attr: TokenStream2, item: venial::Item) -> ParseResult<TokenStream2> {
    let func = match item {
        venial::Item::Function(f) => f,
        _ => {
            return Err(venial::Error::new(
                "#[unity::from_offset] can only be applied to function declarations",
            ));
        }
    };

    if func.body.is_some() {
        return Err(venial::Error::new(
            "#[unity::from_offset] requires a bare function declaration (no body)",
        ));
    }

    let parsed = parse_hook_args(attr, "from_offset")?;
    let args_count = parsed.args_override.unwrap_or_else(|| infer_il_arg_count(&func));

    let fn_name = &func.name;
    let lookup_mod_ident = format_ident!("__unity2_offset_lookup_{}", fn_name);
    let namespace = &parsed.namespace;
    let class = &parsed.class;
    let method = &parsed.method;

    Ok(quote! {
        #[doc(hidden)]
        #[allow(non_snake_case)]
        mod #lookup_mod_ident {
            static OFFSET: ::std::sync::LazyLock<::unity::Il2CppResult<usize>> =
                ::std::sync::LazyLock::new(|| {
                    ::unity::lookup::method_offset_by_name(#namespace, #class, #method, #args_count)
                });
            pub fn get_offset() -> usize {
                match &*OFFSET {
                    ::core::result::Result::Ok(o) => *o,
                    ::core::result::Result::Err(e) => panic!(
                        "#[unity::from_offset({:?}, {:?}, {:?})] lookup failed: {}",
                        #namespace, #class, #method, e
                    ),
                }
            }
        }

        #[::skyline::from_offset(#lookup_mod_ident::get_offset() as usize)]
        #func
    })
}

fn methods_inner(attr: TokenStream2, item: venial::Item) -> ParseResult<TokenStream2> {
    let mut is_value_type = false;
    for tt in attr.into_iter() {
        if let proc_macro2::TokenTree::Ident(id) = tt {
            if id == "value" {
                is_value_type = true;
            }
        }
    }

    let impl_block = match item {
        venial::Item::Impl(i) => i,
        _ => {
            return Err(venial::Error::new(
                "#[unity::methods] can only be applied to inherent impl blocks",
            ));
        }
    };

    if impl_block.trait_ty.is_some() {
        return Err(venial::Error::new(
            "#[unity::methods] does not support trait impls; use `impl Foo { ... }`",
        ));
    }

    let self_ty = impl_block.self_ty.clone();
    let self_ident = util::extract_typename(&self_ty)
        .map(|seg| seg.ident)
        .ok_or_else(|| {
            venial::Error::new("#[unity::methods] requires `impl Foo` (single type ident)")
        })?;

    let mut methods = Vec::new();
    for member in &impl_block.body_items {
        match member {
            venial::ImplMember::AssocFunction(func) => {
                methods.push(Method::parse(func)?);
            }
            _ => {
                return Err(venial::Error::new(
                    "#[unity::methods] only supports function items",
                ));
            }
        }
    }

    // Generic impl blocks route through a trait so children inherit parent methods, offset
    // / pattern / vtable resolutions are rejected, they bind a fixed function pointer and
    // can't carry a per-instantiation MethodInfo
    if let Some(impl_generics) = impl_block.impl_generic_params.as_ref() {
        for m in &methods {
            if !matches!(m.resolution, Resolution::Name { .. }) {
                return Err(venial::Error::new(
                    "generic `impl<T> Foo<T>` blocks only support name-based method \
                     resolution (`#[method]` / `#[method(name = \"...\")]`); \
                     offset / pattern / vtable_index bind a fixed function pointer \
                     and can't carry the per-instantiation MethodInfo",
                ));
            }
        }

        // Skip lifetime and const generics, IL2CPP has no analogue
        let type_param_idents: Vec<&proc_macro2::Ident> = impl_generics
            .params
            .items()
            .filter(|p| p.tk_prefix.is_none())
            .map(|p| &p.name)
            .collect();
        if type_param_idents.is_empty() {
            return Err(venial::Error::new(
                "generic `#[unity::methods]` requires at least one type parameter \
                 on the impl block; non-generic impls already use the static-offset \
                 path automatically",
            ));
        }

        let methods_trait_ident = format_ident!("I{}Methods", self_ident);
        let field_trait_ident = format_ident!("I{}", self_ident);

        let method_defaults = methods.iter().map(|m| build_generic_trait_default(m, &type_param_idents));

        let blanket_bounds = if is_value_type {
            quote! {
                impl<
                    #(#type_param_idents: ::unity::ClassIdentity,)*
                    __U: ::unity::ClassIdentity
                > #methods_trait_ident<#(#type_param_idents),*> for __U
                {}
            }
        } else {
            quote! {
                impl<
                    #(#type_param_idents: ::unity::ClassIdentity,)*
                    __U: #field_trait_ident<#(#type_param_idents),*> + ::unity::ClassIdentity
                > #methods_trait_ident<#(#type_param_idents),*> for __U
                {}
            }
        };

        return Ok(quote! {
            pub trait #methods_trait_ident<
                #(#type_param_idents: ::unity::ClassIdentity),*
            >: ::unity::ClassIdentity {
                #(#method_defaults)*
            }

            #blanket_bounds
        });
    }

    let raw_module_ident = format_ident!("__{}_unity2_raw", self_ident);
    let trait_ident = format_ident!("I{}", self_ident);
    let methods_trait_ident = format_ident!("I{}Methods", self_ident);

    let raw_items = methods.iter().map(|m| {
        let name = &m.name;
        let lookup_mod_ident = format_ident!("__lookup_{}", name);

        let (raw_attr, lookup_mod) = match &m.resolution {
            Resolution::Offset(lit) => (
                quote! { #[::skyline::from_offset(#lit)] },
                quote! {},
            ),
            Resolution::Pattern(s) => (
                quote! { #[::unity_nx_macro::from_pattern(#s)] },
                quote! {},
            ),
            Resolution::Name { name: il_name, args } => {
                let args_count = args.unwrap_or(m.params.len());
                let il_name_lit = il_name.as_str();
                let is_static_lit = m.is_static;
                let param_type_exprs = m.params.iter().map(|p| {
                    let pty = &p.ty;
                    quote! { <#pty as ::unity::IlType>::il_type() }
                });
                (
                    quote! { #[::skyline::from_offset(#lookup_mod_ident::get_offset() as usize)] },
                    quote! {
                        #[doc(hidden)]
                        #[allow(non_snake_case)]
                        pub mod #lookup_mod_ident {
                            use super::*;
                            static METHOD: ::std::sync::LazyLock<
                                ::unity::Il2CppResult<&'static ::unity::il2cpp::MethodInfo>,
                            > = ::std::sync::LazyLock::new(|| {
                                let param_types: &[&'static ::unity::il2cpp::Il2CppType] = &[
                                    #(#param_type_exprs),*
                                ];
                                ::unity::lookup::method_info_on_class_with_signature(
                                    <#self_ty as ::unity::ClassIdentity>::class(),
                                    #il_name_lit,
                                    #args_count,
                                    param_types,
                                    #is_static_lit,
                                )
                            });
                            pub fn get_method_info() -> &'static ::unity::il2cpp::MethodInfo {
                                match &*METHOD {
                                    ::core::result::Result::Ok(mi) => *mi,
                                    ::core::result::Result::Err(e) => panic!(
                                        "#[unity::methods] {}::{} lookup failed: {}",
                                        <#self_ty as ::unity::ClassIdentity>::NAME,
                                        #il_name_lit,
                                        e
                                    ),
                                }
                            }
                            pub fn get_offset() -> usize {
                                let method_ptr = get_method_info().method_ptr;
                                let text = ::unity::scan::get_text();
                                unsafe {
                                    (method_ptr as *const u8).offset_from(text.as_ptr()) as usize
                                }
                            }
                        }
                    },
                )
            }
            Resolution::VtableIndex(idx) => (
                quote! { #[::skyline::from_offset(#lookup_mod_ident::get_offset() as usize)] },
                quote! {
                    #[doc(hidden)]
                    #[allow(non_snake_case)]
                    pub mod #lookup_mod_ident {
                        use super::*;
                        static METHOD: ::std::sync::LazyLock<
                            ::unity::Il2CppResult<&'static ::unity::il2cpp::MethodInfo>,
                        > = ::std::sync::LazyLock::new(|| {
                            ::unity::lookup::method_info_by_vtable_index_on_class(
                                <#self_ty as ::unity::ClassIdentity>::class(),
                                #idx,
                            )
                        });
                        pub fn get_method_info() -> &'static ::unity::il2cpp::MethodInfo {
                            match &*METHOD {
                                ::core::result::Result::Ok(mi) => *mi,
                                ::core::result::Result::Err(e) => panic!(
                                    "#[unity::methods] {}[vtable {}] lookup failed: {}",
                                    <#self_ty as ::unity::ClassIdentity>::NAME,
                                    #idx,
                                    e
                                ),
                            }
                        }
                        pub fn get_offset() -> usize {
                            let method_ptr = get_method_info().method_ptr;
                            let text = ::unity::scan::get_text();
                            unsafe {
                                (method_ptr as *const u8).offset_from(text.as_ptr()) as usize
                            }
                        }
                    }
                },
            ),
        };

        let typed_params = m.params.iter().map(|p| {
            let pname = &p.name;
            let pty = &p.ty;
            quote! { #pname: #pty }
        });
        let receiver = if m.is_static {
            quote! {}
        } else {
            quote! { this: #self_ty, }
        };
        let ret = match &m.return_ty {
            Some(t) => quote! { -> #t },
            None => quote! {},
        };
        quote! {
            #lookup_mod

            #raw_attr
            // Hygienic name for the trailing MethodInfo* slot so users can still declare
            // their own `method_info` param on methods taking *const MethodInfo
            pub fn #name(#receiver #(#typed_params,)* __unity2_method_info: ::unity::OptionalMethod) #ret;
        }
    });

    let static_fns = methods
        .iter()
        .filter(|m| m.is_static)
        .map(|m| build_static_wrapper(m, &raw_module_ident));
    let static_block = if methods.iter().any(|m| m.is_static) {
        quote! {
            impl #self_ty {
                #(#static_fns)*
            }
        }
    } else {
        quote! {}
    };

    let instance_block = if methods.iter().any(|m| !m.is_static) {
        let instance_fns = methods
            .iter()
            .filter(|m| !m.is_static)
            .map(|m| build_instance_wrapper(m, &self_ty, &raw_module_ident, is_value_type));
        if is_value_type {
            quote! {
                impl #self_ty {
                    #(#instance_fns)*
                }
            }
        } else {
            quote! {
                pub trait #methods_trait_ident: #trait_ident {
                    #(#instance_fns)*
                }

                impl<__T: #trait_ident> #methods_trait_ident for __T {}
            }
        }
    } else {
        quote! {}
    };

    let method_info_fns: Vec<TokenStream2> = methods
        .iter()
        .filter(|m| matches!(m.resolution, Resolution::Name { .. } | Resolution::VtableIndex(_)))
        .map(|m| {
            let name = &m.name;
            let helper_name = format_ident!("{}_method_info", name);
            let lookup_mod_ident = format_ident!("__lookup_{}", name);
            quote! {
                pub fn #helper_name() -> &'static ::unity::il2cpp::MethodInfo {
                    #raw_module_ident::#lookup_mod_ident::get_method_info()
                }
            }
        })
        .collect();
    let method_info_block = if method_info_fns.is_empty() {
        quote! {}
    } else {
        quote! {
            impl #self_ty {
                #(#method_info_fns)*
            }
        }
    };

    Ok(quote! {
        #[doc(hidden)]
        #[allow(non_snake_case, non_camel_case_types, clippy::too_many_arguments)]
        mod #raw_module_ident {
            use super::*;
            #(#raw_items)*
        }

        #static_block
        #instance_block
        #method_info_block
    })
}

// Skips `impl Into<T>` coercion on reference-typed params, anonymous lifetimes inside
// impl Trait aren't stable and explicit lifetimes would bloat the wrapper signature
fn type_has_reference(ty: &venial::TypeExpr) -> bool {
    ty.tokens.iter().any(|tt| match tt {
        proc_macro2::TokenTree::Punct(p) => p.as_char() == '&',
        _ => false,
    })
}

fn type_is_mut_pointer(ty: &venial::TypeExpr) -> Option<TokenStream2> {
    let mut iter = ty.tokens.iter();
    let first = iter.next()?;
    let proc_macro2::TokenTree::Punct(p) = first else { return None };
    if p.as_char() != '*' {
        return None;
    }
    let second = iter.next()?;
    let proc_macro2::TokenTree::Ident(id) = second else { return None };
    if id != "mut" {
        return None;
    }
    let inner: TokenStream2 = iter.cloned().collect();
    if inner.is_empty() { None } else { Some(inner) }
}

fn method_info_expr(_m: &Method, _raw_mod: &proc_macro2::Ident) -> TokenStream2 {
    quote! { ::core::option::Option::None }
}

fn build_static_wrapper(m: &Method, raw_mod: &proc_macro2::Ident) -> TokenStream2 {
    let name = &m.name;
    let vis = &m.vis;

    let mut typed_params: Vec<TokenStream2> = Vec::new();
    let mut arg_exprs: Vec<TokenStream2> = Vec::new();
    let mut slot_inits: Vec<TokenStream2> = Vec::new();
    let mut slot_reads: Vec<TokenStream2> = Vec::new();
    let mut out_inner_tys: Vec<TokenStream2> = Vec::new();
    for p in m.params.iter() {
        if let Some(inner) = type_is_mut_pointer(&p.ty) {
            let slot = proc_macro2::Ident::new(
                &format!("__out_{}", out_inner_tys.len()),
                proc_macro2::Span::call_site(),
            );
            slot_inits.push(quote! {
                let mut #slot = ::core::mem::MaybeUninit::<#inner>::uninit();
            });
            arg_exprs.push(quote! { #slot.as_mut_ptr() });
            slot_reads.push(quote! { #slot.assume_init() });
            out_inner_tys.push(inner);
        } else {
            let pname = &p.name;
            let pty = &p.ty;
            if type_has_reference(pty) {
                typed_params.push(quote! { #pname: #pty });
                arg_exprs.push(quote! { #pname });
            } else {
                typed_params.push(quote! { #pname: impl ::core::convert::Into<#pty> });
                arg_exprs.push(quote! { ::core::convert::Into::into(#pname) });
            }
        }
    }

    let ret = match (&m.return_ty, out_inner_tys.len()) {
        (None, 0) => quote! {},
        (Some(r), 0) => quote! { -> #r },
        (None, 1) => {
            let t = &out_inner_tys[0];
            quote! { -> #t }
        }
        (None, _) => {
            let tys = &out_inner_tys;
            quote! { -> (#(#tys),*) }
        }
        (Some(r), 1) => {
            let t = &out_inner_tys[0];
            quote! { -> (#r, #t) }
        }
        (Some(r), _) => {
            let tys = &out_inner_tys;
            quote! { -> (#r, #(#tys),*) }
        }
    };

    let mi_expr = method_info_expr(m, raw_mod);
    let call = quote! { #raw_mod::#name(#(#arg_exprs,)* #mi_expr) };
    let body = match (&m.return_ty, out_inner_tys.len()) {
        (None, 0) | (Some(_), 0) => call,
        (None, 1) => quote! {
            #(#slot_inits)*
            #call;
            #(#slot_reads)*
        },
        (None, _) => quote! {
            #(#slot_inits)*
            #call;
            (#(#slot_reads),*)
        },
        (Some(_), 1) => {
            let read = &slot_reads[0];
            quote! {
                #(#slot_inits)*
                let __ret = { #call };
                (__ret, #read)
            }
        }
        (Some(_), _) => quote! {
            #(#slot_inits)*
            let __ret = { #call };
            (__ret, #(#slot_reads),*)
        },
    };

    if m.is_unsafe {
        quote! {
            #vis unsafe fn #name(#(#typed_params),*) #ret {
                #body
            }
        }
    } else {
        quote! {
            #vis fn #name(#(#typed_params),*) #ret {
                unsafe { #body }
            }
        }
    }
}

// Converts &T/&mut T to *const T/*mut T (same ABI), avoids higher-ranked lifetimes that
// break MethodSignature inference, cast happens at the call site
fn reference_to_raw_pointer(ty: &venial::TypeExpr) -> (proc_macro2::TokenStream, bool) {
    let mut iter = ty.tokens.iter().peekable();
    let Some(proc_macro2::TokenTree::Punct(p)) = iter.peek() else {
        return (ty.to_token_stream(), false);
    };
    if p.as_char() != '&' {
        return (ty.to_token_stream(), false);
    }
    iter.next(); // consume &

    // Optional lifetime ('a), a single punct `'` followed by an ident
    if let Some(proc_macro2::TokenTree::Punct(p)) = iter.peek() {
        if p.as_char() == '\'' {
            iter.next();
            if matches!(iter.peek(), Some(proc_macro2::TokenTree::Ident(_))) {
                iter.next();
            }
        }
    }

    let is_mut = matches!(
        iter.peek(),
        Some(proc_macro2::TokenTree::Ident(id)) if *id == "mut"
    );
    if is_mut {
        iter.next();
    }

    let inner: proc_macro2::TokenStream = iter.cloned().collect();
    let ptr_kind = if is_mut {
        quote! { *mut }
    } else {
        quote! { *const }
    };
    (quote! { #ptr_kind #inner }, true)
}

fn build_generic_trait_default(m: &Method, type_params: &[&proc_macro2::Ident]) -> TokenStream2 {
    let name = &m.name;

    let il2cpp_name = match &m.resolution {
        Resolution::Name { name, .. } => name.clone(),
        _ => unreachable!("non-Name resolutions are rejected upstream for generic impls"),
    };
    let il2cpp_name_lit = il2cpp_name.as_str();

    let il_type_expr = |ty: &venial::TypeExpr| -> TokenStream2 {
        let ty_str = quote! { #ty }.to_string().replace(char::is_whitespace, "");
        if type_params.iter().any(|p| p.to_string() == ty_str) {
            quote! { &<#ty as ::unity::ClassIdentity>::class().raw()._1.byval_arg }
        } else {
            quote! { <#ty as ::unity::IlType>::il_type() }
        }
    };

    let mut typed_params: Vec<TokenStream2> = Vec::new();
    let mut call_exprs: Vec<TokenStream2> = Vec::new();
    let mut abi_types: Vec<TokenStream2> = Vec::new();
    let mut slot_inits: Vec<TokenStream2> = Vec::new();
    let mut slot_reads: Vec<TokenStream2> = Vec::new();
    let mut out_inner_tys: Vec<TokenStream2> = Vec::new();
    let mut sig_type_exprs: Vec<TokenStream2> = Vec::new();
    for p in m.params.iter() {
        sig_type_exprs.push(il_type_expr(&p.ty));
        if let Some(inner) = type_is_mut_pointer(&p.ty) {
            let slot = proc_macro2::Ident::new(
                &format!("__out_{}", out_inner_tys.len()),
                proc_macro2::Span::call_site(),
            );
            slot_inits.push(quote! {
                let mut #slot = ::core::mem::MaybeUninit::<#inner>::uninit();
            });
            abi_types.push(quote! { *mut #inner });
            call_exprs.push(quote! { #slot.as_mut_ptr() });
            slot_reads.push(quote! { #slot.assume_init() });
            out_inner_tys.push(inner);
            continue;
        }
        let pname = &p.name;
        let pty = &p.ty;
        typed_params.push(quote! { #pname: #pty });
        let (abi_ty, is_ref) = reference_to_raw_pointer(&p.ty);
        abi_types.push(abi_ty.clone());
        if is_ref {
            call_exprs.push(quote! { #pname as #abi_ty });
        } else {
            call_exprs.push(quote! { #pname });
        }
    }

    let ret_ty = match &m.return_ty {
        Some(t) => quote! { #t },
        None => quote! { () },
    };
    let ret_clause = match (&m.return_ty, out_inner_tys.len()) {
        (None, 0) => quote! {},
        (Some(r), 0) => quote! { -> #r },
        (None, 1) => {
            let t = &out_inner_tys[0];
            quote! { -> #t }
        }
        (None, _) => {
            let tys = &out_inner_tys;
            quote! { -> (#(#tys),*) }
        }
        (Some(r), 1) => {
            let t = &out_inner_tys[0];
            quote! { -> (#r, #t) }
        }
        (Some(r), _) => {
            let tys = &out_inner_tys;
            quote! { -> (#r, #(#tys),*) }
        }
    };

    let extern_fn_ty = if m.is_static {
        quote! {
            extern "C" fn(
                #(#abi_types,)*
                ::core::option::Option<&'static ::unity::MethodInfo>,
            ) -> #ret_ty
        }
    } else {
        quote! {
            extern "C" fn(
                Self,
                #(#abi_types,)*
                ::core::option::Option<&'static ::unity::MethodInfo>,
            ) -> #ret_ty
        }
    };

    let receiver = if m.is_static {
        quote! {}
    } else {
        quote! { self, }
    };
    let sized_bound = if m.is_static {
        quote! {}
    } else if m.is_abstract {
        quote! { where Self: ::core::marker::Sized + ::core::convert::Into<::unity::IlInstance> }
    } else {
        quote! { where Self: ::core::marker::Sized }
    };
    let call_args = if m.is_static {
        quote! { #(#call_exprs,)* ::core::option::Option::Some(__info) }
    } else {
        quote! { self, #(#call_exprs,)* ::core::option::Option::Some(__info) }
    };

    let unsafe_kw = if m.is_unsafe { quote! { unsafe } } else { quote! {} };
    let missing_msg = format!(
        "unity::methods: `{}` not found on this class or any ancestor",
        il2cpp_name,
    );

    let il2cpp_arg_count = m.params.len();
    let is_static_lit = m.is_static;

    let final_expr = match (&m.return_ty, out_inner_tys.len()) {
        (None, 0) | (Some(_), 0) => quote! { __f(#call_args) },
        (None, 1) => quote! {
            __f(#call_args);
            unsafe { #(#slot_reads)* }
        },
        (None, _) => quote! {
            __f(#call_args);
            unsafe { (#(#slot_reads),*) }
        },
        (Some(_), 1) => {
            let read = &slot_reads[0];
            quote! {
                let __ret = __f(#call_args);
                (__ret, unsafe { #read })
            }
        }
        (Some(_), _) => quote! {
            let __ret = __f(#call_args);
            unsafe { (__ret, #(#slot_reads),*) }
        },
    };

    let resolve_block = if m.is_abstract && !m.is_static {
        quote! {
            let __vi = ::unity::Cast::get_class(self)
                .raw()
                .get_virtual_method(#il2cpp_name_lit)
                .unwrap_or_else(|| panic!(
                    "unity::methods: abstract `{}` not found on the runtime vtable",
                    #il2cpp_name_lit,
                ));
            let __ptr = __vi.method_ptr as usize;
            let __info: &'static ::unity::MethodInfo = __vi.method_info;
        }
    } else {
        quote! {
            static CACHE: ::std::sync::OnceLock<
                ::std::sync::Mutex<
                    ::std::collections::HashMap<
                        usize,
                        (usize, &'static ::unity::MethodInfo),
                    >,
                >,
            > = ::std::sync::OnceLock::new();
            let __class = <Self as ::unity::ClassIdentity>::class();
            let __key = __class.raw() as *const _ as usize;
            let __map = CACHE.get_or_init(|| {
                ::std::sync::Mutex::new(::std::collections::HashMap::new())
            });
            let (__ptr, __info) = {
                let mut __guard = __map.lock().unwrap();
                *__guard.entry(__key).or_insert_with(|| {
                    let __param_types: &[&'static ::unity::il2cpp::Il2CppType] = &[
                        #(#sig_type_exprs),*
                    ];
                    let __mi = ::unity::lookup::method_info_disambiguated(
                        __class,
                        #il2cpp_name_lit,
                        #il2cpp_arg_count,
                        __param_types,
                        #is_static_lit,
                    )
                    .expect(#missing_msg);
                    if let ::core::option::Option::Some(__decl) = __mi.class {
                        ::unity::Class::from_raw(__decl).init();
                    }
                    (__mi.method_ptr as usize, __mi)
                })
            };
        }
    };

    quote! {
        #unsafe_kw fn #name(#receiver #(#typed_params),*) #ret_clause #sized_bound {
            #resolve_block
            let __f: #extern_fn_ty = unsafe { ::std::mem::transmute(__ptr) };
            #(#slot_inits)*
            #final_expr
        }
    }
}

fn build_instance_wrapper(
    m: &Method,
    self_ty: &venial::TypeExpr,
    raw_mod: &proc_macro2::Ident,
    is_value_type: bool,
) -> TokenStream2 {
    let name = &m.name;

    let mut typed_params: Vec<TokenStream2> = Vec::new();
    let mut arg_exprs: Vec<TokenStream2> = Vec::new();
    let mut slot_inits: Vec<TokenStream2> = Vec::new();
    let mut slot_reads: Vec<TokenStream2> = Vec::new();
    let mut out_inner_tys: Vec<TokenStream2> = Vec::new();
    for p in m.params.iter() {
        if let Some(inner) = type_is_mut_pointer(&p.ty) {
            let slot = proc_macro2::Ident::new(
                &format!("__out_{}", out_inner_tys.len()),
                proc_macro2::Span::call_site(),
            );
            slot_inits.push(quote! {
                let mut #slot = ::core::mem::MaybeUninit::<#inner>::uninit();
            });
            arg_exprs.push(quote! { #slot.as_mut_ptr() });
            slot_reads.push(quote! { #slot.assume_init() });
            out_inner_tys.push(inner);
        } else {
            let pname = &p.name;
            let pty = &p.ty;
            if type_has_reference(pty) {
                typed_params.push(quote! { #pname: #pty });
                arg_exprs.push(quote! { #pname });
            } else {
                typed_params.push(quote! { #pname: impl ::core::convert::Into<#pty> });
                arg_exprs.push(quote! { ::core::convert::Into::into(#pname) });
            }
        }
    }

    let ret = match (&m.return_ty, out_inner_tys.len()) {
        (None, 0) => quote! {},
        (Some(r), 0) => quote! { -> #r },
        (None, 1) => {
            let t = &out_inner_tys[0];
            quote! { -> #t }
        }
        (None, _) => {
            let tys = &out_inner_tys;
            quote! { -> (#(#tys),*) }
        }
        (Some(r), _) => {
            let tys = &out_inner_tys;
            quote! { -> (#r, #(#tys),*) }
        }
    };

    let mi_expr = method_info_expr(m, raw_mod);
    let call_expr = if is_value_type {
        quote! { #raw_mod::#name(self, #(#arg_exprs,)* #mi_expr) }
    } else {
        quote! {
            let __receiver = <#self_ty as ::unity::FromIlInstance>::from_il_instance(
                <Self as ::unity::SystemObject>::as_instance(self),
            );
            #raw_mod::#name(__receiver, #(#arg_exprs,)* #mi_expr)
        }
    };

    let body = match (&m.return_ty, out_inner_tys.len()) {
        (None, 0) | (Some(_), 0) => call_expr,
        (None, 1) => quote! {
            #(#slot_inits)*
            #call_expr;
            #(#slot_reads)*
        },
        (None, _) => quote! {
            #(#slot_inits)*
            #call_expr;
            (#(#slot_reads),*)
        },
        (Some(_), 1) => {
            let read = &slot_reads[0];
            quote! {
                #(#slot_inits)*
                let __ret = { #call_expr };
                (__ret, #read)
            }
        }
        (Some(_), _) => quote! {
            #(#slot_inits)*
            let __ret = { #call_expr };
            (__ret, #(#slot_reads),*)
        },
    };

    let vis = if is_value_type {
        let v = &m.vis;
        quote! { #v }
    } else {
        quote! {}
    };

    if m.is_unsafe {
        quote! {
            #vis unsafe fn #name(self, #(#typed_params),*) #ret {
                #body
            }
        }
    } else {
        quote! {
            #vis fn #name(self, #(#typed_params),*) #ret {
                unsafe { #body }
            }
        }
    }
}

struct InjectedField {
    name: proc_macro2::Ident,
    ty: TokenStream2,
}

fn inject_inner(attr: TokenStream2, item: venial::Item) -> ParseResult<TokenStream2> {
    use proc_macro2::{Literal, Span, TokenTree};
    use quote::spanned::Spanned;

    let class = match item {
        venial::Item::Struct(s) => s,
        other => {
            return Err(venial::Error::new_at_span(
                other.__span(),
                "#[unity::inject] can only be applied to structs",
            ));
        }
    };

    let class_ident = class.name.clone();
    let vis = class.vis_marker.clone();

    let mut parser = crate::util::KvParser::parse_args("inject", attr, Span::call_site())?;

    let namespace = parser
        .handle_literal("namespace", "string")?
        .map(|lit| crate::data_models::field::unquote_string_literal(&lit.to_string()))
        .ok_or_else(|| {
            venial::Error::new_at_span(
                class_ident.__span(),
                "#[unity::inject(...)] requires `namespace = \"...\"`",
            )
        })?;

    let name = parser
        .handle_literal("name", "string")?
        .map(|lit| crate::data_models::field::unquote_string_literal(&lit.to_string()))
        .ok_or_else(|| {
            venial::Error::new_at_span(
                class_ident.__span(),
                "#[unity::inject(...)] requires `name = \"...\"`",
            )
        })?;

    let parent_expr = parser.handle_expr_required("parent")?;

    parser.finish()?;

    let injected_fields: Vec<InjectedField> = match &class.fields {
        venial::Fields::Unit => Vec::new(),
        venial::Fields::Named(named) => named
            .fields
            .iter()
            .map(|(field, _)| InjectedField {
                name: field.name.clone(),
                ty: field.ty.tokens.iter().cloned().collect(),
            })
            .collect(),
        venial::Fields::Tuple(_) => {
            return Err(venial::Error::new_at_span(
                class_ident.__span(),
                "#[unity::inject] expects either a unit struct or a named-field struct (not a tuple struct)",
            ));
        }
    };

    let passthrough_attrs: Vec<&venial::Attribute> = class
        .attributes
        .iter()
        .filter(|a| {
            !crate::util::path_is_single(&a.path, "inject")
                && !crate::util::path_is_single(&a.path, "with")
        })
        .collect();

    let parent_tokens: Vec<TokenTree> = parent_expr.clone().into_iter().collect();
    let base_idx = parent_tokens
        .iter()
        .rposition(|t| matches!(t, TokenTree::Ident(_)))
        .ok_or_else(|| {
            venial::Error::new_at_span(
                class_ident.__span(),
                "#[inject(parent = ...)] must end with a type identifier",
            )
        })?;
    let base_ident = match &parent_tokens[base_idx] {
        TokenTree::Ident(i) => i.clone(),
        _ => unreachable!(),
    };
    let ancestor_macro_ident = format_ident!("__{}_ancestor_impls", base_ident);

    let crate_prefix: TokenStream2 = {
        let is_colon = |t: Option<&TokenTree>| {
            matches!(t, Some(TokenTree::Punct(p)) if p.as_char() == ':')
        };
        let mut out: Vec<TokenTree> = Vec::new();
        let mut i = 0usize;
        if is_colon(parent_tokens.get(0)) && is_colon(parent_tokens.get(1)) {
            out.push(parent_tokens[0].clone());
            out.push(parent_tokens[1].clone());
            i = 2;
        }
        if matches!(parent_tokens.get(i), Some(TokenTree::Ident(_)))
            && is_colon(parent_tokens.get(i + 1))
            && is_colon(parent_tokens.get(i + 2))
        {
            out.push(parent_tokens[i].clone());
            out.push(parent_tokens[i + 1].clone());
            out.push(parent_tokens[i + 2].clone());
        }
        out.into_iter().collect()
    };

    let parent_module: TokenStream2 = {
        let mut prefix_end = base_idx;
        while prefix_end > 0
            && matches!(parent_tokens.get(prefix_end - 1), Some(TokenTree::Punct(p)) if p.as_char() == ':')
        {
            prefix_end -= 1;
        }
        parent_tokens[..prefix_end].iter().cloned().collect()
    };

    let parent_trait_ident = format_ident!("I{}", base_ident);
    let (seed_ty, seed_tr): (TokenStream2, TokenStream2) = if parent_module.is_empty() {
        (quote! { #base_ident }, quote! { #parent_trait_ident })
    } else {
        (
            quote! { #parent_module::#base_ident },
            quote! { #parent_module::#parent_trait_ident },
        )
    };
    let raw_cascade: TokenStream2 = if crate_prefix.is_empty() {
        quote! { #ancestor_macro_ident!(#class_ident, [#seed_ty], [#seed_tr]); }
    } else {
        quote! { #crate_prefix #ancestor_macro_ident!(#class_ident, [#seed_ty], [#seed_tr]); }
    };
    let inheritance_invocation: TokenStream2 = quote! {
        const _: () = {
            #raw_cascade
        };
    };

    let namespace_cstr = Literal::byte_string(format!("{}\0", namespace).as_bytes());
    let name_cstr = Literal::byte_string(format!("{}\0", name).as_bytes());

    let extra_bytes_const: TokenStream2 = if injected_fields.is_empty() {
        quote! { 0u32 }
    } else {
        let field_size_terms = injected_fields.iter().map(|f| {
            let ty = &f.ty;
            quote! {
                __off = __align_up(__off, ::core::mem::align_of::<#ty>())
                    + ::core::mem::size_of::<#ty>();
            }
        });
        quote! {
            {
                const fn __align_up(off: usize, align: usize) -> usize {
                    (off + align - 1) & !(align - 1)
                }
                let mut __off: usize = 0;
                #(#field_size_terms)*
                __align_up(__off, 8) as u32
            }
        }
    };

    let field_accessors_block: TokenStream2 = if injected_fields.is_empty() {
        TokenStream2::new()
    } else {
        let mut field_items: Vec<TokenStream2> = Vec::new();

        for (i, f) in injected_fields.iter().enumerate() {
            let getter = f.name.clone();
            let setter = format_ident!("set_{}", f.name);
            let offset_fn = format_ident!("__{}_offset", f.name);
            let ty = &f.ty;

            let prev_terms = injected_fields[..i].iter().map(|prev| {
                let pty = &prev.ty;
                quote! {
                    let __a = ::core::mem::align_of::<#pty>();
                    __off = (__off + __a - 1) & !(__a - 1);
                    __off += ::core::mem::size_of::<#pty>();
                }
            });

            field_items.push(quote! {
                #[inline]
                fn #offset_fn() -> usize {
                    let mut __off: usize =
                        <#parent_expr as ::unity::ClassIdentity>::class().instance_size() as usize;
                    #(#prev_terms)*
                    let __a = ::core::mem::align_of::<#ty>();
                    (__off + __a - 1) & !(__a - 1)
                }

                #[inline]
                pub fn #getter(self) -> #ty {
                    ::unity::field_get_value_at_offset(self, #class_ident::#offset_fn())
                }

                #[inline]
                pub fn #setter(self, value: #ty) {
                    ::unity::field_set_value_at_offset(self, #class_ident::#offset_fn(), value);
                }
            });
        }

        let field_descriptors = injected_fields.iter().map(|f| {
            let fname = &f.name;
            let pname_lit = Literal::byte_string(format!("{}\0", fname).as_bytes());
            let offset_fn = format_ident!("__{}_offset", fname);
            let ty = &f.ty;
            quote! {
                ::unity::injection::InjectedFieldDescriptor {
                    name: unsafe { ::core::ffi::CStr::from_bytes_with_nul_unchecked(#pname_lit) },
                    ty: <#ty as ::unity::IlType>::il_type(),
                    offset: #class_ident::#offset_fn() as u32,
                }
            }
        });

        quote! {
            impl #class_ident {
                #(#field_items)*

                pub fn __injected_fields() -> ::std::vec::Vec<::unity::injection::InjectedFieldDescriptor> {
                    ::std::vec![
                        #(#field_descriptors),*
                    ]
                }
            }
        }
    };

    Ok(quote! {
        #(#passthrough_attrs)*
        #[repr(transparent)]
        #[derive(::core::clone::Clone, ::core::marker::Copy)]
        #vis struct #class_ident(::unity::IlInstance);

        impl ::core::convert::From<#class_ident> for ::unity::IlInstance {
            #[inline]
            fn from(value: #class_ident) -> Self {
                value.0
            }
        }

        impl ::core::convert::AsRef<::unity::IlInstance> for #class_ident {
            #[inline]
            fn as_ref(&self) -> &::unity::IlInstance {
                &self.0
            }
        }

        impl ::unity::ClassIdentity for #class_ident {
            const NAMESPACE: &'static str = #namespace;
            const NAME: &'static str = #name;

            fn class() -> ::unity::Class {
                *<Self as ::unity::injection::InjectedClass>::cache().get_or_init(|| {
                    ::unity::Class::try_lookup(Self::NAMESPACE, Self::NAME).unwrap_or_else(|_| {
                        panic!(concat!(
                            "injected class ",
                            stringify!(#class_ident),
                            " not found, register it once with register::<",
                            stringify!(#class_ident),
                            ">() before use",
                        ))
                    })
                })
            }
        }

        impl ::unity::FromIlInstance for #class_ident {
            #[inline]
            fn from_il_instance(instance: ::unity::IlInstance) -> Self {
                Self(instance)
            }
        }

        impl ::unity::IlType for #class_ident {
            fn il_type() -> &'static ::unity::il2cpp::Il2CppType {
                &<Self as ::unity::ClassIdentity>::class().raw()._1.byval_arg
            }
        }

        impl ::unity::injection::InjectedClass for #class_ident {
            type Parent = #parent_expr;
            const EXTRA_BYTES: u32 = #extra_bytes_const;

            const NAME_CSTR: &'static ::core::ffi::CStr = match ::core::ffi::CStr::from_bytes_with_nul(#name_cstr) {
                ::core::result::Result::Ok(s) => s,
                ::core::result::Result::Err(_) => panic!("invalid CStr literal in #[unity::inject]"),
            };
            const NAMESPACE_CSTR: &'static ::core::ffi::CStr = match ::core::ffi::CStr::from_bytes_with_nul(#namespace_cstr) {
                ::core::result::Result::Ok(s) => s,
                ::core::result::Result::Err(_) => panic!("invalid CStr literal in #[unity::inject]"),
            };

            fn cache() -> &'static ::std::sync::OnceLock<::unity::Class> {
                static CACHE: ::std::sync::OnceLock<::unity::Class> = ::std::sync::OnceLock::new();
                &CACHE
            }

            fn injected_fields() -> ::std::vec::Vec<::unity::injection::InjectedFieldDescriptor> {
                use ::unity::injection::DefaultInjectedMembers as _;
                Self::__injected_fields()
            }

            fn injected_methods() -> ::std::vec::Vec<::unity::injection::InjectedMethodDescriptor> {
                use ::unity::injection::DefaultInjectedMembers as _;
                Self::__injected_methods()
            }

            fn injected_overrides() -> ::std::vec::Vec<::unity::injection::InjectedOverrideDescriptor> {
                use ::unity::injection::DefaultInjectedMembers as _;
                Self::__injected_overrides()
            }
        }

        #inheritance_invocation

        #field_accessors_block
    })
}

fn injected_methods_inner(
    _attr: TokenStream2,
    item: venial::Item,
) -> ParseResult<TokenStream2> {
    use proc_macro2::Literal;

    let mut impl_block = match item {
        venial::Item::Impl(i) => i,
        _ => {
            return Err(venial::Error::new(
                "#[unity::injected_methods] requires an inherent `impl` block",
            ));
        }
    };

    if impl_block.trait_ty.is_some() {
        return Err(venial::Error::new(
            "#[unity::injected_methods] does not support trait impls; use `impl Foo { ... }`",
        ));
    }

    let self_ty = impl_block.self_ty.clone();
    let self_ident = util::extract_typename(&self_ty)
        .map(|seg| seg.ident)
        .ok_or_else(|| {
            venial::Error::new(
                "#[unity::injected_methods] requires `impl Foo {...}` (single type ident)",
            )
        })?;

    let mut shims: Vec<TokenStream2> = Vec::new();
    let mut descriptors: Vec<TokenStream2> = Vec::new();
    let mut overrides: Vec<TokenStream2> = Vec::new();

    for member in &mut impl_block.body_items {
        let func = match member {
            venial::ImplMember::AssocFunction(f) => f,
            _ => {
                return Err(venial::Error::new(
                    "#[unity::injected_methods] only supports function items",
                ));
            }
        };

        let fn_ident = func.name.clone();

        let mut il2cpp_name = fn_ident.to_string();
        let mut had_rename = false;
        if let Some(mut parser) = crate::util::KvParser::parse_remove(&mut func.attributes, "rename")? {
            if let Some(lit) = parser.handle_literal("name", "string")? {
                il2cpp_name = crate::data_models::field::unquote_string_literal(&lit.to_string());
            }
            parser.finish()?;
            had_rename = true;
        }

        let mut override_slot: Option<String> = None;
        if let Some(mut parser) = crate::util::KvParser::parse_remove(&mut func.attributes, "override_virtual")? {
            let slot = match parser.handle_literal("name", "string")? {
                Some(lit) => crate::data_models::field::unquote_string_literal(&lit.to_string()),
                None => fn_ident.to_string(),
            };
            parser.finish()?;
            override_slot = Some(slot);
        }

        if had_rename && override_slot.is_some() {
            return Err(venial::Error::new(
                "a method can be #[rename] or #[override_virtual], not both",
            ));
        }

        let shim_ident = format_ident!("__inject_shim_{}_{}", self_ident, fn_ident);
        let il2cpp_name_lit = Literal::byte_string(format!("{}\0", il2cpp_name).as_bytes());

        let mut typed_params: Vec<(proc_macro2::Ident, TokenStream2)> = Vec::new();
        let mut has_receiver = false;
        for (param, _) in func.params.inner.iter() {
            match param {
                venial::FnParam::Receiver(_) => {
                    has_receiver = true;
                }
                venial::FnParam::Typed(t) => {
                    let ty_tokens: TokenStream2 = t.ty.tokens.iter().cloned().collect();
                    typed_params.push((t.name.clone(), ty_tokens));
                }
            }
        }
        if !has_receiver {
            return Err(venial::Error::new(
                "#[unity::injected_methods] functions must take `self` as the receiver",
            ));
        }

        let return_ty_tokens: TokenStream2 = match &func.return_ty {
            Some(rt) => rt.tokens.iter().cloned().collect(),
            None => quote! { () },
        };

        let shim_param_decls = typed_params.iter().map(|(n, t)| quote! { #n: #t });
        let shim_call_args = typed_params.iter().map(|(n, _)| quote! { #n });

        shims.push(quote! {
            #[allow(non_snake_case, unused_variables)]
            extern "C" fn #shim_ident(
                this: #self_ty,
                #(#shim_param_decls,)*
                _mi: ::unity::OptionalMethod,
            ) -> #return_ty_tokens {
                <#self_ty>::#fn_ident(this, #(#shim_call_args),*)
            }
        });

        if let Some(slot) = override_slot {
            let slot_lit = Literal::byte_string(format!("{}\0", slot).as_bytes());
            overrides.push(quote! {
                ::unity::injection::InjectedOverrideDescriptor {
                    name: unsafe { ::core::ffi::CStr::from_bytes_with_nul_unchecked(#slot_lit) },
                    method_ptr: #shim_ident as *mut u8,
                }
            });
            continue;
        }

        let param_descriptors = typed_params.iter().map(|(n, t)| {
            let pname_lit = Literal::byte_string(format!("{}\0", n).as_bytes());
            quote! {
                ::unity::injection::InjectedParameterDescriptor {
                    name: unsafe { ::core::ffi::CStr::from_bytes_with_nul_unchecked(#pname_lit) },
                    ty: <#t as ::unity::IlType>::il_type(),
                }
            }
        });

        descriptors.push(quote! {
            ::unity::injection::InjectedMethodDescriptor {
                name: unsafe { ::core::ffi::CStr::from_bytes_with_nul_unchecked(#il2cpp_name_lit) },
                method_ptr: #shim_ident as *mut u8,
                return_type: <#return_ty_tokens as ::unity::IlType>::il_type(),
                parameters: ::std::vec![#(#param_descriptors),*],
            }
        });
    }

    Ok(quote! {
        #impl_block

        #(#shims)*

        impl #self_ty {
            pub fn __injected_methods() -> ::std::vec::Vec<::unity::injection::InjectedMethodDescriptor> {
                ::std::vec![
                    #(#descriptors),*
                ]
            }

            pub fn __injected_overrides() -> ::std::vec::Vec<::unity::injection::InjectedOverrideDescriptor> {
                ::std::vec![
                    #(#overrides),*
                ]
            }
        }
    })
}

#[cfg(test)]
mod infer_il_arg_count_tests {
    use super::infer_il_arg_count;

    fn count(src: &str) -> usize {
        let item = venial::parse_item(src.parse().unwrap()).unwrap();
        match item {
            venial::Item::Function(f) => infer_il_arg_count(&f),
            _ => panic!("expected a function"),
        }
    }

    #[test]
    fn drops_this_and_method_info_only() {
        assert_eq!(
            count(r#"extern "C" fn h(this: u64, a: i32, b: i32, method_info: u64) {}"#),
            2
        );
        assert_eq!(
            count(r#"extern "C" fn h(_this: u64, a: i32, _method_info: u64) {}"#),
            1
        );
    }

    #[test]
    fn keeps_unused_underscore_arguments() {
        assert_eq!(
            count(r#"extern "C" fn h(this: u64, _unused: i32, b: i32, method_info: u64) {}"#),
            2
        );
        assert_eq!(
            count(r#"extern "C" fn h(this: u64, _a: i32, _b: i32, _method_info: u64) {}"#),
            2
        );
    }

    #[test]
    fn static_method_has_no_this() {
        assert_eq!(count(r#"extern "C" fn h(a: i32, method_info: u64) {}"#), 1);
    }
}
