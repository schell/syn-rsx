//! RSX Parser

use proc_macro2::{TokenStream, TokenTree};
use std::iter;
use syn::{
    ext::IdentExt,
    parse::{discouraged::Speculative, Parse, ParseStream, Parser as _, Peek},
    punctuated::Punctuated,
    token::{Brace, Colon},
    Expr, ExprBlock, ExprLit, ExprPath, Ident, Path, PathSegment, Result, Token,
};

use crate::{node::*, punctuation::*};

struct Tag {
    name: NodeName,
    attributes: Vec<Node>,
    selfclosing: bool,
}

/// Configures the `Parser` behavior
pub struct ParserConfig {
    /// Whether the returned node tree should be nested or flat. Defaults to `false`
    pub flatten: bool,
}

impl Default for ParserConfig {
    fn default() -> Self {
        Self { flatten: false }
    }
}

/// RSX Parser
pub struct Parser {
    config: ParserConfig,
}

impl Parser {
    /// Create a new parser with the given config
    pub fn new(config: ParserConfig) -> Parser {
        Parser { config }
    }

    /// Parse a given `syn::ParseStream`
    pub fn parse(&self, input: ParseStream) -> Result<Vec<Node>> {
        let mut nodes = vec![];
        while !input.cursor().eof() {
            nodes.append(&mut self.node(input)?);
        }

        Ok(nodes)
    }

    fn node(&self, input: ParseStream) -> Result<Vec<Node>> {
        let node = self
            .text(input)
            .or_else(|_| self.block(input))
            .or_else(|_| self.element(input))?;

        let mut nodes = vec![node];
        if self.config.flatten {
            let mut children = vec![];
            children.append(&mut nodes[0].children);
            nodes.append(&mut children);
        }

        Ok(nodes)
    }

    fn text(&self, input: ParseStream) -> Result<Node> {
        let text = input.parse::<ExprLit>()?.into();

        Ok(Node {
            name: None,
            value: Some(text),
            node_type: NodeType::Text,
            attributes: vec![],
            children: vec![],
        })
    }

    fn block(&self, input: ParseStream) -> Result<Node> {
        let block = self.block_expr(input)?;

        Ok(Node {
            name: None,
            value: Some(block),
            node_type: NodeType::Block,
            attributes: vec![],
            children: vec![],
        })
    }

    fn block_expr(&self, input: ParseStream) -> Result<Expr> {
        let fork = input.fork();
        let parser = move |input: ParseStream| input.parse();
        let group: TokenTree = fork.parse()?;
        let block: ExprBlock = parser.parse2(iter::once(group).collect())?;
        input.advance_to(&fork);

        Ok(block.into())
    }

    fn element(&self, input: ParseStream) -> Result<Node> {
        let fork = &input.fork();
        if let Ok(_) = self.tag_close(&input.fork()) {
            return Err(fork.error("close tag has no corresponding open tag"));
        }
        let tag_open = self.tag_open(fork)?;

        let mut children = vec![];
        if !tag_open.selfclosing {
            loop {
                if !self.has_children(&tag_open, fork)? {
                    break;
                }

                children.append(&mut self.node(fork)?);
            }

            self.tag_close(fork)?;
        }
        input.advance_to(fork);

        Ok(Node {
            name: Some(tag_open.name),
            value: None,
            node_type: NodeType::Element,
            attributes: tag_open.attributes,
            children,
        })
    }

    fn has_children(&self, tag_open: &Tag, input: ParseStream) -> Result<bool> {
        // an empty input at this point means the tag wasn't closed
        if input.is_empty() {
            return Err(input.error("open tag has no corresponding close tag"));
        }

        if let Ok(tag_close_ident) = self.tag_close(&input.fork()) {
            if tag_open.name == tag_close_ident {
                // if the next token is a matching close tag then there are no child nodes
                return Ok(false);
            } else {
                // if the next token is a closing tag with a different name it's an invalid tree
                return Err(input.error("close tag has no corresponding open tag"));
            }
        }

        Ok(true)
    }

    fn tag_open(&self, input: ParseStream) -> Result<Tag> {
        input.parse::<Token![<]>()?;
        let name = self.node_name(input)?;

        let mut attributes = TokenStream::new();
        let selfclosing = loop {
            if let Ok(selfclosing) = self.tag_open_end(input) {
                break selfclosing;
            }

            let next: TokenTree = input.parse()?;
            attributes.extend(Some(next));
        };

        let parser = move |input: ParseStream| self.attributes(input);
        let attributes = parser.parse2(attributes)?;

        Ok(Tag {
            name,
            attributes,
            selfclosing,
        })
    }

