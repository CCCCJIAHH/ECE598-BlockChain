// SPDX-License-Identifier: MIT
pragma solidity >=0.8.0 <0.9.0;
import "@openzeppelin/contracts/access/Ownable.sol";
import "./interfaces/ISwap.sol";
import "./sAsset.sol";

contract Swap is Ownable, ISwap {

    address token0;
    address token1;
    uint reserve0;
    uint reserve1;
    mapping (address => uint) shares;
    uint public totalShares;

    constructor(address addr0, address addr1) {
        token0 = addr0;
        token1 = addr1;
    }

    function init(uint token0Amount, uint token1Amount) external override onlyOwner {
        require(reserve0 == 0 && reserve1 == 0, "init - already has liquidity");
        require(token0Amount > 0 && token1Amount > 0, "init - both tokens are needed");
        
        require(sAsset(token0).transferFrom(msg.sender, address(this), token0Amount));
        require(sAsset(token1).transferFrom(msg.sender, address(this), token1Amount));
        reserve0 = token0Amount;
        reserve1 = token1Amount;
        totalShares = sqrt(token0Amount * token1Amount);
        shares[msg.sender] = totalShares;
    }

    // https://github.com/Uniswap/v2-core/blob/v1.0.1/contracts/libraries/Math.sol
    function sqrt(uint y) internal pure returns (uint z) {
        if (y > 3) {
            z = y;
            uint x = y / 2 + 1;
            while (x < z) {
                z = x;
                x = (y / x + x) / 2;
            }
        } else if (y != 0) {
            z = 1;
        }
    }

    function getReserves() external view returns (uint, uint) {
        return (reserve0, reserve1);
    }

    function getTokens() external view returns (address, address) {
        return (token0, token1);
    }

    function getShares(address LP) external view returns (uint) {
        return shares[LP];
    }

    /* TODO: implement your functions here */

    function addLiquidity(uint token0Amount) external override{
        uint amount1=reserve1 * token0Amount / reserve0;
        require(token0Amount == amount1, "requires an equivalent value of two tokens");
        sAsset(token0).transferFrom(msg.sender,address(this),token0Amount);
        sAsset(token1).transferFrom(msg.sender,address (this),amount1);
        uint new_shares = totalShares * token0Amount / reserve0;
        totalShares+=new_shares;
        shares[msg.sender]+=new_shares;
        reserve0+=token0Amount;
        reserve1+=amount1;
    }
    function token0To1(uint token0Amount) external override{
        sAsset(token0).transferFrom(msg.sender,address (this),token0Amount);
        uint token0_to_exchange=token0Amount*997/1000;
        uint invariant=reserve1*reserve0;
        uint token1_to_return=reserve1-invariant/(reserve0+token0_to_exchange);
        sAsset(token1).transfer(msg.sender,token1_to_return);
        reserve0+=token0Amount;
        reserve1-=token1_to_return;
    }
    function token1To0(uint token1Amount) external override{
        sAsset(token1).transferFrom(msg.sender,address (this),token1Amount);
        uint token1_to_exchange=token1Amount*997/1000;
        uint invariant=reserve1*reserve0;
        uint token0_to_return=reserve0-invariant/(reserve1+token1_to_exchange);
        sAsset(token0).transfer(msg.sender,token0_to_return);
        reserve1+=token1Amount;
        reserve0-=token0_to_return;
    }
    function removeLiquidity(uint withdrawShares) external override{
        require(shares[msg.sender]>=withdrawShares,"do not have so much shares");
        uint amount0 = reserve0 * withdrawShares / totalShares;
        uint amount1 = reserve1 * withdrawShares / totalShares;
        totalShares-=withdrawShares;
        reserve0-=amount0;
        reserve1-=amount1;
        shares[msg.sender]-=withdrawShares;
        sAsset(token1).transfer(msg.sender,amount1);
        sAsset(token0).transfer(msg.sender,amount0);

    }



    
}