use std::{cmp::Ordering, io, sync::Arc};

use futures::AsyncReadExt;
use indexmap::IndexMap;
use leakpolicy::PathConfiguration;

use crate::{
    evaluator::{self, MatcherMetadata, MatcherState},
    pipe::PipeReader,
    policy::{ContentType, Policy, PolicyAction},
    proto::Match,
};

use super::ParseResponse;

#[derive(Clone, Copy, PartialEq, Eq)]
struct PolicyMatch<'a> {
    category_name: &'a str,
    action: &'a PolicyAction,
    start: usize,
    length: usize,
}

impl<'a> PartialOrd for PolicyMatch<'a> {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        Some(self.start.cmp(&other.start))
    }
}

impl<'a> Ord for PolicyMatch<'a> {
    fn cmp(&self, other: &Self) -> Ordering {
        self.start.cmp(&other.start)
    }
}

fn prepare_match_state<'a>(
    policy: &'a Policy,
    configuration: &'a IndexMap<Arc<String>, PathConfiguration>,
) -> MatcherState<'a> {
    let mut match_state = MatcherState::default();

    for (category_name, action) in configuration {
        if !action.category_config.content_types.is_empty() {
            if !action
                .category_config
                .content_types
                .contains(&ContentType::Html)
            {
                continue;
            }
        }

        if matches!(
            action.category_config.action.unwrap_or_default(),
            PolicyAction::Ignore
        ) {
            continue;
        }

        // let is_block = matches!(action.action, PolicyAction::Block);

        let metadata = MatcherMetadata {
            policy_path: action.matcher_path.clone(),
            category_name: category_name.to_string(),
            action: action.category_config.action.unwrap_or_default(),
            local_report_style: action.report_style,
            correlation: None,
        };

        evaluator::prepare_matches(
            &policy,
            &**category_name,
            &mut match_state,
            &metadata,
            &action.category_config.ignore,
        );
    }

    match_state
}

const CHUNK_SIZE: usize = 1024 * 64;
const CHUNK_OVERLAP: usize = 512;

//todo: remove unwraps for regex failure
//todo: some kind of bufreader in here
pub async fn parse_html(
    policy: &Policy,
    body: &mut PipeReader,
    configuration: &IndexMap<Arc<String>, PathConfiguration>,
    matches: &mut Vec<Match>,
) -> io::Result<ParseResponse> {
    let mut chunk = Vec::<u8>::with_capacity(CHUNK_SIZE);
    let mut overlap_index = 0usize;
    let mut chunk_len = 0usize;
    //TODO: safety, use ManuallyUninit?
    unsafe { chunk.set_len(CHUNK_SIZE) };

    let match_state = prepare_match_state(policy, configuration);

    loop {
        let minimum_end_index = body.total_read();
        let index = minimum_end_index - overlap_index;
        chunk.copy_within(chunk_len - overlap_index..chunk_len, 0);
        chunk_len = body.read(&mut chunk[overlap_index..]).await?;
        if chunk_len == 0 {
            break;
        }
        chunk_len += overlap_index;
        let data = &chunk[..chunk_len];

        overlap_index = CHUNK_OVERLAP.min(chunk_len);
        //TODO: make a better way to trim truncated utf8 characters to avoid allocation
        match match_state.do_matching(
            index,
            minimum_end_index,
            &*String::from_utf8_lossy(&data[..]),
            matches,
        ) {
            ParseResponse::Continue => (),
            ParseResponse::Block => return Ok(ParseResponse::Block),
        }
    }
    Ok(ParseResponse::Continue)
}
