use std::any::TypeId;

use burn_tensor::{
    ops::{conv::calculate_conv_output_size, ConvOptions},
    Shape,
};
use cubecl::{
    flex32,
    ir::{Elem, FloatKind},
    linalg::matmul::{self, components::MatrixLayout},
    tensor_line_size, tf32, Feature,
};
use half::{bf16, f16};

use super::{
    precision::ConvPrecision,
    selection::{Balanced, ConvSelector, Large},
};
use crate::{
    kernel::{
        conv::{
            conv2d::gemm::{
                algorithm::{Algorithm, ImplicitCmmaConv},
                base::{ConvolutionLaunch, ConvolutionProblem},
            },
            nchw_to_nhwc, Conv2dAutotuneKey,
        },
        into_contiguous,
    },
    ops::{numeric::empty_device, permute, reshape},
    tensor::JitTensor,
    FloatElement, JitElement, JitRuntime,
};

/// Perform a 2D convolution using the implicit GEMM (im2col) algorithm, using cubecl tiling matmul
/// components. Uses [`CmmaLargeMAlgorithm`] for the stage size
///
/// * `input` - The input feature map
/// * `weight` - The weights (filter) applied to each kernel
/// * `bias` - The bias added to each channel
/// * `options` - The options to use for the convolution
pub fn conv2d_gemm_cmma_large_m<R: JitRuntime, F: FloatElement>(
    input: JitTensor<R>,
    weight: JitTensor<R>,
    bias: Option<JitTensor<R>>,
    options: ConvOptions<2>,
) -> JitTensor<R> {
    conv2d_gemm_cmma_strategy::<R, F, ImplicitCmmaConv, Large>(input, weight, bias, options)
}

/// Perform a 2D convolution using the implicit GEMM (im2col) algorithm, using cubecl tiling matmul
/// components. Uses [`CmmaBalancedAlgorithm`] for the stage size
///
/// * `input` - The input feature map
/// * `weight` - The weights (filter) applied to each kernel
/// * `bias` - The bias added to each channel
/// * `options` - The options to use for the convolution
pub fn conv2d_gemm_cmma_balanced<R: JitRuntime, F: FloatElement>(
    input: JitTensor<R>,
    weight: JitTensor<R>,
    bias: Option<JitTensor<R>>,
    options: ConvOptions<2>,
) -> JitTensor<R> {
    conv2d_gemm_cmma_strategy::<R, F, ImplicitCmmaConv, Balanced>(input, weight, bias, options)
}

fn conv2d_gemm_cmma_strategy<
    R: JitRuntime,
    F: FloatElement,
    Alg: Algorithm,
    S: ConvSelector<Alg>,
>(
    input: JitTensor<R>,
    weight: JitTensor<R>,
    bias: Option<JitTensor<R>>,
    options: ConvOptions<2>,
) -> JitTensor<R> {
    if TypeId::of::<F>() == TypeId::of::<flex32>() {
        conv2d_gemm_with_algo::<R, (F, f16, f32), Alg, S>(input, weight, bias, options)
    } else if TypeId::of::<F>() == TypeId::of::<bf16>() || TypeId::of::<F>() == TypeId::of::<f16>()
    {
        conv2d_gemm_with_algo::<R, (F, F, f32), Alg, S>(input, weight, bias, options)
    } else {
        conv2d_gemm_with_algo::<R, (F, tf32, f32), Alg, S>(input, weight, bias, options)
    }
}

/// Perform a 2D convolution using the implicit GEMM (im2col) algorithm, using cubecl tiling matmul
/// components, using the specified algorithm.
///
/// * `input` - The input feature map
/// * `weight` - The weights (filter) applied to each kernel
/// * `bias` - The bias added to each channel
/// * `options` - The options to use for the convolution
pub fn conv2d_gemm_with_algo<
    R: JitRuntime,
    SP: ConvPrecision,
    Alg: Algorithm,
    S: ConvSelector<Alg>,
