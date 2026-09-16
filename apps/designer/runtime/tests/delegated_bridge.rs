use tachiko_designer_runtime::{
    DelegatedOutcome, DesignerRequest, DesignerResponse, DesignerRuntime, FieldTarget,
    ScalarEditInput, StoredValueProjection, close_project, open_project,
};

const OCCURRENCE: &str = "00000000-0000-4000-8000-000000000099";

fn runtime() -> DesignerRuntime {
    DesignerRuntime::moonfall(OCCURRENCE).expect("fixture must admit")
}

fn target() -> FieldTarget {
    FieldTarget {
        entity: "iron_sword".to_owned(),
        field: "damage".to_owned(),
    }
}

fn proposal(runtime: &mut DesignerRuntime) -> String {
    let DesignerResponse::DelegatedProposal(proposal) = runtime
        .handle(DesignerRequest::DelegatedPropose {
            expected_revision: "resident/0".to_owned(),
            target: target(),
            input: ScalarEditInput::Number {
                input: "3".to_owned(),
            },
        })
        .expect("proposal must be admitted through the lifecycle")
    else {
        panic!("expected delegated proposal response");
    };
    assert_eq!(proposal.value, StoredValueProjection::Number { value: 3.0 });
    proposal.proposal_id
}

fn review(runtime: &mut DesignerRuntime, proposal_id: &str) -> DesignerResponse {
    runtime
        .handle(DesignerRequest::DelegatedPreview {
            proposal_id: proposal_id.to_owned(),
        })
        .expect("review request must receive a reduced response")
}

fn approve(runtime: &mut DesignerRuntime, proposal_id: &str) -> DesignerResponse {
    runtime
        .handle(DesignerRequest::DelegatedApprove {
            proposal_id: proposal_id.to_owned(),
        })
        .expect("approval request must receive a reduced response")
}

fn execute(runtime: &mut DesignerRuntime, proposal_id: &str) -> DesignerResponse {
    runtime
        .handle(DesignerRequest::DelegatedExecute {
            proposal_id: proposal_id.to_owned(),
        })
        .expect("execute request must receive a reduced response")
}

fn bytes(runtime: &DesignerRuntime, revision: &str) -> Vec<u8> {
    runtime
        .export_project(revision)
        .expect("current project must export")
        .bytes
}

