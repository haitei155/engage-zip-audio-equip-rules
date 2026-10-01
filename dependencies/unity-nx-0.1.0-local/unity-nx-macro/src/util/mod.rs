/*
 * Copyright (c) godot-rust; Bromeon and contributors.
 * This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at https://mozilla.org/MPL/2.0/.
 */

use proc_macro2::{Ident, Span, TokenTree};
use quote::format_ident;
use quote::spanned::Spanned;

use crate::ParseResult;

mod kv_parser;

pub(crate) use kv_parser::KvParser;

/// Creates an identifier with a fresh `Span::call_site()` span.
///
/// Use this to generate internal/synthetic identifiers that should *not* be attributed to user code. This prevents IDE features
/// (like "unsafe call site" syntax highlighting) from pointing to unrelated user symbols.
///
/// For identifiers that *should* map back to user code (for navigation, error messages), use `format_ident!("...", span = original.span())`.
pub fn ident(s: &str) -> Ident {
    format_ident!("{}", s)
}

pub fn bail_fn<R, T>(msg: impl AsRef<str>, tokens: T) -> ParseResult<R>
where
    T: Spanned,
{
    Err(error_fn(msg, tokens))
}

macro_rules! bail {
    ($tokens:expr_2021, $format_string:literal $($rest:tt)*) => {
        $crate::util::bail_fn(format!($format_string $($rest)*), $tokens)
    }
}

pub fn span_of<T: Spanned>(tokens: &T) -> Span {
    tokens.__span()
}

pub fn error_fn<T: Spanned>(msg: impl AsRef<str>, tokens: T) -> venial::Error {
    let span = span_of(&tokens);
    venial::Error::new_at_span(span, msg.as_ref())
}

macro_rules! error {
    ($tokens:expr_2021, $format_string:literal $($rest:tt)*) => {
        $crate::util::error_fn(format!($format_string $($rest)*), $tokens)
    }
}

pub(crate) use bail;
pub(crate) use error;

fn is_punct(tt: &TokenTree, c: char) -> bool {
    match tt {
        TokenTree::Punct(punct) => punct.as_char() == c,
        _ => false,
    }
}

/// Gets the right-most type name in the path.
pub(crate) fn extract_typename(ty: &venial::TypeExpr) -> Option<venial::PathSegment> {
    match ty.as_path() {
        Some(mut path) => path.segments.pop(),
        _ => None,
    }
}

pub(crate) fn path_is_single(path: &[TokenTree], expected: &str) -> bool {
    path.len() == 1 && path[0].to_string() == expected
}
