use crate::service::events::EventChange;

const MAX_SSE_INLINE_PUSH_BYTES: usize = 64 * 1024;

pub(super) struct SseInlineBudget {
    per_file_max: usize,
    remaining: usize,
}

impl SseInlineBudget {
    pub(super) fn new(per_file_max: usize) -> Self {
        Self {
            per_file_max,
            remaining: MAX_SSE_INLINE_PUSH_BYTES,
        }
    }
}

pub(super) fn text_event_with_budget(
    path: &str,
    content: &str,
    budget: &mut SseInlineBudget,
) -> EventChange {
    if content.len() <= budget.per_file_max && content.len() <= budget.remaining {
        budget.remaining = budget.remaining.saturating_sub(content.len());
        EventChange::TextInline {
            path: path.to_string(),
            content: content.to_string(),
        }
    } else {
        EventChange::TextRef {
            path: path.to_string(),
            size: content.len() as u64,
        }
    }
}
