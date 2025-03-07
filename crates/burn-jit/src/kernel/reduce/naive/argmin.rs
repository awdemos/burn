use cubecl::prelude::*;

use crate::kernel::reduce::Argmin;

use super::base::{ReduceDimNaive, ReduceDimNaiveFamily};

impl ReduceDimNaiveFamily for Argmin {
    type Reduce<E: Numeric> = Self;
}

#[cube]
impl<EI: Numeric> ReduceDimNaive<EI> for Argmin {
    type Accumulator = (EI, u32);

    fn initialize_naive() -> Self::Accumulator {
        // TODO: switch to using f32::INFINITY when it's supported: https://github.com/tracel-ai/cubecl/issues/68
        (EI::max_value(), 0u32)
    }

    fn inner_loop_naive(accumulator: &mut Self::Accumulator, current_value: EI, i: u32) {
        let (min, index) = accumulator;
        if current_value < *min {
            *min = current_value;
            *index = i;
        }
    }

    fn assign_naive<EO: Numeric>(
        output: &mut Tensor<EO>,
        accumulator: Self::Accumulator,
        _shape_reduce_dim: u32,
    ) {
        let (_, index) = accumulator;
        output[ABSOLUTE_POS] = EO::cast_from(index);
    }
}
