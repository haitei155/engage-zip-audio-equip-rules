// generates a lazy pattern-scan + transmute shim from an extern fn declaration,
// equivalent to the original lazysimd_macro but parsed with venial

use proc_macro2::TokenStream as TokenStream2;
use quote::quote;

use crate::ParseResult;

pub fn expand(attr: TokenStream2, item: venial::Item) -> ParseResult<TokenStream2> {
    let func = match item {
        venial::Item::Function(f) => f,
        _ => {
            return Err(venial::Error::new(
                "#[unity_nx_macro::from_pattern] can only be applied to fn declarations",
            ));
        }
    };

    let vis = func.vis_marker.as_ref().map(|v| quote! { #v }).unwrap_or_default();
    let name = &func.name;
    let generics = func
        .generic_params
        .as_ref()
        .map(|g| quote! { #g })
        .unwrap_or_default();
    let where_clause = func
        .where_clause
        .as_ref()
        .map(|w| quote! { #w })
        .unwrap_or_default();

    // collect typed params, reject self receivers
    let mut typed_params: Vec<&venial::FnTypedParam> = Vec::new();
    for (p, _) in func.params.inner.iter() {
        match p {
            venial::FnParam::Typed(t) => typed_params.push(t),
            _ => {
                return Err(venial::Error::new(
                    "#[unity_nx_macro::from_pattern] does not support self receivers",
                ));
            }
        }
    }

    let param_decls = typed_params.iter().map(|t| {
        let n = &t.name;
        let ty = &t.ty;
        quote! { #n: #ty }
    });
    let param_types = typed_params.iter().map(|t| &t.ty);
    let param_names = typed_params.iter().map(|t| &t.name);

    let ret_arrow = match &func.return_ty {
        Some(t) => quote! { -> #t },
        None => quote! {},
    };

    Ok(quote! {
        #vis unsafe fn #name #generics (#(#param_decls),*) #ret_arrow #where_clause {
            static OFFSETS: ::std::sync::LazyLock<usize> = ::std::sync::LazyLock::new(|| {
                let text = ::unity::scan::get_text();
                ::lazysimd::get_offset_neon(&text, #attr).unwrap()
            });

            let inner = ::core::mem::transmute::<_, extern "C" fn(#(#param_types),*) #ret_arrow>(
                unsafe { ::skyline::hooks::getRegionAddress(::skyline::hooks::Region::Text) as *const u8 }
                    .offset(*OFFSETS as isize)
            );
            inner(#(#param_names),*)
        }
    })
}
