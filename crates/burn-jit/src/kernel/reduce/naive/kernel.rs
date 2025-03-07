use crate::{
    element::JitElement, kernel::reduce::init_reduce_output, tensor::JitTensor, JitRuntime,
};
use cubecl::calculate_cube_count_elemwise;
use cubecl::prelude::*;

use super::base::ReduceDimNaive;
use super::base::ReduceDimNaiveFamily;

#[cube(launch_unchecked)]
pub(crate) fn naive_reduce_dim_kernel<RD: ReduceDimNaiveFamily, EI: Numeric, EO: Numeric>(
    input: &Tensor<EI>,
    output: &mut Tensor<EO>,
    dim: u32,
) {
    naive_reduce::<RD::Reduce<EI>, EI, EO>(input, output, dim)
}

#[cube]
fn naive_reduce<RD: ReduceDimNaive<EI>, EI: Numeric, EO: Numeric>(
    input: &Tensor<EI>,
    output: &mut Tensor<EO>,
    dim: u32,
) {
    if ABSOLUTE_POS >= output.len() {
        return;
    }

    let mut offset_input = 0;

    for i in 0..input.rank() {
        let mut offset_local = ABSOLUTE_POS / output.stride(i);
        offset_local %= output.shape(i);
        if i != dim {
            offset_input += offset_local * input.stride(i);
        }
    }

    let mut accumulator = RD::initialize_naive();

    for i in 0..input.shape(dim) {
        let index = i * input.stride(dim) + offset_input;
        RD::inner_loop_naive(&mut accumulator, input[index], i);
    }

    RD::assign_naive::<EO>(output, accumulator, input.shape(dim));
}

/// Executes the naive kernel for reduce dim
pub fn reduce_dim_naive<RD: ReduceDimNaiveFamily, R: JitRuntime, EI: JitElement, EO: JitElement>(
    input: JitTensor<R>,
    dim: usize,
) -> JitTensor<R> {
    let output = init_reduce_output::<R, EI, EO>(&input, dim);

    let cube_dim = CubeDim::default();
    let cube_count = calculate_cube_count_elemwise(output.shape.num_elements(), cube_dim);

    unsafe {
        naive_reduce_dim_kernel::launch_unchecked::<RD, EI, EO, R>(
            &input.client,
            cube_count,
            cube_dim,
            input.as_tensor_arg::<EI>(1),
            output.as_tensor_arg::<EO>(1),
            ScalarArg::new(dim as u32),
        );
    }

    output
}
