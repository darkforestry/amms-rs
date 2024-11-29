// //SPDX-License-Identifier: MIT
// pragma solidity ^0.8.0;

// import {IBPool} from "./interfaces/IBalancer.sol";
// import {IUniswapV2Pair} from "./interfaces/IUniswapV2.sol";
// import {IUniswapV2Factory} from "./interfaces/IUniswapV2.sol";
// import {IUniswapV3Pool} from "./interfaces/IUniswapV3.sol";
// import {IUniswapV3Factory} from "./interfaces/IUniswapV3.sol";
// import {IERC20} from "./interfaces/Token.sol";
// import {FixedPointMath} from "./FixedPoint.sol";

// contract WethValueInPools {
//     /// @notice Address of Uniswap V2 factory
//     /// @dev Used as the first priority for quoting WETH value
//     address UNISWAP_V2_FACTORY;
// // 
//     /// @notice Address of Uniswap V3 factory
//     /// @dev Used as the second priority for quoting WETH value
//     address UNISWAP_V3_FACTORY;

//     /// @notice Address of WETH
//     address WETH;

//     /// @notice The minimum WETH liquidity to consider a `quote` valid.
//     uint256 private constant MIN_WETH_LIQUIDITY = 1 ether;

//     address private constant ADDRESS_ZERO = address(0);

//     uint8 private constant WETH_DECIMALS = 18;

//     constructor(
//         address _uniswapV2Factory,
//         address _uniswapV3Factory,
//         address _wethcd
//     ) {
//         UNISWAP_V2_FACTORY = _uniswapV2Factory;
//         UNISWAP_V3_FACTORY = _uniswapV3Factory;
//         WETH = _weth;
//     }

//     /// @notice Enum for pool types
//     enum PoolType {
//         Balancer,
//         UniswapV2,
//         UniswapV3
//     }

//     /// @notice Struct for pool info
//     struct PoolInfo {
//         PoolType poolType;
//         address poolAddress;
//     }

//     /// @notice Struct for pool info return
//     struct PoolInfoReturn {
//         PoolType poolType;
//         address poolAddress;
//         uint256 wethValue;
//     }

//     /// @notice Returns an array of `PoolInfoReturn` for the consumer to determine wether to filter or not to save gas.
//     /// @dev We require a 1 ETH minimum liquidity in the quoting pool for it to be considered.
//     function getWethValueInPools(
//         PoolInfo[] memory pools
//     ) public returns (PoolInfoReturn[] memory) {
//         PoolInfoReturn[] memory poolInfoReturns = new PoolInfoReturn[](
//             pools.length
//         );
//         for (uint256 i = 0; i < pools.length; i++) {
//             PoolInfo memory info = pools[i];
//             if (info.poolType == PoolType.Balancer) {
//                 uint256 wethValue = handleBalancerPool(info.poolAddress);
//                 poolInfoReturns[i] = PoolInfoReturn(
//                     info.poolType,
//                     info.poolAddress,
//                     wethValue
//                 );
//             } else if (info.poolType == PoolType.UniswapV2) {
//                 uint256 wethValue = handleUniswapV2Pool(info.poolAddress);
//                 poolInfoReturns[i] = PoolInfoReturn(
//                     info.poolType,
//                     info.poolAddress,
//                     wethValue
//                 );
//             } else if (info.poolType == PoolType.UniswapV3) {
//                 uint256 wethValue = handleUniswapV3Pool(info.poolAddress);
//                 poolInfoReturns[i] = PoolInfoReturn(
//                     info.poolType,
//                     info.poolAddress,
//                     wethValue
//                 );
//             }
//         }
//         return poolInfoReturns;
//     }

//     function handleBalancerPool(address pool) internal returns (uint256) {
//         // Get pool tokens
//         address[] memory tokens;
//         try IBPool(pool).getCurrentTokens() returns (address[] memory _tokens) {
//             tokens = _tokens;
//         } catch {
//             return 0;
//         }

//         // First check if we have WETH in the pool. If so, return Weth Value * # of tokens in the pool.
//         for (uint256 i = 0; i < tokens.length; i++) {
//             if (tokens[i] == WETH) {
//                 try IBPool(pool).getBalance(tokens[i]) returns (
//                     uint256 _balance
//                 ) {
//                     // Obviously assuming an even distribution of value. Which is a "good enough" approximation.
//                     // For a value filter.
//                     return _balance * tokens.length;
//                 } catch {
//                     return 0;
//                 }
//             }
//         }

