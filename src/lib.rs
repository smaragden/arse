use nom::{
    branch::alt,
    bytes::streaming::{is_not, tag, take_until, take_while},
    character::streaming::char,
    combinator::map,
    error::ParseError,
    sequence::{delimited, pair, preceded},
    Err, IResult,
};

use std::{
    ffi::OsStr,
    fs::File,
    io::{BufRead, BufReader, Read},
    path::Path,
    str::Utf8Error,
};

pub struct ArseParser<R: Read> {
    reader: BufReader<R>,
    buffer: String,
}

impl<R: Read> ArseParser<R> {
    pub fn new(reader: R) -> Self {
        Self {
            reader: BufReader::new(reader),
            buffer: String::new(),
        }
    }

    pub fn with_capacity(reader: R, capacity: usize) -> Self {
        Self {
            reader: BufReader::with_capacity(4 * 1024 * 1024, reader),
            buffer: String::new(),
        }
    }

    fn fill(&mut self) -> Result<usize, Utf8Error> {
        let buffer = self.reader.fill_buf().unwrap();
        let nb = buffer.len();
        if nb > 0 {
            let data = std::str::from_utf8(buffer)?;
            self.buffer.push_str(data);
            self.reader.consume(nb);
        }
        Ok(nb)
    }
}

impl<R: Read> Iterator for ArseParser<R> {
    type Item = Node;

    fn next(&mut self) -> Option<Self::Item> {
        loop {
            let read = self.fill().unwrap();
            if self.buffer.is_empty() {
                return None;
            }
            match root(self.buffer.clone().as_str()) {
                Ok((rest, RootElement::Node(node))) => {
                    self.buffer = rest.to_string();
                    return Some(node);
                }
                Ok((rest, RootElement::Comment(_))) => {
                    self.buffer = rest.to_string();
                }
                Err(Err::Incomplete(_)) => {
                    if read == 0 {
                        break;
                    }
                }
                Err(_) => break,
            }
        }
        None
    }
}

#[derive(Debug, PartialEq)]
pub struct Node {
    pub node_type: String,
    pub name: String,
}

/// Comments are lines starting with a hash
fn comment(i: &str) -> nom::IResult<&str, &str> {
    delimited(char('#'), is_not("\n"), char('\n'))(i)
}

const SPACELIKE_CHARS: &str = " \t\r\n";

/// Anything that is a spacelike e.g. spaces, tabs, newlines ...
fn spacelike<'a, E: ParseError<&'a str>>(i: &'a str) -> IResult<&'a str, &'a str, E> {
    take_while(move |c| SPACELIKE_CHARS.contains(c))(i)
}

/// Valid names are string that does not include spacelike,
/// parentheses or braces
fn name(i: &str) -> nom::IResult<&str, &str> {
    is_not(" \t\r\n{}[]()")(i)
}

/// Name parameter preceded by `name `
fn node_name(i: &str) -> nom::IResult<&str, &str> {
    delimited(tag("name "), name, spacelike)(i)
}

/// The node body, we only parse the name parameter for now
fn node_body(i: &str) -> nom::IResult<&str, &str> {
    delimited(
        char('{'),
        delimited(take_until("name "), node_name, take_until("}")),
        char('}'),
    )(i)
}

/// Node with it's preceding type_name and delegate the body to `node_body`
fn node_parser(i: &str) -> nom::IResult<&str, (&str, &str)> {
    pair(preceded(spacelike, name), preceded(spacelike, node_body))(i)
}

#[derive(Debug, PartialEq)]
enum RootElement<'a> {
    Node(Node),
    Comment(&'a str),
}

/// The root element of an ass parser is an array of nodes.
/// body can contain nodes or comments
fn root<'a>(i: &'a str) -> IResult<&'a str, RootElement<'a>> {
    alt((
        map(comment, |c| RootElement::Comment(c)),
        map(node_parser, |n| {
            RootElement::Node(Node {
                node_type: n.0.to_owned(),
                name: n.1.to_owned(),
            })
        }),
    ))(i)
}

/// Get a buffered reader for filename.
/// Supports both text and gz files.
pub fn reader(filename: &str) -> Box<dyn Read> {
    let path = Path::new(filename);
    let file = match File::open(&path) {
        Err(why) => panic!("couldn't open {}, {}", path.display(), why),
        Ok(file) => file,
    };

    // We are only checking for extension right now to
    // determine if it is a gz file.
    // TODO: Use other heuristics to determine file type
    if path.extension() == Some(OsStr::new("gz")) {
        Box::new(flate2::read::GzDecoder::new(file))
    } else {
        Box::new(file)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use indoc::indoc;
    #[test]
    fn nodes() {
        let data = indoc! {"
        # This is a comment

        sphere
        {
            name Sphere02
        }
        box {
            name Box02
        }
        "};
        let mut parser = ArseParser::new(data.as_bytes());
        let first = parser.next();
        assert_eq!(first.unwrap().name.as_str(), "Sphere02");
        let second = parser.next();
        assert_eq!(second.unwrap().name.as_str(), "Box02");
        assert_eq!(parser.next(), None);
    }
}