#[test]
fn delegated_bridge_preserves_the_eight_fixed_proposal_lifecycle_cases() {
    // M2-01: proposal/review never publishes canonical state.
    let mut first = runtime();
    let initial = bytes(&first, "resident/0");
    let first_proposal = proposal(&mut first);
    let DesignerResponse::DelegatedReview(first_review) = review(&mut first, &first_proposal)
    else {
        panic!("expected delegated review response");
    };
    assert_eq!(first_review.outcome, DelegatedOutcome::Ready);
    assert_eq!(bytes(&first, "resident/0"), initial);

    // M2-02: delegated execution has no approval bypass.
    let DesignerResponse::DelegatedExecution(unapproved) = execute(&mut first, &first_proposal)
    else {
        panic!("expected delegated execution response");
    };
    assert_eq!(unapproved.outcome, DelegatedOutcome::Denied);
    assert_eq!(bytes(&first, "resident/0"), initial);

    // M2-03: exact Human approval publishes once; replay cannot publish again.
    let mut second = runtime();
    let second_proposal = proposal(&mut second);
    let DesignerResponse::DelegatedApproval(approved) = approve(&mut second, &second_proposal)
    else {
        panic!("expected delegated approval response");
    };
    assert_eq!(approved.outcome, DelegatedOutcome::Approved);
    let DesignerResponse::DelegatedExecution(published) = execute(&mut second, &second_proposal)
    else {
        panic!("expected delegated execution response");
    };
    assert_eq!(published.outcome, DelegatedOutcome::Published);
    let publication = published
        .publication
        .expect("publication receipt is required");
    let DesignerResponse::Fields(fields) = second
        .handle(DesignerRequest::QueryFields {
            expected_revision: publication.resulting_revision.clone(),
            fields: vec![target()],
        })
        .expect("published field remains queryable")
    else {
        panic!("expected field projection");
    };
    assert_eq!(
        fields.fields[0].stored,
        Some(StoredValueProjection::Number { value: 3.0 })
    );
    let after_publish = bytes(&second, &publication.resulting_revision);
    let DesignerResponse::DelegatedExecution(replay) = execute(&mut second, &second_proposal)
    else {
        panic!("expected replay response");
    };
    assert_eq!(replay.outcome, DelegatedOutcome::Denied);
    assert_eq!(
        bytes(&second, &publication.resulting_revision),
        after_publish
    );

    // M2-04: an intervening ordinary Human edit invalidates the exact proposal.
    let mut third = runtime();
    let third_proposal = proposal(&mut third);
    let _ = approve(&mut third, &third_proposal);
    let DesignerResponse::Published(human_edit) = third
        .handle(DesignerRequest::EditScalar {
            expected_revision: "resident/0".to_owned(),
            target: target(),
            input: ScalarEditInput::Number {
                input: "4".to_owned(),
            },
        })
        .expect("ordinary human edit must publish")
    else {
        panic!("expected ordinary publication");
    };
    let third_after_human = bytes(&third, &human_edit.resulting_revision);
    let DesignerResponse::DelegatedExecution(stale) = execute(&mut third, &third_proposal) else {
        panic!("expected stale execution response");
    };
    assert_eq!(stale.outcome, DelegatedOutcome::Denied);
    assert_eq!(
        bytes(&third, &human_edit.resulting_revision),
        third_after_human
    );

    // M2-05: revoking the delegated grant denies publication.
    let mut fourth = runtime();
    let fourth_proposal = proposal(&mut fourth);
    let _ = approve(&mut fourth, &fourth_proposal);
    let before_revoke = bytes(&fourth, "resident/0");
    let _ = fourth
        .handle(DesignerRequest::DelegatedRevokeAuthority)
        .expect("test-only revocation must be available");
    let DesignerResponse::DelegatedExecution(revoked) = execute(&mut fourth, &fourth_proposal)
    else {
        panic!("expected revoked execution response");
    };
    assert_eq!(revoked.outcome, DelegatedOutcome::Denied);
    assert_eq!(bytes(&fourth, "resident/0"), before_revoke);

    // M2-06: revoked delegated Query exposes neither proposal evidence nor values.
    let mut fifth = runtime();
    let fifth_proposal = proposal(&mut fifth);
    let _ = fifth
        .handle(DesignerRequest::DelegatedRevokeQuery)
        .expect("test-only query revocation must be available");
    let DesignerResponse::DelegatedReview(hidden) = review(&mut fifth, &fifth_proposal) else {
        panic!("expected denied review response");
    };
    assert_eq!(hidden.outcome, DelegatedOutcome::Denied);
    assert!(hidden.disclosed_subjects.is_empty());
    assert!(hidden.disclosed_values.is_empty());

    // M2-07: the execution endpoint has no alternate-body admission path.
    let mut sixth = runtime();
    let sixth_proposal = proposal(&mut sixth);
    let _ = approve(&mut sixth, &sixth_proposal);
    let before_altered = bytes(&sixth, "resident/0");
    let DesignerResponse::DelegatedExecution(altered) = sixth
        .handle(DesignerRequest::DelegatedExecuteAltered {
            proposal_id: sixth_proposal,
            target: target(),
            input: ScalarEditInput::Number {
                input: "9".to_owned(),
            },
        })
        .expect("hostile altered execution must be reduced")
    else {
        panic!("expected altered execution response");
    };
    assert_eq!(altered.outcome, DelegatedOutcome::Denied);
    assert_eq!(bytes(&sixth, "resident/0"), before_altered);

    // M2-08: a fresh occurrence does not retain prior proposal/approval state.
    let mut seventh = runtime();
    let seventh_proposal = proposal(&mut seventh);
    let _ = approve(&mut seventh, &seventh_proposal);
    let project = bytes(&seventh, "resident/0");
    let mut reopened = Some(seventh);
    close_project(&mut reopened);
    open_project(
        &mut reopened,
        &project,
        "00000000-0000-4000-8000-000000000100",
    )
    .expect("fresh occurrence must admit the same bytes");
    let DesignerResponse::DelegatedExecution(reopened_execution) = execute(
        reopened.as_mut().expect("fresh occurrence exists"),
        &seventh_proposal,
    ) else {
        panic!("expected fresh-occurrence execution response");
    };
    assert_eq!(reopened_execution.outcome, DelegatedOutcome::Denied);
}

#[test]
fn delegated_approval_expiry_denies_before_and_inside_publication_without_mutation() {
    // A preview consumes one trusted activity tick.  After approval, 1,022
    // previews leave Execute one tick before expiry; the publication boundary
    // receives the final tick and must still refuse installation.
    let mut boundary_expiry = runtime();
    let proposal_id = proposal(&mut boundary_expiry);
    let _ = approve(&mut boundary_expiry, &proposal_id);
    for _ in 0..1_022 {
        let _ = review(&mut boundary_expiry, &proposal_id);
    }
    let before_boundary = bytes(&boundary_expiry, "resident/0");
    let DesignerResponse::DelegatedExecution(boundary) =
        execute(&mut boundary_expiry, &proposal_id)
    else {
        panic!("expected execution response");
    };
    assert_eq!(boundary.outcome, DelegatedOutcome::Denied);
    assert_eq!(bytes(&boundary_expiry, "resident/0"), before_boundary);

    // One additional preview makes Execute itself reach the finite expiry.
    let mut start_expiry = runtime();
    let proposal_id = proposal(&mut start_expiry);
    let _ = approve(&mut start_expiry, &proposal_id);
    for _ in 0..1_023 {
        let _ = review(&mut start_expiry, &proposal_id);
    }
    let before_start = bytes(&start_expiry, "resident/0");
    let DesignerResponse::DelegatedExecution(start) = execute(&mut start_expiry, &proposal_id)
    else {
        panic!("expected execution response");
    };
    assert_eq!(start.outcome, DelegatedOutcome::Denied);
    assert_eq!(bytes(&start_expiry, "resident/0"), before_start);
}
