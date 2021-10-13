use std::str::FromStr;

use ast::{
    Attribute, Decimal, Expr, Extension, Ident, KeyValue, List, Map, Ron, Sign, SignedInteger,
    Spanned, Struct, UnsignedInteger,
};
use basic::{multispace0, one_char, one_of_chars, one_of_tags, tag};
use combinators::{alt2, comma_list0, comma_list1, context, cut, delimited, lookahead, many0, map, map_res, opt, pair, preceded, recognize, take1_if, take_while, terminated};
use primitive::{ident, number, str};
pub use primitive::string::parse_string as string;

use crate::{
    parser::{
        char_categories::{is_digit, is_digit_first, is_ident_first_char, is_ident_other_char},
        input::position,
    },
};

pub use self::{
    error::{BaseErrorKind, ErrorTree, Expectation, InputParseErr, InputParseError},
    input::{Input, Location, Offset},
};

//pub type IResultFatal<'a, O> = Result<(Input<'a>, O), InputParseError<'a>>;
pub type IResultLookahead<'a, O> = Result<(Input<'a>, O), InputParseErr<'a>>;
pub type OutputResult<'a, O> = Result<O, InputParseErr<'a>>;

/// All AST elements generated by the parser
pub mod ast;
/// Basic parsers which receive `Input`
mod basic;
/// Tables for fast lookup of char categories
mod char_categories;
/// Parser combinators which take one or more parsers and modify / combine them
mod combinators;
/// RON container parsers
mod containers;
/// Parser error collection
mod error;
/// Parsers for arbitrary RON expression
mod expr;
/// `Input` abstraction to slice the input that is being parsed and keep track of the line + column
mod input;
/// RON primitive parsers
mod primitive;
#[cfg(test)]
pub mod tests;

/// Utility functions for parsing
mod util;

fn extension_name(input: Input) -> IResultLookahead<Extension> {
    one_of_tags(
        &["unwrap_newtypes", "implicit_some"],
        &[Extension::UnwrapNewtypes, Extension::ImplicitSome],
    )(input)
}

fn attribute_enable(input: Input) -> IResultLookahead<Attribute> {
    let start = preceded(tag("enable"), combinators::ws(one_char('(')));
    let end = one_char(')');

    delimited(
        start,
        map(combinators::spanned(comma_list1(extension_name)), Attribute::Enable),
        end,
    )(input)
}

pub fn attribute(input: Input) -> IResultLookahead<Attribute> {
    let start = preceded(
        preceded(lookahead(one_char('#')), combinators::ws(one_char('!'))),
        combinators::ws(one_char('[')),
    );
    let end = one_char(']');

    context("attribute", delimited(start, combinators::ws(attribute_enable), end))(input)
}

#[derive(Clone, Debug)]
pub enum ExprClass {
    StructTuple,
    Map,
    StrString,
    List,
    Bool,
    Signed,
    Dec,
    UnsignedDec,
    LeadingIdent,
}

impl ExprClass {
    pub fn parse(input: Input) -> IResultLookahead<Self> {
        let all_but_ident = one_of_chars(
            "({\"[tf+-.0123456789",
            &[
                ExprClass::StructTuple,
                ExprClass::Map,
                ExprClass::StrString,
                ExprClass::List,
                ExprClass::Bool,
                ExprClass::Bool,
                ExprClass::Signed,
                ExprClass::Signed,
                ExprClass::Dec,
                ExprClass::Dec,
                ExprClass::UnsignedDec,
                ExprClass::UnsignedDec,
                ExprClass::UnsignedDec,
                ExprClass::UnsignedDec,
                ExprClass::UnsignedDec,
                ExprClass::UnsignedDec,
                ExprClass::UnsignedDec,
                ExprClass::UnsignedDec,
                ExprClass::UnsignedDec,
            ],
        );

        alt2(
            lookahead(all_but_ident),
            map(
                take1_if(
                    is_ident_first_char,
                    Expectation::OneOfExpectations(&[Expectation::Alpha, Expectation::Char('_')]),
                ),
                |_| ExprClass::LeadingIdent,
            ),
        )(input)
    }
}

fn expr_inner(input: Input) -> IResultLookahead<Expr> {
    // Copy input and discard its offset ("peek")
    let (_, expr_class): (Input, ExprClass) = ExprClass::parse(input)?;

    match expr_class {
        ExprClass::StructTuple => cut(alt2(map(containers::r#struct, Expr::Struct), map(containers::tuple, Expr::Tuple)))(input),
        ExprClass::Map => map(containers::rmap, Expr::Map)(input),
        ExprClass::StrString => alt2(
            map(lookahead(str::unescaped_str), Expr::Str),
            map(string, Expr::String),
        )(input),
        ExprClass::List => map(containers::list, Expr::List)(input),
        ExprClass::Bool => map(primitive::bool, Expr::Bool)(input),
        ExprClass::Signed => map(number::signed_integer, SignedInteger::to_expr)(input),
        ExprClass::Dec => map(number::decimal, Expr::Decimal)(input),
        ExprClass::UnsignedDec => alt2(
            map(number::unsigned, UnsignedInteger::to_expr),
            map(number::decimal, Expr::Decimal),
        )(input),
        ExprClass::LeadingIdent => map(containers::r#struct, Expr::Struct)(input),
    }
}

pub fn expr(input: Input) -> IResultLookahead<Expr> {
    cut(context("expression", expr_inner))(input)
}

fn ron_inner(input: Input) -> IResultLookahead<Ron> {
    map(
        pair(many0(combinators::spanned(attribute)), combinators::spanned(expr)),
        |(attributes, expr)| Ron { attributes, expr },
    )(input)
}

pub fn ron(input: &str) -> Result<Ron, InputParseError> {
    let input = Input::new(input);

    match ron_inner(input) {
        Ok((i, ron)) if i.is_empty() => Ok(ron),
        Ok((i, _)) => Err(ErrorTree::expected(i, Expectation::Eof)),
        Err(InputParseErr::Fatal(e)) | Err(InputParseErr::Recoverable(e)) => Err(e),
    }
}