    fn tag_open_end(&self, input: ParseStream) -> Result<bool> {
        let selfclosing = input.parse::<Option<Token![/]>>()?.is_some();
        input.parse::<Token![>]>()?;

        Ok(selfclosing)
    }

    fn tag_close(&self, input: ParseStream) -> Result<NodeName> {
        input.parse::<Token![<]>()?;
        input.parse::<Token![/]>()?;
        let name = self.node_name(input)?;
        input.parse::<Token![>]>()?;

        Ok(name)
    }

    fn attributes(&self, input: ParseStream) -> Result<Vec<Node>> {
        let mut nodes = vec![];
        if input.is_empty() {
            return Ok(nodes);
        }

        while let Ok((key, value)) = self.attribute(input) {
            nodes.push(Node {
                name: Some(key),
                node_type: NodeType::Attribute,
                value,
                attributes: vec![],
                children: vec![],
            });

            if input.is_empty() {
                break;
            }
        }

        Ok(nodes)
    }

    fn attribute(&self, input: ParseStream) -> Result<(NodeName, Option<Expr>)> {
        let fork = &input.fork();
        let key = self.node_name(fork)?;
        let eq = fork.parse::<Option<Token![=]>>()?;
        let value = if eq.is_some() {
            if fork.peek(Brace) {
                Some(self.block_expr(fork)?)
            } else {
                Some(fork.parse()?)
            }
        } else {
            None
        };
        input.advance_to(fork);

        Ok((key, value))
    }

    fn node_name(&self, input: ParseStream) -> Result<NodeName> {
        let node_name = self
            .node_name_punctuated_ident::<Dash, fn(_) -> Dash>(input, Dash)
            .map(|ok| NodeName::Dash(ok))
            .or_else(|_| {
                self.node_name_punctuated_ident::<Colon, fn(_) -> Colon>(input, Colon)
                    .map(|ok| NodeName::Colon(ok))
            })
            .or_else(|_| self.node_name_mod_style(input))
            .or(Err(input.error("invalid node name")))?;

        Ok(node_name)
    }

    fn node_name_punctuated_ident<T: Parse, F: Peek>(
        &self,
        input: ParseStream,
        punct: F,
    ) -> Result<Punctuated<Ident, T>> {
        let fork = &input.fork();
        let mut segments = Punctuated::<Ident, T>::new();

        while !fork.is_empty() && fork.peek(Ident::peek_any) {
            let ident = Ident::parse_any(fork)?;
            segments.push_value(ident.clone());

            if fork.peek(punct) {
                segments.push_punct(fork.parse()?);
            } else {
                break;
            }
        }

        if segments.len() > 1 {
            input.advance_to(fork);
            Ok(segments)
        } else {
            Err(fork.error("expected punctuated node name"))
        }
    }

    // Modified version of `Path::parse_mod_style` that uses `Ident::peek_any`
    // in order to allow parsing reserved keywords
    //
    // https://docs.rs/syn/1.0.30/src/syn/path.rs.html#388-418
    // TODO: consider PR upstream
    fn node_name_mod_style(&self, input: ParseStream) -> Result<NodeName> {
        let given_input = input;
        let input = &input.fork();

        let path = Path {
            leading_colon: input.parse()?,
            segments: {
                let mut segments = Punctuated::new();
                loop {
                    if !input.peek(Ident::peek_any)
                        && !input.peek(Token![super])
                        && !input.peek(Token![self])
                        && !input.peek(Token![Self])
                        && !input.peek(Token![crate])
                    {
                        break;
                    }
                    let ident = Ident::parse_any(input)?;
                    segments.push_value(PathSegment::from(ident));
                    if !input.peek(Token![::]) {
                        break;
                    }
                    let punct = input.parse()?;
                    segments.push_punct(punct);
                }
                if segments.is_empty() {
                    return Err(input.error("expected path"));
                } else if segments.trailing_punct() {
                    return Err(input.error("expected path segment"));
                }
                segments
            },
        };
        given_input.advance_to(input);

        Ok(NodeName::Path(ExprPath {
            attrs: vec![],
            qself: None,
            path,
        }))
    }
}
