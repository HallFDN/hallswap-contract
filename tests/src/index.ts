import * as fs from "fs";
import * as path from "path";
import { CosmosBaseV1beta1Coin as Coin } from "cosmes/protobufs";
import { base64, utf8 } from "cosmes/codec";
import {
  MsgExecuteContract,
  getCw20Balance,
  getNativeBalances,
  queryContract,
} from "cosmes/client";

import { wallet } from "./wallet";
import { MsgStoreCode } from "./models/MsgStoreCode";
import { MsgInstantiateContract } from "./models/MsgInstantiateContract";
import { Asset } from "./models/Asset";
import { AssetInfo } from "./models/AssetInfo";
import { assertEquals, assertThrows } from "./assert";

const ARCH = process.env.ARCH;

const EXTERNAL_WASM_DIR = path.join(import.meta.dir, "..", "bin");
const HALLSWAP_WASM_DIR = path.join(import.meta.dir, "..", "..", "artifacts");
const WASM_FILES = {
  TERRASWAP_FACTORY: path.join(EXTERNAL_WASM_DIR, "terraswap_factory.wasm"),
  TERRASWAP_PAIR: path.join(EXTERNAL_WASM_DIR, "terraswap_pair.wasm"),
  TERRASWAP_TOKEN: path.join(EXTERNAL_WASM_DIR, "terraswap_token.wasm"),
  HALLSWAP: path.join(HALLSWAP_WASM_DIR, `hallswap${ARCH}.wasm`),
};
const FEE_COLLECTOR = "terra1g4ms8hh54tglt4pruns3jyfv4vxq00r7gw3lhc";

async function store(filepath: string): Promise<bigint> {
  console.log(`Uploading wasm binary [${path.basename(filepath)}]...`);
  const { txResponse } = await wallet.broadcastTxSync({
    msgs: [
      new MsgStoreCode({
        sender: wallet.address,
        wasmByteCode: fs.readFileSync(filepath),
      }),
    ],
  });
  const event = txResponse.events.find((event) => event.type === "store_code");
  if (!event) {
    throw new Error(`failed to parse MsgStoreCode for ${filepath}`);
  }
  const attr = event.attributes.find((attr) => attr.key === "code_id");
  if (!attr) {
    throw new Error(`failed to parse MsgStoreCode for ${filepath}`);
  }
  return BigInt(attr.value);
}

async function instantiate(
  codeID: bigint,
  label: string,
  msg: unknown,
  funds: Coin[] = []
): Promise<string> {
  console.log(`Instantiating contract [${label}]...`);
  const { txResponse } = await wallet.broadcastTxSync({
    msgs: [
      new MsgInstantiateContract({
        sender: wallet.address,
        admin: wallet.address,
        codeId: codeID,
        label: label,
        msg: utf8.decode(JSON.stringify(msg)),
        funds: funds,
      }),
    ],
  });
  const event = txResponse.events.find((event) => event.type === "instantiate");
  if (!event) {
    throw new Error(`failed to parse MsgInstantiateContract for ${codeID}`);
  }
  const attr = event.attributes.find(
    (attr) => attr.key === "_contract_address"
  );
  if (!attr) {
    throw new Error(`failed to parse MsgInstantiateContract for ${codeID}`);
  }
  return attr.value;
}

async function initToken(tokenCodeID: bigint, symbol: string): Promise<string> {
  return instantiate(tokenCodeID, "terraswap_token", {
    name: symbol,
    symbol: symbol,
    decimals: 6,
    initial_balances: [
      {
        address: wallet.address,
        amount: 1_000_000_000_000_000n.toString(),
      },
    ],
  });
}

async function initTerraswap(tokenCodeID: bigint): Promise<string> {
  const factoryCodeID = await store(WASM_FILES.TERRASWAP_FACTORY);
  const pairCodeID = await store(WASM_FILES.TERRASWAP_PAIR);
  const factoryAddress = await instantiate(factoryCodeID, "terraswap_factory", {
    pair_code_id: Number(pairCodeID),
    token_code_id: Number(tokenCodeID),
  });
  await wallet.broadcastTxSync({
    msgs: [
      new MsgExecuteContract({
        sender: wallet.address,
        contract: factoryAddress,
        msg: {
          add_native_token_decimals: {
            denom: "uluna",
            decimals: 6,
          },
        },
        funds: [{ denom: "uluna", amount: "1" }],
      }),
    ],
  });
  return factoryAddress;
}

