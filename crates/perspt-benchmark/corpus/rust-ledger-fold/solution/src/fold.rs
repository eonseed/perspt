use crate::{Event, State};

pub fn fold(events: &[Event]) -> Result<State, String> {
    let mut state = State::default();
    for event in events {
        if state.finished {
            return Err("event after finish".into());
        }
        match event {
            Event::Started { job } if state.job.is_none() => state.job = Some(job.clone()),
            Event::Started { .. } => return Err("duplicate start".into()),
            Event::Progress(delta) if state.job.is_some() => {
                state.progress = state
                    .progress
                    .checked_add(*delta)
                    .ok_or("progress overflow")?
            }
            Event::Progress(_) => return Err("progress before start".into()),
            Event::Finished if state.job.is_some() => state.finished = true,
            Event::Finished => return Err("finish before start".into()),
        }
    }
    Ok(state)
}
