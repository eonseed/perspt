use rust_ledger_fold::{fold, Event, State};

#[test]
fn folds_valid_history() {
    assert_eq!(
        fold(&[
            Event::Started {
                job: "build".into()
            },
            Event::Progress(2),
            Event::Progress(3),
            Event::Finished
        ])
        .unwrap(),
        State {
            job: Some("build".into()),
            progress: 5,
            finished: true
        }
    );
}

#[test]
fn rejects_invalid_histories() {
    assert!(fold(&[Event::Progress(1)]).is_err());
    assert!(fold(&[
        Event::Started { job: "a".into() },
        Event::Started { job: "b".into() }
    ])
    .is_err());
    assert!(fold(&[
        Event::Started { job: "a".into() },
        Event::Finished,
        Event::Progress(1)
    ])
    .is_err());
    assert!(fold(&[
        Event::Started { job: "a".into() },
        Event::Progress(u64::MAX),
        Event::Progress(1)
    ])
    .is_err());
}
