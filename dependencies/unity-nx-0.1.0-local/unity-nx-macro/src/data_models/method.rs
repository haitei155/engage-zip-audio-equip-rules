use proc_macro2::Literal;
use venial::{FnParam, FnTypedParam, TypeExpr, VisMarker};

use crate::ParseResult;
use crate::data_models::field::unquote_string_literal;
use crate::util::{KvParser, bail};

pub enum Resolution {
    Offset(Literal),
    Pattern(String),
    Name {
        name: String,
        args: Option<usize>,
    },
    VtableIndex(Literal),
}

pub struct Method {
    pub name: proc_macro2::Ident,
    pub vis: Option<VisMarker>,
    pub is_static: bool,
    pub params: Vec<FnTypedParam>,
    pub return_ty: Option<TypeExpr>,
    pub resolution: Resolution,
    pub is_unsafe: bool,
    pub is_abstract: bool,
}

impl Method {
    pub fn parse(func: &venial::Function) -> ParseResult<Self> {
        if func.body.is_some() {
            return bail!(
                &func.name,
                "method declarations inside #[unity::methods] must have no body"
            );
        }

        // Bare #[method] (no parens) means "look up by name = rust ident"
        let mut parser = KvParser::parse(&func.attributes, "method")?;

        if let Some(p) = parser.as_mut() {
            if let Some((id, _)) = p.handle_any_entry("token") {
                return bail!(
                    id,
                    "`token` resolution is not yet supported; use `name`, `vtable_index`, `offset`, or `pattern`"
                );
            }
        }

        let offset_lit = parser
            .as_mut()
            .map(|p| p.handle_literal("offset", "integer"))
            .transpose()?
            .flatten();
        let pattern_lit = parser
            .as_mut()
            .map(|p| p.handle_literal("pattern", "string"))
            .transpose()?
            .flatten();
        let name_lit = parser
            .as_mut()
            .map(|p| p.handle_literal("name", "string"))
            .transpose()?
            .flatten();
        let args_lit = parser
            .as_mut()
            .map(|p| p.handle_usize("args"))
            .transpose()?
            .flatten();
        let vtable_index_lit = parser
            .as_mut()
            .map(|p| p.handle_literal("vtable_index", "integer"))
            .transpose()?
            .flatten();
        let is_unsafe = parser
            .as_mut()
            .map(|p| p.handle_alone("unsafe"))
            .transpose()?
            .unwrap_or(false);
        let is_abstract = parser
            .as_mut()
            .map(|p| p.handle_alone("abstract_dispatch"))
            .transpose()?
            .unwrap_or(false);
        if let Some(p) = parser {
            p.finish()?;
        }

        // The four resolution kinds are mutually exclusive, `args` pairs with `name` only
        let mut chosen: Vec<&'static str> = Vec::new();
        if offset_lit.is_some() {
            chosen.push("offset");
        }
        if pattern_lit.is_some() {
            chosen.push("pattern");
        }
        if name_lit.is_some() {
            chosen.push("name");
        }
        if vtable_index_lit.is_some() {
            chosen.push("vtable_index");
        }
        if chosen.len() > 1 {
            return bail!(
                &func.name,
                "`{}` and `{}` are mutually exclusive on the same method",
                chosen[0],
                chosen[1]
            );
        }

        let resolution = if let Some(o) = offset_lit {
            Resolution::Offset(o)
        } else if let Some(p) = pattern_lit {
            Resolution::Pattern(unquote_string_literal(&p.to_string()))
        } else if let Some(v) = vtable_index_lit {
            if args_lit.is_some() {
                return bail!(
                    &func.name,
                    "`args` is only meaningful with `name` resolution"
                );
            }
            Resolution::VtableIndex(v)
        } else {
            // Name resolution defaults the il2cpp name to the Rust ident
            let name = match name_lit {
                Some(lit) => unquote_string_literal(&lit.to_string()),
                None => func.name.to_string(),
            };
            Resolution::Name {
                name,
                args: args_lit,
            }
        };

        let mut is_static = true;
        let mut params = Vec::new();
        for (param, _) in func.params.inner.iter() {
            match param {
                FnParam::Receiver(_) => is_static = false,
                FnParam::Typed(typed) => params.push(typed.clone()),
            }
        }

        Ok(Self {
            name: func.name.clone(),
            vis: func.vis_marker.clone(),
            is_static,
            params,
            return_ty: func.return_ty.clone(),
            resolution,
            is_unsafe,
            is_abstract,
        })
    }
}