//         address baseToken = tokens[0];
//         uint256 balance;
//         try IBPool(pool).getBalance(baseToken) returns (uint256 _balance) {
//             balance = _balance;
//         } catch {
//             return 0;
//         }

//         uint256 wethValue = quoteTokenToWethValue(baseToken, balance);
//         return wethValue * tokens.length;
//     }

//     function handleUniswapV2Pool(address pool) internal returns (uint256) {
//         address token0;
//         try IUniswapV2Pair(pool).token0() returns (address _token0) {
//             token0 = _token0;
//         } catch {
//             return 0;
//         }
//         address token1;
//         try IUniswapV2Pair(pool).token1() returns (address _token1) {
//             token1 = _token1;
//         } catch {
//             return 0;
//         }
//         try IUniswapV2Pair(pool).getReserves() returns (
//             uint112 reserve0,
//             uint112 reserve1,
//             uint32
//         ) {
//             if (token0 == WETH) {
//                 return reserve0 * 2;
//             } else if (token1 == WETH) {
//                 return reserve1 * 2;
//             }
//             // No WETH in the pool Quote token0.
//             uint256 wethValue = quoteTokenToWethValue(token0, reserve0);
//             return wethValue * 2;
//         } catch {
//             return 0;
//         }
//     }

//     function handleUniswapV3Pool(address pool) internal returns (uint256) {
//         address token0;
//         try IUniswapV2Pair(address(pool)).token0() returns (address _token0) {
//             token0 = _token0;
//         } catch {
//             return 0;
//         }
//         address token1;
//         try IUniswapV2Pair(address(pool)).token1() returns (address _token1) {
//             token1 = _token1;
//         } catch {
//             return 0;
//         }

//         if (token0 == WETH) {
//             try IERC20(token0).balanceOf(address(pool)) returns (
//                 uint256 balance
//             ) {
//                 return balance * 2;
//             } catch {
//                 return 0;
//             }
//         } else if (token1 == WETH) {
//             try IERC20(token1).balanceOf(address(pool)) returns (
//                 uint256 balance
//             ) {
//                 return balance * 2;
//             } catch {
//                 return 0;
//             }
//         }

//         // No WETH in the pool Quote token0.
//         try IERC20(token0).balanceOf(address(pool)) returns (uint256 balance) {
//             uint256 wethValue = quoteTokenToWethValue(token0, balance);
//             return wethValue * 2;
//         } catch {
//             return 0;
//         }
//     }

//     /// @dev Returns the value of `amount` of `token` in terms of WETH.
//     function quoteTokenToWethValue(
//         address token,
//         uint256 amount
//     ) internal returns (uint256) {
//         // Try Uniswap V2.
//         uint128 price = quoteToken(token);
//         if (price > 0) {
//             return FixedPointMath.mul64u(price, amount);
//         } else {
//             return price;
//         }
//     }

//     /// @dev Quotes a Q64 quote of `token` in terms of WETH.
//     function quoteToken(address token) internal returns (uint128) {
//         // Get the token decimals
//         uint128 price;
//         // Try Uniswap V2.
//         price = quoteTokenUniswapV2(token);
//         if (price > 0) {
//             return price;
//         }
//         // Try Uniswap V3.
//         price = quoteTokenUniswapV3(token);
//         return price;
//     }

//     function quoteTokenUniswapV2(
//         address token
//     ) internal returns (uint128 price) {
//         // Get the pair
//         IUniswapV2Pair pair = IUniswapV2Pair(
//             IUniswapV2Factory(UNISWAP_V2_FACTORY).getPair(token, WETH)
//         );
//         if (address(pair) == ADDRESS_ZERO) {
//             return 0;
//         }

//         // Get the reserves
//         // (uint112 reserve0, uint112 reserve1, ) = pair.getReserves();
//         uint112 reserve0;
//         uint112 reserve1;
//         try pair.getReserves() returns (
//             uint112 _reserve0,
//             uint112 _reserve1,
//             uint32
//         ) {
//             reserve0 = _reserve0;
//             reserve1 = _reserve1;
//         } catch {
//             return 0;
//         }
//         if (reserve0 == 0 || reserve1 == 0) {
//             return 0;
//         }

