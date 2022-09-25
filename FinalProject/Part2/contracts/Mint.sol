// SPDX-License-Identifier: MIT
pragma solidity >=0.8.0 <0.9.0;

import "@openzeppelin/contracts/access/Ownable.sol";
import "./interfaces/IPriceFeed.sol";
import "./interfaces/IMint.sol";
import "./sAsset.sol";
import "./EUSD.sol";

contract Mint is Ownable, IMint {

    struct Asset {
        address token;
        uint minCollateralRatio;
        address priceFeed;
    }

    struct Position {
        uint idx;
        address owner;
        uint collateralAmount;
        address assetToken;
        uint assetAmount;
    }

    mapping(address => Asset) _assetMap;
    uint _currentPositionIndex;
    mapping(uint => Position) _idxPositionMap;
    address public collateralToken;


    constructor(address collateral) {
        collateralToken = collateral;
    }

    function registerAsset(address assetToken, uint minCollateralRatio, address priceFeed) external override onlyOwner {
        require(assetToken != address(0), "Invalid assetToken address");
        require(minCollateralRatio >= 1, "minCollateralRatio must be greater than 100%");
        require(_assetMap[assetToken].token == address(0), "Asset was already registered");

        _assetMap[assetToken] = Asset(assetToken, minCollateralRatio, priceFeed);
    }

    function getPosition(uint positionIndex) external view returns (address, uint, address, uint) {
        require(positionIndex < _currentPositionIndex, "Invalid index");
        Position storage position = _idxPositionMap[positionIndex];
        return (position.owner, position.collateralAmount, position.assetToken, position.assetAmount);
    }

    function getMintAmount(uint collateralAmount, address assetToken, uint collateralRatio) public view returns (uint) {
        Asset storage asset = _assetMap[assetToken];
        (int relativeAssetPrice,) = IPriceFeed(asset.priceFeed).getLatestPrice();
        uint8 decimal = sAsset(assetToken).decimals();
        uint mintAmount = collateralAmount * (10 ** uint256(decimal)) / uint(relativeAssetPrice) / collateralRatio;
        return mintAmount;
    }

    function checkRegistered(address assetToken) public view returns (bool) {
        return _assetMap[assetToken].token == assetToken;
    }

    /* TODO: implement your functions here */
    function openPosition(uint collateralAmount, address assetToken, uint collateralRatio) external override {
        // Make sure the asset is registered
        require(checkRegistered(assetToken), "Asset is not registered");
        // the input collateral ratio is not less than the asset MCR
        Asset storage asset = _assetMap[assetToken];
        require(collateralRatio >= asset.minCollateralRatio, "input collateral ratio is less than the asset MCR");
        // calculate the number of minted tokens
        uint mintAmount = getMintAmount(collateralAmount, assetToken, collateralRatio);
        // transferring tokens
        EUSD(collateralToken).transferFrom(msg.sender, address(this), collateralAmount);
        sAsset(asset.token).mint(msg.sender, mintAmount);
        // Create a new position
        _idxPositionMap[_currentPositionIndex] = Position(_currentPositionIndex, msg.sender, collateralAmount, assetToken, mintAmount);
        _currentPositionIndex++;
    }

    function closePosition(uint positionIndex) external override {
        require(positionIndex < _currentPositionIndex, "Invalid index");
        Position storage position = _idxPositionMap[positionIndex];
        require(position.owner == msg.sender, "message sender does not own the position");
        // burn these tokens
        sAsset(position.assetToken).burn(msg.sender, position.assetAmount);
        // Transfer EUSD tokens locked in the position to the message sender
        EUSD(collateralToken).transfer(msg.sender, position.collateralAmount);
        // delete the position at the given index
        delete _idxPositionMap[positionIndex];
    }

    function deposit(uint positionIndex, uint collateralAmount) external override {
        require(positionIndex < _currentPositionIndex, "Invalid index");
        Position storage position = _idxPositionMap[positionIndex];
        // Make sure the message sender owns the position
        require(position.owner == msg.sender, "message sender does not own the position");
        // transfer deposited tokens from the sender to the contract
        EUSD(collateralToken).transferFrom(msg.sender, address(this), collateralAmount);
        // Add collateral amount
        position.collateralAmount += collateralAmount;
    }

    function withdraw(uint positionIndex, uint withdrawAmount) external override {
        require(positionIndex < _currentPositionIndex, "Invalid index");
        Position storage position = _idxPositionMap[positionIndex];
        // Make sure the message sender owns the position
        require(position.owner == msg.sender, "message sender does not own the position");
        // the collateral ratio won't go below the MCR
        uint collateralAmount = position.collateralAmount - withdrawAmount;
        Asset storage asset = _assetMap[position.assetToken];
        require(collateralAmount / position.assetAmount >= asset.minCollateralRatio, "collateral ratio goes below the MCR");
        // Transfer withdrawn tokens from the contract to the sender
        EUSD(collateralToken).transfer(msg.sender, collateralAmount);
        position.collateralAmount = collateralAmount;
    }

    function mint(uint positionIndex, uint mintAmount) external override {
        require(positionIndex < _currentPositionIndex, "Invalid index");
        Position storage position = _idxPositionMap[positionIndex];
        // Make sure the message sender owns the position
        require(position.owner == msg.sender, "message sender does not own the position");
        // the collateral ratio won't go below the MCR
        Asset storage asset = _assetMap[position.assetToken];
        uint assetAmount = position.assetAmount + mintAmount;
        require(position.collateralAmount / assetAmount >= asset.minCollateralRatio, "collateral ratio goes below the MCR");
        // mint asset tokens
        sAsset(asset.token).mint(msg.sender, mintAmount);
        position.assetAmount = assetAmount;
    }

    function burn(uint positionIndex, uint burnAmount) external override {
        require(positionIndex < _currentPositionIndex, "Invalid index");
        Position storage position = _idxPositionMap[positionIndex];
        // Make sure the message sender owns the position
        require(position.owner == msg.sender, "message sender does not own the position");
        // burns asset tokens in the position
        Asset storage asset = _assetMap[position.assetToken];
        uint assetAmount = position.assetAmount - burnAmount;
        sAsset(asset.token).burn(msg.sender, burnAmount);
        position.assetAmount = assetAmount;
    }


}