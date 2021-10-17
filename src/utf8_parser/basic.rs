use crate::utf8_parser::{
    char_categories::is_ws,
    combinators,
    combinators::{lookahead, pair, recognize, take_while},
    util,
    util::base_err,
    BaseErrorKind, ErrorTree, Expectation, IResultLookahead, Input, InputParseErr,
};

/// Matches always and doesn't advance the parser
pub fn nothing(input: Input) -> IResultLookahead<Input> {
    Ok(input.take_split(0).into())
}

pub fn multispace0(input: Input) -> IResultLookahead<Input> {
    combinators::take_while(is_ws)(input)
}

pub fn multispace1(input: Input) -> IResultLookahead<Input> {
    combinators::take_while1(is_ws, Expectation::Multispace)(input)
}

pub fn eol_comment(input: Input) -> IResultLookahead<Input> {
    recognize(pair(
        lookahead(tag("//")),
        take_while(|c| c != '\n' && c != '\r'),
    ))(input)
}

pub fn block_comment(input: Input) -> IResultLookahead<Input> {
    recognize(pair(
        lookahead(tag("/*")),
        block_comment_tail,
    ))(input)
}

fn block_comment_tail(input: Input) -> IResultLookahead<()> {
    let comment_end = input
        .fragment()
        .find("*/")
        .ok_or_else(|| base_err::<()>(input.slice(input.len() - 1..), Expectation::BlockCommentEnd).unwrap_err())?;
    let nested_start = input.fragment().find("/*");

    if let Some(nested_start) = nested_start {
       if nested_start < comment_end {
           return input.take_split(nested_start).and_then(block_comment, |_, _| ())?.and_then(block_comment_tail, |_, _| ());
       }
    }

    let advanced = input.take_split(comment_end);

    advanced.and_then(tag("*/"), |_, _| ())
}

pub fn tag(tag: &'static str) -> impl Clone + Fn(Input) -> IResultLookahead<Input> {
    let tag_len = tag.len();

    move |input: Input| match input.fragment().starts_with(tag) {
        true => Ok(input.take_split(tag_len)),
        false => util::base_err(input, Expectation::Tag(tag)),
    }
}

pub fn one_char(c: char) -> impl Fn(Input) -> IResultLookahead<char> {
    move |input: Input| match input.chars().next().map(|t| {
        let b = t == c;
        (&c, b)
    }) {
        Some((&c, true)) => Ok((input.slice(c.len_utf8()..), c).into()),
        _ => Err(InputParseErr::fatal(ErrorTree::Base {
            location: input,
            kind: BaseErrorKind::Expected(Expectation::Char(c)),
        })),
    }
}

pub fn one_of_chars<O: Clone>(
    one_of: &'static str,
    mapping: &'static [O],
) -> impl Fn(Input) -> IResultLookahead<O> {
    assert_eq!(one_of.len(), mapping.len());

    move |input: Input| match input.chars().next().map(|t| {
        let b = one_of.chars().position(|c| c == t);
        (t, b)
    }) {
        Some((c, Some(i))) => Ok((input.slice(c.len_utf8()..), mapping[i].clone()).into()),
        _ => Err(InputParseErr::fatal(ErrorTree::Base {
            location: input,
            kind: BaseErrorKind::Expected(Expectation::OneOfChars(one_of)),
        })),
    }
}

pub fn one_of_tags<O: Clone>(
    one_of: &'static [&'static str],
    mapping: &'static [O],
) -> impl Fn(Input) -> IResultLookahead<O> {
    move |input: Input| match one_of
        .iter()
        .enumerate()
        .find(|(_, &t)| input.fragment().starts_with(t))
    {
        Some((i, tag)) => Ok((input.slice(tag.len()..), mapping[i].clone()).into()),
        _ => Err(InputParseErr::fatal(ErrorTree::Base {
            location: input,
            kind: BaseErrorKind::Expected(Expectation::OneOfTags(one_of)),
        })),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::utf8_parser::test_util::eval;

    #[test]
    fn basic_block_comment() {
        assert_eq!(
            eval!(block_comment, "/* Hello I am a block comment! */").fragment(),
            "/* Hello I am a block comment! */"
        );

        assert_eq!(
            eval!(block_comment, "/* Hello I am a\n block comment! */ parser ignores this */ /*").fragment(),
            "/* Hello I am a\n block comment! */"
        );
    }

    #[test]
    fn nested_block_comment() {
        assert_eq!(
            eval!(block_comment, "/* Hello I am /* a nested */ block comment! */").fragment(),
            "/* Hello I am /* a nested */ block comment! */"
        );
    }
}
