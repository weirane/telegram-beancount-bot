#![allow(clippy::while_let_on_iterator)]
use anyhow::Result;

// got the idea from `shlex` crate
mod shlex {
    use anyhow::{bail, Result};
    use std::iter::Peekable;
    pub(super) struct Shlex<'a> {
        in_iter: Peekable<core::str::Chars<'a>>,
    }

    impl<'a> Shlex<'a> {
        pub(super) fn new(in_str: &'a str) -> Self {
            Shlex {
                in_iter: in_str.chars().peekable(),
            }
        }

        fn parse_word(&mut self) -> Result<Option<String>> {
            // skip initial whitespace
            while self.in_iter.next_if(|x| matches!(x, ' ' | '\t')).is_some() {}
            if self.in_iter.peek().is_none() {
                // nothing left to parse
                return Ok(None);
            }
            let mut result = String::new();
            while let Some(ch) = self.in_iter.next() {
                match ch {
                    '"' => self.parse_double(&mut result)?,
                    '\'' => self.parse_single(&mut result)?,
                    '\n' => bail!("newline within argument"),
                    ' ' | '\t' => break,
                    _ => result.push(ch),
                }
            }
            Ok(Some(result))
        }

        fn parse_double(&mut self, result: &mut String) -> Result<()> {
            while let Some(ch) = self.in_iter.next() {
                match ch {
                    '"' => return Ok(()),
                    '\n' => bail!("newline within double quote"),
                    '\\' => {
                        if let Some(ch2) = self.in_iter.next() {
                            match ch2 {
                                '"' | '\\' => result.push(ch2),
                                '\n' => bail!("newline within double quote"),
                                _ => {
                                    result.push('\\');
                                    result.push(ch2);
                                }
                            }
                        }
                    }
                    _ => result.push(ch),
                }
            }
            bail!("unmatched double quote");
        }

        fn parse_single(&mut self, result: &mut String) -> Result<()> {
            while let Some(ch) = self.in_iter.next() {
                match ch {
                    '\'' => return Ok(()),
                    '\n' => bail!("newline within single quote"),
                    _ => result.push(ch),
                }
            }
            bail!("unmatched single quote");
        }
    }

    impl<'a> Iterator for Shlex<'a> {
        type Item = Result<String>;
        fn next(&mut self) -> Option<Self::Item> {
            self.parse_word().transpose()
        }
    }
}

/// Splits a command `s` into a list of arguments in a syntax similar to shell's:
/// - arguments are whitespace-separated
/// - arguments containing spaces can be quoted in double or single quotes
/// - double quotes within double quotes can be escaped by `\"`
/// - no escape is allowed in single quotes
pub fn command_split(s: &str) -> Result<Vec<String>> {
    shlex::Shlex::new(s).collect::<Result<_>>()
}

pub fn escape_string(s: &str) -> String {
    s.replace(r"\", r"\\").replace("\"", "\\\"")
}

pub fn naive_today() -> chrono::NaiveDate {
    chrono::offset::Local::today().naive_local()
}

pub fn elapsed(time: i64) -> i64 {
    let now = chrono::Utc::now().naive_utc();
    let from = chrono::NaiveDateTime::from_timestamp(time, 0);
    (now - from).num_seconds()
}

/// Returns the last component of a colon-separated account string
pub fn last_component(s: &str) -> &str {
    s.rsplit_once(':').map(|x| x.1).unwrap_or(s)
}

// taken from once_cell documentation
macro_rules! regex {
    ($re:literal $(,)?) => {{
        static RE: once_cell::sync::OnceCell<regex::Regex> = once_cell::sync::OnceCell::new();
        RE.get_or_init(|| regex::Regex::new($re).unwrap())
    }};
}

#[cfg(test)]
mod tests {
    use super::command_split;

    fn verify(input: &str, result: &[&str]) {
        assert_eq!(
            command_split(input).unwrap(),
            result.iter().map(|&x| x.to_owned()).collect::<Vec<_>>(),
        );
    }
    fn verify_none(input: &str, msg: &str) {
        let err = command_split(input).unwrap_err();
        assert_eq!(&format!("{}", err), msg);
    }

    #[test]
    fn test_split() {
        verify("foo$baz", &["foo$baz"]);
        verify("foo baz", &["foo", "baz"]);
        verify("foo\"bar\"baz", &["foobarbaz"]);
        verify("foo \"bar\"baz", &["foo", "barbaz"]);
        verify("'baz\\$b'", &["baz\\$b"]);
        verify("foo #bar", &["foo", "#bar"]);
        verify("foo#bar", &["foo#bar"]);
        verify(r"'\n'", &[r"\n"]);
        verify(r"'\\n'", &[r"\\n"]);
        verify("foo #bar  baz", &["foo", "#bar", "baz"]);
        verify("\\", &[r"\"]);
        verify(r#""def\\\"abc" \"#, &[r#"def\"abc"#, r"\"]);

        verify_none("   foo \nbar", "newline within argument");
        verify_none("foo\\\nbar", "newline within argument");
        verify_none("foo \"b\nar\"", "newline within double quote");
        verify_none("foo '\nba'r", "newline within single quote");
        verify_none("foo\"#bar", "unmatched double quote");
        verify_none(r"'baz\''", "unmatched single quote");
        verify_none("\"\\", "unmatched double quote");
        verify_none(r"'\", "unmatched single quote");
        verify_none("\"", "unmatched double quote");
        verify_none("'", "unmatched single quote");
    }

    #[test]
    fn test_bean_command() {
        verify(
            ">公司\t#trip  '10 CNY' \tali \"food out\"  \t 'the rest'  ",
            &[">公司", "#trip", "10 CNY", "ali", "food out", "the rest"],
        );
        verify(
            r#"  >公司 '10 CNY' ali "food \"out"  'narr the rest'"#,
            &[">公司", "10 CNY", "ali", "food \"out", "narr the rest"],
        );
    }
}
