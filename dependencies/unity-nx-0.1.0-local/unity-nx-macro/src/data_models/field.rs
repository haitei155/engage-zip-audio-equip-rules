use proc_macro2::{Ident, TokenTree};

use crate::ParseResult;
use crate::util::KvParser;

pub struct Field {
    pub name: Ident,
    pub ty: venial::TypeExpr,
    // IL2CPP-side field name passed to class_get_field_from_name, defaults to the Rust ident
    pub il2cpp_name: String,
    // `static` is a Rust keyword so the marker is #[static_field] instead
    pub is_static: bool,
    pub offset: Option<u64>,
}

impl Field {
    pub fn parse(field: &venial::NamedField) -> ParseResult<Self> {
        let mut rename_override: Option<String> = None;
        if let Some(mut parser) = KvParser::parse(&field.attributes, "rename")? {
            if let Some(lit) = parser.handle_literal("name", "string")? {
                rename_override = Some(unquote_string_literal(&lit.to_string()));
            }
            parser.finish()?;
        }

        // #[backing] computes <PropName>k__BackingField, without an explicit name snake_case is converted to PascalCase
        let backing_prop: Option<String> = if let Some(mut parser) =
            KvParser::parse(&field.attributes, "backing")?
        {
            let name = if let Some(lit) = parser.handle_literal("name", "string")? {
                unquote_string_literal(&lit.to_string())
            } else {
                snake_to_pascal(&field.name.to_string())
            };
            parser.finish()?;
            Some(name)
        } else {
            None
        };

        let il2cpp_name = match (rename_override, backing_prop) {
            (Some(_), Some(_)) => {
                return Err(venial::Error::new(
                    "a field can carry either #[rename(name = \"...\")] or #[backing], not both",
                ));
            }
            (Some(name), None) => name,
            (None, Some(prop)) => format!("<{}>k__BackingField", prop),
            (None, None) => field.name.to_string(),
        };

        let is_static = field
            .attributes
            .iter()
            .any(|attr| crate::util::path_is_single(&attr.path, "static_field"));

        let offset = field.attributes.iter().find_map(|attr| {
            if !crate::util::path_is_single(&attr.path, "offset") {
                return None;
            }
            attr.value.get_value_tokens().iter().find_map(|tt| match tt {
                TokenTree::Literal(lit) => lit.to_string().parse::<u64>().ok(),
                _ => None,
            })
        });

        Ok(Self {
            name: field.name.clone(),
            ty: field.ty.clone(),
            il2cpp_name,
            is_static,
            offset,
        })
    }
}

// Acronyms that must be preserved need explicit #[backing(name = "...")]
fn snake_to_pascal(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    let mut upper_next = true;
    for c in s.chars() {
        if c == '_' {
            upper_next = true;
        } else if upper_next {
            out.extend(c.to_uppercase());
            upper_next = false;
        } else {
            out.push(c);
        }
    }
    out
}

pub(crate) fn unquote_string_literal(s: &str) -> String {
    let trimmed = s.trim();
    if trimmed.len() >= 2 && trimmed.starts_with('"') && trimmed.ends_with('"') {
        trimmed[1..trimmed.len() - 1].to_string()
    } else {
        trimmed.to_string()
    }
}
