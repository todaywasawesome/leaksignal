use std::borrow::Cow;

use log::warn;

use crate::evaluator::CategoryMatch;

pub mod html;
// pub mod jpeg;
pub mod json;

#[allow(dead_code)]
pub enum ParseResponse {
    Continue,
    Block,
}

#[allow(dead_code)]
fn replace_matches<'a>(
    body: &'a str,
    matches: &[CategoryMatch],
    expected_length: usize,
    replace_with: &str,
) -> Cow<'a, str> {
    if matches.is_empty() {
        return Cow::Borrowed(body);
    }
    let mut output = String::with_capacity(expected_length);
    let mut body_index = 0usize;
    for policy_match in matches {
        if body_index > policy_match.start {
            warn!("overlapping matches detected!");
            body_index = policy_match.start;
        }
        output.push_str(&body[body_index..policy_match.start]);
        output.push_str(replace_with);
        body_index = policy_match.start + policy_match.length;
    }
    output.push_str(&body[body_index..]);
    Cow::Owned(output)
}
