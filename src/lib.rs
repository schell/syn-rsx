//! [`syn`]-powered parser for JSX-like [`TokenStream`]s. The parsed result is a
//! nested [`Node`] structure, similar to the browser DOM, where node name and
//! value are syn expressions to support building proc macros. A [`ParserConfig`]
//! to customize parsing behavior is available, so if you have slightly
//! different requirements for parsing and it's not yet customizable feel free
//! to open an issue or pull request to extend the configuration.
//!
//! [`syn`]: /syn
//! [`TokenStream`]: https://doc.rust-lang.org/proc_macro/struct.TokenStream.html
//! [`Node`]: struct.Node.html
//! [`ParserConfig`]: struct.ParserConfig.html
//!
//! ```
//! use quote::quote;
//! use syn_rsx::parse2;
//!
//! let tokens = quote! {
//!     <div foo={bar}>
//!         <div>"hello"</div>
//!         <world />
//!     </div>
//! };
//!
//! let nodes = parse2(tokens, None).unwrap();
//!
//! let node = &nodes[0];
//! assert_eq!(node.attributes[0].name_as_string().unwrap(), "foo");
//!
//! let children = &node.children;
//! assert_eq!(children.len(), 2);
//! assert_eq!(children[0].children[0].value_as_string().unwrap(), "hello");
//! assert_eq!(children[1].name_as_string().unwrap(), "world");
//! ```

extern crate proc_macro;

use syn::{
    parse::{ParseStream, Parser as _},
    Result,
};

mod node;
mod parser;

pub mod punctuation {
    //! Custom syn punctuations
    use syn::custom_punctuation;

    custom_punctuation!(Dash, -);
}

pub use node::{Node, NodeName, NodeType};
pub use parser::{Parser, ParserConfig};

/// Parse the given [`proc-macro::TokenStream`] into a [`Node`] tree
///
/// [`proc-macro::TokenStream`]: https://doc.rust-lang.org/proc_macro/struct.TokenStream.html
/// [`Node`]: struct.Node.html
pub fn parse(tokens: proc_macro::TokenStream, config: Option<ParserConfig>) -> Result<Vec<Node>> {
    let parser = move |input: ParseStream| {
        let config = config.unwrap_or_else(ParserConfig::default);
        Parser::new(config).parse(input)
    };

    parser.parse(tokens)
}

/// Parse the given [`proc-macro2::TokenStream`] into a [`Node`] tree
///
/// [`proc-macro2::TokenStream`]: https://docs.rs/proc-macro2/latest/proc_macro2/struct.TokenStream.html
/// [`Node`]: struct.Node.html
pub fn parse2(tokens: proc_macro2::TokenStream, config: Option<ParserConfig>) -> Result<Vec<Node>> {
    let parser = move |input: ParseStream| {
        let config = config.unwrap_or_else(ParserConfig::default);
        Parser::new(config).parse(input)
    };

    parser.parse2(tokens)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_single_empty_element() {
        let tokens = quote::quote! {
            <foo></foo>
        };
        let nodes = parse2(tokens, None).unwrap();
        assert_eq!(nodes[0].name_as_string().unwrap(), "foo");
    }

    #[test]
    fn test_single_element_with_attributes() {
        let tokens = quote::quote! {
            <foo bar="moo" baz="42"></foo>
        };
        let nodes = parse2(tokens, None).unwrap();

        let attribute = &nodes[0].attributes[0];
        assert_eq!(attribute.name_as_string().unwrap(), "bar");
        assert_eq!(attribute.value_as_string().unwrap(), "moo");
    }

    #[test]
    fn test_single_element_with_text() {
        let tokens = quote::quote! {
            <foo>"bar"</foo>
        };

        let nodes = parse2(tokens, None).unwrap();
        assert_eq!(nodes[0].children[0].value_as_string().unwrap(), "bar");
    }

    #[test]
    fn test_reserved_keyword_attributes() {
        let tokens = quote::quote! {
            <input type="foo" />
        };
        let nodes = parse2(tokens, None).unwrap();

        assert_eq!(nodes[0].name_as_string().unwrap(), "input");
        assert_eq!(nodes[0].attributes[0].name_as_string().unwrap(), "type");
    }

    #[test]
    fn test_block_node() {
        let tokens = quote::quote! {
            <div>{hello}</div>
        };
        let nodes = parse2(tokens, None).unwrap();

        assert_eq!(nodes[0].children.len(), 1);
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

    #[test]
    fn test_path_as_tag_name() {
        let tokens = quote::quote! {
            <some::path />
        };

        let nodes = parse2(tokens, None).unwrap();
        assert_eq!(nodes[0].name_as_string().unwrap(), "some::path");
    }

    #[test]
    fn test_dashed_attribute_name() {
        let tokens = quote::quote! {
            <div data-foo="bar" />
        };

        let nodes = parse2(tokens, None).unwrap();
        assert_eq!(nodes[0].attributes[0].name_as_string().unwrap(), "data-foo");
    }

    #[test]
    fn test_coloned_attribute_name() {
        let tokens = quote::quote! {
            <div on:click={foo} />
        };

        let nodes = parse2(tokens, None).unwrap();
        assert_eq!(nodes[0].attributes[0].name_as_string().unwrap(), "on:click");
    }
}
