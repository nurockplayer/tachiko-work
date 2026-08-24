use tachiko_tw05_reference_runtime::run_sequence;

fn main() {
    let facts = run_sequence();
    println!(
        "[{{\"step\":\"open\",\"revision\":0}},{{\"step\":\"overview\",\"entity_count\":2,\"formula_count\":2}},{{\"step\":\"calculate\",\"first_product\":2,\"second_product\":4}},{{\"step\":\"set_first_base\",\"revision\":{},\"first_product\":{}}},{{\"step\":\"stale_set_first_base\",\"typed_stale_revision_error\":{},\"actual_revision\":{},\"state_unchanged\":{}}},{{\"step\":\"snapshot\",\"revision\":{},\"first_base\":{},\"first_product\":{}}}]",
        facts.first_revision,
        facts.first_product,
        facts.stale_rejected,
        facts.actual_revision,
        facts.state_unchanged,
        facts.actual_revision,
        facts.final_base,
        facts.final_product,
    );
}