//         // Get the decimals of token.
//         (
//             uint8 tokenDecimals,
//             bool tokenDecimalsSuccess
//         ) = getTokenDecimalsUnsafe(token);
//         if (!tokenDecimalsSuccess) {
//             return 0;
//         }

//         // Normalize r0/r1 to 18 decimals.
//         uint112 reserveWeth = token < WETH ? reserve1 : reserve0;
//         uint112 reserveToken = token < WETH ? reserve0 : reserve1;

//         reserveToken = tokenDecimals <= WETH_DECIMALS
//             ? uint112(reserveToken * 10 ** (WETH_DECIMALS - tokenDecimals))
//             : uint112(reserveToken / 10 ** (tokenDecimals - WETH_DECIMALS));
//         price = FixedPointMath.divuu(reserveWeth, reserveToken);
//     }

//     function quoteTokenUniswapV3(address token) internal returns (uint128) {
//         uint16[3] memory feeTiers = [500, 3000, 10000];
//         IUniswapV3Pool pool;
//         for (uint256 i = 0; i < feeTiers.length; ++i) {
//             // Get the pool
//             IUniswapV3Pool pair = IUniswapV3Pool(
//                 IUniswapV3Factory(UNISWAP_V3_FACTORY).getPool(
//                     token,
//                     WETH,
//                     feeTiers[i]
//                 )
//             );
//             if (address(pool) != ADDRESS_ZERO) {
//                 pool = pair;
//                 break;
//             }
//         }

//         if (address(pool) == ADDRESS_ZERO) {
//             return 0;
//         }

//         // Get slot 0 sqrtPriceX96
//         uint160 sqrtPriceX96;
//         try pool.slot0() returns (
//             uint160 _sqrtPriceX96,
//             int24,
//             uint16,
//             uint16,
//             uint16,
//             uint8,
//             bool
//         ) {
//             sqrtPriceX96 = _sqrtPriceX96;
//         } catch {
//             return 0;
//         }

//         bool token0IsReserve0 = token < WETH;
//         (
//             uint8 tokenDecimals,
//             bool token0DecimalsSuccess
//         ) = getTokenDecimalsUnsafe(token);
//         if (!token0DecimalsSuccess) {
//             return 0;
//         }
//         // Q128 -> Q64
//         return
//             uint128(
//                 FixedPointMath.fromSqrtX96(
//                     sqrtPriceX96,
//                     token0IsReserve0,
//                     token0IsReserve0
//                         ? int8(tokenDecimals)
//                         : int8(WETH_DECIMALS),
//                     token0IsReserve0 ? int8(WETH_DECIMALS) : int8(tokenDecimals)
//                 ) >> 64
//             );
//     }

//     /// @notice returns true as the second return value if the token decimals can be successfully retrieved
//     function getTokenDecimalsUnsafe(
//         address token
//     ) internal returns (uint8, bool) {
//         (bool tokenDecimalsSuccess, bytes memory tokenDecimalsData) = token
//             .call{gas: 20000}(abi.encodeWithSignature("decimals()"));

//         if (tokenDecimalsSuccess) {
//             uint256 tokenDecimals;

//             if (tokenDecimalsData.length == 32) {
//                 (tokenDecimals) = abi.decode(tokenDecimalsData, (uint256));

//                 if (tokenDecimals == 0 || tokenDecimals > 255) {
//                     return (0, false);
//                 } else {
//                     return (uint8(tokenDecimals), true);
//                 }
//             } else {
//                 return (0, false);
//             }
//         } else {
//             return (0, false);
//         }
//     }
// }

// contract WethValueInPoolsBatchRequest is WethValueInPools {
//     constructor(
//         address _uniswapV2Factory,
//         address _uniswapV3Factory,
//         address _weth,
//         WethValueInPools.PoolInfo[] memory pools
//     ) WethValueInPools(_uniswapV2Factory, _uniswapV3Factory, _weth) {
//         WethValueInPools.PoolInfoReturn[]
//             memory poolInfoReturn = getWethValueInPools(pools);
//         // insure abi encoding, not needed here but increase reusability for different return types
//         // note: abi.encode add a first 32 bytes word with the address of the original data
//         bytes memory abiEncodedData = abi.encode(poolInfoReturn);
//         assembly {
//             // Return from the start of the data (discarding the original data address)
//             // up to the end of the memory used
//             let dataStart := add(abiEncodedData, 0x20)
//             return(dataStart, sub(msize(), dataStart))
//         }
//     }
// }