async function initPair(
  factoryAddress: string,
  tokenAddress1: string,
  tokenAddress2 = "uluna"
): Promise<string> {
  console.log(`Creating liquidity pair with [${tokenAddress1}]...`);
  const amount = 1_000_000_000n.toString();
  const { txResponse } = await wallet.broadcastTxSync({
    msgs: [
      new MsgExecuteContract({
        sender: wallet.address,
        contract: tokenAddress1,
        msg: {
          increase_allowance: {
            spender: factoryAddress,
            amount,
          },
        },
        funds: [],
      }),
      ...(tokenAddress2 === "uluna"
        ? []
        : [
            new MsgExecuteContract({
              sender: wallet.address,
              contract: tokenAddress2,
              msg: {
                increase_allowance: {
                  spender: factoryAddress,
                  amount,
                },
              },
              funds: [],
            }),
          ]),
      new MsgExecuteContract({
        sender: wallet.address,
        contract: factoryAddress,
        msg: {
          create_pair: {
            assets: [
              new Asset(tokenAddress1, amount).toJSON(),
              new Asset(tokenAddress2, amount).toJSON(),
            ],
          },
        },
        funds: tokenAddress2 === "uluna" ? [{ denom: "uluna", amount }] : [],
      }),
    ],
  });
  for (const { type, attributes } of txResponse.events) {
    if (type !== "wasm" || attributes.length !== 3) {
      continue;
    }
    const attr = attributes.find((attr) => attr.key === "pair_contract_addr");
    if (!attr) {
      continue;
    }
    return attr.value;
  }
  throw new Error(`failed to parse address of liquidity pair created`);
}

async function initHallswap() {
  const id = await store(WASM_FILES.HALLSWAP);
  return instantiate(id, "hallswap", {
    owner: wallet.address,
    fee_address: FEE_COLLECTOR,
    fee_bps: 100,
    fee_assets: ["uluna"],
  });
}

async function queryHallswapSimulation(
  hallswapAddress: string,
  routes: {
    route: {
      contract_addr: string;
      offer_asset:
        | { token: { contract_addr: string } }
        | { native_token: { denom: string } };
      return_asset:
        | { token: { contract_addr: string } }
        | { native_token: { denom: string } };
    }[];
    offer_amount: string;
  }[]
) {
  console.log("Querying hallswap simulation...");
  const { return_asset, fee_asset } = await queryContract<any>(wallet.rpc, {
    address: hallswapAddress,
    query: {
      simulation: {
        routes: routes,
      },
    },
  });
  return [Asset.fromJSON(return_asset), Asset.fromJSON(fee_asset)];
}

async function queryPairSimulation(
  pairAddress: string,
  offerAsset: string,
  offerAmount: bigint
) {
  console.log("Querying pair simulation...");
  const { return_amount } = await queryContract<any>(wallet.rpc, {
    address: pairAddress,
    query: {
      simulation: {
        offer_asset: new Asset(offerAsset, offerAmount).toJSON(),
      },
    },
  });
  return BigInt(return_amount);
}

