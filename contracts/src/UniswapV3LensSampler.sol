pragma solidity ^0.6;
pragma experimental ABIEncoderV2;

import "./interfaces/IUniswapV3.sol";

interface ITickLens {
    struct PopulatedTick {
        int24 tick;
        int128 liquidityNet;
        uint128 liquidityGross;
    }

    function getPopulatedTicksInWord(
        address pool,
        int16 tickBitmapIndex
    ) external view returns (PopulatedTick[] memory populatedTicks);
}

contract UniswapV3LensSampler {
    struct PoolState {
        uint160 sqrtPriceX96;
        int24 tick;
        int24 tickSpacing;
        uint24 fee;
        uint8 feeProtocol;
        uint128 liquidity;
    }

    function sampleUniswapV3LensData(
        address pool,
        address lens,
        uint256 numWords
    )
        public
        view
        returns (PoolState memory poolState, uint256[] memory bitmaps, ITickLens.PopulatedTick[][] memory ticks)
    {
        IUniswapV3Pool p = IUniswapV3Pool(pool);
        poolState = fetchPoolState(p);

        int256 currentWord = (poolState.tick / poolState.tickSpacing) / 256; // this actually fits into i16

        bitmaps = new uint256[](2 * numWords + 1);
        ticks = new ITickLens.PopulatedTick[][](2 * numWords + 1);

        ITickLens lens = ITickLens(lens);

        int256 words = int256(numWords); // for compatibility
        for (int256 i = currentWord - words; i < currentWord + words + 1; i++) {
            bitmaps[uint256(i - currentWord + words)] = p.tickBitmap(int16(i));
            ticks[uint256(i - currentWord + words)] = lens.getPopulatedTicksInWord(pool, int16(i));
        }
    }

    function fetchPoolState(IUniswapV3Pool pool) internal view returns (PoolState memory poolState) {
        (uint160 sqrtPriceX96, int24 tick, , , , uint8 feeProtocol, ) = pool.slot0();
        uint128 liquidity = pool.liquidity();
        uint24 fee = pool.fee();
        int24 tickSpacing = pool.tickSpacing();
        poolState = PoolState(sqrtPriceX96, tick, tickSpacing, fee, feeProtocol, liquidity);
    }
}
