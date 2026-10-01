use proc_macro2::{Ident, Span, TokenStream, TokenTree};

use crate::ParseResult;
use crate::data_models::field::unquote_string_literal;
use crate::util::{KvParser, bail, path_is_single};

pub struct ClassAttrs {
    pub namespace: Option<String>,
    pub name: String,
    // First entry is the direct parent, rest are progressively older ancestors
    pub parents: Vec<ParentType>,
}

pub struct ParentType {
    pub path_prefix: TokenStream,
    pub base: Ident,
    pub generics: TokenStream,
}

impl ParentType {
    pub fn macro_path_prefix(&self) -> TokenStream {
        let toks: Vec<TokenTree> = self.path_prefix.clone().into_iter().collect();
        let mut out: Vec<TokenTree> = Vec::new();
        let mut i = 0;
        let is_colon = |t: Option<&TokenTree>| {
            matches!(t, Some(TokenTree::Punct(p)) if p.as_char() == ':')
        };
        if is_colon(toks.get(0)) && is_colon(toks.get(1)) {
            out.push(toks[0].clone());
            out.push(toks[1].clone());
            i = 2;
        }
        if matches!(toks.get(i), Some(TokenTree::Ident(_))) {
            out.push(toks[i].clone());
            i += 1;
        } else {
            return out.into_iter().collect();
        }
        if is_colon(toks.get(i)) && is_colon(toks.get(i + 1)) {
            out.push(toks[i].clone());
            out.push(toks[i + 1].clone());
        }
        out.into_iter().collect()
    }
}

pub fn parse_parent_attr(attributes: &[venial::Attribute]) -> ParseResult<Vec<ParentType>> {
    let mut found: Vec<ParentType> = Vec::new();

    for attr in attributes {
        if !path_is_single(&attr.path, "parent") {
            continue;
        }

        let tokens = attr.value.get_value_tokens();
        let mut entries: Vec<ParentType> = Vec::new();
        let mut path_tokens: Vec<TokenTree> = Vec::new();
        let mut current_base: Option<Ident> = None;
        let mut current_generics: TokenStream = TokenStream::new();
        let mut in_generics = false;
        let mut depth: i32 = 0;

        let finalize = |path_tokens: &mut Vec<TokenTree>,
                            current_base: &mut Option<Ident>,
                            current_generics: &mut TokenStream,
                            entries: &mut Vec<ParentType>|
         -> ParseResult<()> {
            let base = current_base.take().ok_or_else(|| {
                venial::Error::new("#[parent(...)] entry is missing a type")
            })?;
            entries.push(ParentType {
                path_prefix: std::mem::take(path_tokens).into_iter().collect(),
                base,
                generics: std::mem::take(current_generics),
            });
            Ok(())
        };

        for tt in tokens.iter() {
            if in_generics {
                match tt {
                    TokenTree::Punct(p) if p.as_char() == '<' => {
                        depth += 1;
                        current_generics.extend(std::iter::once(tt.clone()));
                    }
                    TokenTree::Punct(p) if p.as_char() == '>' => {
                        depth -= 1;
                        current_generics.extend(std::iter::once(tt.clone()));
                        if depth == 0 {
                            in_generics = false;
                        }
                    }
                    _ => current_generics.extend(std::iter::once(tt.clone())),
                }
                continue;
            }

            match tt {
                TokenTree::Punct(p) if p.as_char() == ',' => {
                    finalize(
                        &mut path_tokens,
                        &mut current_base,
                        &mut current_generics,
                        &mut entries,
                    )?;
                }
                TokenTree::Punct(p) if p.as_char() == '<' => {
                    depth = 1;
                    in_generics = true;
                    current_generics.extend(std::iter::once(tt.clone()));
                }
                TokenTree::Punct(p) if p.as_char() == ':' => {
                    if let Some(prev) = current_base.take() {
                        path_tokens.push(TokenTree::Ident(prev));
                    }
                    path_tokens.push(tt.clone());
                }
                TokenTree::Ident(id) => {
                    if let Some(prev) = current_base.take() {
                        return bail!(
                            tt,
                            "#[parent(...)] expected `::` between path segments, got `{} {}`",
                            prev,
                            id,
                        );
                    }
                    current_base = Some(id.clone());
                }
                _ => {
                    return bail!(
                        tt,
                        "#[parent(...)] entry must be a path like `Foo`, \
                         `unity::Foo`, or `::unity::Foo<T>`",
                    );
                }
            }
        }

        if current_base.is_some() {
            finalize(
                &mut path_tokens,
                &mut current_base,
                &mut current_generics,
                &mut entries,
            )?;
        }

        if entries.is_empty() {
            return bail!(attr, "#[parent(...)] expects at least one type");
        }

        found.extend(entries);
    }

    Ok(found)
}

impl ClassAttrs {
    pub fn parse(
        attr: TokenStream,
        default_name: String,
        struct_attrs: &[venial::Attribute],
    ) -> ParseResult<Self> {
        let mut parser = KvParser::parse_args("class", attr, Span::call_site())?;

        let namespace = parser
            .handle_literal("namespace", "string")?
            .map(|lit| unquote_string_literal(&lit.to_string()));

        let name = parser
            .handle_literal("name", "string")?
            .map(|lit| unquote_string_literal(&lit.to_string()))
            .unwrap_or(default_name);

        parser.finish()?;

        let parents = parse_parent_attr(struct_attrs)?;

        Ok(Self {
            namespace,
            name,
            parents,
        })
    }
}