async function executeHallswap(
  hallswapAddress: string,
  offerAsset: string,
  routes: { route: { contract_addr: string }[]; offer_amount: string }[],
  minimum_receive: bigint,
  to?: string | undefined
) {
  console.log("Executing hallswap...");
  const offerAssetInfo = new AssetInfo(offerAsset);
  const offerAmount = routes
    .reduce((accum, r) => accum + BigInt(r.offer_amount), 0n)
    .toString();
  const { txResponse } = await wallet.broadcastTxSync({
    msgs: [
      offerAssetInfo.isCW20()
        ? new MsgExecuteContract({
            sender: wallet.address,
            contract: offerAssetInfo.id,
            msg: {
              send: {
                contract: hallswapAddress,
                amount: offerAmount.toString(),
                msg: base64.encode(
                  utf8.decode(
                    JSON.stringify({
                      execute_routes: {
                        offer_asset_info: new AssetInfo(offerAsset).toJSON(),
                        routes,
                        minimum_receive: minimum_receive.toString(),
                        ...(to != null ? { to } : {}),
                      },
                    })
                  )
                ),
              },
            },
            funds: [],
          })
        : new MsgExecuteContract({
            sender: wallet.address,
            contract: hallswapAddress,
            msg: {
              execute_routes: {
                offer_asset_info: offerAssetInfo.toJSON(),
                routes,
                minimum_receive: minimum_receive.toString(),
                ...(to != null ? { to } : {}),
              },
            },
            funds: [
              { denom: offerAssetInfo.id, amount: offerAmount.toString() },
            ],
          }),
    ],
  });

  for (const { type, attributes } of txResponse.events) {
    if (type !== "wasm") {
      continue;
    }
    if (
      attributes[0].key !== "_contract_address" ||
      attributes[0].value !== hallswapAddress
    ) {
      continue;
    }
    const offerAsset = attributes.find((attr) => attr.key === "offer_asset");
    const offerAmount = attributes.find((attr) => attr.key === "offer_amount");
    const returnAsset = attributes.find((attr) => attr.key === "return_asset");
    const returnAmount = attributes.find(
      (attr) => attr.key === "return_amount"
    );
    const feeAsset = attributes.find((attr) => attr.key === "fee_asset");
    const feeAmount = attributes.find((attr) => attr.key === "fee_amount");
    if (!offerAsset || !offerAmount || !returnAsset || !returnAmount) {
      continue;
    }
    return {
      offerAsset: offerAsset.value,
      offerAmount: BigInt(offerAmount.value),
      returnAsset: returnAsset.value,
      returnAmount: BigInt(returnAmount.value),
      ...(feeAmount != null && feeAsset != null
        ? { feeAsset: feeAsset.value, feeAmount: BigInt(feeAmount.value) }
        : {}),
    };
  }

  throw new Error("failed to parse hallswap");
}

async function getBalance(account: string, asset: string): Promise<bigint> {
  console.log(`Querying balance of [${asset}] in [${account}]...`);
  if (new AssetInfo(asset).isCW20()) {
    return getCw20Balance(wallet.rpc, {
      address: account,
      token: asset,
    });
  } else {
    const balances = await getNativeBalances(wallet.rpc, {
      address: account,
    });
    for (const coin of balances) {
      if (coin.denom === asset) {
        return BigInt(coin.amount);
      }
    }
    return 0n;
  }
}

