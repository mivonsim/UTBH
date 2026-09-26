//! Operasi workload — parsing nama `[operation].op` dari test definition.

/// Operasi yang dipahami workload engine.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Op {
    Add,
    Sub,
    Mul,
    Fma,
    MatMul,
    ReduceSum,
    Compare,
    Copy,
    /// Pola akses memory: seq | rand — untuk latency/bandwidth test.
    MemSeq,
    MemRand,
    /// Atomic correctness: banyak thread fetch_add ke shared counter.
    AtomicAdd,
    /// Multicore reduce: partial sum per thread, digabung.
    ParallelSum,
}

impl Op {
    pub fn parse(s: &str) -> Result<Op, String> {
        Ok(match s {
            "add" | "vector_add" => Op::Add,
            "sub" | "vector_sub" => Op::Sub,
            "mul" | "vector_mul" => Op::Mul,
            "fma" | "vector_fma" => Op::Fma,
            "matmul" | "matrix_mul" => Op::MatMul,
            "reduce_sum" | "reduce" => Op::ReduceSum,
            "compare" => Op::Compare,
            "copy" | "memcopy" => Op::Copy,
            "mem_seq" | "seq" => Op::MemSeq,
            "mem_rand" | "rand" => Op::MemRand,
            "atomic_add" => Op::AtomicAdd,
            "parallel_sum" | "multicore_sum" => Op::ParallelSum,
            other => return Err(format!("op '{}' tidak dikenal workload engine", other)),
        })
    }

    /// Butuh dua operand?
    pub fn binary(self) -> bool {
        matches!(
            self,
            Op::Add | Op::Sub | Op::Mul | Op::Fma | Op::MatMul | Op::Compare
        )
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_aliases() {
        assert_eq!(Op::parse("vector_add").unwrap(), Op::Add);
        assert_eq!(Op::parse("add").unwrap(), Op::Add);
        assert_eq!(Op::parse("matrix_mul").unwrap(), Op::MatMul);
    }

    #[test]
    fn rejects_unknown() {
        assert!(Op::parse("quantum_shuffle").is_err());
    }

    #[test]
    fn binary_classification() {
        assert!(Op::Add.binary());
        assert!(Op::MatMul.binary());
        assert!(!Op::Copy.binary());
        assert!(!Op::ReduceSum.binary());
    }
}
