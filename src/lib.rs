//! [`syn`]-powered parser for JSX-like `TokenStream`s. The parsed result is a
//! nested [`Node`] structure modelled after the browser DOM.
//!
//! [`syn`]: /syn
//! [`Node`]: struct.Node.html
//!
//! ```
//! use quote::quote;
//! use syn_rsx::parse2;
//!
//! let tokens = quote! {
//!     <div>
//!         <div>"hello"</div>
//!         <div>{world}</div>
//!     </div>
//! };
//!
//! let nodes = parse2(tokens, None).unwrap();
//! assert_eq!(nodes.get(0).unwrap().child_nodes.len(), 2);
//! ```

extern crate proc_macro;

use syn::{
    parse::{ParseStream, Parser as _},
    Result,
};

mod node;
mod parser;

pub use node::{Expr, Node, NodeType};
pub use parser::{Parser, ParserConfig};

/// Parse the given `proc-macro::TokenStream` into `Node`s
pub fn parse(tokens: proc_macro::TokenStream, config: Option<ParserConfig>) -> Result<Vec<Node>> {
    let parser = move |input: ParseStream| {
        let config = config.unwrap_or_else(|| ParserConfig::default());
        Parser::new(config).parse(input)
    };

    parser.parse(tokens)
}

/// Parse the given `proc-macro2::TokenStream` into `Node`s
pub fn parse2(tokens: proc_macro2::TokenStream, config: Option<ParserConfig>) -> Result<Vec<Node>> {
    let parser = move |input: ParseStream| {
        let config = config.unwrap_or_else(|| ParserConfig::default());
        Parser::new(config).parse(input)
    };

    parser.parse2(tokens)
}

#[cfg(test)]
mod tests {
    use super::*;
    use syn::{Expr, Lit};

    #[test]
    fn test_single_empty_element() {
        let tokens = quote::quote! {
            <foo></foo>
        };
        let node = parse2(tokens, None).unwrap();
        assert_eq!(node[0].node_name, "foo");
    }

    #[test]
    fn test_single_element_with_attributes() {
        let tokens = quote::quote! {
            <foo bar="moo" baz="42"></foo>
        };
        let node = parse2(tokens, None).unwrap();

        let attribute = &node[0].attributes[0];
        let attribute_value = match attribute.node_value.as_ref().unwrap() {
            Expr::Lit(expr) => match &expr.lit {
                Lit::Str(lit_str) => Some(lit_str.value()),
                _ => None,
            },
            _ => None,
        }
        .unwrap();

        assert_eq!(attribute.node_name, "bar");
        assert_eq!(attribute_value, "moo");
    }

    #[test]
    fn test_single_element_with_text() {
        let tokens = quote::quote! {
            <foo>"bar"</foo>
        };
        let node = parse2(tokens, None).unwrap();

        let node_value = match node[0].child_nodes[0].node_value.as_ref().unwrap() {
            Expr::Lit(expr) => match &expr.lit {
                Lit::Str(lit_str) => Some(lit_str.value()),
                _ => None,
            },
            _ => None,
        }
        .unwrap();

        assert_eq!(node_value, "bar");
    }

    #[test]
    fn test_reserved_keyword_attributes() {
        let tokens = quote::quote! {
            <input type="foo" />
        };
        let node = parse2(tokens, None).unwrap();

        assert_eq!(node[0].node_name, "input");
        assert_eq!(node[0].attributes[0].node_name, "type");
    }

    #[test]
    fn test_braced_expr_as_text_node() {
        let tokens = quote::quote! {
            <div>{hello}</div>
        };
        let node = parse2(tokens, None).unwrap();

        assert_eq!(node[0].child_nodes.len(), 1);
    }

    #[test]
    fn test_flat_tree() {
        let config = ParserConfig { flatten: true };

        let tokens = quote::quote! {
            <div>
                <div>
                    <div>{hello}</div>
                    <div>"world"</div>
                </div>
            </div>
            <div />
        };

        let nodes = parse2(tokens, Some(config)).unwrap();
        assert_eq!(nodes.len(), 7);
    }
}