async function main() {
  // !---------------- Initialisation of contracts ----------------!

  const hallswapAddress = await initHallswap();
  const tokenCodeID = await store(WASM_FILES.TERRASWAP_TOKEN);
  const terraswapFactoryAddress = await initTerraswap(tokenCodeID);
  const memeTokenAddress = await initToken(tokenCodeID, "MEME");
  const degenTokenAddress = await initToken(tokenCodeID, "DEGEN");
  const memeLunaPairAddress = await initPair(
    terraswapFactoryAddress,
    memeTokenAddress
  );
  const degenLunaPairAddress = await initPair(
    terraswapFactoryAddress,
    degenTokenAddress
  );
  const memeDegenPairAddress = await initPair(
    terraswapFactoryAddress,
    memeTokenAddress,
    degenTokenAddress
  );

  // !---------------- Start of actual tests ----------------!

  {
    console.log("\nTest 1: simulation query (1 pool; LUNA->MEME)");
    const offerAsset = "uluna";
    const [returnAsset, feeAsset] = await queryHallswapSimulation(
      hallswapAddress,
      [
        {
          route: [
            {
              contract_addr: memeLunaPairAddress,
              offer_asset: new AssetInfo(offerAsset).toJSON(),
              return_asset: new AssetInfo(memeTokenAddress).toJSON(),
            },
          ],
          offer_amount: 1_000_000n.toString(),
        },
      ]
    );
    const returnAmount = await queryPairSimulation(
      memeLunaPairAddress,
      offerAsset,
      1_000_000n - 10_000n
    );

    assertEquals(feeAsset.id, "uluna");
    assertEquals(feeAsset.amount, 10_000n);
    assertEquals(returnAsset.amount, returnAmount);
    console.log("Passed!");
  }

  {
    console.log("\nTest 2: simulation query (1 pool; MEME->LUNA)");
    const offerAsset = memeTokenAddress;
    const [returnAsset, feeAsset] = await queryHallswapSimulation(
      hallswapAddress,
      [
        {
          route: [
            {
              contract_addr: memeLunaPairAddress,
              offer_asset: new AssetInfo(offerAsset).toJSON(),
              return_asset: new AssetInfo("uluna").toJSON(),
            },
          ],
          offer_amount: 1_000_000n.toString(),
        },
      ]
    );
    const returnAmount = await queryPairSimulation(
      memeLunaPairAddress,
      offerAsset,
      1_000_000n
    );

    const fees = BigInt(Math.floor(Number(returnAmount) * 0.01));

    assertEquals(feeAsset.id, "uluna");
    assertEquals(feeAsset.amount, fees);
    assertEquals(returnAsset.amount, returnAmount - fees);
    console.log("Passed!");
  }

  {
    console.log("\nTest 3: swap execution (1 pool; LUNA->MEME)");
    const offerAsset = "uluna";
    const offerAmount = 1_000_000n;
    // assume simulation query is correct as it was already tested
    const [simulatedReturnAsset, simulatedFeeAsset] =
      await queryHallswapSimulation(hallswapAddress, [
        {
          route: [
            {
              contract_addr: memeLunaPairAddress,
              offer_asset: new AssetInfo(offerAsset).toJSON(),
              return_asset: new AssetInfo(memeTokenAddress).toJSON(),
            },
          ],
          offer_amount: 1_000_000n.toString(),
        },
      ]);
    const traderMemeBalanceBefore = await getBalance(
      wallet.address,
      memeTokenAddress
    );
    const hallswapLunaBalanceBefore = await getBalance(
      hallswapAddress,
      offerAsset
    );
    const feeLunaBalanceBefore = await getBalance(FEE_COLLECTOR, offerAsset);

    const res = await executeHallswap(
      hallswapAddress,
      offerAsset,
      [
        {
          route: [{ contract_addr: memeLunaPairAddress }],
          offer_amount: 1_000_000n.toString(),
        },
      ],
      simulatedReturnAsset.amount // use actual return as min received
    );

    const traderMemeBalanceAfter = await getBalance(
      wallet.address,
      memeTokenAddress
    );
    const hallswapBalanceAfter = await getBalance(hallswapAddress, offerAsset);
    const feeBalanceAfter = await getBalance(FEE_COLLECTOR, offerAsset);

    assertEquals(res.offerAsset, offerAsset);
    assertEquals(res.offerAmount, offerAmount);

    assertEquals(res.returnAsset, memeTokenAddress);
    assertEquals(res.returnAmount, simulatedReturnAsset.amount);
    assertEquals(
      traderMemeBalanceAfter - traderMemeBalanceBefore,
      res.returnAmount
    );

    assertEquals(res.feeAsset, simulatedFeeAsset.id);
    assertEquals(res.feeAmount, simulatedFeeAsset.amount);
    assertEquals(feeBalanceAfter - feeLunaBalanceBefore, res.feeAmount);
    assertEquals(hallswapBalanceAfter - hallswapLunaBalanceBefore, 0n);
    console.log("Passed!");
  }

  {
    console.log("\nTest 4: swap execution assert minimum received");
    const offerAsset = "uluna";
    const offerAmount = 1_000_000n;
    // assume simulation query is correct as it was already tested
    const [simulatedReturnAsset] = await queryHallswapSimulation(
      hallswapAddress,
      [
        {
          route: [
            {
              contract_addr: memeLunaPairAddress,
              offer_asset: new AssetInfo(offerAsset).toJSON(),
              return_asset: new AssetInfo(memeTokenAddress).toJSON(),
            },
          ],
          offer_amount: offerAmount.toString(),
        },
      ]
    );

    await assertThrows(() =>
      executeHallswap(
        hallswapAddress,
        offerAsset,
        [
          {
            route: [{ contract_addr: memeLunaPairAddress }],
            offer_amount: 1_000_000n.toString(),
          },
        ],
        simulatedReturnAsset.amount + 1n
      )
    );

    console.log("Passed!");
  }

  {
    console.log("\nTest 5: simulation query (2 pool; MEME->DEGEN)");
    const offerAsset = memeTokenAddress;
    const [returnAsset, feeAsset] = await queryHallswapSimulation(
      hallswapAddress,
      [
        {
          route: [
            {
              contract_addr: memeLunaPairAddress,
              offer_asset: new AssetInfo(offerAsset).toJSON(),
              return_asset: new AssetInfo("uluna").toJSON(),
            },
            {
              contract_addr: degenLunaPairAddress,
              offer_asset: new AssetInfo("uluna").toJSON(),
              return_asset: new AssetInfo(degenTokenAddress).toJSON(),
            },
          ],
          offer_amount: 1_000_000n.toString(),
        },
      ]
    );
    const returnAmountMemeToUluna = await queryPairSimulation(
      memeLunaPairAddress,
      offerAsset,
      1_000_000n
    );

    const returnAmountUlunaToDegen = await queryPairSimulation(
      degenLunaPairAddress,
      "uluna",
      returnAmountMemeToUluna
    );
    const feeFromStimulation = BigInt(
      Math.floor(Number(returnAmountUlunaToDegen) * 0.01)
    );

    assertEquals(feeAsset.id, degenTokenAddress);
    assertEquals(feeAsset.amount, feeFromStimulation);
    assertEquals(
      returnAsset.amount,
      returnAmountUlunaToDegen - feeFromStimulation
    );
    console.log("Passed!");
  }

  {
    console.log("\nTest 6: swap execution (2 pool; MEME->DEGEN)");
    const offerAsset = memeTokenAddress;
    const offerAmount = 1_000_000n;
    // assume simulation query is correct as it was already tested
    const [simulatedReturnAsset, simulatedFeeAsset] =
      await queryHallswapSimulation(hallswapAddress, [
        {
          route: [
            {
              contract_addr: memeLunaPairAddress,
              offer_asset: new AssetInfo(offerAsset).toJSON(),
              return_asset: new AssetInfo("uluna").toJSON(),
            },
            {
              contract_addr: degenLunaPairAddress,
              offer_asset: new AssetInfo("uluna").toJSON(),
              return_asset: new AssetInfo(degenTokenAddress).toJSON(),
            },
          ],
          offer_amount: offerAmount.toString(),
        },
      ]);
    const traderDegenBalanceBefore = await getBalance(
      wallet.address,
      degenTokenAddress
    );
    const hallswapBalanceBefore = await getBalance(
      hallswapAddress,
      degenTokenAddress
    );
    const feeBalanceBefore = await getBalance(FEE_COLLECTOR, degenTokenAddress);

    const res = await executeHallswap(
      hallswapAddress,
      offerAsset,
      [
        {
          route: [
            { contract_addr: memeLunaPairAddress },
            { contract_addr: degenLunaPairAddress },
          ],
          offer_amount: 1_000_000n.toString(),
        },
      ],
      simulatedReturnAsset.amount // use actual return as min received
    );

    const traderDegenBalanceAfter = await getBalance(
      wallet.address,
      degenTokenAddress
    );
    const hallswapBalanceAfter = await getBalance(
      hallswapAddress,
      degenTokenAddress
    );
    const feeBalanceAfter = await getBalance(FEE_COLLECTOR, degenTokenAddress);

    assertEquals(res.offerAsset, offerAsset);
    assertEquals(res.offerAmount, offerAmount);

    assertEquals(res.returnAsset, degenTokenAddress);
    assertEquals(res.returnAmount, simulatedReturnAsset.amount);
    assertEquals(
      traderDegenBalanceAfter - traderDegenBalanceBefore,
      res.returnAmount
    );

    assertEquals(res.feeAsset, simulatedFeeAsset.id);
    assertEquals(res.feeAmount, simulatedFeeAsset.amount);
    assertEquals(feeBalanceAfter - feeBalanceBefore, res.feeAmount);
    assertEquals(hallswapBalanceAfter - hallswapBalanceBefore, 0n);
    console.log("Passed!");
  }

  {
    console.log(
      "\nTest 6: simulation query (multi-route; MEME->DEGEN && MEME->ULUNA->DEGEN)"
    );
    const offerAsset = memeTokenAddress;
    const [returnAsset, feeAsset] = await queryHallswapSimulation(
      hallswapAddress,
      [
        {
          route: [
            {
              contract_addr: memeLunaPairAddress,
              offer_asset: new AssetInfo(offerAsset).toJSON(),
              return_asset: new AssetInfo("uluna").toJSON(),
            },
            {
              contract_addr: degenLunaPairAddress,
              offer_asset: new AssetInfo("uluna").toJSON(),
              return_asset: new AssetInfo(degenTokenAddress).toJSON(),
            },
          ],
          offer_amount: 1_000_000n.toString(),
        },
        {
          route: [
            {
              contract_addr: memeDegenPairAddress,
              offer_asset: new AssetInfo(offerAsset).toJSON(),
              return_asset: new AssetInfo(degenTokenAddress).toJSON(),
            },
          ],
          offer_amount: 1_000_000n.toString(),
        },
      ]
    );
    const returnAmountMemeToUluna = await queryPairSimulation(
      memeLunaPairAddress,
      offerAsset,
      1_000_000n
    );

    const returnAmountUlunaToDegen = await queryPairSimulation(
      degenLunaPairAddress,
      "uluna",
      returnAmountMemeToUluna
    );

    const returnAmountMemeToDegen = await queryPairSimulation(
      memeDegenPairAddress,
      memeTokenAddress,
      1_000_000n
    );
    const totalReturn = returnAmountUlunaToDegen + returnAmountMemeToDegen;
    const feeFromStimulation = BigInt(Math.floor(Number(totalReturn) * 0.01));

    assertEquals(feeAsset.id, degenTokenAddress);
    assertEquals(feeAsset.amount, feeFromStimulation);
    assertEquals(returnAsset.amount, totalReturn - feeFromStimulation);
    console.log("Passed!");
  }

  {
    console.log(
      "\nTest 7: swap execution (multi-route; MEME->DEGEN && MEME->ULUNA->DEGEN)"
    );
    const offerAsset = memeTokenAddress;
    const offerAmount = 2_000_000n;
    // assume simulation query is correct as it was already tested
    const [simulatedReturnAsset, simulatedFeeAsset] =
      await queryHallswapSimulation(hallswapAddress, [
        {
          route: [
            {
              contract_addr: memeLunaPairAddress,
              offer_asset: new AssetInfo(offerAsset).toJSON(),
              return_asset: new AssetInfo("uluna").toJSON(),
            },
            {
              contract_addr: degenLunaPairAddress,
              offer_asset: new AssetInfo("uluna").toJSON(),
              return_asset: new AssetInfo(degenTokenAddress).toJSON(),
            },
          ],
          offer_amount: (offerAmount / 2n).toString(),
        },
        {
          route: [
            {
              contract_addr: memeDegenPairAddress,
              offer_asset: new AssetInfo(offerAsset).toJSON(),
              return_asset: new AssetInfo(degenTokenAddress).toJSON(),
            },
          ],
          offer_amount: (offerAmount / 2n).toString(),
        },
      ]);
    const traderDegenBalanceBefore = await getBalance(
      wallet.address,
      degenTokenAddress
    );
    const hallswapBalanceBefore = await getBalance(
      hallswapAddress,
      degenTokenAddress
    );
    const feeBalanceBefore = await getBalance(FEE_COLLECTOR, degenTokenAddress);

    const res = await executeHallswap(
      hallswapAddress,
      offerAsset,
      [
        {
          route: [
            { contract_addr: memeLunaPairAddress },
            { contract_addr: degenLunaPairAddress },
          ],
          offer_amount: (offerAmount / 2n).toString(),
        },
        {
          route: [{ contract_addr: memeDegenPairAddress }],
          offer_amount: (offerAmount / 2n).toString(),
        },
      ],
      simulatedReturnAsset.amount // use actual return as min received
    );

    const traderDegenBalanceAfter = await getBalance(
      wallet.address,
      degenTokenAddress
    );
    const hallswapBalanceAfter = await getBalance(
      hallswapAddress,
      degenTokenAddress
    );
    const feeBalanceAfter = await getBalance(FEE_COLLECTOR, degenTokenAddress);

    assertEquals(res.offerAsset, offerAsset);
    assertEquals(res.offerAmount, offerAmount);

    assertEquals(res.returnAsset, degenTokenAddress);
    assertEquals(res.returnAmount, simulatedReturnAsset.amount);
    assertEquals(
      traderDegenBalanceAfter - traderDegenBalanceBefore,
      res.returnAmount
    );

    assertEquals(res.feeAsset, simulatedFeeAsset.id);
    assertEquals(res.feeAmount, simulatedFeeAsset.amount);
    assertEquals(feeBalanceAfter - feeBalanceBefore, res.feeAmount);
    assertEquals(hallswapBalanceAfter - hallswapBalanceBefore, 0n);
    console.log("Passed!");
  }

  {
    console.log(
      "\nTest 8: swap execution (multi-route; MEME->DEGEN && MEME->ULUNA->DEGEN; to another recipient)"
    );
    const randomRecipientAddress =
      "terra1exgqc9f7ya3ns9c7puwht9vhpmu324wgjr4cgj";
    const offerAsset = memeTokenAddress;
    const offerAmount = 2_000_000n;
    // assume simulation query is correct as it was already tested
    const [simulatedReturnAsset, simulatedFeeAsset] =
      await queryHallswapSimulation(hallswapAddress, [
        {
          route: [
            {
              contract_addr: memeLunaPairAddress,
              offer_asset: new AssetInfo(offerAsset).toJSON(),
              return_asset: new AssetInfo("uluna").toJSON(),
            },
            {
              contract_addr: degenLunaPairAddress,
              offer_asset: new AssetInfo("uluna").toJSON(),
              return_asset: new AssetInfo(degenTokenAddress).toJSON(),
            },
          ],
          offer_amount: (offerAmount / 2n).toString(),
        },
        {
          route: [
            {
              contract_addr: memeDegenPairAddress,
              offer_asset: new AssetInfo(offerAsset).toJSON(),
              return_asset: new AssetInfo(degenTokenAddress).toJSON(),
            },
          ],
          offer_amount: (offerAmount / 2n).toString(),
        },
      ]);
    const recipientDegenBalanceBefore = await getBalance(
      randomRecipientAddress,
      degenTokenAddress
    );
    const hallswapBalanceBefore = await getBalance(
      hallswapAddress,
      degenTokenAddress
    );
    const feeBalanceBefore = await getBalance(FEE_COLLECTOR, degenTokenAddress);

    const res = await executeHallswap(
      hallswapAddress,
      offerAsset,
      [
        {
          route: [
            { contract_addr: memeLunaPairAddress },
            { contract_addr: degenLunaPairAddress },
          ],
          offer_amount: (offerAmount / 2n).toString(),
        },
        {
          route: [{ contract_addr: memeDegenPairAddress }],
          offer_amount: (offerAmount / 2n).toString(),
        },
      ],
      simulatedReturnAsset.amount, // use actual return as min received
      randomRecipientAddress
    );

    const recipientDegenBalanceAfter = await getBalance(
      randomRecipientAddress,
      degenTokenAddress
    );
    const hallswapBalanceAfter = await getBalance(
      hallswapAddress,
      degenTokenAddress
    );
    const feeBalanceAfter = await getBalance(FEE_COLLECTOR, degenTokenAddress);

    assertEquals(res.offerAsset, offerAsset);
    assertEquals(res.offerAmount, offerAmount);

    assertEquals(res.returnAsset, degenTokenAddress);
    assertEquals(res.returnAmount, simulatedReturnAsset.amount);
    assertEquals(
      recipientDegenBalanceAfter - recipientDegenBalanceBefore,
      res.returnAmount
    );

    assertEquals(res.feeAsset, simulatedFeeAsset.id);
    assertEquals(res.feeAmount, simulatedFeeAsset.amount);
    assertEquals(feeBalanceAfter - feeBalanceBefore, res.feeAmount);
    assertEquals(hallswapBalanceAfter - hallswapBalanceBefore, 0n);
    console.log("Passed!");
  }

  console.log("\nALL TESTS PASSED!");
}

main();
