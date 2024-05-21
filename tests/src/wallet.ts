import { MnemonicWallet } from "cosmes/wallet";

export const wallet = new MnemonicWallet({
  bech32Prefix: "terra",
  chainId: "localterra",
  gasPrice: {
    amount: "0.015",
    denom: "uluna",
  },
  mnemonic:
    "notice oak worry limit wrap speak medal online prefer cluster roof addict wrist behave treat actual wasp year salad speed social layer crew genius",
  rpc: "http://localhost:26657",
  coinType: 330,
});
