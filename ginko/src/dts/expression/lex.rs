use crate::dts::expression::token::Token;
use crate::dts::expression::SyntaxKind::*;

pub(crate) fn lex(input: &str) -> Vec<Token> {
    let mut result = Vec::new();
    let mut iter = input.chars().peekable();
    loop {
        let Some(ch) = iter.next() else { break };
        match ch {
            '(' => result.push(Token::new(L_PAR, ch.into())),
            ')' => result.push(Token::new(R_PAR, ch.into())),
            '-' => result.push(Token::new(MINUS, ch.into())),
            '~' => result.push(Token::new(TILDE, ch.into())),
            '^' => result.push(Token::new(CIRC, ch.into())),
            '!' => match iter.peek() {
                Some('=') => {
                    iter.next();
                    result.push(Token::new(NEQ, "!=".into()))
                }
                _ => result.push(Token::new(TILDE, ch.into())),
            },
            '*' => result.push(Token::new(STAR, ch.into())),
            '/' => result.push(Token::new(SLASH, ch.into())),
            '%' => result.push(Token::new(PERCENT, ch.into())),
            '+' => result.push(Token::new(PLUS, ch.into())),
            ':' => result.push(Token::new(COLON, ch.into())),
            '?' => result.push(Token::new(QUESTION_MARK, ch.into())),
            '>' => match iter.peek() {
                Some('>') => {
                    iter.next();
                    result.push(Token::new(DOUBLE_GT, ">>".into()));
                }
                Some('=') => {
                    iter.next();
                    result.push(Token::new(GTE, ">=".into()));
                }
                _ => result.push(Token::new(GT, ">".into())),
            },
            '<' => match iter.peek() {
                Some('<') => {
                    iter.next();
                    result.push(Token::new(DOUBLE_LT, ">>".into()));
                }
                Some('=') => {
                    iter.next();
                    result.push(Token::new(LTE, "<=".into()));
                }
                _ => result.push(Token::new(LT, "<".into())),
            },
            '=' => match iter.peek() {
                Some('=') => {
                    iter.next();
                    result.push(Token::new(EQ, "==".into()))
                }
                _ => result.push(Token::new(ERROR, "=".into())),
            },
            '&' => match iter.peek() {
                Some('&') => {
                    iter.next();
                    result.push(Token::new(DOUBLE_AMP, "&&".into()))
                }
                _ => result.push(Token::new(AMP, "&".into())),
            },
            '|' => match iter.peek() {
                Some('|') => {
                    iter.next();
                    result.push(Token::new(DOUBLE_BAR, "||".into()))
                }
                _ => result.push(Token::new(BAR, "|".into())),
            },
            ' ' | '\n' | '\t' => {
                let mut buf = String::new();
                buf.push(ch);
                while iter.peek().is_some_and(|ch| ch.is_whitespace()) {
                    buf.push(iter.next().unwrap())
                }
                result.push(Token::new(WHITESPACE, buf))
            }
            '0'..='9' => {
                let mut buf = String::new();
                buf.push(ch);
                while iter.peek().is_some_and(|ch| ch.is_ascii_alphanumeric()) {
                    buf.push(iter.next().unwrap())
                }
                result.push(Token::new(NUMBER, buf))
            }
            _ => result.push(Token::new(ERROR, ch.into())),
        }
    }
    result
}
