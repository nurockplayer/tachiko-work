#[derive(Clone, Copy)]
pub struct SequenceFacts {
    pub first_revision: u32,
    pub first_product: u32,
    pub stale_rejected: bool,
    pub actual_revision: u32,
    pub state_unchanged: bool,
    pub final_base: u32,
    pub final_product: u32,
}

struct Runtime {
    revision: u32,
    first_base: u32,
}

impl Runtime {
    fn apply(&mut self, expected_revision: u32, input: u32) -> Result<(), u32> {
        if expected_revision != self.revision {
            return Err(self.revision);
        }
        self.first_base = input;
        self.revision += 1;
        Ok(())
    }

    fn first_product(&self) -> u32 {
        self.first_base * 2
    }
}

#[must_use]
pub fn run_sequence() -> SequenceFacts {
    let mut runtime = Runtime {
        revision: 0,
        first_base: 1,
    };
    runtime.apply(0, 11).expect("fresh revision applies");
    let first_revision = runtime.revision;
    let first_product = runtime.first_product();
    let stale = runtime.apply(0, 12);
    SequenceFacts {
        first_revision,
        first_product,
        stale_rejected: stale.is_err(),
        actual_revision: stale.unwrap_err(),
        state_unchanged: runtime.revision == 1 && runtime.first_base == 11,
        final_base: runtime.first_base,
        final_product: runtime.first_product(),
    }
}

#[unsafe(no_mangle)]
pub extern "C" fn tw05_first_revision() -> u32 {
    run_sequence().first_revision
}

#[unsafe(no_mangle)]
pub extern "C" fn tw05_first_product() -> u32 {
    run_sequence().first_product
}

#[unsafe(no_mangle)]
pub extern "C" fn tw05_stale_rejected() -> u32 {
    u32::from(run_sequence().stale_rejected)
}

#[unsafe(no_mangle)]
pub extern "C" fn tw05_actual_revision() -> u32 {
    run_sequence().actual_revision
}

#[unsafe(no_mangle)]
pub extern "C" fn tw05_state_unchanged() -> u32 {
    u32::from(run_sequence().state_unchanged)
}

#[unsafe(no_mangle)]
pub extern "C" fn tw05_final_base() -> u32 {
    run_sequence().final_base
}

#[unsafe(no_mangle)]
pub extern "C" fn tw05_final_product() -> u32 {
    run_sequence().final_product
}
