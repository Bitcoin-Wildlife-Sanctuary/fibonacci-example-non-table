use crate::bitcoin_script::fiat_shamir::FibonacciFiatShamirGadget;
use crate::bitcoin_script::fold::FibonacciPerQueryFoldGadget;
use crate::bitcoin_script::prepare::FibonacciPrepareGadget;
use crate::bitcoin_script::quotients::FibonacciPerQueryQuotientGadget;
use bitcoin_circle_stark::treepp::*;
use bitcoin_circle_stark::utils::clean_stack;
use stwo_prover::core::channel::Sha256Channel;
use stwo_prover::core::prover::N_QUERIES;

mod composition;

pub(crate) mod fiat_shamir;

pub(crate) mod quotients;

pub(crate) mod prepare;

pub(crate) mod fold;

/// The Fibonacci log size in this test.
pub const FIB_LOG_SIZE: u32 = 5;

/// A verifier for the Fibonacci proof.
pub struct FibonacciVerifierGadget;

impl FibonacciVerifierGadget {
    /// Run the verifier in the Bitcoin script.
    pub fn run_verifier(channel: &Sha256Channel) -> Script {
        script! {
            // Run the Fiat-Shamir gadget
            { FibonacciFiatShamirGadget::run(channel) }

            // Run prepare gadget
            { FibonacciPrepareGadget::run() }

            // stack:
            //    circle_poly_alpha (4)
            //    (commitment, alpha), ..., (commitment, alpha) (1 + 4) * FIB_LOG_SIZE
            //    last layer (4)
            //    queries (N_QUERIES)
            //    trace queries (2 * N_QUERIES)
            //    composition queries (8 * N_QUERIES)
            //    masked points (3 * 8 = 24)
            //    oods point (8)
            //    (a, b), (a, b), (a, b) for trace (3 * 2 * 2 = 12)
            //    (a, b), (a, b), (a, b), (a, b) for composition (4 * 2 * 2 = 16)
            //    prepared masked points (3 * 4 = 12)
            //    prepared oods point (4)
            //    coeff^6, coeff^5, ..., coeff (24)

            for i in 0..N_QUERIES {
                { FibonacciPerQueryQuotientGadget::run(i) }
                { FibonacciPerQueryFoldGadget::run(i) }
            }

            // stack:
            //    circle_poly_alpha (4)
            //    (commitment, alpha), ..., (commitment, alpha) (1 + 4) * FIB_LOG_SIZE
            //    last layer (4)
            //    queries (N_QUERIES)
            //    trace queries (2 * N_QUERIES)
            //    composition queries (8 * N_QUERIES)
            //    masked points (3 * 8 = 24)
            //    oods point (8)
            //    (a, b), (a, b), (a, b) for trace (3 * 2 * 2 = 12)
            //    (a, b), (a, b), (a, b), (a, b) for composition (4 * 2 * 2 = 16)
            //    prepared masked points (3 * 4 = 12)
            //    prepared oods point (4)
            //    coeff^6, coeff^5, ..., coeff (24)

            // clean up the stack
            { clean_stack(24 + 4 + 12 + 16 + 12 + 8 + 24 + (2 + 8 + 1) * N_QUERIES + 4 + (1 + 4) * FIB_LOG_SIZE as usize + 4) }
        }
    }
}

#[cfg(test)]
mod test {
    use crate::bitcoin_script::FIB_LOG_SIZE;
    use crate::{verify_with_hints, FibonacciVerifierGadget};
    use bitcoin_circle_stark::tests_utils::report::report_bitcoin_script_size;
    use bitcoin_circle_stark::treepp::*;
    use bitcoin_scriptexec::execute_script_with_witness_unlimited_stack;
    use stwo_prover::core::channel::Sha256Channel;
    use stwo_prover::core::fields::m31::{BaseField, M31};
    use stwo_prover::core::fields::IntoSlice;
    use stwo_prover::core::pcs::PcsConfig;
    use stwo_prover::core::vcs::sha256_hash::Sha256Hasher;
    use stwo_prover::core::vcs::sha256_merkle::Sha256MerkleChannel;
    use stwo_prover::examples::fibonacci::Fibonacci;
    use stwo_prover::trace_generation::{commit_and_prove, commit_and_verify};

    #[test]
    fn test_verifier() {
        let fib = Fibonacci::new(FIB_LOG_SIZE, M31::reduce(443693538));
        let config = PcsConfig::default();

        let trace = fib.get_trace();
        let channel = &mut Sha256Channel::default();
        channel.update_digest(Sha256Hasher::hash(BaseField::into_slice(&[fib
            .air
            .component
            .claim])));
        let proof =
            commit_and_prove::<_, Sha256MerkleChannel>(&fib.air, channel, vec![trace], config)
                .unwrap();

        {
            let channel = &mut Sha256Channel::default();
            channel.update_digest(Sha256Hasher::hash(BaseField::into_slice(&[fib
                .air
                .component
                .claim])));
            commit_and_verify::<Sha256MerkleChannel>(proof.clone(), &fib.air, channel, config)
                .unwrap();
        }

        let channel = &mut Sha256Channel::default();
        channel.update_digest(Sha256Hasher::hash(BaseField::into_slice(&[fib
            .air
            .component
            .claim])));
        let channel_clone = channel.clone();

        let hint = verify_with_hints(proof, &fib.air, channel).unwrap();

        let witness = script! {
            { hint }
        };

        let script = script! {
            { FibonacciVerifierGadget::run_verifier(&channel_clone) }
            OP_TRUE
        };

        report_bitcoin_script_size("Fibonacci", "verifier", script.len());

        let exec_result = execute_script_with_witness_unlimited_stack(
            script,
            convert_to_witness(witness).unwrap(),
        );
        assert!(exec_result.success);
        #[cfg(feature = "profiler")]
        exec_result.profiler.print_stats();
    }
}