>(
    input: JitTensor<R>,
    weight: JitTensor<R>,
    bias: Option<JitTensor<R>>,
    options: ConvOptions<2>,
) -> JitTensor<R>
where
    SP::EG: JitElement,
{
    let [batch_size, in_channels, height, width] = input.shape.dims();
    let [out_channels, _, kernel_h, kernel_w] = weight.shape.dims();

    let out_h = calculate_conv_output_size(
        kernel_h,
        options.stride[0],
        options.padding[0],
        options.dilation[0],
        height,
    );
    let out_w = calculate_conv_output_size(
        kernel_w,
        options.stride[1],
        options.padding[1],
        options.dilation[1],
        width,
    );

    let input = match input.is_contiguous() {
        true => nchw_to_nhwc::<R, SP::EG>(input),
        false => into_contiguous(permute(input, &[0, 2, 3, 1])),
    };
    let weight = into_contiguous(permute(weight, &[2, 3, 1, 0]));

    // Implicit GEMM matrix size
    let gemm_m = batch_size * out_h * out_w;
    let gemm_n = out_channels;
    let gemm_k = kernel_h * kernel_w * in_channels;

    let weight = reshape(weight, Shape::new([gemm_k, gemm_n]));

    let out_shape = Shape::new([gemm_m, gemm_n]);
    let out = empty_device::<R, SP::EG>(input.client.clone(), input.device.clone(), out_shape);

    // Target 128 bit accesses
    let available_vectorizations = R::supported_line_sizes()
        .iter()
        .copied()
        .filter(|it| *it as usize * size_of::<SP::EG>() <= 16)
        .collect::<Vec<_>>();
    let lhs_line_size = tensor_line_size(
        &available_vectorizations,
        &input.shape.dims,
        &input.strides,
        3,
    );
    let rhs_line_size = tensor_line_size(
        &available_vectorizations,
        &weight.shape.dims,
        &weight.strides,
        1,
    );
    let out_line_size =
        tensor_line_size(&available_vectorizations, &out.shape.dims, &out.strides, 1);

    let problem = ConvolutionProblem {
        m: gemm_m,
        n: gemm_n,
        k: gemm_k,
        lhs_layout: matmul::components::MatrixLayout::RowMajor,
        rhs_layout: matmul::components::MatrixLayout::RowMajor,
        lhs_line_size,
        rhs_line_size,
        out_line_size,
        kernel_size: (kernel_h as u32, kernel_w as u32),
        options,
        out_shape_y: out_h,
        out_shape_x: out_w,
        has_bias: bias.is_some(),
    };

    let plane_dim = input
        .client
        .properties()
        .hardware_properties()
        .defined_plane_size()
        .unwrap_or(32);

    let (selection, config_input) = S::select_kernel::<R, SP>(plane_dim);
    let cube_dim = Alg::cube_dim(&selection);
    let cube_count = Alg::cube_count(&selection, &problem);

    let advanced_config = Default::default();
    let config = match Alg::make_config(
        config_input,
        &problem,
        &cube_dim,
        &cube_count,
        &advanced_config,
    ) {
        Ok(val) => val,
        Err(err) => {
            panic!("Can't launch conv kernel because of an invalid config: {err}")
        }
    };

    let bias = bias.unwrap_or_else(|| {
        empty_device::<R, SP::EG>(input.client.clone(), input.device.clone(), Shape::new([1]))
    });

    unsafe {
        Alg::GlobalConvolution::launch_unchecked::<SP, R>(
            &input.client,
            cube_dim,
            cube_count,
            input.as_tensor_arg::<SP::EG>(lhs_line_size),
            weight.as_tensor_arg::<SP::EG>(rhs_line_size),
            bias.as_tensor_arg::<SP::EG>(out_line_size),
            out.as_tensor_arg::<SP::EG>(out_line_size),
            config,
        );
    }

    // Reset to NCHW
    let out = reshape(out, Shape::new([batch_size, out_h, out_w, out_channels]));
    permute(out, &[0, 3, 1, 2])
}

pub fn problem_from_key<R: JitRuntime, F: FloatElement>(
    key: &Conv2dAutotuneKey,
    out_h: usize,
    out_w: usize,
) -> ConvolutionProblem {
    let in_stride_2 = key.in_channels;
    let in_stride_1 = key.width * in_stride_2;
    let in_stride_0 = key.height * in_stride_1;

    let m = key.batch_size * out_h * out_w;
    let n = key.out_channels;
    let k = key.kernel_size[0] * key.kernel_size[1] * key.in_channels;

    let options = ConvOptions {
        stride: key.stride,
        padding: key.padding,
        dilation: key.dilation,
        groups: key.groups,
    };

    // Target 128 bit accesses
    let available_vectorizations = R::supported_line_sizes()
        .iter()
        .copied()
        .filter(|it| *it as usize * size_of::<F>() <= 16)
        .collect::<Vec<_>>();
    let lhs_line_size = tensor_line_size(
        &available_vectorizations,
        &[key.batch_size, key.height, key.width, key.in_channels],
        &[in_stride_0, in_stride_1, in_stride_2, 1],
        3,
    );
    let rhs_line_size = tensor_line_size(&available_vectorizations, &[k, n], &[n, 1], 1);
    let out_line_size = tensor_line_size(&available_vectorizations, &[m, n], &[n, 1], 1);

    ConvolutionProblem {
        m,
        n,
        k,
        lhs_layout: MatrixLayout::RowMajor,
        rhs_layout: MatrixLayout::RowMajor,
        lhs_line_size,
        rhs_line_size,
        out_line_size,
        kernel_size: (key.kernel_size[0] as u32, key.kernel_size[1] as u32),
        options,
        out_shape_y: out_h,
        out_shape_x: out_w,
        has_bias: key.has_bias,
    }
}

pub(crate) fn has_tf32<R: JitRuntime>(c: &JitTensor<R>) -> bool {
    c.client
        .properties()
        .feature_enabled(Feature::Type(Elem::Float(FloatKind::TF32)))
}
