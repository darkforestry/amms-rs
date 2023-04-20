 function cross(
        mapping(int24 => Tick.Info) storage self,
        int24 tick,
        uint256 feeGrowthGlobal0X128,
        uint256 feeGrowthGlobal1X128,
        uint160 secondsPerLiquidityCumulativeX128,
        int56 tickCumulative,
        uint32 time
    ) internal returns (int128 liquidityNet) {
        Tick.Info storage info = self[tick];
        info.feeGrowthOutside0X128 = feeGrowthGlobal0X128 - info.feeGrowthOutside0X128;
        info.feeGrowthOutside1X128 = feeGrowthGlobal1X128 - info.feeGrowthOutside1X128;
        info.secondsPerLiquidityOutsideX128 = secondsPerLiquidityCumulativeX128 - info.secondsPerLiquidityOutsideX128;
        info.tickCumulativeOutside = tickCumulative - info.tickCumulativeOutside;
        info.secondsOutside = time - info.secondsOutside;
        liquidityNet = info.liquidityNet;
    }
}




    //  function swap(
    //     address recipient,
    //     bool zeroForOne,
    //     int256 amountSpecified,
    //     uint160 sqrtPriceLimitX96,
    //     bytes calldata data
    // ) external override noDelegateCall returns (int256 amount0, int256 amount1) {
    //     if (amountSpecified == 0) revert AS();

    //     Slot0 memory slot0Start = slot0;

    //     if (!slot0Start.unlocked) revert LOK();
    //     require(
    //         zeroForOne
    //             ? sqrtPriceLimitX96 < slot0Start.sqrtPriceX96 && sqrtPriceLimitX96 > TickMath.MIN_SQRT_RATIO
    //             : sqrtPriceLimitX96 > slot0Start.sqrtPriceX96 && sqrtPriceLimitX96 < TickMath.MAX_SQRT_RATIO,
    //         'SPL'
    //     );

    //     slot0.unlocked = false;

    //     SwapCache memory cache = SwapCache({
    //         liquidityStart: liquidity,
    //         blockTimestamp: _blockTimestamp(),
    //         feeProtocol: zeroForOne ? (slot0Start.feeProtocol % 16) : (slot0Start.feeProtocol >> 4),
    //         secondsPerLiquidityCumulativeX128: 0,
    //         tickCumulative: 0,
    //         computedLatestObservation: false
    //     });

    //     bool exactInput = amountSpecified > 0;

    //     SwapState memory state = SwapState({
    //         amountSpecifiedRemaining: amountSpecified,
    //         amountCalculated: 0,
    //         sqrtPriceX96: slot0Start.sqrtPriceX96,
    //         tick: slot0Start.tick,
    //         feeGrowthGlobalX128: zeroForOne ? feeGrowthGlobal0X128 : feeGrowthGlobal1X128,
    //         protocolFee: 0,
    //         liquidity: cache.liquidityStart
    //     });

    //     // continue swapping as long as we haven't used the entire input/output and haven't reached the price limit
    //     while (state.amountSpecifiedRemaining != 0 && state.sqrtPriceX96 != sqrtPriceLimitX96) {
    //         StepComputations memory step;

    //         step.sqrtPriceStartX96 = state.sqrtPriceX96;

    //         (step.tickNext, step.initialized) = tickBitmap.nextInitializedTickWithinOneWord(
    //             state.tick,
    //             tickSpacing,
    //             zeroForOne
    //         );

    //         // ensure that we do not overshoot the min/max tick, as the tick bitmap is not aware of these bounds
    //         if (step.tickNext < TickMath.MIN_TICK) {
    //             step.tickNext = TickMath.MIN_TICK;
    //         } else if (step.tickNext > TickMath.MAX_TICK) {
    //             step.tickNext = TickMath.MAX_TICK;
    //         }

    //         // get the price for the next tick
    //         step.sqrtPriceNextX96 = TickMath.getSqrtRatioAtTick(step.tickNext);

    //         // compute values to swap to the target tick, price limit, or point where input/output amount is exhausted
    //         (state.sqrtPriceX96, step.amountIn, step.amountOut, step.feeAmount) = SwapMath.computeSwapStep(
    //             state.sqrtPriceX96,
    //             (zeroForOne ? step.sqrtPriceNextX96 < sqrtPriceLimitX96 : step.sqrtPriceNextX96 > sqrtPriceLimitX96)
    //                 ? sqrtPriceLimitX96
    //                 : step.sqrtPriceNextX96,
    //             state.liquidity,
    //             state.amountSpecifiedRemaining,
    //             fee
    //         );

    //         if (exactInput) {
    //             // safe because we test that amountSpecified > amountIn + feeAmount in SwapMath
    //             unchecked {
    //                 state.amountSpecifiedRemaining -= (step.amountIn + step.feeAmount).toInt256();
    //             }
    //             state.amountCalculated -= step.amountOut.toInt256();
    //         } else {
    //             unchecked {
    //                 state.amountSpecifiedRemaining += step.amountOut.toInt256();
    //             }
    //             state.amountCalculated += (step.amountIn + step.feeAmount).toInt256();
    //         }

    //         // if the protocol fee is on, calculate how much is owed, decrement feeAmount, and increment protocolFee
    //         if (cache.feeProtocol > 0) {
    //             unchecked {
    //                 uint256 delta = step.feeAmount / cache.feeProtocol;
    //                 step.feeAmount -= delta;
    //                 state.protocolFee += uint128(delta);
    //             }
    //         }

    //         // update global fee tracker
    //         if (state.liquidity > 0) {
    //             unchecked {
    //                 state.feeGrowthGlobalX128 += FullMath.mulDiv(step.feeAmount, FixedPoint128.Q128, state.liquidity);
    //             }
    //         }

    //         // shift tick if we reached the next price
    //         if (state.sqrtPriceX96 == step.sqrtPriceNextX96) {
    //             // if the tick is initialized, run the tick transition
    //             if (step.initialized) {
    //                 // check for the placeholder value, which we replace with the actual value the first time the swap
    //                 // crosses an initialized tick
    //                 if (!cache.computedLatestObservation) {
    //                     (cache.tickCumulative, cache.secondsPerLiquidityCumulativeX128) = observations.observeSingle(
    //                         cache.blockTimestamp,
    //                         0,
    //                         slot0Start.tick,
    //                         slot0Start.observationIndex,
    //                         cache.liquidityStart,
    //                         slot0Start.observationCardinality
    //                     );
    //                     cache.computedLatestObservation = true;
    //                 }
    //                 int128 liquidityNet = ticks.cross(
    //                     step.tickNext,
    //                     (zeroForOne ? state.feeGrowthGlobalX128 : feeGrowthGlobal0X128),
    //                     (zeroForOne ? feeGrowthGlobal1X128 : state.feeGrowthGlobalX128),
    //                     cache.secondsPerLiquidityCumulativeX128,
    //                     cache.tickCumulative,
    //                     cache.blockTimestamp
    //                 );
    //                 // if we're moving leftward, we interpret liquidityNet as the opposite sign
    //                 // safe because liquidityNet cannot be type(int128).min
    //                 unchecked {
    //                     if (zeroForOne) liquidityNet = -liquidityNet;
    //                 }

    //                 state.liquidity = liquidityNet < 0
    //                     ? state.liquidity - uint128(-liquidityNet)
    //                     : state.liquidity + uint128(liquidityNet);
    //             }

    //             unchecked {
    //                 state.tick = zeroForOne ? step.tickNext - 1 : step.tickNext;
    //             }
    //         } else if (state.sqrtPriceX96 != step.sqrtPriceStartX96) {
    //             // recompute unless we're on a lower tick boundary (i.e. already transitioned ticks), and haven't moved
    //             state.tick = TickMath.getTickAtSqrtRatio(state.sqrtPriceX96);
    //         }
    //     }

    //     // update tick and write an oracle entry if the tick change
    //     if (state.tick != slot0Start.tick) {
    //         (uint16 observationIndex, uint16 observationCardinality) = observations.write(
    //             slot0Start.observationIndex,
    //             cache.blockTimestamp,
    //             slot0Start.tick,
    //             cache.liquidityStart,
    //             slot0Start.observationCardinality,
    //             slot0Start.observationCardinalityNext
    //         );
    //         (slot0.sqrtPriceX96, slot0.tick, slot0.observationIndex, slot0.observationCardinality) = (
    //             state.sqrtPriceX96,
    //             state.tick,
    //             observationIndex,
    //             observationCardinality
    //         );
    //     } else {
    //         // otherwise just update the price
    //         slot0.sqrtPriceX96 = state.sqrtPriceX96;
    //     }

    //     // update liquidity if it changed
    //     if (cache.liquidityStart != state.liquidity) liquidity = state.liquidity;

    //     // update fee growth global and, if necessary, protocol fees
    //     // overflow is acceptable, protocol has to withdraw before it hits type(uint128).max fees
    //     if (zeroForOne) {
    //         feeGrowthGlobal0X128 = state.feeGrowthGlobalX128;
    //         unchecked {
    //             if (state.protocolFee > 0) protocolFees.token0 += state.protocolFee;
    //         }
    //     } else {
    //         feeGrowthGlobal1X128 = state.feeGrowthGlobalX128;
    //         unchecked {
    //             if (state.protocolFee > 0) protocolFees.token1 += state.protocolFee;
    //         }
    //     }

    //     unchecked {
    //         (amount0, amount1) = zeroForOne == exactInput
    //             ? (amountSpecified - state.amountSpecifiedRemaining, state.amountCalculated)
    //             : (state.amountCalculated, amountSpecified - state.amountSpecifiedRemaining);
    //     }

    //     // do the transfers and collect payment
    //     if (zeroForOne) {
    //         unchecked {
    //             if (amount1 < 0) TransferHelper.safeTransfer(token1, recipient, uint256(-amount1));
    //         }

    //         uint256 balance0Before = balance0();
    //         IUniswapV3SwapCallback(msg.sender).uniswapV3SwapCallback(amount0, amount1, data);
    //         if (balance0Before + uint256(amount0) > balance0()) revert IIA();
    //     } else {
    //         unchecked {
    //             if (amount0 < 0) TransferHelper.safeTransfer(token0, recipient, uint256(-amount0));
    //         }

    //         uint256 balance1Before = balance1();
    //         IUniswapV3SwapCallback(msg.sender).uniswapV3SwapCallback(amount0, amount1, data);
    //         if (balance1Before + uint256(amount1) > balance1()) revert IIA();
    //     }

    //     emit Swap(msg.sender, recipient, amount0, amount1, state.sqrtPriceX96, state.liquidity, state.tick);
    //     slot0.unlocked = true;
    // }